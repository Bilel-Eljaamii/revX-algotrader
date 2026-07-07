use axum::{
    http::{Request, StatusCode},
    routing::get,
    Json, Router,
};
use ed25519_dalek::SigningKey;
use rand::{rngs::SysRng, TryRng};
use revx_bot::{
    api::{
        client::RevolutXClient,
        proxy::{build_router, run_proxy},
    },
    core::db::TradeDb,
};
use tokio::sync::watch;
use tower::ServiceExt;

// use std::sync::Arc;
use crate::mock_server::MockHttpsServer;

async fn setup_proxy_client(server: &MockHttpsServer) -> RevolutXClient {
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
async fn test_proxy_local_trades_handler() {
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    // Insert a mock order string
    let order_json = serde_json::json!({ "id": "1", "symbol": "BTC-USD" }).to_string();
    db.execute_raw_with_args(
        "INSERT INTO orders (id, symbol, side, order_type, status, created_at, raw_json) VALUES ('1', 'BTC-USD', 'buy', 'limit', 'filled', 1000, ?)",
        &[&order_json]
    ).unwrap();

    let server = MockHttpsServer::start(Router::new()).await;
    let client = setup_proxy_client(&server).await;
    let router = build_router(client, db, vec!["BTC-USD".into()]);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/proxy/local-trades?symbols=BTC-USD")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_proxy_local_trades_db_error() {
    let db = TradeDb::open(":memory:").unwrap();
    // Do not migrate, will cause error

    let server = MockHttpsServer::start(Router::new()).await;
    let client = setup_proxy_client(&server).await;
    let router = build_router(client, db, vec![]);

    let response = router
        .oneshot(
            Request::builder().uri("/proxy/local-trades").body(axum::body::Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_proxy_active_orders_handler() {
    let app = Router::new()
        .route("/api/1.0/orders/active", get(|| async { Json(serde_json::json!({"data": []})) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_proxy_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    let router = build_router(client, db, vec![]);

    let response = router
        .oneshot(
            Request::builder().uri("/proxy/active-orders").body(axum::body::Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_proxy_ticker_handler() {
    let app = Router::new()
        .route("/api/1.0/tickers", get(|| async { Json(serde_json::json!({ "data": [{ "symbol": "BTC-USD", "bid": "1.0", "ask": "1.1", "mid": "1.05", "last_price": "1.0" }] })) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_proxy_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    let router = build_router(client, db, vec![]);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/proxy/ticker?symbol=BTC-USD")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_proxy_symbols_handler() {
    let server = MockHttpsServer::start(Router::new()).await;
    let client = setup_proxy_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    let router = build_router(client, db, vec!["BTC-USD".into()]);

    let response = router
        .oneshot(Request::builder().uri("/proxy/symbols").body(axum::body::Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_run_proxy_shutdown() {
    let server = MockHttpsServer::start(Router::new()).await;
    let client = setup_proxy_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();

    let (tx, rx) = watch::channel(false);

    // Use a port that is likely free
    let _addr = "127.0.0.1:0";

    // We need to know which port was actually bound to test run_proxy fully,
    // but run_proxy doesn't return it.
    // For coverage, we can just start it and shut it down.

    let handle =
        tokio::spawn(async move { run_proxy(client, db, vec![], "127.0.0.1:0", rx).await });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    tx.send(true).unwrap();

    let result = handle.await.unwrap();
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_proxy_active_orders_error() {
    let app = Router::new().route(
        "/api/1.0/orders/active",
        get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "fail") }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_proxy_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    let router = build_router(client, db, vec![]);

    let response = router
        .oneshot(
            Request::builder().uri("/proxy/active-orders").body(axum::body::Body::empty()).unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_proxy_ticker_error() {
    let app = Router::new()
        .route("/api/1.0/tickers", get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "fail") }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_proxy_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    let router = build_router(client, db, vec![]);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/proxy/ticker?symbol=BTC-USD")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn test_proxy_historical_orders_handler() {
    let app = Router::new().route(
        "/api/1.0/orders/historical",
        get(|| async { Json(serde_json::json!({"data": []})) }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_proxy_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    let router = build_router(client, db, vec![]);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/proxy/historical-orders")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_proxy_historical_orders_error() {
    let app = Router::new().route(
        "/api/1.0/orders/historical",
        get(|| async { (StatusCode::INTERNAL_SERVER_ERROR, "fail") }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_proxy_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    let router = build_router(client, db, vec![]);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/proxy/historical-orders")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
}
