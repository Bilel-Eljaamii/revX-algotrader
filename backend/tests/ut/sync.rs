use axum::{routing::get, Json, Router};
use ed25519_dalek::SigningKey;
use rand::{rngs::SysRng, TryRng};
use revx_bot::{
    api::client::RevolutXClient,
    core::{
        db::TradeDb,
        models::{Order, OrderSide, OrderStatus, OrderType},
        sync::{sync_active_orders, sync_historical_orders},
    },
};

use crate::mock_server::MockHttpsServer;

async fn setup_test_client(server: &MockHttpsServer) -> RevolutXClient {
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
async fn test_sync_active_orders_success() {
    let order = Order {
        id: "order_123".into(),
        prev_order_id: None,
        client_order_id: Some("client_123".into()),
        symbol: "USDC-USD".into(),
        side: OrderSide::Buy,
        order_type: Some(OrderType::Limit),
        base_quantity: Some("100.0".into()),
        filled_quantity: Some("0.0".into()),
        remaining_quantity: Some("100.0".into()),
        quote_quantity: Some("100.0".into()),
        limit_price: Some("1.0".into()),
        avg_price: Some("0.0".into()),
        status: OrderStatus::New,
        reject_reason: None,
        time_in_force: None,
        execution_instructions: vec![],
        trigger: None,
        take_profit: None,
        stop_loss: None,
        created_at: Some(1000),
        updated_at: Some(1000),
        completed_at: None,
    };

    let response_body = serde_json::json!({
        "data": [order]
    });

    let app =
        Router::new().route("/api/1.0/orders/active", get(move || async { Json(response_body) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_test_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    sync_active_orders(&client, &db, &["USDC-USD".into()]).await.unwrap();

    // Verify order is in DB using public query_orders
    let orders = db.query_orders("USDC-USD", None, None, 10).unwrap();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].contains("order_123"));
}

#[tokio::test]
async fn test_sync_active_orders_empty_symbols() {
    let server = MockHttpsServer::start(Router::new()).await;
    let client = setup_test_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();

    // Should return immediately without calling API
    sync_active_orders(&client, &db, &[]).await.unwrap();
}

#[tokio::test]
async fn test_sync_active_orders_api_error() {
    let app = Router::new().route(
        "/api/1.0/orders/active",
        get(|| async { (axum::http::StatusCode::INTERNAL_SERVER_ERROR, "fail") }),
    );
    let server = MockHttpsServer::start(app).await;
    let client = setup_test_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();

    // Should log warning but return Ok
    sync_active_orders(&client, &db, &["USDC-USD".into()]).await.unwrap();
}

#[tokio::test]
async fn test_sync_historical_orders_success() {
    let order = Order {
        id: "hist_123".into(),
        prev_order_id: None,
        client_order_id: Some("client_123".into()),
        symbol: "USDC-USD".into(),
        side: OrderSide::Buy,
        order_type: Some(OrderType::Limit),
        base_quantity: Some("100.0".into()),
        filled_quantity: Some("100.0".into()),
        remaining_quantity: Some("0.0".into()),
        quote_quantity: Some("100.0".into()),
        limit_price: Some("1.0".into()),
        avg_price: Some("1.0".into()),
        status: OrderStatus::Filled,
        reject_reason: None,
        time_in_force: None,
        execution_instructions: vec![],
        trigger: None,
        take_profit: None,
        stop_loss: None,
        created_at: Some(1000),
        updated_at: Some(2000),
        completed_at: Some(2000),
    };

    let response_body = serde_json::json!({
        "data": [order]
    });

    let app = Router::new()
        .route("/api/1.0/orders/historical", get(move || async { Json(response_body) }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_test_client(&server).await;
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    sync_historical_orders(&client, &db, &["USDC-USD".into()]).await.unwrap();

    // Verify order is in DB and marked as filled
    let orders = db.query_orders("USDC-USD", None, None, 10).unwrap();
    assert_eq!(orders.len(), 1);
    assert!(orders[0].contains("\"status\":\"filled\""));
}
