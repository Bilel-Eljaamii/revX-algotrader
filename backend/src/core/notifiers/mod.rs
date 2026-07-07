use anyhow::Result;
use async_trait::async_trait;
use tracing::{debug, error};

use crate::core::config::NotifierConfig;

pub mod ntfy;

/// The Notifier trait represents a destination that can receive string-based
/// alerts.
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, title: &str, message: &str) -> Result<()>;
}

/// A manager that holds a collection of notifiers and broadcasts messages to
/// all of them.
pub struct NotifierManager {
    notifiers: Vec<Box<dyn Notifier>>,
}

impl NotifierManager {
    /// Initialize the manager based on the provided configuration.
    pub fn new(configs: Option<&[NotifierConfig]>) -> Self {
        let mut notifiers: Vec<Box<dyn Notifier>> = Vec::new();

        if let Some(cfg_list) = configs {
            for cfg in cfg_list {
                match cfg {
                    NotifierConfig::Ntfy { topic_url, auth_token } => {
                        notifiers.push(Box::new(ntfy::NtfyNotifier::new(
                            topic_url.clone(),
                            auth_token.clone(),
                        )));
                    }
                }
            }
        }

        Self { notifiers }
    }

    /// Broadcast a notification to all configured destinations.
    pub async fn broadcast(&self, title: &str, message: &str) {
        if self.notifiers.is_empty() {
            return;
        }
        debug!("Broadcasting notification: [{}] {}", title, message);
        for notifier in &self.notifiers {
            if let Err(e) = notifier.notify(title, message).await {
                error!("Failed to send notification: {:#}", e);
            }
        }
    }
}
