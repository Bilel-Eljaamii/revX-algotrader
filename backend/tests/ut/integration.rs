use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::{SigningKey, Verifier, VerifyingKey};
use revx_bot::api::auth;

/// Create a deterministic test key from a fixed seed.
fn test_key() -> SigningKey {
    let seed: [u8; 32] = [
        0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec, 0x2c,
        0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03, 0x1c, 0xae,
        0x7f, 0x60,
    ];
    SigningKey::from_bytes(&seed)
}

#[test]
fn auth_sign_and_verify_roundtrip() {
    let signing_key = test_key();
    let verifying_key: VerifyingKey = signing_key.verifying_key();

    let timestamp = "1765360896219";
    let method = "POST";
    let path = "/api/1.0/orders";
    let query = "";
    let body = r#"{"client_order_id":"abc","symbol":"BTC-USD","side":"buy","order_configuration":{"limit":{"base_size":"0.1","price":"90000.1"}}}"#;

    let sig_b64 = auth::sign_request(&signing_key, timestamp, method, path, query, body);

    let sig_bytes = B64.decode(&sig_b64).expect("valid base64");
    let sig_arr: [u8; 64] = sig_bytes.try_into().expect("64-byte signature");
    let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);

    let message = format!("{timestamp}{method}{path}{query}{body}");
    verifying_key.verify(message.as_bytes(), &signature).expect("signature must be valid");
}

#[test]
fn auth_get_request_empty_body_and_query() {
    let signing_key = test_key();
    let sig =
        auth::sign_request(&signing_key, "1000000000000", "GET", "/api/1.0/orders/active", "", "");
    assert!(!sig.is_empty());
}

#[test]
fn auth_get_with_query_string() {
    let signing_key = test_key();
    let sig = auth::sign_request(
        &signing_key,
        "1746007718237",
        "GET",
        "/api/1.0/orders/active",
        "status=new&limit=10",
        "",
    );
    assert!(!sig.is_empty());
}

#[test]
fn auth_signature_is_deterministic() {
    let key = test_key();
    let sig1 = auth::sign_request(&key, "1000", "GET", "/api/1.0/test", "", "");
    let sig2 = auth::sign_request(&key, "1000", "GET", "/api/1.0/test", "", "");
    assert_eq!(sig1, sig2, "same inputs must produce the same signature");
}

use revx_bot::core::{
    db::TradeDb,
    models::{Order, OrderSide, OrderStatus, OrderType},
};

#[test]
fn db_query_orders_limit_zero() {
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();
    let rows = db.query_orders("BTC-USD", None, None, 0).unwrap();
    assert_eq!(rows.len(), 0);
}

fn make_order(id: &str, status: OrderStatus) -> Order {
    Order {
        id: id.to_owned(),
        prev_order_id: None,
        client_order_id: Some(format!("cid-{id}")),
        symbol: "BTC-USD".into(),
        side: OrderSide::Buy,
        order_type: Some(OrderType::Limit),
        base_quantity: Some("0.1".into()),
        filled_quantity: Some("0.1".into()),
        remaining_quantity: Some("0.0".into()),
        quote_quantity: None,
        limit_price: Some("50000".into()),
        avg_price: Some("49999.5".into()),
        status,
        reject_reason: None,
        time_in_force: None,
        execution_instructions: vec![],
        trigger: None,
        take_profit: None,
        stop_loss: None,
        created_at: Some(1_700_000_000_000),
        updated_at: None,
        completed_at: None,
    }
}

#[test]
fn db_open_migrate_upsert_query() {
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    let order = make_order("order-1", OrderStatus::Filled);
    db.upsert_order(&order).unwrap();

    // Upsert same order again — should not error
    db.upsert_order(&order).unwrap();

    let rows = db.query_orders("BTC-USD", None, None, 10).unwrap();
    assert_eq!(rows.len(), 1);

    let parsed: Order = serde_json::from_str(&rows[0]).unwrap();
    assert_eq!(parsed.id, "order-1");
}

