use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::{header, Client};

use super::Notifier;

/// A Notifier implementation for ntfy.sh (or self-hosted ntfy servers).
pub struct NtfyNotifier {
    client: Client,
    topic_url: String,
    auth_token: Option<String>,
}

impl NtfyNotifier {
    pub fn new(topic_url: String, auth_token: Option<String>) -> Self {
        Self { client: Client::new(), topic_url, auth_token }
    }
}

#[async_trait]
impl Notifier for NtfyNotifier {
    async fn notify(&self, title: &str, message: &str) -> Result<()> {
        let mut req =
            self.client.post(&self.topic_url).header("Title", title).body(message.to_string());

        if let Some(token) = &self.auth_token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {}", token));
        }

        let resp = req.send().await.context("Failed to send HTTP request to ntfy server")?;

        if !resp.status().is_success() {
            anyhow::bail!("ntfy server returned error: {}", resp.status());
        }

        Ok(())
    }
}
