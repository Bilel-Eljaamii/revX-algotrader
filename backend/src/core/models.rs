//! All Revolut X API request / response types.
//!
//! Prices and quantities are kept as `String` to avoid floating-point
//! precision loss (the API uses decimal strings, e.g. `"50000.50"`).

use serde::{Deserialize, Serialize};

// ── Enums ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    Market,
    Limit,
    Conditional,
    Tpsl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    PendingNew,
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
    Replaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeInForce {
    Gtc,
    Ioc,
    Fok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionInstruction {
    AllowTaker,
    PostOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerDirection {
    Ge, // ≥ trigger price
    Le, // ≤ trigger price
}

// ── Trigger / TPSL sub-types ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDetails {
    pub trigger_price: String,
    pub order_type: Option<OrderType>,
    pub trigger_direction: TriggerDirection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<TimeInForce>,
    #[serde(default)]
    pub execution_instructions: Vec<ExecutionInstruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TpSlDetails {
    pub trigger_price: String,
    pub order_type: Option<OrderType>,
    pub trigger_direction: TriggerDirection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_in_force: Option<TimeInForce>,
    #[serde(default)]
    pub execution_instructions: Vec<ExecutionInstruction>,
}

// ── Place Order
// ───────────────────────────────────────────────────────────────

/// POST /api/1.0/orders
#[derive(Debug, Serialize)]
pub struct PlaceOrderRequest {
    pub client_order_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub order_configuration: OrderConfiguration,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderConfiguration {
    Limit(LimitConfig),
    Market(MarketConfig),
    Tpsl(TpslOrderConfig),
}

#[derive(Debug, Serialize)]
pub struct LimitConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_size: Option<String>,
    pub price: String,
    #[serde(default)]
    pub execution_instructions: Vec<ExecutionInstruction>,
}

#[derive(Debug, Serialize)]
pub struct MarketConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_size: Option<String>,
}

/// TPSL order configuration for POST /api/1.0/orders.
///
/// Places an order with embedded take-profit and optional stop-loss triggers.
/// The entry fill happens at `base_size` / `quote_size`.  The TP/SL legs fire
/// automatically once the entry is filled.
#[derive(Debug, Serialize)]
pub struct TpslOrderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_size: Option<String>,
    /// Take-profit trigger (required for TPSL orders).
    pub take_profit: TpslTrigger,
    /// Stop-loss trigger (optional — omit when stop_loss == 0 in config).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_loss: Option<TpslTrigger>,
}

/// A single TP or SL trigger leg.
#[derive(Debug, Serialize)]
pub struct TpslTrigger {
    /// The price level that fires the trigger.
    pub trigger_price: String,
    /// `"limit"` or `"market"` for the resulting exit order.
    pub order_type: OrderType,
    /// `ge` = fires when price ≥ trigger (used for sell TP on a buy leg).
    /// `le` = fires when price ≤ trigger (used for buy  TP on a sell leg).
    pub trigger_direction: TriggerDirection,
    /// Required when `order_type` is `limit`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_price: Option<String>,
}

/// Response to POST /api/1.0/orders
#[derive(Debug, Deserialize)]
pub struct PlaceOrderResponse {
    pub order: PlacedOrder,
}

#[derive(Debug, Deserialize)]
pub struct PlacedOrder {
    pub id: String,
    pub client_order_id: String,
    pub status: OrderStatus,
}

// ── Replace Order
// ─────────────────────────────────────────────────────────────

/// Request body for PUT /api/1.0/orders/{venue_order_id}
///
/// All fields except `client_order_id` are optional — omitted fields are
/// inherited from the original order.
#[derive(Debug, Serialize)]
pub struct ReplaceOrderRequest {
    /// Client-generated unique identifier for idempotency.
    pub client_order_id: String,
    /// New limit price (decimal string, e.g. `"50000.50"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<String>,
    /// New base-currency quantity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_size: Option<String>,
    /// New quote-currency quantity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_size: Option<String>,
    /// Override execution instructions; pass `Some(vec![])` to clear all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_instructions: Option<Vec<ExecutionInstruction>>,
}

/// Response body for PUT /api/1.0/orders/{venue_order_id}
///
/// ```json
/// { "data": [{ "venue_order_id": "…", "client_order_id": "…", "state": "new" }] }
/// ```
#[derive(Debug, Deserialize)]
pub struct ReplaceOrderResponse {
    pub data: Vec<ReplacedOrder>,
}

#[derive(Debug, Deserialize)]
pub struct ReplacedOrder {
    /// New venue order ID assigned after replacement.
    pub venue_order_id: String,
    pub client_order_id: String,
    pub state: OrderStatus,
}

// ── Active Orders
// ─────────────────────────────────────────────────────────────

