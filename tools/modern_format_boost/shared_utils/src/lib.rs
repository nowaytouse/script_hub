//! Shared Utilities for modern_format_boost tools
//! 
//! This crate provides common functionality shared across imgquality, vidquality, and vidquality-hevc:
//! - Progress bar with ETA
//! - Safety checks (dangerous directory detection)
//! - Batch processing utilities
//! - Common logging and reporting

pub mod progress;
pub mod safety;
pub mod batch;
pub mod report;

pub use progress::*;
pub use safety::*;
pub use batch::*;
pub use report::*;