#[test]
fn db_query_respects_time_range() {
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    let mut o1 = make_order("order-early", OrderStatus::Filled);
    o1.created_at = Some(1_000_000);
    let mut o2 = make_order("order-late", OrderStatus::Filled);
    o2.created_at = Some(2_000_000);

    db.upsert_order(&o1).unwrap();
    db.upsert_order(&o2).unwrap();

    // Only ask for orders after 1_500_000
    let rows = db.query_orders("BTC-USD", Some(1_500_000), None, 10).unwrap();
    assert_eq!(rows.len(), 1);
    let parsed: Order = serde_json::from_str(&rows[0]).unwrap();
    assert_eq!(parsed.id, "order-late");
}

use revx_bot::{
    api::client::RevolutXClient,
    core::models::{
        ActiveOrdersResponse, ExecutionInstruction, LimitConfig, OrderConfiguration,
        PlaceOrderRequest,
    },
};

#[test]
fn models_place_order_request_serializes_correctly() {
    let req = PlaceOrderRequest {
        client_order_id: "test-123".into(),
        symbol: "BTC-USD".into(),
        side: OrderSide::Buy,
        order_configuration: OrderConfiguration::Limit(LimitConfig {
            base_size: Some("0.1".into()),
            quote_size: None,
            price: "50000.50".into(),
            execution_instructions: vec![ExecutionInstruction::AllowTaker],
        }),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["symbol"], "BTC-USD");
    assert_eq!(json["side"], "buy");
    assert_eq!(json["order_configuration"]["limit"]["price"], "50000.50");
}

#[test]
fn models_order_status_roundtrip() {
    let s: OrderStatus = serde_json::from_str(r#""partially_filled""#).unwrap();
    assert_eq!(s, OrderStatus::PartiallyFilled);
}

#[test]
fn models_active_orders_response_deserialization() {
    let json = r#"{
        "data": [],
        "metadata": { "next_cursor": null }
    }"#;
    let resp: ActiveOrdersResponse = serde_json::from_str(json).unwrap();
    assert!(resp.data.is_empty());
}

#[test]
fn client_build_active_orders_query_with_symbol() {
    use revx_bot::{api::client, core::models::ActiveOrdersFilter};
    let f = ActiveOrdersFilter {
        symbols: Some("BTC-USD".into()),
        limit: Some(10),
        ..Default::default()
    };
    let q = client::build_active_orders_query(&f);
    assert!(q.contains("symbols=BTC-USD"));
    assert!(q.contains("limit=10"));
}

// ── Config Tests
// ──────────────────────────────────────────────────────────────
use revx_bot::core::config;

#[test]
fn config_expand_tilde_logic() {
    let p = "~/test.txt";
    let expanded = config::expand_tilde(p);
    if let Some(home) = dirs::home_dir() {
        assert!(expanded.starts_with(&home.to_string_lossy().to_string()));
        assert!(expanded.ends_with("/test.txt"));
    } else {
        assert_eq!(expanded, p);
    }
}

#[test]
fn config_json_deserialization_roundtrip() {
    let json = r#"{
        "api_key": "test_key",
        "private_key_path": "~/key.pem",
        "polling_interval_ms": 1000,
        "db_path": "test.db",
        "api_port": 1234,
        "symbols": []
    }"#;
    let cfg: config::DummyConfig = serde_json::from_str(json).expect("valid DummyConfig JSON");
    assert_eq!(cfg.api_key, "test_key");
    assert_eq!(cfg.port, 1234);
}

#[test]
fn db_history_retention_and_sorting() {
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    let mut o1 = make_order("id-1", OrderStatus::Filled);
    o1.created_at = Some(5000);
    o1.updated_at = Some(5000);
    let mut o2 = make_order("id-2", OrderStatus::Filled);
    o2.created_at = Some(1000); // older
    o2.updated_at = Some(1000); // older

    db.upsert_order(&o1).unwrap();
    db.upsert_order(&o2).unwrap();

    let rows = db.query_orders("BTC-USD", None, None, 10).unwrap();
    assert_eq!(rows.len(), 2);

    // Default sort should be created_at DESC (newest first)
    let p1: Order = serde_json::from_str(&rows[0]).unwrap();
    assert_eq!(p1.id, "id-1");
}

