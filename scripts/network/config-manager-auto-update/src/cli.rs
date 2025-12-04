use anyhow::Result;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

use crate::config::load_config;
use crate::subscription::SubscriptionManager;
use crate::types::Config;
use crate::updater::{CoreUpdater, MihomoUpdater};

/// Run CLI mode
pub async fn run_cli(config_path: PathBuf, once_mode: bool) -> Result<()> {
    println!("🚀 Starting (CLI mode), loading config file: {}", config_path.display());

    // Load config
    let config = load_config(&config_path).await?;

    // Execute tasks
    run_tasks(&config).await?;

    // Exit if once mode or update interval is 0
    if once_mode || config.update_interval_hours == 0 {
        println!("✅ All tasks complete, exiting.");
        return Ok(());
    }

    // Scheduled execution
    println!("⏰ Scheduled task set, updating every {} hours. Press Ctrl+C to stop.", config.update_interval_hours);

    let interval = Duration::from_secs(config.update_interval_hours * 3600);

    loop {
        sleep(interval).await;
        println!("⏰ Scheduled task triggered, starting update...");
        if let Err(e) = run_tasks(&config).await {
            eprintln!("❌ Task execution error: {}", e);
        }
    }
}

/// Execute update tasks
async fn run_tasks(config: &Config) -> Result<()> {
    // Update subscriptions
    let sub_manager = SubscriptionManager::new();
    sub_manager.run_all(config).await?;

    // Update sing-box core
    let core_updater = CoreUpdater::new();
    core_updater.run_all(config).await?;

    // Update mihomo core (if configured)
    if let Some(mihomo_config) = &config.mihomo_core_update {
        let mihomo_updater = MihomoUpdater::new();
        mihomo_updater.run_all(mihomo_config).await?;
    }

    Ok(())
}
