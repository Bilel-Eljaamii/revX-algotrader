use tracing::info;

use crate::{
    api::client::RevolutXClient,
    core::{config::SymbolConfig, db::TradeDb, models::ActiveOrdersFilter},
};

pub async fn run_tick(
    client: &RevolutXClient,
    sym_cfg: &SymbolConfig,
    db: &TradeDb,
    dry_run: bool,
) -> anyhow::Result<()> {
    info!(symbol = %sym_cfg.symbol, "Dummy strategy tick");

    // Fetch active orders to prove API connection
    let active_orders_filter =
        ActiveOrdersFilter { symbols: Some(sym_cfg.symbol.clone()), ..Default::default() };
    let active_orders_resp = client.get_active_orders(&active_orders_filter).await?;
    info!("Found {} active orders for {}", active_orders_resp.data.len(), sym_cfg.symbol);

    // Fetch orderbook to prove API connection
    let ob = client.get_order_book(&sym_cfg.symbol, 20).await?;
    let best_bid = ob.best_bid();
    let best_ask = ob.best_ask();
    info!("Orderbook for {}: best_bid={}, best_ask={}", sym_cfg.symbol, best_bid, best_ask);

    // Check DB
    let long_pos = db.get_position_state(&sym_cfg.symbol, "buy")?;
    let short_pos = db.get_position_state(&sym_cfg.symbol, "sell")?;
    let mut open_positions_count = 0;
    if long_pos.is_some() {
        open_positions_count += 1;
    }
    if short_pos.is_some() {
        open_positions_count += 1;
    }
    info!("Found {} open positions in DB for {}", open_positions_count, sym_cfg.symbol);

    if dry_run {
        info!("Dry run mode enabled. No actions taken.");
    }

    Ok(())
}