#[test]
fn models_order_serialization_roundtrip() {
    let order = make_order("round-1", OrderStatus::PartiallyFilled);
    let json = serde_json::to_string(&order).unwrap();
    let back: Order = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, order.id);
    assert_eq!(back.status, order.status);
}

#[tokio::test]
async fn client_sign_helper_is_deterministic() {
    let secret_bytes = [0u8; 32];
    let key = SigningKey::from_bytes(&secret_bytes);
    let _client = RevolutXClient::new("key".into(), key).unwrap();

    // Internal sign helper is private but we can test it indirectly or just use
    // auth directly. Client has no public sign helper anymore.
}

#[test]
fn db_upsert_order_status_update() {
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    let order1 = make_order("id-123", OrderStatus::New);
    db.upsert_order(&order1).unwrap();

    let mut order2 = order1.clone();
    order2.status = OrderStatus::Filled;
    db.upsert_order(&order2).unwrap();

    let rows = db.query_orders("BTC-USD", None, None, 10).unwrap();
    assert_eq!(rows.len(), 1);
    let o: Order = serde_json::from_str(&rows[0]).unwrap();
    assert_eq!(o.status, OrderStatus::Filled);
}

#[test]
fn db_query_orders_symbol_filter() {
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();

    db.upsert_order(&make_order("o1", OrderStatus::New)).unwrap();
    let mut o2 = make_order("o2", OrderStatus::New);
    o2.symbol = "ETH-USD".to_string();
    db.upsert_order(&o2).unwrap();

    let btc_rows = db.query_orders("BTC-USD", None, None, 10).unwrap();
    let eth_rows = db.query_orders("ETH-USD", None, None, 10).unwrap();
    assert_eq!(btc_rows.len(), 1);
    assert_eq!(eth_rows.len(), 1);
}

