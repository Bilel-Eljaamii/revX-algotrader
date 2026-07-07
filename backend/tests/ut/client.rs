use axum::{
    routing::{get, post},
    Json, Router,
};
use ed25519_dalek::SigningKey;
use rand::{rngs::SysRng, TryRng};
use revx_bot::{
    api::client::{build_active_orders_query, RevolutXClient},
    core::models::{
        ActiveOrderStatus, ActiveOrdersFilter, ExecutionInstruction, LimitConfig,
        OrderConfiguration, OrderSide, PlaceOrderRequest,
    },
};

use crate::mock_server::MockHttpsServer;

async fn setup_https_client(server: &MockHttpsServer) -> RevolutXClient {
    let mut csprng = SysRng;
    let mut secret_bytes = [0u8; 32];
    csprng.try_fill_bytes(&mut secret_bytes).unwrap();
    let signing_key = SigningKey::from_bytes(&secret_bytes);

    let cert = reqwest::Certificate::from_pem(&server.cert_pem).unwrap();

    RevolutXClient::builder("test_key".into(), signing_key)
        .base_url(server.uri.clone())
        .add_root_certificate(cert)
        .build()
        .unwrap()
}

#[tokio::test]
async fn test_get_active_orders_success() {
    let response_body = serde_json::json!({
        "data": [
            {
                "id": "order-1",
                "symbol": "BTC-USD",
                "side": "buy",
                "status": "new",
                "order_type": "limit"
            }
        ],
        "metadata": { "next_cursor": null }
    });

    let app =
        Router::new().route("/api/1.0/orders/active", get(move || async { Json(response_body) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let filter = ActiveOrdersFilter { symbols: Some("BTC-USD".into()), ..Default::default() };
    let resp = client.get_active_orders(&filter).await.unwrap();
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].id, "order-1");
}

#[tokio::test]
async fn test_place_order_success() {
    let response_body = serde_json::json!({
        "order": {
            "id": "new-order-id",
            "client_order_id": "cid-1",
            "symbol": "BTC-USD",
            "side": "buy",
            "status": "new"
        }
    });

    let app = Router::new().route(
        "/api/1.0/orders",
        post(move || async { (axum::http::StatusCode::CREATED, Json(response_body)) }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let req = PlaceOrderRequest {
        client_order_id: "cid-1".into(),
        symbol: "BTC-USD".into(),
        side: OrderSide::Buy,
        order_configuration: OrderConfiguration::Limit(LimitConfig {
            base_size: Some("0.1".into()),
            quote_size: None,
            price: "50000".into(),
            execution_instructions: vec![ExecutionInstruction::AllowTaker],
        }),
    };

    let resp = client.place_order(&req).await.unwrap();
    assert_eq!(resp.order.id, "new-order-id");
}

#[tokio::test]
async fn test_get_tickers_success() {
    let response_body = serde_json::json!({
        "data": [
            {
                "symbol": "BTC-USD",
                "bid": "60000.1",
                "ask": "60000.2",
                "mid": "60000.15",
                "last_price": "60000.12"
            }
        ]
    });

    let app = Router::new().route("/api/1.0/tickers", get(move || async { Json(response_body) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let ticker = client.get_tickers("BTC-USD").await.unwrap();
    assert_eq!(ticker.symbol, "BTC-USD");
    assert_eq!(ticker.bid, "60000.1");
}

#[tokio::test]
async fn test_get_order_book_success() {
    let response_body = serde_json::json!({
        "data": {
            "asks": [{"p": "60001", "q": "1", "no": "1"}],
            "bids": [{"p": "60000", "q": "1", "no": "1"}]
        }
    });

    let app = Router::new()
        .route("/api/1.0/public/order-book/BTC-USD", get(move || async { Json(response_body) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let book = client.get_order_book("BTC-USD", 1).await.unwrap();
    assert_eq!(book.asks.len(), 1);
    assert_eq!(book.best_bid(), 60000.0);
}

#[tokio::test]
async fn test_handle_response_error() {
    let app = Router::new().route(
        "/api/1.0/tickers",
        get(move || async { (axum::http::StatusCode::BAD_REQUEST, "invalid symbol") }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let result = client.get_tickers("INVALID").await;
    assert!(result.is_err());
    let err_msg = result.err().unwrap().to_string();
    assert!(err_msg.contains("400"));
    assert!(err_msg.contains("invalid symbol"));
}

#[tokio::test]
async fn test_get_tickers_multi_success() {
    let response_body = serde_json::json!({
        "data": [
            { "symbol": "BTC-USD", "bid": "1.0", "ask": "2.0", "mid": "1.5", "last_price": "1.5" },
            { "symbol": "ETH-USD", "bid": "10.0", "ask": "11.0", "mid": "10.5", "last_price": "10.5" }
        ]
    });
    let app = Router::new().route("/api/1.0/tickers", get(move || async { Json(response_body) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let res = client.get_tickers_multi(&["BTC-USD".into(), "ETH-USD".into()]).await.unwrap();
    assert_eq!(res.len(), 2);
}

#[tokio::test]
async fn test_get_historical_orders_success() {
    let response_body = serde_json::json!({
        "data": [],
        "metadata": { "next_cursor": null }
    });
    let app = Router::new()
        .route("/api/1.0/orders/historical", get(move || async { Json(response_body) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let filter = revx_bot::core::models::HistoricalOrdersFilter::default();
    let res = client.get_historical_orders(&filter).await.unwrap();
    assert!(res.data.is_empty());
}

#[tokio::test]
async fn test_handle_response_deserialization_error() {
    // Return something that isn't a valid TickerResponse (e.g. missing 'data'
    // field)
    let app = Router::new().route(
        "/api/1.0/tickers",
        get(move || async { Json(serde_json::json!({"wrong": "field"})) }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let res = client.get_tickers("BTC-USD").await;
    assert!(res.is_err());
    assert!(res.err().unwrap().to_string().contains("failed to deserialise"));
}

#[test]
fn test_client_with_base_url() {
    let mut csprng = SysRng;
    let mut secret_bytes = [0u8; 32];
    csprng.try_fill_bytes(&mut secret_bytes).unwrap();
    let signing_key = SigningKey::from_bytes(&secret_bytes);

    let client = RevolutXClient::new("k".into(), signing_key).unwrap();
    let _client = client.with_base_url("http://localhost:1234".into());
    // verify it changed (it's private but we use it in tests if we want,
    // or just hit the code path)
}

#[test]
fn test_build_active_orders_query_full() {
    let f = ActiveOrdersFilter {
        symbols: Some("BTC-USD".into()),
        status: Some(vec![ActiveOrderStatus::New]),
        types: None,
        side: Some(OrderSide::Buy),
        cursor: Some("c1".into()),
        limit: Some(10),
    };
    let q = build_active_orders_query(&f);
    assert!(q.contains("symbols=BTC-USD"));
    assert!(q.contains("status=new"));
    assert!(q.contains("side=buy"));
    assert!(q.contains("cursor=c1"));
    assert!(q.contains("limit=10"));
}

#[test]
fn test_build_historical_orders_query_full() {
    let f = revx_bot::core::models::HistoricalOrdersFilter {
        symbols: Some("ETH-USD".into()),
        status: Some(vec![revx_bot::core::models::HistoricalOrderStatus::Filled]),
        types: None,
        start_date: Some(100),
        end_date: Some(200),
        cursor: Some("c2".into()),
        limit: Some(20),
    };
    let q = revx_bot::api::client::build_historical_orders_query(&f);
    assert!(q.contains("symbols=ETH-USD"));
    assert!(q.contains("status=filled"));
    assert!(q.contains("start_date=100"));
    assert!(q.contains("end_date=200"));
    assert!(q.contains("cursor=c2"));
    assert!(q.contains("limit=20"));
}

#[tokio::test]
async fn test_handle_response_error_with_body() {
    let app = Router::new().route(
        "/api/1.0/tickers",
        get(|| async { (axum::http::StatusCode::BAD_REQUEST, "insufficient funds") }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let res = client.get_tickers("BTC-USD").await;
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    assert!(err_msg.contains("400"));
    assert!(err_msg.contains("insufficient funds"));
}

#[test]
fn test_build_active_orders_query_complex() {
    let f = revx_bot::core::models::ActiveOrdersFilter {
        status: Some(vec![
            revx_bot::core::models::ActiveOrderStatus::New,
            revx_bot::core::models::ActiveOrderStatus::PartiallyFilled,
        ]),
        types: Some(vec![revx_bot::core::models::OrderType::Limit]),
        ..Default::default()
    };
    let q = revx_bot::api::client::build_active_orders_query(&f);
    assert!(q.contains("status=new,partially_filled"));
    assert!(q.contains("order_types=limit"));
}

#[test]
fn test_build_historical_orders_query_complex() {
    let f = revx_bot::core::models::HistoricalOrdersFilter {
        status: Some(vec![revx_bot::core::models::HistoricalOrderStatus::Filled]),
        types: Some(vec![revx_bot::core::models::OrderType::Market]),
        ..Default::default()
    };
    let q = revx_bot::api::client::build_historical_orders_query(&f);
    assert!(q.contains("status=filled"));
    assert!(q.contains("order_types=market"));
}

#[tokio::test]
async fn test_get_order_book_depth_clamping() {
    let app = Router::new().route(
        "/api/1.0/public/order-book/BTC-USD",
        get(
            |axum::extract::Query(params): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >| async move {
                let limit = params.get("limit").unwrap();
                assert_eq!(limit, "20"); // 100 clamped to 20
                Json(serde_json::json!({ "data": { "asks": [], "bids": [] } }))
            },
        ),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let res = client.get_order_book("BTC-USD", 100).await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn test_get_tickers_multi_empty() {
    let server = MockHttpsServer::start(Router::new()).await;
    let client = setup_https_client(&server).await;

    let res = client.get_tickers_multi(&[]).await.unwrap();
    assert!(res.is_empty());
}

#[tokio::test]
async fn test_get_active_orders_no_metadata() {
    let app = Router::new().route(
        "/api/1.0/orders/active",
        get(|| async {
            Json(serde_json::json!({ "data": [] })) // No metadata
        }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let res = client.get_active_orders(&Default::default()).await.unwrap();
    assert!(res.data.is_empty());
    assert!(res.metadata.is_none());
}

// ── cancel_order
// ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_cancel_order_success() {
    let app = Router::new().route(
        "/api/1.0/orders/{id}",
        axum::routing::delete(|| async { axum::http::StatusCode::NO_CONTENT }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let result = client.cancel_order("order-abc").await;
    assert!(result.is_ok(), "cancel should succeed on 2xx: {:?}", result);
}

#[tokio::test]
async fn test_cancel_order_404_treated_as_success() {
    // A 404 means the order is already gone — should be treated as Ok
    let app = Router::new().route(
        "/api/1.0/orders/{id}",
        axum::routing::delete(|| async { axum::http::StatusCode::NOT_FOUND }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let result = client.cancel_order("already-gone").await;
    assert!(result.is_ok(), "404 on cancel should be treated as success");
}

#[tokio::test]
async fn test_cancel_order_error_propagates() {
    let app = Router::new().route(
        "/api/1.0/orders/{id}",
        axum::routing::delete(|| async { (axum::http::StatusCode::FORBIDDEN, "not authorised") }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let result = client.cancel_order("order-xyz").await;
    assert!(result.is_err(), "non-2xx non-404 should be an error");
    assert!(result.err().unwrap().to_string().contains("403"));
}

// ── get_order
// ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_get_order_success() {
    let app = Router::new().route(
        "/api/1.0/orders/{id}",
        axum::routing::get(|| async {
            Json(serde_json::json!({
                "data": {
                    "id": "order-1",
                    "client_order_id": "cid-1",
                    "symbol": "BTC-USD",
                    "side": "buy",
                    "status": "filled",
                    "order_type": "limit",
                    "filled_quantity": "0.1",
                    "created_at": 1_700_000_000_000_i64
                }
            }))
        }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let order = client.get_order("order-1").await.unwrap();
    assert_eq!(order.id, "order-1");
    assert_eq!(order.status, revx_bot::core::models::OrderStatus::Filled);
}

#[tokio::test]
async fn test_get_order_error_propagates() {
    let app = Router::new().route(
        "/api/1.0/orders/{id}",
        axum::routing::get(|| async { (axum::http::StatusCode::NOT_FOUND, "order not found") }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let result = client.get_order("ghost-order").await;
    assert!(result.is_err(), "404 on get_order should be an error");
    assert!(result.err().unwrap().to_string().contains("404"));
}