/// Query parameters for GET /api/1.0/orders/active
#[derive(Debug, Default)]
pub struct ActiveOrdersFilter {
    /// Comma-separated list of symbols, e.g. `"BTC-USD,ETH-USD"`
    pub symbols: Option<String>,
    pub status: Option<Vec<ActiveOrderStatus>>,
    pub types: Option<Vec<OrderType>>,
    pub side: Option<OrderSide>,
    pub cursor: Option<String>,
    /// 1..=300, default 300
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveOrderStatus {
    PendingNew,
    New,
    PartiallyFilled,
}

/// Response body for GET /api/1.0/orders/active
#[derive(Debug, Deserialize, Serialize)]
pub struct ActiveOrdersResponse {
    pub data: Vec<Order>,
    pub metadata: Option<PaginationMetadata>,
}

/// A single order (shared between active and historical responses).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Order {
    pub id: String,
    pub prev_order_id: Option<String>,
    pub client_order_id: Option<String>,
    pub symbol: String,
    pub side: OrderSide,
    #[serde(rename = "type")]
    pub order_type: Option<OrderType>,
    pub base_quantity: Option<String>,
    pub filled_quantity: Option<String>,
    pub remaining_quantity: Option<String>,
    pub quote_quantity: Option<String>,
    pub limit_price: Option<String>,
    pub avg_price: Option<String>,
    pub status: OrderStatus,
    pub reject_reason: Option<String>,
    pub time_in_force: Option<TimeInForce>,
    #[serde(default)]
    pub execution_instructions: Vec<ExecutionInstruction>,
    pub trigger: Option<TriggerDetails>,
    pub take_profit: Option<TpSlDetails>,
    pub stop_loss: Option<TpSlDetails>,
    #[serde(alias = "created_date")]
    pub created_at: Option<u64>,
    #[serde(alias = "updated_date")]
    pub updated_at: Option<u64>,
    #[serde(alias = "completed_date")]
    pub completed_at: Option<u64>,
}

/// Pagination metadata returned alongside order lists.
#[derive(Debug, Deserialize, Serialize)]
pub struct PaginationMetadata {
    pub next_cursor: Option<String>,
}

// ── Historical Orders
// ─────────────────────────────────────────────────────────

/// Query parameters for GET /api/1.0/orders/historical
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HistoricalOrdersFilter {
    pub symbols: Option<String>,
    pub status: Option<Vec<HistoricalOrderStatus>>,
    pub types: Option<Vec<OrderType>>,
    pub start_date: Option<u64>,
    pub end_date: Option<u64>,
    pub cursor: Option<String>,
    /// 1..=1900, default 1900
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoricalOrderStatus {
    Filled,
    Cancelled,
    Rejected,
    Replaced,
}

/// Response body for GET /api/1.0/orders/historical
#[derive(Debug, Deserialize, Serialize)]
pub struct HistoricalOrdersResponse {
    pub data: Vec<Order>,
    pub metadata: Option<PaginationMetadata>,
}

// ── Ticker ────────────────────────────────────────────────────────────────────

/// GET /api/1.0/tickers?symbols=…
#[derive(Debug, Deserialize)]
pub struct TickerResponse {
    pub data: Vec<Ticker>,
}

/// A single ticker entry per symbol.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Ticker {
    pub symbol: String,
    pub bid: String,
    pub ask: String,
    pub mid: String,
    pub last_price: String,
}

// ── Accounts
// ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Account {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub balance: String,
    #[serde(default)]
    pub available_balance: Option<String>,
    #[serde(default)]
    pub available: Option<String>,
}

impl Account {
    pub fn get_available(&self) -> f64 {
        if let Some(ref a) = self.available_balance {
            if let Ok(v) = a.parse::<f64>() {
                return v;
            }
        }
        if let Some(ref a) = self.available {
            if let Ok(v) = a.parse::<f64>() {
                return v;
            }
        }
        self.balance.parse().unwrap_or(0.0)
    }
}

// ── Order Book
// ────────────────────────────────────────────────────────────────

/// GET /api/1.0/public/order-book/{symbol}?limit=N
#[derive(Debug, Deserialize)]
pub struct OrderBookWrapper {
    pub data: OrderBookResponse,
}

#[derive(Debug, Deserialize)]
pub struct OrderBookResponse {
    /// Ask levels sorted by price ascending (best ask first).
    #[serde(default)]
    pub asks: Vec<OrderBookLevel>,
    /// Bid levels sorted by price descending (best bid first).
    #[serde(default)]
    pub bids: Vec<OrderBookLevel>,
}

/// One price level in the order book.
#[derive(Debug, Deserialize)]
pub struct OrderBookLevel {
    /// Price as a decimal string (e.g. `"63500.00"`).
    #[serde(alias = "p")]
    pub price: String,
    /// Aggregated quantity at this price level.
    #[serde(alias = "q")]
    pub quantity: String,
    /// Number of orders at this level.
    #[serde(alias = "no")]
    pub order_count: Option<String>,
}

impl OrderBookLevel {
    /// Parse `price` as f64 — returns 0.0 on error.
    pub fn price_f64(&self) -> f64 { self.price.parse().unwrap_or(0.0) }

    /// Parse `quantity` as f64 — returns 0.0 on error.
    pub fn quantity_f64(&self) -> f64 { self.quantity.parse().unwrap_or(0.0) }
}

