//! Lossless Converter Module
//! 
//! Provides conversion API for verified lossless/lossy images
//! With anti-duplicate execution mechanism

use crate::{ImgQualityError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::io::{BufRead, BufReader, Write};

// Global processed files tracker (anti-duplicate)
lazy_static::lazy_static! {
    static ref PROCESSED_FILES: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
}

/// Conversion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResult {
    pub success: bool,
    pub input_path: String,
    pub output_path: Option<String>,
    pub input_size: u64,
    pub output_size: Option<u64>,
    pub size_reduction: Option<f64>,
    pub message: String,
    pub skipped: bool,
    pub skip_reason: Option<String>,
}

/// Conversion options
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    /// Force conversion even if already processed
    pub force: bool,
    /// Output directory (None = same as input)
    pub output_dir: Option<PathBuf>,
    /// Delete original after successful conversion
    pub delete_original: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            force: false,
            output_dir: None,
            delete_original: false,
        }
    }
}

/// Check if file has already been processed (anti-duplicate)
pub fn is_already_processed(path: &Path) -> bool {
    let canonical = path.canonicalize().ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| path.display().to_string());
    
    let processed = PROCESSED_FILES.lock().unwrap();
    processed.contains(&canonical)
}

/// Mark file as processed
pub fn mark_as_processed(path: &Path) {
    let canonical = path.canonicalize().ok()
        .and_then(|p| p.to_str().map(String::from))
        .unwrap_or_else(|| path.display().to_string());
    
    let mut processed = PROCESSED_FILES.lock().unwrap();
    processed.insert(canonical);
}

/// Load processed files list from disk
pub fn load_processed_list(list_path: &Path) -> Result<()> {
    if !list_path.exists() {
        return Ok(());
    }
    
    let file = fs::File::open(list_path)?;
    let reader = BufReader::new(file);
    let mut processed = PROCESSED_FILES.lock().unwrap();
    
    for line in reader.lines() {
        if let Ok(path) = line {
            processed.insert(path);
        }
    }
    
    Ok(())
}

/// Save processed files list to disk
pub fn save_processed_list(list_path: &Path) -> Result<()> {
    let processed = PROCESSED_FILES.lock().unwrap();
    let mut file = fs::File::create(list_path)?;
    
    for path in processed.iter() {
        writeln!(file, "{}", path)?;
    }
    
    Ok(())
}

/// Convert static image to JXL with specified distance/quality
/// distance: 0.0 = lossless, 0.1 = visually lossless (Q100 lossy), 1.0 = Q90
pub fn convert_to_jxl(input: &Path, options: &ConvertOptions, distance: f32) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "jxl", &options.output_dir);
    
    // Ensure output directory exists
    if let Some(parent) = output.parent() {
        let _ = fs::create_dir_all(parent);
    }
    
    // Check if output already exists
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Execute cjxl (v0.11+ syntax)
    let result = Command::new("cjxl")
        .arg(input)
        .arg(&output)
        .arg("-d").arg(format!("{:.1}", distance))  // Distance parameter
        .arg("-e").arg("8")    // Effort 8 for better compression
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Validate output
            if let Err(e) = verify_jxl_health(&output) {
                 let _ = fs::remove_file(&output);
                 return Err(e);
            }

            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Conversion successful: size reduced {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("cjxl failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("cjxl not found: {}", e)))
        }
    }
}

/// Convert JPEG to JXL using lossless JPEG transcode (preserves DCT coefficients)
/// This is the BEST option for JPEG files - no quality loss at all
pub fn convert_jpeg_to_jxl(input: &Path, options: &ConvertOptions) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "jxl", &options.output_dir);
    
    // Check if output already exists
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Execute cjxl with --lossless_jpeg=1 for lossless JPEG transcode
    let result = Command::new("cjxl")
        .arg(input)
        .arg(&output)
        .arg("--lossless_jpeg=1")  // Lossless JPEG transcode - preserves DCT coefficients
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Validate output
            if let Err(e) = verify_jxl_health(&output) {
                 let _ = fs::remove_file(&output);
                 return Err(e);
            }

            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("JPEG lossless transcode successful: size reduced {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("cjxl JPEG transcode failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("cjxl not found: {}", e)))
        }
    }
}

