//! Local SQLite trade history + position-state tracking.
//!
//! Two tables:
//!   - `orders`         — every filled order (historical log)
//!   - `position_state` — one row per (symbol, side) tracking the two-phase
//!     limit-order lifecycle: entry placed → entry filled → exit placed → exit
//!     filled → repeat.

use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use tracing::debug;

use crate::core::models::{Order, OrderSide};

// ── PositionState
// ─────────────────────────────────────────────────────────────

/// Tracks the lifecycle of one directional position (long or short) for a
/// single symbol.  Persisted in SQLite so the bot can survive restarts without
/// losing track of in-flight orders.
#[derive(Debug, Clone)]
pub struct PositionState {
    /// Trading pair, e.g. `"USDT-USD"`.
    pub symbol: String,
    /// `"buy"` = long position (buy entry → sell exit).
    /// `"sell"` = short position (sell entry → buy exit).
    pub side: String,
    /// Exchange-assigned order ID of the entry limit order.
    pub entry_order_id: String,
    /// `client_order_id` used when placing the entry (for lookup in history).
    pub entry_client_id: String,
    /// Whether the entry has been confirmed filled.
    pub entry_filled: bool,
    /// Exchange-assigned order ID of the exit limit order (set after entry
    /// fills).
    pub exit_order_id: Option<String>,
    /// Unix epoch milliseconds — last write time.
    pub updated_at: i64,
}

impl PositionState {
    /// Composite primary key stored in the table.
    pub fn pk(symbol: &str, side: &str) -> String {
        let normalized = symbol.replace('-', "/");
        format!("{normalized}:{side}")
    }
}

// ── PositionMetrics
// ────────────────────────────────────────────────────────────

/// Rich trade snapshot recorded at the moment of entry and exit placement.
/// Separate from `PositionState` so the core lifecycle logic remains unchanged.
/// Used by the frontend to visualise trade economics.
#[derive(Debug, Clone)]
pub struct PositionMetrics {
    /// Same composite PK as `position_state`: `"{symbol}:{side}"`.
    pub id: String,
    pub symbol: String,
    pub side: String,
    /// Price at which the BUY entry was placed.
    pub entry_price: Option<f64>,
    /// Quantity placed for the BUY entry.
    pub entry_quantity: Option<f64>,
    /// Total resting quantity in the order book at `entry_price` at the moment
    /// of entry — indicates how many units are ahead of us in the FIFO queue.
    pub book_qty_at_entry: Option<f64>,
    /// Price at which the SELL exit was placed.
    pub exit_price: Option<f64>,
    /// Quantity placed for the SELL exit.
    pub exit_quantity: Option<f64>,
    pub updated_at: i64,
}

// ── TradeDb
// ───────────────────────────────────────────────────────────────────

/// Thread-safe wrapper around a SQLite connection.
///
/// `rusqlite::Connection` is `!Send`, so we wrap it in `Arc<Mutex<…>>` to
/// allow sharing across the engine and the proxy server if needed.
#[derive(Clone)]
pub struct TradeDb {
    conn: Arc<Mutex<Connection>>,
}

