//! Video Conversion API Module
//!
//! Pure conversion layer - executes video conversions based on detection results.
//! Supports FFV1 archival and AV1 high-quality compression.

use crate::{VidQualityError, Result};
use crate::detection_api::{detect_video, VideoDetectionResult, CompressionType, DetectedCodec};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

/// Target video format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TargetVideoFormat {
    /// FFV1 in MKV container - for archival
    FFV1_MKV,
    /// AV1 in MP4 container - for compression
    AV1_MP4,
}

impl TargetVideoFormat {
    pub fn extension(&self) -> &str {
        match self {
            TargetVideoFormat::FFV1_MKV => "mkv",
            TargetVideoFormat::AV1_MP4 => "mp4",
        }
    }
    
    pub fn as_str(&self) -> &str {
        match self {
            TargetVideoFormat::FFV1_MKV => "FFV1 MKV (Archival)",
            TargetVideoFormat::AV1_MP4 => "AV1 MP4 (High Quality)",
        }
    }
}

/// Conversion strategy result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionStrategy {
    pub target: TargetVideoFormat,
    pub reason: String,
    pub command: String,
    pub preserve_audio: bool,
}

/// Conversion configuration
#[derive(Debug, Clone)]
pub struct ConversionConfig {
    pub output_dir: Option<PathBuf>,
    pub simple_mode: bool,
    pub force: bool,
    pub delete_original: bool,
    pub preserve_metadata: bool,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            output_dir: None,
            simple_mode: false,
            force: false,
            delete_original: false,
            preserve_metadata: true,
        }
    }
}

/// Conversion output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionOutput {
    pub input_path: String,
    pub output_path: String,
    pub strategy: ConversionStrategy,
    pub input_size: u64,
    pub output_size: u64,
    pub size_ratio: f64,
    pub success: bool,
    pub message: String,
}

/// Determine conversion strategy based on detection result
pub fn determine_strategy(result: &VideoDetectionResult) -> ConversionStrategy {
    let (target, reason) = match result.compression {
        CompressionType::Lossless => {
            // Already lossless - archive with FFV1
            (
                TargetVideoFormat::FFV1_MKV,
                format!("Source is {} (lossless) - archiving to FFV1 MKV for standard archival format", 
                    result.codec.as_str())
            )
        }
        CompressionType::VisuallyLossless => {
            // Visually lossless (ProRes, DNxHD, high-bitrate) - archive with FFV1
            (
                TargetVideoFormat::FFV1_MKV,
                format!("Source is {} (visually lossless) - preserving quality with FFV1 MKV",
                    result.codec.as_str())
            )
        }
        _ => {
            // Lossy source - compress with AV1
            (
                TargetVideoFormat::AV1_MP4,
                format!("Source is {} ({}) - compressing with AV1 CRF 0 (visually lossless)",
                    result.codec.as_str(), result.compression.as_str())
            )
        }
    };
    
    let command = generate_ffmpeg_command(result, target);
    
    ConversionStrategy {
        target,
        reason,
        command,
        preserve_audio: result.has_audio,
    }
}

/// Generate FFmpeg command for conversion
fn generate_ffmpeg_command(result: &VideoDetectionResult, target: TargetVideoFormat) -> String {
    let input = &result.file_path;
    let ext = target.extension();
    
    match target {
        TargetVideoFormat::FFV1_MKV => {
            // FFV1 archival encoding
            // -level 3: Maximum compatibility
            // -coder 1: Range coder for better compression
            // -context 1: Context model for better compression
            // -g 1: GOP size 1 for maximum error resilience
            // -slices 24: Multi-slice for parallel decoding
            // -slicecrc 1: CRC for error detection
            let audio = if result.has_audio { "-c:a flac" } else { "-an" };
            format!(
                "ffmpeg -i '{}' -c:v ffv1 -level 3 -coder 1 -context 1 -g 1 -slices 24 -slicecrc 1 {} '{{}}.{}'",
                input, audio, ext
            )
        }
        TargetVideoFormat::AV1_MP4 => {
            // AV1 high-quality encoding (CRF 0 = visually lossless)
            // -crf 0: Highest quality
            // -b:v 0: Pure CRF mode
            // -cpu-used 4: Balanced speed/quality
            // -row-mt 1: Enable row-based multithreading
            // -tiles 2x2: Tile-based parallelism
            let audio = if result.has_audio { "-c:a aac -b:a 320k" } else { "-an" };
            format!(
                "ffmpeg -i '{}' -c:v libaom-av1 -crf 0 -b:v 0 -cpu-used 4 -row-mt 1 -tiles 2x2 {} '{{}}.{}'",
                input, audio, ext
            )
        }
    }
}