#[test]
fn db_open_real_path_creates_directory() {
    let dir = std::env::temp_dir().join("revx-test-db-open");
    let _ = std::fs::remove_dir_all(&dir);
    let db_path = dir.join("test.db");

    // Opens and migrates with a real file-system path (covers create_dir_all
    // branch)
    let db = TradeDb::open(db_path.to_str().unwrap()).unwrap();
    db.migrate().unwrap();

    // Verify it works by doing a round-trip
    let rows = db.query_orders("BTC-USD", None, None, 10).unwrap();
    assert!(rows.is_empty());

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

// ── Position State DB Tests
// ───────────────────────────────────────────────────

use revx_bot::core::db::PositionState;

fn mem_db() -> TradeDb {
    let db = TradeDb::open(":memory:").unwrap();
    db.migrate().unwrap();
    db
}

#[test]
fn position_state_get_returns_none_when_missing() {
    let db = mem_db();
    let result = db.get_position_state("USDT-USD", "buy").unwrap();
    assert!(result.is_none());
}

#[test]
fn position_state_upsert_and_get_roundtrip() {
    let db = mem_db();
    let ps = PositionState {
        symbol: "USDT-USD".into(),
        side: "buy".into(),
        entry_order_id: "entry-1".into(),
        entry_client_id: "client-1".into(),
        entry_filled: false,
        exit_order_id: None,
        updated_at: 1_000_000,
    };
    db.upsert_position_state(&ps).unwrap();

    let loaded = db.get_position_state("USDT-USD", "buy").unwrap().unwrap();
    assert_eq!(loaded.symbol, "USDT-USD");
    assert_eq!(loaded.side, "buy");
    assert_eq!(loaded.entry_order_id, "entry-1");
    assert_eq!(loaded.entry_client_id, "client-1");
    assert!(!loaded.entry_filled);
    assert!(loaded.exit_order_id.is_none());
}

#[test]
fn position_state_upsert_updates_existing_row() {
    let db = mem_db();
    let ps = PositionState {
        symbol: "USDT-USD".into(),
        side: "buy".into(),
        entry_order_id: "entry-1".into(),
        entry_client_id: "client-1".into(),
        entry_filled: false,
        exit_order_id: None,
        updated_at: 1_000,
    };
    db.upsert_position_state(&ps).unwrap();

    // Mark as filled, add exit order
    let updated = PositionState {
        entry_filled: true,
        exit_order_id: Some("exit-1".into()),
        updated_at: 2_000,
        ..ps
    };
    db.upsert_position_state(&updated).unwrap();

    let loaded = db.get_position_state("USDT-USD", "buy").unwrap().unwrap();
    assert!(loaded.entry_filled);
    assert_eq!(loaded.exit_order_id, Some("exit-1".into()));
    assert_eq!(loaded.updated_at, 2_000);
}

#[test]
fn position_state_clear_removes_row() {
    let db = mem_db();
    let ps = PositionState {
        symbol: "USDT-USD".into(),
        side: "sell".into(),
        entry_order_id: "entry-s1".into(),
        entry_client_id: "client-s1".into(),
        entry_filled: false,
        exit_order_id: None,
        updated_at: 0,
    };
    db.upsert_position_state(&ps).unwrap();
    assert!(db.get_position_state("USDT-USD", "sell").unwrap().is_some());

    db.clear_position_state("USDT-USD", "sell").unwrap();
    assert!(db.get_position_state("USDT-USD", "sell").unwrap().is_none());
}

#[test]
fn position_state_sides_are_independent() {
    let db = mem_db();

    let buy_ps = PositionState {
        symbol: "USDT-USD".into(),
        side: "buy".into(),
        entry_order_id: "e-buy".into(),
        entry_client_id: "c-buy".into(),
        entry_filled: false,
        exit_order_id: None,
        updated_at: 0,
    };
    let sell_ps = PositionState {
        symbol: "USDT-USD".into(),
        side: "sell".into(),
        entry_order_id: "e-sell".into(),
        entry_client_id: "c-sell".into(),
        entry_filled: true,
        exit_order_id: Some("exit-sell".into()),
        updated_at: 1,
    };
    db.upsert_position_state(&buy_ps).unwrap();
    db.upsert_position_state(&sell_ps).unwrap();

    let buy = db.get_position_state("USDT-USD", "buy").unwrap().unwrap();
    let sell = db.get_position_state("USDT-USD", "sell").unwrap().unwrap();

    assert_eq!(buy.entry_order_id, "e-buy");
    assert!(!buy.entry_filled);

    assert_eq!(sell.entry_order_id, "e-sell");
    assert!(sell.entry_filled);
    assert_eq!(sell.exit_order_id, Some("exit-sell".into()));
}

#[test]
fn position_state_clear_nonexistent_is_ok() {
    let db = mem_db();
    // Clearing a row that doesn't exist should not error
    db.clear_position_state("GHOST-USD", "buy").unwrap();
}

#[test]
fn position_state_different_symbols_are_independent() {
    let db = mem_db();

    db.upsert_position_state(&PositionState {
        symbol: "USDT-USD".into(),
        side: "buy".into(),
        entry_order_id: "e1".into(),
        entry_client_id: "c1".into(),
        entry_filled: false,
        exit_order_id: None,
        updated_at: 0,
    })
    .unwrap();
    db.upsert_position_state(&PositionState {
        symbol: "BTC-USD".into(),
        side: "buy".into(),
        entry_order_id: "e2".into(),
        entry_client_id: "c2".into(),
        entry_filled: true,
        exit_order_id: Some("ex2".into()),
        updated_at: 0,
    })
    .unwrap();

    let usdt = db.get_position_state("USDT-USD", "buy").unwrap().unwrap();
    let btc = db.get_position_state("BTC-USD", "buy").unwrap().unwrap();

    assert_eq!(usdt.entry_order_id, "e1");
    assert_eq!(btc.entry_order_id, "e2");
    assert!(btc.entry_filled);
}

#[test]
fn test_position_state_pk() {
    // Ensure hyphens are normalized to slashes
    assert_eq!(PositionState::pk("USDC-USD", "buy"), "USDC/USD:buy");
    assert_eq!(PositionState::pk("BTC-USD", "sell"), "BTC/USD:sell");
    // Ensure existing slashes are kept as is
    assert_eq!(PositionState::pk("USDC/USD", "buy"), "USDC/USD:buy");
}
