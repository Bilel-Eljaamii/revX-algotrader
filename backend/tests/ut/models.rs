use revx_bot::core::models::{OrderBookLevel, OrderBookResponse};

#[test]
fn test_order_book_level_parsing() {
    let level =
        OrderBookLevel { price: "63500.25".into(), quantity: "1.5".into(), order_count: None };
    assert_eq!(level.price_f64(), 63500.25);
    assert_eq!(level.quantity_f64(), 1.5);

    let invalid =
        OrderBookLevel { price: "invalid".into(), quantity: "bad".into(), order_count: None };
    assert_eq!(invalid.price_f64(), 0.0);
    assert_eq!(invalid.quantity_f64(), 0.0);
}

#[test]
fn test_order_book_best_ask_bid() {
    let empty_book = OrderBookResponse { asks: vec![], bids: vec![] };
    assert_eq!(empty_book.best_ask(), f64::MAX);
    assert_eq!(empty_book.best_bid(), 0.0);

    let book = OrderBookResponse {
        asks: vec![OrderBookLevel {
            price: "100.5".into(),
            quantity: "1".into(),
            order_count: None,
        }],
        bids: vec![OrderBookLevel {
            price: "99.5".into(),
            quantity: "1".into(),
            order_count: None,
        }],
    };
    assert_eq!(book.best_ask(), 100.5);
    assert_eq!(book.best_bid(), 99.5);
}

#[test]
fn test_order_serialization_aliases() {
    let json_data = r#"{
        "id": "123",
        "symbol": "BTC-USD",
        "side": "buy",
        "status": "new",
        "created_date": 123456789,
        "updated_date": 987654321
    }"#;

    let order: revx_bot::core::models::Order = serde_json::from_str(json_data).unwrap();
    assert_eq!(order.id, "123");
    assert_eq!(order.created_at, Some(123456789));
    assert_eq!(order.updated_at, Some(987654321));
}