/// Simple mode conversion - automatic strategy selection
pub fn simple_convert(input: &Path, output_dir: Option<&Path>) -> Result<ConversionOutput> {
    let config = ConversionConfig {
        output_dir: output_dir.map(|p| p.to_path_buf()),
        simple_mode: true,
        ..Default::default()
    };
    
    smart_convert(input, &config)
}

/// Smart conversion with full configuration
pub fn smart_convert(input: &Path, config: &ConversionConfig) -> Result<ConversionOutput> {
    // Detect video properties
    let detection = detect_video(input)?;
    
    // Determine strategy
    let strategy = determine_strategy(&detection);
    
    // Generate output path
    let output_dir = config.output_dir.clone()
        .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf());
    
    let stem = input.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    
    let output_path = output_dir.join(format!("{}.{}", stem, strategy.target.extension()));
    
    // Check if output exists
    if output_path.exists() && !config.force {
        return Err(VidQualityError::ConversionError(
            format!("Output file already exists: {}", output_path.display())
        ));
    }
    
    // Create output directory
    std::fs::create_dir_all(&output_dir)?;
    
    // Build FFmpeg command
    let ffmpeg_args = build_ffmpeg_args(&detection, &strategy, &output_path);
    
    info!("🎬 Converting: {} → {}", input.display(), output_path.display());
    info!("   Strategy: {}", strategy.target.as_str());
    info!("   Reason: {}", strategy.reason);
    
    // Execute FFmpeg
    let output = Command::new("ffmpeg")
        .args(&ffmpeg_args)
        .output()?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(VidQualityError::FFmpegError(stderr.to_string()));
    }
    
    // Get output size
    let output_size = std::fs::metadata(&output_path)?.len();
    let size_ratio = output_size as f64 / detection.file_size as f64;
    
    info!("   ✅ Complete: {:.1}% of original size", size_ratio * 100.0);
    
    // Delete original if requested
    if config.delete_original {
        std::fs::remove_file(input)?;
        info!("   🗑️  Original deleted");
    }
    
    Ok(ConversionOutput {
        input_path: input.display().to_string(),
        output_path: output_path.display().to_string(),
        strategy,
        input_size: detection.file_size,
        output_size,
        size_ratio,
        success: true,
        message: "Conversion successful".to_string(),
    })
}

/// Build FFmpeg arguments
fn build_ffmpeg_args(
    detection: &VideoDetectionResult,
    strategy: &ConversionStrategy,
    output_path: &Path,
) -> Vec<String> {
    let mut args = vec![
        "-y".to_string(),  // Overwrite output
        "-i".to_string(),
        detection.file_path.clone(),
    ];
    
    match strategy.target {
        TargetVideoFormat::FFV1_MKV => {
            args.extend(vec![
                "-c:v".to_string(), "ffv1".to_string(),
                "-level".to_string(), "3".to_string(),
                "-coder".to_string(), "1".to_string(),
                "-context".to_string(), "1".to_string(),
                "-g".to_string(), "1".to_string(),
                "-slices".to_string(), "24".to_string(),
                "-slicecrc".to_string(), "1".to_string(),
            ]);
            
            if detection.has_audio {
                args.extend(vec!["-c:a".to_string(), "flac".to_string()]);
            } else {
                args.push("-an".to_string());
            }
        }
        TargetVideoFormat::AV1_MP4 => {
            args.extend(vec![
                "-c:v".to_string(), "libaom-av1".to_string(),
                "-crf".to_string(), "0".to_string(),
                "-b:v".to_string(), "0".to_string(),
                "-cpu-used".to_string(), "4".to_string(),
                "-row-mt".to_string(), "1".to_string(),
                "-tiles".to_string(), "2x2".to_string(),
            ]);
            
            if detection.has_audio {
                args.extend(vec![
                    "-c:a".to_string(), "aac".to_string(),
                    "-b:a".to_string(), "320k".to_string(),
                ]);
            } else {
                args.push("-an".to_string());
            }
        }
    }
    
    args.push(output_path.display().to_string());
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_target_format() {
        assert_eq!(TargetVideoFormat::FFV1_MKV.extension(), "mkv");
        assert_eq!(TargetVideoFormat::AV1_MP4.extension(), "mp4");
    }
}
