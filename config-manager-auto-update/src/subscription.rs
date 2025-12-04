use anyhow::{Context, Result};
use tokio::fs;

use crate::types::{Config, Subscription};

/// Subscription Manager (Pure CLI Mode)
pub struct SubscriptionManager;

impl SubscriptionManager {
    pub fn new() -> Self {
        Self
    }

    /// Download a single subscription
    pub async fn download_subscription(&self, sub: &Subscription) -> Result<()> {
        println!("📥 Downloading subscription [{}] from {}", sub.name, sub.url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let response = client
            .get(&sub.url)
            .send()
            .await
            .context("Download failed")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed, server returned status: {}", response.status());
        }

        let content = response.bytes().await.context("Failed to read response content")?;

        if let Some(parent) = sub.save_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create directory")?;
        }

        fs::write(&sub.save_path, content)
            .await
            .context("Failed to save file")?;

        println!(
            "✅ Subscription [{}] downloaded successfully, saved to {}",
            sub.name,
            sub.save_path.display()
        );

        Ok(())
    }

    /// Run subscription downloader (download all subscriptions)
    pub async fn run_all(&self, config: &Config) -> Result<()> {
        println!("🔄 Checking subscription updates...");

        for sub in &config.subscriptions {
            if let Err(e) = self.download_subscription(sub).await {
                eprintln!("❌ Error processing subscription [{}]: {}", sub.name, e);
            }
        }

        println!("✅ Subscription update check complete");
        Ok(())
    }
}
