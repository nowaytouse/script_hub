//! Video Conversion API Module
//!
//! Pure conversion layer - executes video conversions based on detection results.
//! - Auto Mode: FFV1 for lossless sources, AV1 for lossy sources
//! - Simple Mode: Always AV1 MP4
//! - Size Exploration: Tries higher CRF if output is larger than input

use crate::{VidQualityError, Result};
use crate::detection_api::{detect_video, VideoDetectionResult, CompressionType};
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
    pub crf: u8,
}

/// Conversion configuration
#[derive(Debug, Clone)]
pub struct ConversionConfig {
    pub output_dir: Option<PathBuf>,
    pub force: bool,
    pub delete_original: bool,
    pub preserve_metadata: bool,
    /// Enable size exploration (try higher CRF if output > input)
    pub explore_smaller: bool,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            output_dir: None,
            force: false,
            delete_original: false,
            preserve_metadata: true,
            explore_smaller: false,
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
    /// CRF used for final output (if exploration was done)
    pub final_crf: u8,
    /// Number of exploration attempts
    pub exploration_attempts: u8,
}

/// Determine conversion strategy based on detection result (for auto mode)
pub fn determine_strategy(result: &VideoDetectionResult) -> ConversionStrategy {
    let (target, reason, crf) = match result.compression {
        CompressionType::Lossless => {
            (
                TargetVideoFormat::FFV1_MKV,
                format!("Source is {} (lossless) - archiving to FFV1 MKV", result.codec.as_str()),
                0
            )
        }
        CompressionType::VisuallyLossless => {
            (
                TargetVideoFormat::FFV1_MKV,
                format!("Source is {} (visually lossless) - preserving with FFV1 MKV", result.codec.as_str()),
                0
            )
        }
        _ => {
            (
                TargetVideoFormat::AV1_MP4,
                format!("Source is {} ({}) - compressing with AV1 CRF 0", result.codec.as_str(), result.compression.as_str()),
                0
            )
        }
    };
    
    ConversionStrategy {
        target,
        reason,
        command: String::new(),
        preserve_audio: result.has_audio,
        crf,
    }
}

/// Simple mode conversion - ALWAYS use AV1 MP4
pub fn simple_convert(input: &Path, output_dir: Option<&Path>) -> Result<ConversionOutput> {
    let detection = detect_video(input)?;
    
    let output_dir = output_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf());
    
    std::fs::create_dir_all(&output_dir)?;
    
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let output_path = output_dir.join(format!("{}.mp4", stem));
    
    info!("🎬 Simple Mode: {} → AV1 MP4", input.display());
    
    // Always AV1 MP4 with CRF 0
    let output_size = execute_av1_conversion(&detection, &output_path, 0)?;
    
    // Preserve metadata
    preserve_metadata(input, &output_path)?;
    preserve_timestamps(input, &output_path)?;
    
    let size_ratio = output_size as f64 / detection.file_size as f64;
    
    info!("   ✅ Complete: {:.1}% of original", size_ratio * 100.0);
    
    Ok(ConversionOutput {
        input_path: input.display().to_string(),
        output_path: output_path.display().to_string(),
        strategy: ConversionStrategy {
            target: TargetVideoFormat::AV1_MP4,
            reason: "Simple mode: All videos → AV1 MP4".to_string(),
            command: String::new(),
            preserve_audio: detection.has_audio,
            crf: 0,
        },
        input_size: detection.file_size,
        output_size,
        size_ratio,
        success: true,
        message: "Simple conversion successful".to_string(),
        final_crf: 0,
        exploration_attempts: 0,
    })
}

/// Auto mode conversion with intelligent strategy selection
pub fn auto_convert(input: &Path, config: &ConversionConfig) -> Result<ConversionOutput> {
    let detection = detect_video(input)?;
    let strategy = determine_strategy(&detection);
    
    let output_dir = config.output_dir.clone()
        .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf());
    
    std::fs::create_dir_all(&output_dir)?;
    
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let output_path = output_dir.join(format!("{}.{}", stem, strategy.target.extension()));
    
    if output_path.exists() && !config.force {
        return Err(VidQualityError::ConversionError(
            format!("Output exists: {}", output_path.display())
        ));
    }
    
    info!("🎬 Auto Mode: {} → {}", input.display(), strategy.target.as_str());
    info!("   Reason: {}", strategy.reason);
    
    let (output_size, final_crf, attempts) = match strategy.target {
        TargetVideoFormat::FFV1_MKV => {
            let size = execute_ffv1_conversion(&detection, &output_path)?;
            (size, 0, 0)
        }
        TargetVideoFormat::AV1_MP4 => {
            if config.explore_smaller {
                // Size exploration mode
                explore_smaller_size(&detection, &output_path)?
            } else {
                let size = execute_av1_conversion(&detection, &output_path, 0)?;
                (size, 0, 0)
            }
        }
    };
    
    // Preserve metadata
    if config.preserve_metadata {
        preserve_metadata(input, &output_path)?;
    }
    preserve_timestamps(input, &output_path)?;
    
    let size_ratio = output_size as f64 / detection.file_size as f64;
    
    if config.delete_original {
        std::fs::remove_file(input)?;
        info!("   🗑️  Original deleted");
    }
    
    info!("   ✅ Complete: {:.1}% of original", size_ratio * 100.0);
    
    Ok(ConversionOutput {
        input_path: input.display().to_string(),
        output_path: output_path.display().to_string(),
        strategy: ConversionStrategy {
            target: strategy.target,
            reason: strategy.reason,
            command: String::new(),
            preserve_audio: detection.has_audio,
            crf: final_crf,
        },
        input_size: detection.file_size,
        output_size,
        size_ratio,
        success: true,
        message: if attempts > 0 {
            format!("Explored {} CRF values, final CRF: {}", attempts, final_crf)
        } else {
            "Conversion successful".to_string()
        },
        final_crf,
        exploration_attempts: attempts,
    })
}

