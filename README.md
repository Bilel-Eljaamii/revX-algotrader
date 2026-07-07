# revX-algotrader

<p align="center">
  <a href="https://github.com/Bilel-Eljaamii/revX-algotrader/"><img src="https://img.shields.io/github/stars/bilel/revX-algotrader?style=for-the-badge&color=f5a623&logo=github" alt="Stars"></a>
  <a href="https://github.com/Bilel-Eljaamii/revX-algotrader/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=for-the-badge" alt="License"></a>
  <img src="https://img.shields.io/badge/rust-2021_edition-orange?style=for-the-badge&logo=rust" alt="Rust 2021">
  <img src="https://img.shields.io/badge/build-passing-brightgreen?style=for-the-badge&logo=githubactions&logoColor=white" alt="Build">
  <img src="https://img.shields.io/badge/coverage-70%25+-success?style=for-the-badge&logo=codecov" alt="Coverage">
  <img src="https://img.shields.io/badge/platform-linux%20%7C%20RPi-lightgrey?style=for-the-badge&logo=linux" alt="Platform">
  <img src="https://img.shields.io/badge/dry--run-safe-9cf?style=for-the-badge" alt="Dry-run safe">
</p>

<p align="center">
<!-- Generated with Lumo AI - https://lumo.proton.me -->
  <img src="assets/banner.png" alt="Algorithmic Trading Banner" width="auto" height="auto">
</p>