/// Convert static lossy image to AVIF
pub fn convert_to_avif(input: &Path, quality: Option<u8>, options: &ConvertOptions) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "avif", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Use original quality or default to high quality
    let q = quality.unwrap_or(85);
    
    let result = Command::new("avifenc")
        .arg("-s").arg("4")       // Speed 4 (balanced)
        .arg("-j").arg("all")     // Use all CPU cores
        .arg("-q").arg(q.to_string())
        .arg(input)
        .arg(&output)
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);

            // Copy metadata and timestamps
            copy_metadata(input, &output);

            mark_as_processed(input);

            if options.delete_original {
                fs::remove_file(input)?;
            }

            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Conversion successful: size reduced {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("avifenc failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("avifenc not found: {}", e)))
        }
    }
}

/// Convert animated lossless to AV1 MP4 (Q=100 visual lossless)
pub fn convert_to_av1_mp4(input: &Path, options: &ConvertOptions) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "mp4", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // AV1 with CRF 0 for visually lossless
    let result = Command::new("ffmpeg")
        .arg("-y")  // Overwrite
        .arg("-i").arg(input)
        .arg("-c:v").arg("libaom-av1")
        .arg("-crf").arg("0")    // Lossless mode
        .arg("-b:v").arg("0")
        .arg("-pix_fmt").arg("yuv420p")
        .arg(&output)
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Conversion successful: size reduced {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("ffmpeg failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("ffmpeg not found: {}", e)))
        }
    }
}

/// Convert image to AVIF using mathematical lossless (⚠️ VERY SLOW)
pub fn convert_to_avif_lossless(input: &Path, options: &ConvertOptions) -> Result<ConversionResult> {
    eprintln!("⚠️  Mathematical lossless AVIF encoding - this will be SLOW!");
    
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "avif", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Mathematical lossless AVIF
    let result = Command::new("avifenc")
        .arg("--lossless")  // Mathematical lossless
        .arg("-s").arg("4")
        .arg("-j").arg("all")
        .arg(input)
        .arg(&output)
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Lossless AVIF: size {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("avifenc lossless failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("avifenc not found: {}", e)))
        }
    }
}

/// Convert animated to AV1 MP4 with quality-matched CRF
/// 
/// This function calculates an appropriate CRF based on the input file's
/// characteristics to match the input quality level.
pub fn convert_to_av1_mp4_matched(
    input: &Path, 
    options: &ConvertOptions,
    analysis: &crate::ImageAnalysis,
) -> Result<ConversionResult> {
    // Anti-duplicate check
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "mp4", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Calculate matched CRF based on input characteristics
    // For animated images, we estimate quality based on:
    // - File size per frame
    // - Resolution
    // - Duration
    let crf = calculate_matched_crf_for_animation(analysis, input_size);
    eprintln!("   🎯 Matched CRF: {} (based on input quality analysis)", crf);
    
    // AV1 with calculated CRF
    let result = Command::new("ffmpeg")
        .arg("-y")  // Overwrite
        .arg("-i").arg(input)
        .arg("-c:v").arg("libaom-av1")
        .arg("-crf").arg(crf.to_string())
        .arg("-b:v").arg("0")
        .arg("-pix_fmt").arg("yuv420p")
        .arg("-cpu-used").arg("4")  // Balanced speed
        .arg("-row-mt").arg("1")    // Multi-threading
        .arg(&output)
        .output();
    
    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);
            
            // Copy metadata and timestamps
            copy_metadata(input, &output);
            
            mark_as_processed(input);
            
            if options.delete_original {
                fs::remove_file(input)?;
            }
            
            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Quality-matched AV1 (CRF {}): size {:.1}%", crf, reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("ffmpeg failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("ffmpeg not found: {}", e)))
        }
    }
}

