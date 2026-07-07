//! Configuration — loaded once at startup from
//! `~/.config/revolut-x/dummy_config.json`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;

/// Top-level application configuration, matching the user's existing JSON
/// structure.
#[derive(Debug, Deserialize)]
pub struct DummyConfig {
    /// X-Revx-API-Key header value.
    pub api_key: String,

    /// Path to the Ed25519 private key PEM file.
    pub private_key_path: String,

    /// How often to poll active orders (milliseconds).
    #[serde(rename = "polling_interval_ms")]
    pub poll_interval_ms: u64,

    /// Path to the SQLite database file.
    pub db_path: String,

    /// Port for the proxy server.
    #[serde(rename = "api_port")]
    pub port: u16,

    /// Optional base URL (defaults to production if missing).
    pub base_url: Option<String>,

    /// List of trading configurations per symbol.
    pub symbols: Vec<SymbolConfig>,
}

/// Strategy parameters per symbol.
#[derive(Debug, Deserialize, Clone)]
pub struct SymbolConfig {
    pub symbol: String,
    pub buy_trigger_price: f64,
    pub sell_trigger_price: f64,
    pub revert_price: f64,
    pub trade_size_base: f64,
    pub trade_size_quote: f64,
    #[serde(default = "default_tick_size")]
    pub tick_size: f64,
}

fn default_tick_size() -> f64 { 0.0001 }

impl DummyConfig {
    /// Load from `~/.config/revolut-x/dummy_config.json`.
    pub fn load() -> Result<Self> {
        let path = dummy_config_path();
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read config from {}", path.display()))?;
        let mut cfg: DummyConfig =
            serde_json::from_str(&raw).with_context(|| "dummy_config.json is not valid JSON")?;

        // Expand `~` in paths
        cfg.private_key_path = expand_tilde(&cfg.private_key_path);
        cfg.db_path = expand_tilde(&cfg.db_path);

        Ok(cfg)
    }

    /// Helper to get the proxy address string.
    pub fn proxy_addr(&self) -> String { format!("127.0.0.1:{}", self.port) }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn home_dir() -> Option<PathBuf> {
    if std::env::var("MOCK_NO_HOME").is_ok() {
        return None;
    }
    dirs::home_dir()
}

pub fn dummy_config_path() -> PathBuf {
    if let Ok(dir) = std::env::var("REVOLUTX_CONFIG_DIR") {
        return PathBuf::from(dir).join("dummy_config.json");
    }
    home_dir()
        .map(|h| h.join(".config").join("revolut-x").join("dummy_config.json"))
        .unwrap_or_else(|| PathBuf::from("dummy_config.json"))
}

/// Expand a leading `~` to the user's home directory.
pub fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = home_dir() {
            return home.to_string_lossy().to_string() + &path[1..];
        }
    }
    path.to_owned()
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum NotifierConfig {
    #[serde(rename = "ntfy")]
    Ntfy { topic_url: String, auth_token: Option<String> },
}