**Banner generated with [Lumo AI](https://lumo.proton.me)**

**The open-source algorithmic trading core for the [Revolut X](https://revolutx.com) exchange.**

Built in Rust for maximum performance and reliability — deployable on a Raspberry Pi or any Linux server.

---

## Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Project Layout](#project-layout)
- [Prerequisites](#prerequisites)
- [Setup & Configuration](#setup--configuration)
- [Running a Strategy](#running-a-strategy)
- [Build Targets](#build-targets)
- [Testing](#testing)
- [How to Add a New Strategy](#how-to-add-a-new-strategy)
- [LLM Prompt — Implement EMA-14 Strategy](#llm-prompt--implement-ema-14-strategy)
- [API Reference](#api-reference)
- [Security](#security)
- [Notifications](#notifications)
- [Contributing](#contributing)

---

## Overview

revX-algotrader is a modular, async-first trading engine built on top of the Revolut X REST API. It provides:

- **Authenticated REST client** — Ed25519-signed requests, order management, orderbook queries
- **SQLite trade database** — persistent local order history and position state
- **Local proxy server** — expose live data to a frontend dashboard
- **Strategy framework** — a clean, minimal pattern to plug in any trading logic
- **Cross-compilation** — single `make pi5` to ship a binary to a Raspberry Pi 5

The `dummy` strategy ships as the canonical reference implementation. Every new strategy follows the same three-file pattern.

---

## Architecture

```
revX-algotrader
├── backend/                   ← Rust workspace (this is the engine)
│   ├── src/
│   │   ├── api/               ← RevolutX REST client, auth (Ed25519), local proxy
│   │   ├── core/              ← Config loader, SQLite DB, data models, notifiers
│   │   ├── strategies/
│   │   │   └── <name>/
│   │   │       ├── mod.rs     ← Module registration
│   │   │       ├── strategy.rs← Pure trading logic (run_tick)
│   │   │       └── engine.rs  ← Async run loop + shutdown handling
│   │   └── bin/
│   │       └── <name>.rs      ← Binary entrypoint (clap args, config, wiring)
│   └── tests/
│       ├── ut/                ← Unit & integration tests
│       └── ct/                ← Component tests (mock HTTPS server)
└── frontend/                  ← Dashboard (optional)
```

**Data flow per tick:**

```
Clock tick
    └─► strategy::run_tick(client, sym_cfg, db, dry_run)
            ├─► RevolutXClient  →  Revolut X REST API
            ├─► TradeDb         →  SQLite (position state, history)
            └─► PlaceOrder / no-op (dry_run)
```

---

## Project Layout

| Path | Purpose |
|------|---------|
| `src/api/client.rs` | `RevolutXClient` — tickers, orders, orderbook, cancel |
| `src/api/auth.rs` | Ed25519 request signing, PEM key loading |
| `src/api/proxy.rs` | Local Axum HTTP proxy (historical orders, active orders, tickers) |
| `src/core/config.rs` | Config structs + JSON loader (`~/.config/revolut-x/`) |
| `src/core/db.rs` | `TradeDb` — upsert, query, position state, history |
| `src/core/models.rs` | All API request/response types (`Ticker`, `Order`, `OrderBook`, …) |
| `src/core/notifiers/` | Push alert abstraction (`ntfy` driver) |
| `src/strategies/dummy/` | Reference strategy — proves all wiring works |

---

## Prerequisites

| Tool | Notes |
|------|-------|
| Rust (stable + nightly) | `rustup toolchain install nightly` (nightly used only for `fmt`) |
| `cargo-llvm-cov` | For coverage — `cargo install cargo-llvm-cov` |
| `sccache` *(optional)* | Build cache — speeds up incremental compilation |
| Ed25519 private key | Generate with `openssl genpkey -algorithm ed25519 -out private.pem` |
| Revolut X API key | Available in the Revolut X app → Settings → API |

**Cross-compilation (Raspberry Pi):**

```bash
rustup target add aarch64-unknown-linux-gnu
# macOS:  brew install messense/macos-cross-toolchains/aarch64-unknown-linux-gnu
# Linux:  sudo apt install gcc-aarch64-linux-gnu
```

---

## Setup & Configuration

Config files live in `~/.config/revolut-x/` by default.
Override the directory with the env var `REVOLUTX_CONFIG_DIR`.

### `dummy_config.json` (reference template)

```json
{
  "api_key": "YOUR_API_KEY",
  "private_key_path": "~/.config/revolut-x/private.pem",
  "polling_interval_ms": 5000,
  "db_path": "~/.local/share/revx/dummy.db",
  "api_port": 30091,
  "base_url": null,
  "symbols": [
    {
      "symbol": "BTC-USD",
      "buy_trigger_price": 95000.0,
      "sell_trigger_price": 105000.0,
      "revert_price": 100000.0,
      "trade_size_base": 0.001,
      "trade_size_quote": 100.0,
      "tick_size": 0.01
    }
  ]
}
```

> **Config resolution order:**
> 1. `$REVOLUTX_CONFIG_DIR/<strategy>_config.json`
> 2. `~/.config/revolut-x/<strategy>_config.json`
> 3. `./<strategy>_config.json` (current working directory fallback)

---

## Running a Strategy

```bash
# Dry-run (no real orders placed):
./target/release/dummy --dry-run

# Live trading:
./target/release/dummy

# Ctrl-C triggers graceful shutdown.
```

Logs are structured via `tracing`. Pipe to `jq` for pretty output:

```bash
./target/release/dummy --dry-run 2>&1 | jq .
```

---

## Build Targets

```bash
make pc          # Build release binary for local machine
make debug       # Build debug binary
make test        # Run all tests (unit + integration + component)
make coverage    # Run tests with LLVM coverage report
make fmt         # Format with nightly rustfmt

# Cross-compile for Raspberry Pi
make pi5         # Cortex-A76 (RPi 5, optimised)
make pi3         # Cortex-A53 (RPi 3/4, safe fallback)
make generic     # Generic aarch64 (runs on all Pi models)
```

---

## Testing

The project has three test layers:

| Layer | Location | What it tests |
|-------|----------|---------------|
| **Unit** | `tests/ut/` | Auth signing, config loading, DB operations, client query builders |
| **Integration** | `tests/ut/integration.rs` | Cross-module flows (sign → request → DB) |
| **Component (CT)** | `tests/ct/` | Full strategy tick against a mock HTTPS server |

```bash
make test        # Run all layers
make coverage    # HTML coverage report → target/llvm-cov/html/
```

---

## How to Add a New Strategy

Every strategy lives in three files under `src/strategies/<name>/` plus a binary in `src/bin/<name>.rs`.

### Step 1 — Create the strategy module

```
src/strategies/
└── ema14/
    ├── mod.rs
    ├── strategy.rs
    └── engine.rs
```

**`src/strategies/ema14/mod.rs`:**
```rust
pub mod engine;
pub mod strategy;
```

Register in **`src/strategies/mod.rs`:**
```rust
pub mod dummy;
pub mod ema14;   // ← add this line
```

---

### Step 2 — Implement strategy logic

**`src/strategies/ema14/strategy.rs`** — one async `run_tick` function:

```rust
use tracing::info;
use crate::{
    api::client::RevolutXClient,
    core::{config::Ema14SymbolConfig, db::TradeDb},
};

pub async fn run_tick(
    client: &RevolutXClient,
    sym_cfg: &Ema14SymbolConfig,
    db: &TradeDb,
    dry_run: bool,
) -> anyhow::Result<()> {
    // 1. Fetch ticker
    let ticker = client.get_ticker(&sym_cfg.symbol).await?;
    let mid_price: f64 = ticker.mid.parse()?;

    // 2. Compute your indicator (EMA-14, RSI, VWAP, …)
    // ... your logic here ...

    // 3. Act on signal
    if should_buy && !dry_run {
        info!(symbol = %sym_cfg.symbol, "BUY signal — placing order");
        // client.place_order(&req).await?;
    }

    Ok(())
}
```

---

### Step 3 — Create the engine run loop

**`src/strategies/ema14/engine.rs`** — copy from `dummy/engine.rs`, change config type:

```rust
use tokio::{sync::watch::Receiver, time};
use tracing::{error, info};
use crate::{
    api::client::RevolutXClient,
    core::{config::Ema14Config, db::TradeDb},
    strategies::ema14::strategy,
};

pub async fn run(
    client: &RevolutXClient,
    config: &Ema14Config,
    db: TradeDb,
    mut shutdown: Receiver<bool>,
    dry_run: bool,
) {
    let mut ticker = time::interval(
        time::Duration::from_millis(config.poll_interval_ms)
    );
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
    info!(dry_run, "ema14 engine started");

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                for sym_cfg in &config.symbols {
                    if let Err(e) = strategy::run_tick(client, sym_cfg, &db, dry_run).await {
                        error!(symbol = %sym_cfg.symbol, "tick error: {e:#}");
                    }
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() { return; }
            }
        }
    }
}
```

---

### Step 4 — Create the binary entrypoint

**`src/bin/ema14.rs`:**

```rust
use anyhow::Result;
use clap::Parser;
use revx_bot::{
    api::{auth::load_signing_key, client::RevolutXClient},
    core::{config::Ema14Config, db::TradeDb},
    strategies::ema14::engine,
};
use tokio::{signal, sync::watch};
use tracing::{info, Level};

#[derive(Parser)]
#[command(about = "EMA-14 Strategy Bot for Revolut X")]
struct Args {
    #[arg(short, long, default_value_t = false)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().json()with_max_level(Level::INFO).init();

    let args = Args::parse();
    let config = Ema14Config::load()?;
    let db = TradeDb::open(&config.db_path)?;
    db.migrate()?;

    let signing_key = load_signing_key(&config.private_key_path)?;
    let mut client = RevolutXClient::new(config.api_key.clone(), signing_key)?;
    if let Some(ref base_url) = config.base_url {
        client = client.with_base_url(base_url.clone());
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handle = tokio::spawn(async move {
        engine::run(&client, &config, db, shutdown_rx, args.dry_run).await;
    });

    signal::ctrl_c().await?;
    info!("Ctrl-C — shutting down");
    let _ = shutdown_tx.send(true);
    let _ = handle.await;
    Ok(())
}
```

---

### Step 5 — Register in Cargo.toml

```toml
[[bin]]
name = "ema14"
path = "src/bin/ema14.rs"
```

---

### Step 6 — Add a config file

Create `~/.config/revolut-x/ema14_config.json`:

```json
{
  "api_key": "YOUR_API_KEY",
  "private_key_path": "~/.config/revolut-x/private.pem",
  "polling_interval_ms": 15000,
  "db_path": "~/.local/share/revx/ema14.db",
  "api_port": 30092,
  "base_url": null,
  "symbols": [
    {
      "symbol": "BTC-USD",
      "ema_fast_period": 14,
      "ema_slow_period": 50,
      "trade_size_base": 0.001
    }
  ]
}
```

Add `Ema14Config` and `Ema14SymbolConfig` structs to `src/core/config.rs` following the same pattern as `DummyConfig`.

---

### Step 7 — Write tests

**Component test (`tests/ct/ema14.rs`):**

```rust
#[tokio::test]
async fn test_ema14_strategy_tick() {
    // Start mock server, create client, run tick, assert Ok(())
    // See tests/ct/dummy.rs for the full boilerplate
}
```

Register in `tests/ct.rs`:
```rust
mod ema14;
```

---

## LLM Prompt — Implement EMA-14 Strategy

Use this prompt when asking an LLM (Claude, Gemini, GPT-4o, etc.) to implement and dry-test a concrete EMA-14 crossover strategy on top of this framework.

---

> ### 🤖 Prompt — EMA-14 Crossover Strategy for revX-algotrader
>
> I have a Rust algo-trading project called **revX-algotrader** targeting the Revolut X exchange.
>
> **Framework conventions:**
> - Each strategy lives in `src/strategies/<name>/` with three files: `mod.rs`, `strategy.rs`, `engine.rs`
> - The binary entrypoint is `src/bin/<name>.rs`
> - Config is loaded from `~/.config/revolut-x/<name>_config.json` via a `serde::Deserialize` struct in `src/core/config.rs`
> - The key function signature is:
>   ```rust
>   pub async fn run_tick(
>       client: &RevolutXClient,
>       sym_cfg: &Ema14SymbolConfig,
>       db: &TradeDb,
>       dry_run: bool,
>   ) -> anyhow::Result<()>
>   ```
> - `RevolutXClient` provides: `get_ticker(&symbol)`, `get_order_book(&symbol, depth)`, `get_active_orders(&filter)`, `place_order(&req)`, `cancel_order(&order_id)`
> - `TradeDb` provides: `get_position_state(&symbol, &side)`, `upsert_position_state(...)`, `upsert_order(...)`, `clear_position_state(&symbol, &side)`
> - When `dry_run = true`, log decisions but **never** call `place_order` or `cancel_order`
> - No `unwrap()` in production code — use `anyhow::Result` and the `?` operator
> - Follow the code style in `src/strategies/dummy/`
>
> **Strategy to implement: EMA-14 / EMA-50 Crossover**
>
> Rules:
> - On each tick fetch the current mid-price from `get_ticker`
> - Maintain a rolling price history and compute EMA(14) and EMA(50)
> - **BUY signal**: EMA(14) crosses above EMA(50) AND no open long position exists in TradeDb
> - **SELL/EXIT signal**: EMA(14) crosses below EMA(50) AND a long position exists in TradeDb
> - Use `trade_size_base` from the symbol config for order size
> - Persist the EMA state (or price history) between ticks using TradeDb position_state or a shared `Arc<Mutex<HashMap>>`
>
> **Tasks — implement all of the following:**
> 1. `src/strategies/ema14/strategy.rs` — `run_tick` + internal EMA computation helper (no external crates)
> 2. `src/strategies/ema14/engine.rs` — async run loop (copy dummy engine, change config type)
> 3. `src/strategies/ema14/mod.rs` — module registration
> 4. Add `Ema14Config` and `Ema14SymbolConfig` to `src/core/config.rs`
> 5. `src/bin/ema14.rs` — binary entrypoint
> 6. Register the binary in `Cargo.toml`
> 7. **Component test** in `tests/ct/ema14.rs` using `MockHttpsServer` that:
>    - Mocks `/api/1.0/public/ticker/BTC-USD` to return a price sequence that triggers a Golden Cross
>    - Runs `run_tick` with `dry_run = true`
>    - Asserts `Ok(())` is returned
> 8. All tests must pass with `cargo test`

---

## API Reference

### `RevolutXClient` — key methods

| Method | Description |
|--------|-------------|
| `get_ticker(&symbol)` | Best bid/ask/mid/last for one symbol |
| `get_tickers(&[symbol])` | Batch ticker fetch |
| `get_order_book(&symbol, depth)` | Level-2 orderbook (depth clamped to 100) |
| `get_active_orders(&filter)` | Open orders, filterable by symbol/side |
| `get_historical_orders(&query)` | Filled/cancelled orders with time range |
| `place_order(&req)` | Submit a limit or market order |
| `cancel_order(&order_id)` | Cancel by order ID (404 treated as success) |

### `TradeDb` — key methods

| Method | Description |
|--------|-------------|
| `open(&path)` | Open (or create) the SQLite database |
| `migrate()` | Create tables if missing — always call after `open` |
| `upsert_order(&order)` | Insert or update a trade record |
| `query_orders(&filter)` | Query trade history by symbol, side, time range |
| `get_position_state(&symbol, &side)` | Read current long/short position |
| `upsert_position_state(...)` | Write position state |
| `clear_position_state(&symbol, &side)` | Remove position record on exit |

---

## Security

- **Private key** — Ed25519 PEM, never committed to VCS. Store at `~/.config/revolut-x/private.pem` (mode `600`).
- **API key** — stored in config JSON. Protect with filesystem permissions.
- The signing implementation (`api/auth.rs`) has 100% line coverage in the test suite.

```bash
# Generate a fresh Ed25519 key pair
openssl genpkey -algorithm ed25519 -out ~/.config/revolut-x/private.pem
chmod 600 ~/.config/revolut-x/private.pem
```

---

## Notifications

Add an optional `notifiers` field to your config for push alerts via [ntfy](https://ntfy.sh):

```json
{
  "notifiers": [
    {
      "type": "ntfy",
      "topic_url": "https://ntfy.sh/your-private-topic",
      "auth_token": "tk_optional_token"
    }
  ]
}
```

Extend `src/core/notifiers/` to add more drivers (Slack, Telegram, email, …).

---

## Contributing

1. Fork and create a feature branch
2. Run `make fmt && make test` before pushing
3. Add or update tests — the project targets ≥ 70% line coverage
4. Open a PR with a description of the strategy logic and any new config fields

---

*revX-algotrader is not affiliated with Revolut Ltd. Use at your own risk. Always test with `--dry-run` before live trading.*
