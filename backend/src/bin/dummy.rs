use anyhow::Result;
use clap::Parser;
use revx_bot::{
    api::{auth::load_signing_key, client::RevolutXClient},
    core::{config::DummyConfig, db::TradeDb},
    strategies::dummy::engine,
};
use tokio::{signal, sync::watch};
use tracing::{info, Level};

#[derive(Parser, Debug)]
#[command(author, version, about = "Dummy Strategy Bot for Revolut X")]
struct Args {
    #[arg(short, long, default_value_t = false)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = Args::parse();
    info!("Starting Dummy Bot (dry_run={})", args.dry_run);

    let config = DummyConfig::load()?;
    let db = TradeDb::open(&config.db_path)?;

    // Create necessary tables if they don't exist
    db.migrate()?;

    let signing_key = load_signing_key(&config.private_key_path)?;
    let mut client = RevolutXClient::new(config.api_key.clone(), signing_key)?;
    if let Some(ref base_url) = config.base_url {
        client = client.with_base_url(base_url.clone());
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let engine_handle = tokio::spawn(async move {
        engine::run(&client, &config, db, shutdown_rx, args.dry_run).await;
    });

    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Received Ctrl-C, shutting down...");
            let _ = shutdown_tx.send(true);
        }
        Err(err) => {
            tracing::error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    let _ = engine_handle.await;
    info!("Shutdown complete.");

    Ok(())
}