impl TradeDb {
    /// Open (or create) the database at `path`.
    pub fn open(path: &str) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(path).parent() {
            if parent != std::path::Path::new("") {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("cannot create db directory {}", parent.display()))?;
            }
        }

        let conn = Connection::open(path)
            .with_context(|| format!("cannot open SQLite database at {path}"))?;

        // Performance pragmas — WAL mode for concurrent reads while the engine writes
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous  = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
        .context("SQLite pragmas")?;

        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Run migrations — idempotent, safe to call on every startup.
    pub fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "-- Historical filled orders (for the frontend / audit log)
            CREATE TABLE IF NOT EXISTS orders (
                id                  TEXT PRIMARY KEY,
                prev_order_id       TEXT,                 -- Link to replaced order
                client_order_id     TEXT,
                symbol              TEXT NOT NULL,
                side                TEXT NOT NULL,        -- 'buy' | 'sell'
                order_type          TEXT NOT NULL,        -- 'market' | 'limit' | ...
                status              TEXT NOT NULL,        -- 'filled' | 'cancelled' | ...
                base_quantity       TEXT,
                filled_quantity     TEXT,
                remaining_quantity  TEXT,
                quote_quantity      TEXT,
                limit_price         TEXT,
                avg_price           TEXT,
                created_at          INTEGER NOT NULL,     -- Unix epoch ms
                updated_at          INTEGER,
                completed_at        INTEGER,
                raw_json            TEXT NOT NULL         -- full order payload for future use
            ) STRICT;

            -- Index for fast lookup by symbol + time range (frontend queries)
            CREATE INDEX IF NOT EXISTS idx_orders_symbol_created
                ON orders (symbol, created_at DESC);

            -- Index for linking exchange orders to local state
            CREATE INDEX IF NOT EXISTS idx_orders_client_id
                ON orders (client_order_id);
            ",
        )
        .context("create orders table")?;

        // Migration: Add columns to existing DBs if they don't exist
        let _ = conn.execute("ALTER TABLE orders ADD COLUMN prev_order_id TEXT", []);
        let _ = conn.execute("ALTER TABLE orders ADD COLUMN remaining_quantity TEXT", []);

        conn.execute_batch(
            "-- Two-phase position state: one row per (symbol, side) pair.
            -- 'side' = 'buy' (long: buy entry → sell exit)
            --        | 'sell' (short: sell entry → buy exit)
            CREATE TABLE IF NOT EXISTS position_state (
                id               TEXT PRIMARY KEY,   -- '{symbol}:{side}'
                symbol           TEXT NOT NULL,
                side             TEXT NOT NULL,
                entry_order_id   TEXT NOT NULL,
                entry_client_id  TEXT NOT NULL,
                entry_filled     INTEGER NOT NULL DEFAULT 0,  -- 0 = pending, 1 = filled
                exit_order_id    TEXT,                        -- NULL until exit is placed
                updated_at       INTEGER NOT NULL
            ) STRICT;
            ",
        )
        .context("SQLite migration")?;

        conn.execute_batch(
            "-- Rich trade economics snapshot (entry/exit price & qty + queue depth).
             -- One row per (symbol, side) — overwritten each cycle.
            CREATE TABLE IF NOT EXISTS position_metrics (
                id                   TEXT PRIMARY KEY,  -- '{symbol}:{side}'
                symbol               TEXT NOT NULL,
                side                 TEXT NOT NULL,
                entry_price          REAL,
                entry_quantity       REAL,
                book_qty_at_entry    REAL,              -- resting qty ahead of us in FIFO queue
                exit_price           REAL,
                exit_quantity        REAL,
                updated_at           INTEGER NOT NULL
            ) STRICT;
            ",
        )
        .context("SQLite migration position_metrics")?;

        // Migration: silently add columns to existing DBs
        let _ = conn.execute("ALTER TABLE position_metrics ADD COLUMN book_qty_at_entry REAL", []);

        debug!("database migration complete");
        Ok(())
    }

    // ── orders table ──────────────────────────────────────────────────────────

    /// Insert or update an order record (idempotent).
    pub fn upsert_order(&self, order: &Order) -> Result<()> {
        let side = match order.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };
        let symbol = order.symbol.replace('-', "/");
        let status = format!("{:?}", order.status).to_lowercase();
        let order_type = order
            .order_type
            .as_ref()
            .map(|t| format!("{:?}", t).to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());
        let raw_json = serde_json::to_string(order).context("serialize order to JSON")?;

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO orders (
                id, prev_order_id, client_order_id, symbol, side, order_type, status,
                base_quantity, filled_quantity, remaining_quantity, quote_quantity,
                limit_price, avg_price,
                created_at, updated_at, completed_at, raw_json
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)
            ON CONFLICT(id) DO UPDATE SET
                status        = excluded.status,
                filled_quantity = excluded.filled_quantity,
                remaining_quantity = excluded.remaining_quantity,
                avg_price     = excluded.avg_price,
                updated_at    = excluded.updated_at,
                completed_at  = excluded.completed_at,
                raw_json      = excluded.raw_json",
            params![
                order.id,
                order.prev_order_id,
                order.client_order_id,
                symbol,
                side,
                order_type,
                status,
                order.base_quantity,
                order.filled_quantity,
                order.remaining_quantity,
                order.quote_quantity,
                order.limit_price,
                order.avg_price,
                order.created_at.map(|v| v as i64).unwrap_or_else(|| std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    as i64),
                order.updated_at.map(|v| v as i64).unwrap_or_else(|| std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    as i64),
                order.completed_at.map(|v| v as i64),
                raw_json,
            ],
        )
        .context("upsert order")?;

        debug!(order_id = %order.id, status, "order upserted to db");
        Ok(())
    }

    /// Mark an order as replaced in the database.
    pub fn mark_order_replaced(&self, order_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // 1. Retrieve the existing raw_json for this order
        let mut stmt = conn.prepare("SELECT raw_json FROM orders WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![order_id], |row| row.get::<_, String>(0))?;

        if let Some(Ok(raw_json_str)) = rows.next() {
            // 2. Parse the JSON, update the status field and serialize it back
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&raw_json_str) {
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("status".to_string(), serde_json::json!("replaced"));
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;
                    obj.insert("updated_at".to_string(), serde_json::json!(now));
                    obj.insert("completed_at".to_string(), serde_json::json!(now));

                    if let Ok(updated_raw_json) = serde_json::to_string(&val) {
                        conn.execute(
                            "UPDATE orders SET status = 'replaced', updated_at = ?2, completed_at = ?2, raw_json = ?3 WHERE id = ?1",
                            params![order_id, now, updated_raw_json],
                        )?;
                        return Ok(());
                    }
                }
            }
        }

        // If not found or failed to parse, just do a simple column update as fallback
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
                as i64;
        conn.execute(
            "UPDATE orders SET status = 'replaced', updated_at = ?2, completed_at = ?2 WHERE id = ?1",
            params![order_id, now],
        )?;

        Ok(())
    }

    /// Mark an order as cancelled in the database.
    pub fn mark_order_cancelled(&self, order_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // 1. Retrieve the existing raw_json for this order
        let mut stmt = conn.prepare("SELECT raw_json FROM orders WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![order_id], |row| row.get::<_, String>(0))?;

        if let Some(Ok(raw_json_str)) = rows.next() {
            // 2. Parse the JSON, update the status field and serialize it back
            if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&raw_json_str) {
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("status".to_string(), serde_json::json!("cancelled"));
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64;
                    obj.insert("updated_at".to_string(), serde_json::json!(now));
                    obj.insert("completed_at".to_string(), serde_json::json!(now));

                    if let Ok(updated_raw_json) = serde_json::to_string(&val) {
                        conn.execute(
                            "UPDATE orders SET status = 'cancelled', updated_at = ?2, completed_at = ?2, raw_json = ?3 WHERE id = ?1",
                            params![order_id, now, updated_raw_json],
                        )?;
                        return Ok(());
                    }
                }
            }
        }

        // If not found or failed to parse, just do a simple column update as fallback
        let now =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()
                as i64;
        conn.execute(
            "UPDATE orders SET status = 'cancelled', updated_at = ?2, completed_at = ?2 WHERE id = ?1",
            params![order_id, now],
        )?;

        Ok(())
    }

    /// Query stored orders for a symbol within a time range.
    ///
    /// Returns serialised JSON strings (the `raw_json` column) for maximum
    /// flexibility — callers can deserialise as needed.
    pub fn query_orders(
        &self,
        symbol: &str,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        limit: usize,
    ) -> Result<Vec<String>> {
        let symbol = symbol.replace('-', "/");
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT raw_json FROM orders
             WHERE symbol = ?1
               AND (?2 IS NULL OR created_at >= ?2)
               AND (?3 IS NULL OR created_at <= ?3)
             ORDER BY updated_at DESC
             LIMIT ?4",
        )?;
        let rows = stmt
            .query_map(params![symbol, start_ms, end_ms, limit as i64], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("query_orders")?;
        Ok(rows)
    }

    // ── position_state table ──────────────────────────────────────────────────

    /// Load the position state for a (symbol, side) pair.
    /// Returns `None` if no state exists (i.e. no active position).
    pub fn get_position_state(&self, symbol: &str, side: &str) -> Result<Option<PositionState>> {
        let pk = PositionState::pk(symbol, side);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT symbol, side, entry_order_id, entry_client_id,
                    entry_filled, exit_order_id, updated_at
             FROM position_state
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![pk], |row| {
            Ok(PositionState {
                symbol: row.get(0)?,
                side: row.get(1)?,
                entry_order_id: row.get(2)?,
                entry_client_id: row.get(3)?,
                entry_filled: row.get::<_, i64>(4)? != 0,
                exit_order_id: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(Ok(state)) => Ok(Some(state)),
            Some(Err(e)) => Err(e).context("get_position_state: row error"),
            None => Ok(None),
        }
    }

    /// Persist (insert or replace) a position state row.
    pub fn upsert_position_state(&self, state: &PositionState) -> Result<()> {
        let pk = PositionState::pk(&state.symbol, &state.side);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO position_state
                (id, symbol, side, entry_order_id, entry_client_id,
                 entry_filled, exit_order_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
                entry_order_id  = excluded.entry_order_id,
                entry_client_id = excluded.entry_client_id,
                entry_filled    = excluded.entry_filled,
                exit_order_id   = excluded.exit_order_id,
                updated_at      = excluded.updated_at",
            params![
                pk,
                state.symbol,
                state.side,
                state.entry_order_id,
                state.entry_client_id,
                state.entry_filled as i64,
                state.exit_order_id,
                state.updated_at,
            ],
        )
        .context("upsert_position_state")?;
        debug!(pk, entry_filled = state.entry_filled, "position state saved");
        Ok(())
    }

    /// Delete the position state for a (symbol, side) pair (cycle complete).
    pub fn clear_position_state(&self, symbol: &str, side: &str) -> Result<()> {
        let pk = PositionState::pk(symbol, side);
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM position_state WHERE id = ?1", params![pk])
            .context("clear_position_state")?;
        debug!(pk, "position state cleared");
        Ok(())
    }

    // ── position_metrics table ────────────────────────────────────────────────

    /// Persist (insert or replace) the rich trade metrics for a position.
    pub fn upsert_position_metrics(&self, m: &PositionMetrics) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO position_metrics
                (id, symbol, side, entry_price, entry_quantity, book_qty_at_entry,
                 exit_price, exit_quantity, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                entry_price          = COALESCE(excluded.entry_price, entry_price),
                entry_quantity       = COALESCE(excluded.entry_quantity, entry_quantity),
                book_qty_at_entry    = COALESCE(excluded.book_qty_at_entry, book_qty_at_entry),
                exit_price           = COALESCE(excluded.exit_price, exit_price),
                exit_quantity        = COALESCE(excluded.exit_quantity, exit_quantity),
                updated_at           = excluded.updated_at",
            params![
                m.id,
                m.symbol,
                m.side,
                m.entry_price,
                m.entry_quantity,
                m.book_qty_at_entry,
                m.exit_price,
                m.exit_quantity,
                m.updated_at,
            ],
        )
        .context("upsert_position_metrics")?;
        debug!(id = %m.id, "position metrics saved");
        Ok(())
    }

    /// Load the rich trade metrics for a (symbol, side) pair.
    pub fn get_position_metrics(
        &self,
        symbol: &str,
        side: &str,
    ) -> Result<Option<PositionMetrics>> {
        let id = PositionState::pk(symbol, side);
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, symbol, side, entry_price, entry_quantity, book_qty_at_entry,
                    exit_price, exit_quantity, updated_at
             FROM position_metrics
             WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(PositionMetrics {
                id: row.get(0)?,
                symbol: row.get(1)?,
                side: row.get(2)?,
                entry_price: row.get(3)?,
                entry_quantity: row.get(4)?,
                book_qty_at_entry: row.get(5)?,
                exit_price: row.get(6)?,
                exit_quantity: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;
        match rows.next() {
            Some(Ok(m)) => Ok(Some(m)),
            Some(Err(e)) => Err(e).context("get_position_metrics: row error"),
            None => Ok(None),
        }
    }

    /// Delete position metrics for a (symbol, side) pair (cycle complete).
    pub fn clear_position_metrics(&self, symbol: &str, side: &str) -> Result<()> {
        let id = PositionState::pk(symbol, side);
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM position_metrics WHERE id = ?1", params![id])
            .context("clear_position_metrics")?;
        Ok(())
    }

    /// Run a raw SQL command. Primarily for testing and maintenance.
    pub fn execute_raw(&self, sql: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(sql, []).context("execute_raw")?;
        Ok(())
    }

    pub fn execute_raw_with_args(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(sql, rusqlite::params_from_iter(params)).context("execute_raw_with_args")?;
        Ok(())
    }
}