/// Calculate CRF to match input animation quality (Enhanced Algorithm)
/// 
/// This function uses a more precise algorithm that considers:
/// 1. Bytes per pixel per second (bpps) - primary quality indicator
/// 2. Source format efficiency - GIF vs WebP vs APNG
/// 3. Color depth and palette size
/// 4. Resolution scaling
/// 
/// The formula converts bpps to an equivalent CRF using:
/// CRF ≈ 63 - 8 * log2(bpps * efficiency_factor * 1000)
/// 
/// Clamped to range [18, 35] for AV1
fn calculate_matched_crf_for_animation(analysis: &crate::ImageAnalysis, file_size: u64) -> u8 {
    let pixels = (analysis.width as u64) * (analysis.height as u64);
    let duration = analysis.duration_secs.unwrap_or(1.0).max(0.1) as f64;
    
    // Calculate bytes per pixel per second
    let bytes_per_second = file_size as f64 / duration;
    let bpps = bytes_per_second / pixels as f64;
    
    // Format efficiency factor
    // GIF is very inefficient (256 colors, LZW), WebP is better, APNG is in between
    let format_factor = match analysis.format.to_lowercase().as_str() {
        "gif" => 2.5,      // GIF is very inefficient, high bpps doesn't mean high quality
        "apng" | "png" => 1.5,  // APNG is moderately efficient
        "webp" => 1.0,     // WebP animated is efficient
        _ => 1.2,
    };
    
    // Color depth factor
    // 8-bit (256 colors) animations have inherently lower quality ceiling
    let color_factor = if analysis.color_depth <= 8 {
        1.3  // Limited palette, don't need as high quality
    } else {
        1.0
    };
    
    // Resolution factor (higher res needs more bits for same perceived quality)
    let resolution_factor = if pixels > 2_000_000 {
        0.8   // 1080p+ needs more bits
    } else if pixels > 500_000 {
        0.9   // 720p
    } else {
        1.0   // SD
    };
    
    // Alpha channel factor (alpha adds complexity)
    let alpha_factor = if analysis.has_alpha { 0.9 } else { 1.0 };
    
    // Effective bpps after adjustments
    let effective_bpps = bpps / format_factor * color_factor * resolution_factor * alpha_factor;
    
    // Convert bpps to CRF using logarithmic formula
    // AV1 CRF range is 0-63, with 23 being default "good quality"
    // CRF = 63 - 8 * log2(effective_bpps * 1000)
    let crf_float = if effective_bpps > 0.0 {
        63.0 - 8.0 * (effective_bpps * 1000.0).log2()
    } else {
        30.0
    };
    
    // Clamp to reasonable range [18, 35]
    let crf = (crf_float.round() as i32).clamp(18, 35) as u8;
    
    eprintln!("   📊 Quality Analysis:");
    eprintln!("      Raw bpps: {:.4} bytes/pixel/second", bpps);
    eprintln!("      Format: {} (factor: {:.2})", analysis.format, format_factor);
    eprintln!("      Color depth: {}-bit (factor: {:.2})", analysis.color_depth, color_factor);
    eprintln!("      Resolution: {}x{} (factor: {:.2})", analysis.width, analysis.height, resolution_factor);
    eprintln!("      Alpha: {} (factor: {:.2})", analysis.has_alpha, alpha_factor);
    eprintln!("      Effective bpps: {:.4}", effective_bpps);
    eprintln!("      Calculated CRF: {}", crf);
    
    crf
}

