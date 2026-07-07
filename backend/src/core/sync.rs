use anyhow::Result;
use tracing::{info, warn};

use crate::{
    api::client::RevolutXClient,
    core::{db::TradeDb, models::ActiveOrdersFilter},
};

/// Fetch all active orders for the configured symbols and upsert them into the
/// local database.
///
/// This ensures the local `orders` table is in sync with the exchange state on
/// startup.
pub async fn sync_active_orders(
    client: &RevolutXClient,
    db: &TradeDb,
    symbols: &[String],
) -> Result<()> {
    if symbols.is_empty() {
        return Ok(());
    }

    info!(symbols = symbols.join(","), "syncing active orders from exchange…");

    // Revolut X API often expects symbols with '-' or '/' depending on the
    // endpoint. We normalize to the configured format for the filter.
    let filter = ActiveOrdersFilter { symbols: Some(symbols.join(",")), ..Default::default() };

    match client.get_active_orders(&filter).await {
        Ok(resp) => {
            let count = resp.data.len();
            for order in resp.data {
                if let Err(e) = db.upsert_order(&order) {
                    warn!(order_id = %order.id, "failed to upsert synced order: {e:#}");
                }
            }
            info!(count, "active orders sync complete");
        }
        Err(e) => {
            warn!("failed to fetch active orders during sync: {e:#}");
        }
    }

    Ok(())
}

/// Fetch historical orders from the exchange to reconcile the status of orders
/// currently marked as non-terminal (new, partially_filled) in the local DB.
pub async fn sync_historical_orders(
    client: &RevolutXClient,
    db: &TradeDb,
    symbols: &[String],
) -> Result<()> {
    if symbols.is_empty() {
        return Ok(());
    }

    info!("reconciling historical order statuses…");

    // Normalize our interest list to both formats to be safe
    let mut normalized_interest = symbols.iter().map(|s| s.replace('-', "/")).collect::<Vec<_>>();
    normalized_interest.extend(symbols.iter().map(|s| s.replace('/', "-")));

    let filter = crate::core::models::HistoricalOrdersFilter {
        limit: Some(300), // Increased depth
        ..Default::default()
    };

    // Fetch the last 300 historical orders to catch updates that happened while
    // offline
    match client.get_historical_orders(&filter).await {
        Ok(resp) => {
            let mut updated_count = 0;
            for order in resp.data {
                // Normalization check: match USDC-USD or USDC/USD
                let is_interesting = normalized_interest.contains(&order.symbol);

                if is_interesting {
                    if let Err(e) = db.upsert_order(&order) {
                        warn!(order_id = %order.id, "failed to update historical order: {e:#}");
                    }
                    updated_count += 1;
                }
            }
            info!(count = updated_count, "historical reconciliation complete");
        }
        Err(e) => {
            warn!("failed to fetch historical orders during sync: {e:#}");
        }
    }

    Ok(())
}
