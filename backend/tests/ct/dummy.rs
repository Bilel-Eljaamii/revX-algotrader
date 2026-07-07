use axum::{routing::get, Router};
use ed25519_dalek::SigningKey;
use rand::{rngs::SysRng, TryRng};
use revx_bot::{
    api::client::RevolutXClientBuilder,
    core::{config::SymbolConfig, db::TradeDb},
    strategies::dummy::strategy::run_tick,
};
use serde_json::json;
use tempfile::tempdir;

use crate::mock_server::MockHttpsServer;

#[tokio::test]
async fn test_dummy_strategy_tick() {
    // 1. Start mock server
    let app = Router::new()
        .route("/api/1.0/orders/active", get(|| async { axum::Json(json!({ "data": [] })) }))
        .route(
            "/api/1.0/public/order-book/BTC-USD",
            get(|| async { axum::Json(json!({ "data": { "bids": [], "asks": [] } })) }),
        );

    let server = MockHttpsServer::start(app).await;
    let cert = reqwest::Certificate::from_pem(&server.cert_pem).unwrap();

    // 2. Init client
    let mut csprng = SysRng;
    let mut secret_bytes = [0u8; 32];
    csprng.try_fill_bytes(&mut secret_bytes).unwrap();
    let signing_key = SigningKey::from_bytes(&secret_bytes);

    let client = RevolutXClientBuilder::new("test_api_key".to_string(), signing_key)
        .base_url(server.uri)
        .add_root_certificate(cert)
        .build()
        .unwrap();

    // 3. Init DB
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let db = TradeDb::open(db_path.to_str().unwrap()).unwrap();
    db.migrate().unwrap();

    // 4. Run tick
    let sym_cfg = SymbolConfig {
        symbol: "BTC-USD".to_string(),
        buy_trigger_price: 50000.0,
        sell_trigger_price: 52000.0,
        revert_price: 51000.0,
        trade_size_base: 0.01,
        trade_size_quote: 500.0,
        tick_size: 0.1,
    };

    let res = run_tick(&client, &sym_cfg, &db, true).await;
    assert!(res.is_ok());
}
