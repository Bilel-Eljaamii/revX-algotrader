use tokio::{sync::watch::Receiver, time};
use tracing::{error, info};

use crate::{
    api::client::RevolutXClient,
    core::{config::DummyConfig, db::TradeDb},
    strategies::dummy::strategy,
};

pub async fn run(
    client: &RevolutXClient,
    config: &DummyConfig,
    db: TradeDb,
    mut shutdown: Receiver<bool>,
    dry_run: bool,
) {
    let interval = time::Duration::from_millis(config.poll_interval_ms);
    let mut ticker = time::interval(interval);
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    info!(
        poll_ms = config.poll_interval_ms,
        symbols = config.symbols.len(),
        dry_run,
        "dummy engine started"
    );

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                for sym_cfg in &config.symbols {
                    match strategy::run_tick(client, sym_cfg, &db, dry_run).await {
                        Ok(()) => {}
                        Err(e) => {
                            error!(symbol = %sym_cfg.symbol, "tick error: {e:#}");
                        }
                    }
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("engine received shutdown — exiting loop");
                    return;
                }
            }
        }
    }
}
