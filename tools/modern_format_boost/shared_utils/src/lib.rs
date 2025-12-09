//! Shared Utilities for modern_format_boost tools
//! 
//! This crate provides common functionality shared across imgquality, vidquality, and vidquality-hevc:
//! - Progress bar with ETA
//! - Safety checks (dangerous directory detection)
//! - Batch processing utilities
//! - Common logging and reporting
//! - FFprobe wrapper for video analysis
//! - External tools detection
//! - Codec information
//! - Metadata preservation (EXIF/IPTC/xattr/timestamps/ACL)

pub mod progress;
pub mod safety;
pub mod batch;
pub mod report;
pub mod ffprobe;
pub mod tools;
pub mod codecs;
pub mod metadata;

pub use progress::*;
pub use safety::*;
pub use batch::*;
pub use report::*;
pub use ffprobe::{FFprobeResult, FFprobeError, probe_video, get_duration, get_frame_count, parse_frame_rate, detect_bit_depth, is_ffprobe_available};
pub use tools::*;
pub use codecs::*;
pub use metadata::{preserve_metadata, preserve_pro};