/// Explore smaller size by trying higher CRF values (conservative approach)
/// Starts at CRF 0 and increases until output < input (even by 1 byte counts)
fn explore_smaller_size(
    detection: &VideoDetectionResult,
    output_path: &Path,
) -> Result<(u64, u8, u8)> {
    let input_size = detection.file_size;
    let mut current_crf: u8 = 0;
    let mut attempts: u8 = 0;
    const MAX_CRF: u8 = 23;  // Conservative limit
    const CRF_STEP: u8 = 1;   // Step size for exploration (conservative)
    
    info!("   🔍 Exploring smaller size (input: {} bytes)", input_size);
    
    loop {
        let output_size = execute_av1_conversion(detection, output_path, current_crf)?;
        attempts += 1;
        
        info!("   📊 CRF {}: {} bytes ({:.1}%)", 
            current_crf, output_size, (output_size as f64 / input_size as f64) * 100.0);
        
        // Success: output is smaller (even by 1 byte)
        if output_size < input_size {
            info!("   ✅ Found smaller output at CRF {}", current_crf);
            return Ok((output_size, current_crf, attempts));
        }
        
        // Try next CRF
        current_crf += CRF_STEP;
        
        // Safety limit
        if current_crf > MAX_CRF {
            warn!("   ⚠️  Reached CRF limit, using CRF {}", MAX_CRF);
            let output_size = execute_av1_conversion(detection, output_path, MAX_CRF)?;
            return Ok((output_size, MAX_CRF, attempts));
        }
    }
}

/// Execute FFV1 conversion
fn execute_ffv1_conversion(detection: &VideoDetectionResult, output: &Path) -> Result<u64> {
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(), detection.file_path.clone(),
        "-c:v".to_string(), "ffv1".to_string(),
        "-level".to_string(), "3".to_string(),
        "-coder".to_string(), "1".to_string(),
        "-context".to_string(), "1".to_string(),
        "-g".to_string(), "1".to_string(),
        "-slices".to_string(), "24".to_string(),
        "-slicecrc".to_string(), "1".to_string(),
    ];
    
    if detection.has_audio {
        args.extend(vec!["-c:a".to_string(), "flac".to_string()]);
    } else {
        args.push("-an".to_string());
    }
    
    args.push(output.display().to_string());
    
    let result = Command::new("ffmpeg").args(&args).output()?;
    
    if !result.status.success() {
        return Err(VidQualityError::FFmpegError(
            String::from_utf8_lossy(&result.stderr).to_string()
        ));
    }
    
    Ok(std::fs::metadata(output)?.len())
}

/// Execute AV1 conversion with specified CRF
fn execute_av1_conversion(detection: &VideoDetectionResult, output: &Path, crf: u8) -> Result<u64> {
    let mut args = vec![
        "-y".to_string(),
        "-i".to_string(), detection.file_path.clone(),
        "-c:v".to_string(), "libaom-av1".to_string(),
        "-crf".to_string(), crf.to_string(),
        "-b:v".to_string(), "0".to_string(),
        "-cpu-used".to_string(), "4".to_string(),
        "-row-mt".to_string(), "1".to_string(),
        "-tiles".to_string(), "2x2".to_string(),
    ];
    
    if detection.has_audio {
        args.extend(vec![
            "-c:a".to_string(), "aac".to_string(),
            "-b:a".to_string(), "320k".to_string(),
        ]);
    } else {
        args.push("-an".to_string());
    }
    
    args.push(output.display().to_string());
    
    let result = Command::new("ffmpeg").args(&args).output()?;
    
    if !result.status.success() {
        return Err(VidQualityError::FFmpegError(
            String::from_utf8_lossy(&result.stderr).to_string()
        ));
    }
    
    Ok(std::fs::metadata(output)?.len())
}

/// Preserve file timestamps
fn preserve_timestamps(source: &Path, dest: &Path) -> Result<()> {
    let _ = Command::new("touch")
        .args(&["-r", source.to_str().unwrap_or(""), dest.to_str().unwrap_or("")])
        .output();
    Ok(())
}

/// Preserve metadata using exiftool
fn preserve_metadata(source: &Path, dest: &Path) -> Result<()> {
    let _ = Command::new("exiftool")
        .args(&[
            "-overwrite_original",
            "-TagsFromFile", source.to_str().unwrap_or(""),
            "-All:All",
            dest.to_str().unwrap_or(""),
        ])
        .output();
    Ok(())
}

// Legacy alias for backward compatibility
pub fn smart_convert(input: &Path, config: &ConversionConfig) -> Result<ConversionOutput> {
    auto_convert(input, config)
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

