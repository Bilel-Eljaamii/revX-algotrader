//! HTTP client for the Revolut X REST API.
//!
//! `RevolutXClient` is cheap to clone (all fields are Arc-backed internally
//! by reqwest). A single instance is shared across the trading engine and the
//! proxy server.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use ed25519_dalek::SigningKey;
use reqwest::{
    header::{HeaderMap, HeaderValue, CONTENT_TYPE},
    Client,
};
use tracing::{debug, error, instrument};

use crate::{
    api::auth,
    core::models::{
        ActiveOrdersFilter, ActiveOrdersResponse, HistoricalOrdersFilter, HistoricalOrdersResponse,
        OrderBookResponse, PlaceOrderRequest, PlaceOrderResponse, ReplaceOrderRequest,
        ReplaceOrderResponse, Ticker, TickerResponse,
    },
};

/// Thin, latency-optimised wrapper around `reqwest::Client`.
///
/// # Design choices for minimal latency
/// - `reqwest::Client` is kept alive across calls (connection pool, keep-alive)
/// - Headers common to every request are pre-built once
/// - Signing happens on the calling task (no channel overhead)
/// - Timeouts are tuned aggressively (connect 2 s, request 5 s)
#[derive(Clone)]
pub struct RevolutXClient {
    http: Client,
    api_key: String,
    signing_key: SigningKey,
    base_url: String,
}

pub struct RevolutXClientBuilder {
    api_key: String,
    signing_key: SigningKey,
    base_url: String,
    root_cert: Option<reqwest::Certificate>,
}

impl RevolutXClientBuilder {
    pub fn new(api_key: String, signing_key: SigningKey) -> Self {
        Self {
            api_key,
            signing_key,
            base_url: "https://revx.revolut.com".to_string(),
            root_cert: None,
        }
    }

    pub fn base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    pub fn add_root_certificate(mut self, cert: reqwest::Certificate) -> Self {
        self.root_cert = Some(cert);
        self
    }

    pub fn build(self) -> Result<RevolutXClient> {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let mut builder = Client::builder()
            .default_headers(default_headers)
            .connection_verbose(false)
            .tcp_keepalive(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(2))
            .timeout(std::time::Duration::from_secs(5))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .https_only(true)
            .use_rustls_tls()
            .http2_prior_knowledge(); // attempt HTTP/2 — reduces round-trips

        if let Some(cert) = self.root_cert {
            // For tests, allowing prior_knowledge with custom certs might conflict if the
            // mock server isn't h2c, so we turn it off for tests with custom
            // cert But builder doesn't let us easily unset it. We'll leave it
            // or rebuild without it. Actually, we'll recreate the builder to
            // drop prior_knowledge if testing.
            let mut test_builder = Client::builder()
                .tcp_keepalive(std::time::Duration::from_secs(30))
                .connect_timeout(std::time::Duration::from_secs(2))
                .timeout(std::time::Duration::from_secs(5))
                .pool_idle_timeout(std::time::Duration::from_secs(90))
                .https_only(true)
                .use_rustls_tls();

            test_builder = test_builder.add_root_certificate(cert);
            builder = test_builder;
        }

        let http = builder.build().context("failed to build HTTP client")?;

        Ok(RevolutXClient {
            http,
            api_key: self.api_key,
            signing_key: self.signing_key,
            base_url: self.base_url,
        })
    }
}

impl RevolutXClient {
    pub fn builder(api_key: String, signing_key: SigningKey) -> RevolutXClientBuilder {
        RevolutXClientBuilder::new(api_key, signing_key)
    }

    /// Create a new client.  Call once at startup, then clone freely.
    pub fn new(api_key: String, signing_key: SigningKey) -> Result<Self> {
        Self::builder(api_key, signing_key).build()
    }

    /// Set an alternative base URL (useful for testing).
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    // ── GET /api/1.0/orders/active ─────────────────────────────────────────

    /// Fetch active orders, applying the given filters.
    #[instrument(skip(self), name = "get_active_orders")]
    pub async fn get_active_orders(
        &self,
        filter: &ActiveOrdersFilter,
    ) -> Result<ActiveOrdersResponse> {
        let path = "/api/1.0/orders/active";
        let query = build_active_orders_query(filter);
        let (ts, sig) = self.sign("GET", path, &query, "");

        let url = format!("{}{}", self.base_url, path);
        let url = if query.is_empty() { url } else { format!("{}?{}", url, query) };
        debug!(url, query, "→ GET active orders");

        let resp = self
            .http
            .get(&url)
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .send()
            .await
            .context("GET active orders: network error")?;

        handle_response(resp, "GET active orders").await
    }

