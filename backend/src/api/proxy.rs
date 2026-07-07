use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tokio::sync::watch::Receiver;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    api::client::RevolutXClient,
    core::{
        db::TradeDb,
        models::{ActiveOrdersFilter, HistoricalOrdersFilter},
    },
};

#[derive(Clone)]
pub struct ProxyState {
    pub client: RevolutXClient,
    pub db: TradeDb,
    pub symbols: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct HistoricalQuery {
    pub symbols: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TickerQuery {
    pub symbol: String,
}

pub fn build_router(client: RevolutXClient, db: TradeDb, symbols: Vec<String>) -> Router {
    let state = ProxyState { client, db, symbols };
    Router::new()
        .route("/proxy/local-trades", get(local_trades_handler))
        .route("/proxy/historical-orders", get(historical_orders_handler))
        .route("/proxy/active-orders", get(active_orders_handler))
        .route("/proxy/ticker", get(ticker_handler))
        .route("/proxy/symbols", get(symbols_handler))
        .route("/health", get(health_handler))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .with_state(state)
}

async fn local_trades_handler(
    State(state): State<ProxyState>,
    Query(q): Query<HistoricalQuery>,
) -> Response {
    let symbol = q.symbols.as_deref().unwrap_or("");
    match state.db.query_orders(symbol, None, None, 100) {
        Ok(rows) => {
            let trades: Vec<serde_json::Value> =
                rows.into_iter().filter_map(|s| serde_json::from_str(&s).ok()).collect();
            (axum::http::StatusCode::OK, Json(serde_json::json!({ "data": trades })))
                .into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn historical_orders_handler(
    State(state): State<ProxyState>,
    Query(q): Query<HistoricalQuery>,
) -> Response {
    let filter = HistoricalOrdersFilter {
        symbols: q.symbols,
        limit: q.limit,
        cursor: q.cursor,
        ..Default::default()
    };
    match state.client.get_historical_orders(&filter).await {
        Ok(resp) => (axum::http::StatusCode::OK, Json(resp)).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn active_orders_handler(
    State(state): State<ProxyState>,
    Query(q): Query<HistoricalQuery>,
) -> Response {
    let filter = ActiveOrdersFilter {
        symbols: q.symbols,
        limit: q.limit,
        cursor: q.cursor,
        ..Default::default()
    };
    match state.client.get_active_orders(&filter).await {
        Ok(resp) => (axum::http::StatusCode::OK, Json(resp)).into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn ticker_handler(State(state): State<ProxyState>, Query(q): Query<TickerQuery>) -> Response {
    match state.client.get_tickers(&q.symbol).await {
        Ok(ticker) => (axum::http::StatusCode::OK, Json(serde_json::json!({ "data": ticker })))
            .into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn symbols_handler(State(state): State<ProxyState>) -> Response {
    (axum::http::StatusCode::OK, Json(serde_json::json!({ "data": state.symbols }))).into_response()
}

async fn health_handler() -> Response {
    (axum::http::StatusCode::OK, Json(serde_json::json!({ "status": "ok", "uptime": 0 })))
        .into_response()
}

pub async fn run_proxy(
    client: RevolutXClient,
    db: TradeDb,
    symbols: Vec<String>,
    addr: &str,
    mut shutdown: Receiver<bool>,
) -> anyhow::Result<()> {
    let app = build_router(client, db, symbols);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown.changed().await;
        })
        .await?;
    Ok(())
}