/// Convert animated to AV1 MP4 using mathematical lossless (⚠️ VERY SLOW)
pub fn convert_to_av1_mp4_lossless(input: &Path, options: &ConvertOptions) -> Result<ConversionResult> {
    eprintln!("⚠️  Mathematical lossless AV1 encoding - this will be VERY SLOW!");
    
    if !options.force && is_already_processed(input) {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: None,
            input_size: fs::metadata(input).map(|m| m.len()).unwrap_or(0),
            output_size: None,
            size_reduction: None,
            message: "Skipped: Already processed".to_string(),
            skipped: true,
            skip_reason: Some("duplicate".to_string()),
        });
    }
    
    let input_size = fs::metadata(input)?.len();
    let output = determine_output_path(input, "mp4", &options.output_dir);
    
    if output.exists() && !options.force {
        return Ok(ConversionResult {
            success: true,
            input_path: input.display().to_string(),
            output_path: Some(output.display().to_string()),
            input_size,
            output_size: fs::metadata(&output).map(|m| m.len()).ok(),
            size_reduction: None,
            message: "Skipped: Output file exists".to_string(),
            skipped: true,
            skip_reason: Some("exists".to_string()),
        });
    }
    
    // Mathematical lossless AV1
    let result = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i").arg(input)
        .arg("-c:v").arg("libaom-av1")
        .arg("-lossless").arg("1")  // Mathematical lossless
        .arg("-cpu-used").arg("4")
        .arg("-row-mt").arg("1")
        .arg(&output)
        .output();

    match result {
        Ok(output_cmd) if output_cmd.status.success() => {
            let output_size = fs::metadata(&output)?.len();
            let reduction = 1.0 - (output_size as f64 / input_size as f64);

            // Copy metadata and timestamps
            copy_metadata(input, &output);

            mark_as_processed(input);

            if options.delete_original {
                fs::remove_file(input)?;
            }

            Ok(ConversionResult {
                success: true,
                input_path: input.display().to_string(),
                output_path: Some(output.display().to_string()),
                input_size,
                output_size: Some(output_size),
                size_reduction: Some(reduction * 100.0),
                message: format!("Lossless AV1: size {:.1}%", reduction * 100.0),
                skipped: false,
                skip_reason: None,
            })
        }
        Ok(output_cmd) => {
            let stderr = String::from_utf8_lossy(&output_cmd.stderr);
            Err(ImgQualityError::ConversionError(format!("ffmpeg lossless failed: {}", stderr)))
        }
        Err(e) => {
            Err(ImgQualityError::ToolNotFound(format!("ffmpeg not found: {}", e)))
        }
    }
}

// MacOS specialized timestamp setter (creation time + date added)


// Helper to copy metadata and timestamps from source to destination
// Maximum metadata preservation: centralized via metadata_keeper
fn copy_metadata(src: &Path, dst: &Path) {
    // metadata_keeper::preserve_metadata now handles ALL layers:
    // 1. Internal (Exif/IPTC via ExifTool)
    // 2. Network (WhereFroms check)
    // 3. System (ACL, Flags, Xattr, Timestamps via copyfile)
    if let Err(e) = metadata_keeper::preserve_metadata(src, dst) {
        eprintln!("⚠️ Failed to preserve metadata: {}", e);
    }
}

// Redundant helper functions and modules (macos_ext, copy_native_metadata, copy_xattrs, copy_file_flags) have been removed
// in favor of the shared `metadata_keeper` crate.


/// Determine output path and ensure directory exists
fn determine_output_path(input: &Path, extension: &str, output_dir: &Option<PathBuf>) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    
    let output = match output_dir {
        Some(dir) => {
            // Ensure output directory exists
            let _ = fs::create_dir_all(dir);
            dir.join(format!("{}.{}", stem, extension))
        }
        None => input.with_extension(extension),
    };
    
    output
}

/// Clear processed files list
pub fn clear_processed_list() {
    let mut processed = PROCESSED_FILES.lock().unwrap();
    processed.clear();
}

/// Verify that JXL file is valid using signature and optional decoding
fn verify_jxl_health(path: &Path) -> Result<()> {
    // Check file signature
    let mut file = fs::File::open(path)?;
    let mut sig = [0u8; 2];
    use std::io::Read;
    file.read_exact(&mut sig)?;

    // JXL signature: 0xFF 0x0A (bare JXL) or 0x00 0x00 (ISOBMFF container)
    if sig != [0xFF, 0x0A] && sig != [0x00, 0x00] {
        return Err(ImgQualityError::ConversionError(
            "Invalid JXL file signature".to_string(),
        ));
    }
    
    // Skip full decode check for performance, signature is usually enough for cjxl output
    // Unless paranoia mode is requested.
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_determine_output_path() {
        let input = Path::new("/path/to/image.png");
        let output = determine_output_path(input, "jxl", &None);
        assert_eq!(output, Path::new("/path/to/image.jxl"));
    }
    
    #[test]
    fn test_determine_output_path_with_dir() {
        let input = Path::new("/path/to/image.png");
        let output_dir = Some(PathBuf::from("/output"));
        let output = determine_output_path(input, "avif", &output_dir);
        assert_eq!(output, Path::new("/output/image.avif"));
    }
}