    // ── POST /api/1.0/orders ───────────────────────────────────────────────

    /// Place a new order.
    #[instrument(skip(self, order), fields(symbol = %order.symbol, side = ?order.side, cid = %order.client_order_id, cfg = ?order.order_configuration), name = "place_order")]
    pub async fn place_order(&self, order: &PlaceOrderRequest) -> Result<PlaceOrderResponse> {
        let path = "/api/1.0/orders";
        let body = serde_json::to_string(order).context("serialize PlaceOrderRequest")?;
        let (ts, sig) = self.sign("POST", path, "", &body);

        debug!(path, "→ POST place order");

        let resp = self
            .http
            .post(format!("{}{}", self.base_url, path))
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .context("POST place order: network error")?;

        handle_response(resp, "POST place order").await
    }

    // ── DELETE /api/1.0/orders/{id} ───────────────────────────────────────

    /// Cancel an active order by its exchange order ID.
    ///
    /// A 404 is treated as success (order already gone).
    #[instrument(skip(self), fields(order_id), name = "cancel_order")]
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let path = format!("/api/1.0/orders/{order_id}");
        let (ts, sig) = self.sign("DELETE", &path, "", "");

        debug!(path, "→ DELETE cancel order");

        let resp = self
            .http
            .delete(format!("{}{}", self.base_url, path))
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .send()
            .await
            .context("DELETE cancel order: network error")?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            debug!(order_id, "cancel_order: order already gone (404)");
            return Ok(());
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            error!(status = %status, body, "cancel order failed");
            anyhow::bail!("DELETE order returned HTTP {status}: {body}");
        }
        Ok(())
    }

    // ── PUT /api/1.0/orders/{id} (replace) ──────────────────────────────────

    /// Replace an active order's price (and optionally size) atomically.
    ///
    /// Uses the official `PUT /api/1.0/orders/{venue_order_id}` endpoint.
    /// The exchange cancels the original order and creates a new one in a
    /// single atomic operation.  The returned `ReplaceOrderResponse` contains
    /// the **new** `venue_order_id` that callers should persist.
    ///
    /// Only the fields that are `Some` are sent — omitted fields are inherited
    /// from the original order.
    #[instrument(skip(self), fields(order_id, new_price), name = "replace_order")]
    pub async fn replace_order(
        &self,
        order_id: &str,
        new_price: &str,
        base_size: Option<&str>,
    ) -> Result<ReplaceOrderResponse> {
        let path = format!("/api/1.0/orders/{order_id}");

        let req = ReplaceOrderRequest {
            client_order_id: uuid::Uuid::now_v7().to_string(),
            price: Some(new_price.to_string()),
            base_size: base_size.map(|s| s.to_string()),
            quote_size: None,
            execution_instructions: None,
        };
        let body = serde_json::to_string(&req).context("serialize ReplaceOrderRequest")?;

        let (ts, sig) = self.sign("PUT", &path, "", &body);

        debug!(path, body, "→ PUT replace order");

        let resp = self
            .http
            .put(format!("{}{}", self.base_url, path))
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .context("PUT replace order: network error")?;

        handle_response(resp, "PUT replace order").await
    }

    // ── GET /api/1.0/balances ──────────────────────────────────────────────

    #[instrument(skip(self), name = "get_balances")]
    pub async fn get_balances(&self) -> Result<Vec<crate::core::models::Account>> {
        let path = "/api/1.0/balances";
        let (ts, sig) = self.sign("GET", path, "", "");

        let url = format!("{}{}", self.base_url, path);
        debug!(url, "→ GET balances");

        let resp = self
            .http
            .get(&url)
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .send()
            .await
            .context("GET accounts: network error")?;

        let json: serde_json::Value = handle_response(resp, "GET accounts").await?;
        if let Some(_arr) = json.as_array() {
            Ok(serde_json::from_value(json).unwrap_or_default())
        } else if let Some(_arr) = json.get("data").and_then(|d| d.as_array()) {
            Ok(serde_json::from_value(json.get("data").unwrap().clone()).unwrap_or_default())
        } else {
            Ok(vec![])
        }
    }

    // ── GET /api/1.0/orders/{id} ──────────────────────────────────────────

    /// Fetch a single order by its exchange order ID.
    ///
    /// Used by the strategy to confirm whether a disappeared entry was
    /// `filled` or `cancelled`/`rejected`.
    #[instrument(skip(self), fields(order_id), name = "get_order")]
    pub async fn get_order(&self, order_id: &str) -> Result<crate::core::models::Order> {
        let path = format!("/api/1.0/orders/{order_id}");
        let (ts, sig) = self.sign("GET", &path, "", "");

        let url = format!("{}{}", self.base_url, path);
        debug!(url, "→ GET order");

        let resp = self
            .http
            .get(&url)
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .send()
            .await
            .context("GET order: network error")?;

        #[derive(serde::Deserialize)]
        struct GetOrderResponse {
            data: crate::core::models::Order,
        }
        let body: GetOrderResponse = handle_response(resp, "GET order").await?;
        Ok(body.data)
    }

    // ── GET /api/1.0/orders/historical ──────────────────────────────────────
    // Used by the proxy server (frontend historical orders)

    /// Fetch historical orders, applying the given filters.
    #[instrument(skip(self), name = "get_historical_orders")]
    pub async fn get_historical_orders(
        &self,
        filter: &HistoricalOrdersFilter,
    ) -> Result<HistoricalOrdersResponse> {
        let path = "/api/1.0/orders/historical";
        let query = build_historical_orders_query(filter);
        let (ts, sig) = self.sign("GET", path, &query, "");

        let url = format!("{}{}", self.base_url, path);
        let url = if query.is_empty() { url } else { format!("{}?{}", url, query) };
        debug!(url, query, "→ GET historical orders");

        let resp = self
            .http
            .get(&url)
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .send()
            .await
            .context("GET historical orders: network error")?;

        handle_response(resp, "GET historical orders").await
    }

    // ── GET /api/1.0/tickers ──────────────────────────────────────────────

    /// Fetch the current ticker for a specific symbol.
    ///
    /// Returns the first matching `Ticker`.  Used by the strategy to log
    /// bid/ask and validate that the market is live.
    #[instrument(skip(self), name = "get_tickers")]
    pub async fn get_tickers(&self, symbol: &str) -> Result<Ticker> {
        let path = "/api/1.0/tickers";
        let query = format!("symbols={symbol}");
        let (ts, sig) = self.sign("GET", path, &query, "");

        let url = format!("{}{}", self.base_url, path);
        let url = if query.is_empty() { url } else { format!("{}?{}", url, query) };
        debug!(url, query, "→ GET tickers");

        let resp = self
            .http
            .get(&url)
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .send()
            .await
            .context("GET tickers: network error")?;

        let ticker_resp: TickerResponse = handle_response(resp, "GET tickers").await?;
        ticker_resp.data.into_iter().next().context("GET tickers: no ticker data returned")
    }

    /// Fetch tickers for multiple symbols in a single request.
    /// This helps avoid rate limits when scanning triangles.
    #[instrument(skip(self), name = "get_tickers_multi")]
    pub async fn get_tickers_multi(&self, symbols: &[String]) -> Result<Vec<Ticker>> {
        if symbols.is_empty() {
            return Ok(vec![]);
        }
        let path = "/api/1.0/tickers";
        let joined = symbols.join(",");
        let query = format!("symbols={joined}");
        let (ts, sig) = self.sign("GET", path, &query, "");

        let url = format!("{}{}", self.base_url, path);
        let url = if query.is_empty() { url } else { format!("{}?{}", url, query) };
        debug!(url, query, "→ GET tickers multi");

        let resp = self
            .http
            .get(&url)
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .send()
            .await
            .context("GET tickers multi: network error")?;

        let ticker_resp: TickerResponse = handle_response(resp, "GET tickers multi").await?;
        Ok(ticker_resp.data)
    }

    // ── GET /api/1.0/public/order-book/{symbol} ────────────────────────────

    /// Fetch the order book snapshot for a symbol.
    ///
    /// `depth` controls how many price levels to return (1–20, default 20).
    /// Returns best bid/ask immediately via
    /// `OrderBookResponse::best_bid/ask()`.
    #[instrument(skip(self), name = "get_order_book")]
    pub async fn get_order_book(&self, symbol: &str, depth: u8) -> Result<OrderBookResponse> {
        let depth = depth.clamp(1, 20);
        let path_symbol = symbol.replace('/', "-");
        let path = format!("/api/1.0/public/order-book/{path_symbol}");
        let query = format!("limit={depth}");
        let (ts, sig) = self.sign("GET", &path, &query, "");

        let url = format!("{}{}", self.base_url, path);
        let url = if query.is_empty() { url } else { format!("{}?{}", url, query) };
        debug!(url, query, "→ GET order book");

        let resp = self
            .http
            .get(&url)
            .header("X-Revx-API-Key", &self.api_key)
            .header("X-Revx-Timestamp", &ts)
            .header("X-Revx-Signature", &sig)
            .send()
            .await
            .context("GET order book: network error")?;

        let wrapper: crate::core::models::OrderBookWrapper =
            handle_response(resp, "GET order book").await?;
        Ok(wrapper.data)
    }

    // ── Internal signing helper ────────────────────────────────────────────

    /// Returns `(timestamp_str, base64_signature)`.
    fn sign(&self, method: &str, path: &str, query: &str, body: &str) -> (String, String) {
        let ts = now_ms();
        let sig = auth::sign_request(&self.signing_key, &ts, method, path, query, body);
        (ts, sig)
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn now_ms() -> String {
    let mut ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_millis();

    // Subtract 1.5 seconds to account for slight clock drifts ahead of Revolut X
    // servers
    if ms > 1500 {
        ms -= 1500;
    }

    ms.to_string()
}

/// Deserialize a successful response or propagate the API error body.
async fn handle_response<T: serde::de::DeserializeOwned>(
    resp: reqwest::Response,
    label: &str,
) -> Result<T> {
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        error!(status = %status, body, "{label} failed");
        bail!("{label} returned HTTP {status}: {body}");
    }
    resp.json::<T>().await.with_context(|| format!("{label}: failed to deserialise response body"))
}