impl OrderBookResponse {
    /// Best ask price (lowest ask), or `f64::MAX` if the book is empty.
    pub fn best_ask(&self) -> f64 { self.asks.first().map(|l| l.price_f64()).unwrap_or(f64::MAX) }

    /// Best bid price (highest bid), or `0.0` if the book is empty.
    pub fn best_bid(&self) -> f64 { self.bids.first().map(|l| l.price_f64()).unwrap_or(0.0) }

    /// Quantity at the best bid level, or `0.0` if the book is empty.
    pub fn best_bid_qty(&self) -> f64 { self.bids.first().map(|l| l.quantity_f64()).unwrap_or(0.0) }

    /// Quantity at the best ask level, or `0.0` if the book is empty.
    pub fn best_ask_qty(&self) -> f64 { self.asks.first().map(|l| l.quantity_f64()).unwrap_or(0.0) }

    /// Total resting quantity in the **bids** at a specific price level.
    /// Returns `0.0` if that price level is not present.
    pub fn bid_qty_at_price(&self, price: f64) -> f64 {
        self.bids
            .iter()
            .filter(|l| (l.price_f64() - price).abs() < 1e-8)
            .map(|l| l.quantity_f64())
            .sum()
    }

    /// Total resting quantity in the **asks** at a specific price level.
    /// Returns `0.0` if that price level is not present.
    pub fn ask_qty_at_price(&self, price: f64) -> f64 {
        self.asks
            .iter()
            .filter(|l| (l.price_f64() - price).abs() < 1e-8)
            .map(|l| l.quantity_f64())
            .sum()
    }

    /// Best ask price ignoring the bot's own active sell orders.
    pub fn true_best_ask(&self, active_orders: &[crate::core::models::Order]) -> f64 {
        for ask in &self.asks {
            let p = ask.price_f64();
            let is_our_order = active_orders
                .iter()
                .filter(|o| o.side == crate::core::models::OrderSide::Sell)
                .filter_map(|o| o.limit_price.as_ref())
                .filter_map(|lp| lp.parse::<f64>().ok())
                .any(|lp| (lp - p).abs() < 1e-8);
            if !is_our_order {
                return p;
            }
        }
        self.best_ask()
    }

    /// Best bid price ignoring the bot's own active buy orders.
    pub fn true_best_bid(&self, active_orders: &[crate::core::models::Order]) -> f64 {
        for bid in &self.bids {
            let p = bid.price_f64();
            let is_our_order = active_orders
                .iter()
                .filter(|o| o.side == crate::core::models::OrderSide::Buy)
                .filter_map(|o| o.limit_price.as_ref())
                .filter_map(|lp| lp.parse::<f64>().ok())
                .any(|lp| (lp - p).abs() < 1e-8);
            if !is_our_order {
                return p;
            }
        }
        self.best_bid()
    }

    /// Calculate the Orderbook Imbalance (OBI) over the top `levels`.
    ///
    /// Formula: `OBI = Bid Volume / (Bid Volume + Ask Volume)`
    /// Returns 0.5 if there is no volume on either side to prevent division by
    /// zero.
    ///
    /// A value < 0.5 indicates more selling pressure. A value > 0.5 indicates
    /// more buying pressure.
    pub fn calculate_obi(&self, levels: usize) -> f64 {
        let bid_vol: f64 = self.bids.iter().take(levels).map(|l| l.quantity_f64()).sum();
        let ask_vol: f64 = self.asks.iter().take(levels).map(|l| l.quantity_f64()).sum();
        let total_vol = bid_vol + ask_vol;
        if total_vol <= 0.0 {
            0.5
        } else {
            bid_vol / total_vol
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_obi() {
        let book = OrderBookResponse {
            bids: vec![
                OrderBookLevel {
                    price: "100.0".to_string(),
                    quantity: "10.0".to_string(),
                    order_count: None,
                },
                OrderBookLevel {
                    price: "99.0".to_string(),
                    quantity: "5.0".to_string(),
                    order_count: None,
                },
                OrderBookLevel {
                    price: "98.0".to_string(),
                    quantity: "5.0".to_string(),
                    order_count: None,
                },
            ],
            asks: vec![OrderBookLevel {
                price: "101.0".to_string(),
                quantity: "80.0".to_string(),
                order_count: None,
            }],
        };

        // Top 1 level: Bid vol = 10.0, Ask vol = 80.0. Total = 90.0. OBI = 10 / 90 =
        // 0.111...
        assert!((book.calculate_obi(1) - 0.1111111111111111).abs() < 1e-9);

        // Top 2 levels: Bid vol = 15.0, Ask vol = 80.0. Total = 95.0. OBI = 15 / 95 =
        // 0.15789...
        assert!((book.calculate_obi(2) - 0.15789473684210525).abs() < 1e-9);

        // Empty book
        let empty = OrderBookResponse { bids: vec![], asks: vec![] };
        assert_eq!(empty.calculate_obi(5), 0.5);
    }
}
