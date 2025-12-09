//! Progress Bar Module
//! 
//! Provides visual progress feedback with ETA estimation
//! Reference: media/CONTRIBUTING.md - Visual Progress Bar requirement

use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::Duration;

/// Create a styled progress bar for batch processing
/// 
/// # Example
/// ```
/// let pb = create_progress_bar(100, "Converting");
/// for i in 0..100 {
///     pb.inc(1);
/// }
/// pb.finish_with_message("Done!");
/// ```
pub fn create_progress_bar(total: u64, prefix: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.cyan.bold} [{bar:40.green/dim}] {pos}/{len} ({percent}%) | ETA: {eta} | {msg}")
            .expect("Invalid progress bar template")
            .progress_chars("█▓░")
    );
    pb.set_prefix(prefix.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

/// Create a spinner for indeterminate progress
pub fn create_spinner(message: &str) -> ProgressBar {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("Invalid spinner template")
    );
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

/// Create a multi-progress container for parallel operations
pub fn create_multi_progress() -> MultiProgress {
    MultiProgress::new()
}

/// Progress tracker for batch operations with statistics
pub struct BatchProgress {
    pub total: u64,
    pub processed: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub skipped: u64,
    bar: ProgressBar,
}

impl BatchProgress {
    pub fn new(total: u64, prefix: &str) -> Self {
        Self {
            total,
            processed: 0,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            bar: create_progress_bar(total, prefix),
        }
    }

    pub fn success(&mut self, message: &str) {
        self.processed += 1;
        self.succeeded += 1;
        self.bar.set_message(format!("✅ {}", message));
        self.bar.inc(1);
    }

    pub fn fail(&mut self, message: &str) {
        self.processed += 1;
        self.failed += 1;
        self.bar.set_message(format!("❌ {}", message));
        self.bar.inc(1);
    }

    pub fn skip(&mut self, message: &str) {
        self.processed += 1;
        self.skipped += 1;
        self.bar.set_message(format!("⏭️  {}", message));
        self.bar.inc(1);
    }

    pub fn finish(&self) {
        self.bar.finish_with_message(format!(
            "Complete: {} succeeded, {} failed, {} skipped",
            self.succeeded, self.failed, self.skipped
        ));
    }

    /// Get the underlying progress bar for custom operations
    pub fn bar(&self) -> &ProgressBar {
        &self.bar
    }
}

/// Format bytes to human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format duration to human-readable string
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs >= 3600 {
        format!("{}h {}m {}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else if secs >= 60 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m 1s");
    }
}