/// Build a `?key=value&…` query string for active orders.
pub fn build_active_orders_query(f: &ActiveOrdersFilter) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = &f.symbols {
        parts.push(format!("symbols={s}"));
    }
    if let Some(statuses) = &f.status {
        let joined = statuses
            .iter()
            .map(|s| serde_json::to_string(s).unwrap().trim_matches('"').to_owned())
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("status={joined}"));
    }
    if let Some(types) = &f.types {
        let joined = types
            .iter()
            .map(|t| serde_json::to_string(t).unwrap().trim_matches('"').to_owned())
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("order_types={joined}"));
    }
    if let Some(side) = &f.side {
        let s = serde_json::to_string(side).unwrap();
        parts.push(format!("side={}", s.trim_matches('"')));
    }
    if let Some(c) = &f.cursor {
        parts.push(format!("cursor={c}"));
    }
    if let Some(l) = f.limit {
        parts.push(format!("limit={l}"));
    }
    parts.join("&")
}

/// Build a query string for historical orders.
pub fn build_historical_orders_query(f: &HistoricalOrdersFilter) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = &f.symbols {
        parts.push(format!("symbols={s}"));
    }
    if let Some(statuses) = &f.status {
        let joined = statuses
            .iter()
            .map(|s| serde_json::to_string(s).unwrap().trim_matches('"').to_owned())
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("status={joined}"));
    }
    if let Some(types) = &f.types {
        let joined = types
            .iter()
            .map(|t| serde_json::to_string(t).unwrap().trim_matches('"').to_owned())
            .collect::<Vec<_>>()
            .join(",");
        parts.push(format!("order_types={joined}"));
    }
    if let Some(d) = f.start_date {
        parts.push(format!("start_date={d}"));
    }
    if let Some(d) = f.end_date {
        parts.push(format!("end_date={d}"));
    }
    if let Some(c) = &f.cursor {
        parts.push(format!("cursor={c}"));
    }
    if let Some(l) = f.limit {
        parts.push(format!("limit={l}"));
    }
    parts.join("&")
}

// Tests migrated to backend/tests/integration_tests.rs
