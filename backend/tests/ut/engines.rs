use axum::{
    routing::{get, post},
    Json, Router,
};
use ed25519_dalek::SigningKey;
use rand::{rngs::SysRng, TryRng};
use revx_bot::{
    api::client::RevolutXClient,
    core::{config::DummyConfig, db::TradeDb},
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
async fn test_dummy_engine_run_shutdown() {
    let app = Router::new()
        .route("/api/1.0/tickers", get(|| async { Json(serde_json::json!({"data":[]})) }))
        .route("/api/1.0/orders/active", get(|| async { Json(serde_json::json!({"data":[]})) }))
        .route("/api/1.0/public/order-book/BTC-USD", get(|| async {
            Json(serde_json::json!({ "data": { "asks": [{"p": "1.0", "q": "1", "no": "1"}], "bids": [{"p": "0.9", "q": "1", "no": "1"}] } }))
        }));
    let server = MockHttpsServer::start(app).await;
    let client = setup_https_client(&server).await;

    let config = DummyConfig {
        api_key: "key".into(),
        private_key_path: "".into(),
        poll_interval_ms: 10,
        db_path: ":memory:".into(),
        port: 0,
        base_url: None,
        symbols: vec![revx_bot::core::config::SymbolConfig {
            symbol: "BTC-USD".into(),
            buy_trigger_price: 1.0,
            sell_trigger_price: 2.0,
            revert_price: 1.5,
            trade_size_base: 0.1,
            trade_size_quote: 0.0,
            tick_size: 0.0001,
        }],
    };

    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    let (tx, rx) = tokio::sync::watch::channel(false);

    let engine_handle = tokio::spawn(async move {
        revx_bot::strategies::dummy::engine::run(&client, &config, db, rx, false).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    tx.send(true).unwrap();

    engine_handle.await.unwrap();
}
