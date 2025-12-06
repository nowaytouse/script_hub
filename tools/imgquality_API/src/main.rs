use clap::{Parser, Subcommand, ValueEnum};
use imgquality::{analyze_image, get_recommendation, convert_image, ConversionOptions};
use imgquality::{calculate_psnr, calculate_ssim, psnr_quality_description, ssim_quality_description};
use rayon::prelude::*;
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "imgquality")]
#[command(version, about = "Image quality analyzer and format upgrade tool", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze image quality parameters
    Analyze {
        /// Input file or directory
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Recursive directory scan
        #[arg(short, long)]
        recursive: bool,

        /// Output format
        #[arg(short, long, value_enum, default_value = "human")]
        output: OutputFormat,

        /// Include upgrade recommendation
        #[arg(short = 'r', long)]
        recommend: bool,
    },

    /// Convert image to recommended format
    Convert {
        /// Input file or directory
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Target format
        #[arg(short, long, default_value = "jxl")]
        to: String,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Replace original file
        #[arg(long)]
        in_place: bool,

        /// Recursive directory scan
        #[arg(short, long)]
        recursive: bool,
    },

    /// Auto-convert based on format detection (JPEG→JXL, PNG→JXL, etc.)
    Auto {
        /// Input file or directory
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output directory (default: same as input)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Force conversion even if already processed
        #[arg(short, long)]
        force: bool,

        /// Recursive directory scan
        #[arg(short, long)]
        recursive: bool,

        /// Delete original after successful conversion
        #[arg(long)]
        delete_original: bool,

        /// Use mathematical lossless AVIF/AV1 (⚠️ VERY SLOW, huge files)
        #[arg(long)]
        lossless: bool,
    },

    /// Verify conversion quality
    Verify {
        /// Original file
        original: PathBuf,

        /// Converted file
        converted: PathBuf,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    /// Human-readable output
    Human,
    /// JSON output (for API use)
    Json,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze {
            input,
            recursive,
            output,
            recommend,
        } => {
            if input.is_file() {
                analyze_single_file(&input, output, recommend)?;
            } else if input.is_dir() {
                analyze_directory(&input, recursive, output, recommend)?;
            } else {
                eprintln!("❌ Error: Input path does not exist: {}", input.display());
                std::process::exit(1);
            }
        }

        Commands::Convert {
            input,
            to,
            output,
            in_place,
            recursive,
        } => {
            if input.is_file() {
                convert_single_file(&input, &to, output.as_deref(), in_place)?;
            } else if input.is_dir() {
                convert_directory(&input, &to, output.as_deref(), in_place, recursive)?;
            } else {
                eprintln!("❌ Error: Input path does not exist: {}", input.display());
                std::process::exit(1);
            }
        }

        Commands::Auto {
            input,
            output,
            force,
            recursive,
            delete_original,
            lossless,
        } => {
            if lossless {
                eprintln!("⚠️  Mathematical lossless mode: ENABLED (VERY SLOW!)");
            }
            if input.is_file() {
                auto_convert_single_file(&input, output.as_ref(), force, delete_original, lossless)?;
            } else if input.is_dir() {
                auto_convert_directory(&input, output.as_ref(), force, recursive, delete_original, lossless)?;
            } else {
                eprintln!("❌ Error: Input path does not exist: {}", input.display());
                std::process::exit(1);
            }
        }

        Commands::Verify { original, converted } => {
            verify_conversion(&original, &converted)?;
        }
    }

    Ok(())
}

fn analyze_single_file(
    path: &PathBuf,
    output_format: OutputFormat,
    recommend: bool,
) -> anyhow::Result<()> {
    let analysis = analyze_image(path)?;

    if output_format == OutputFormat::Json {
        let mut result = serde_json::to_value(&analysis)?;
        
        if recommend {
            let recommendation = get_recommendation(&analysis);
            result["recommendation"] = serde_json::to_value(&recommendation)?;
        }
        
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_analysis_human(&analysis);
        
        if recommend {
            let recommendation = get_recommendation(&analysis);
            print_recommendation_human(&recommendation);
        }
    }

    Ok(())
}

fn analyze_directory(
    path: &PathBuf,
    recursive: bool,
    output_format: OutputFormat,
    recommend: bool,
) -> anyhow::Result<()> {
    let image_extensions = ["png", "jpg", "jpeg", "webp", "gif", "tiff", "tif"];
    
    let walker = if recursive {
        WalkDir::new(path).follow_links(true)
    } else {
        WalkDir::new(path).max_depth(1)
    };

    let mut results = Vec::new();
    let mut count = 0;

    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if let Some(ext) = path.extension() {
            if image_extensions.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str()) {
                match analyze_image(&path.to_path_buf()) {
                    Ok(analysis) => {
                        count += 1;
                        if output_format == OutputFormat::Json {
                            let mut result = serde_json::to_value(&analysis)?;
                            if recommend {
                                let recommendation = get_recommendation(&analysis);
                                result["recommendation"] = serde_json::to_value(&recommendation)?;
                            }
                            results.push(result);
                        } else {
                            println!("\n{}", "=".repeat(80));
                            print_analysis_human(&analysis);
                            if recommend {
                                let recommendation = get_recommendation(&analysis);
                                print_recommendation_human(&recommendation);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️  Failed to analyze {}: {}", path.display(), e);
                    }
                }
            }
        }
    }

    if output_format == OutputFormat::Json {
        println!("{}", json!({
            "total": count,
            "results": results
        }).to_string());
    } else {
        println!("\n{}", "=".repeat(80));
        println!("✅ Analysis complete: {} files processed", count);
    }

    Ok(())
}

fn convert_single_file(
    input: &PathBuf,
    target_format: &str,
    output_dir: Option<&std::path::Path>,
    in_place: bool,
) -> anyhow::Result<()> {
    let analysis = analyze_image(input)?;
    let recommendation = get_recommendation(&analysis);

    let options = ConversionOptions {
        target_format: target_format.to_string(),
        output_dir: output_dir.map(|p| p.to_path_buf()),
        in_place,
        quality_params: recommendation.command.clone(),
    };

    println!("🔄 Converting: {}", input.display());
    let output_path = convert_image(input, &options)?;
    println!("✅ Converted to: {}", output_path.display());

    Ok(())
}

fn convert_directory(
    input: &PathBuf,
    target_format: &str,
    output_dir: Option<&std::path::Path>,
    in_place: bool,
    recursive: bool,
) -> anyhow::Result<()> {
    let image_extensions = ["png", "jpg", "jpeg", "webp", "gif", "tiff", "tif"];
    
    let walker = if recursive {
        WalkDir::new(input).follow_links(true)
    } else {
        WalkDir::new(input).max_depth(1)
    };

    let mut success = 0;
    let mut failed = 0;

    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        if let Some(ext) = path.extension() {
            if image_extensions.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str()) {
                match convert_single_file(&path.to_path_buf(), target_format, output_dir, in_place) {
                    Ok(_) => success += 1,
                    Err(e) => {
                        eprintln!("❌ Failed to convert {}: {}", path.display(), e);
                        failed += 1;
                    }
                }
            }
        }
    }

    println!("\n{}", "=".repeat(80));
    println!("✅ Conversion complete: {} succeeded, {} failed", success, failed);

    Ok(())
}

fn verify_conversion(original: &PathBuf, converted: &PathBuf) -> anyhow::Result<()> {
    println!("🔍 Verifying conversion quality...");
    println!("   Original:  {}", original.display());
    println!("   Converted: {}", converted.display());

    let original_analysis = analyze_image(original)?;
    let converted_analysis = analyze_image(converted)?;

    println!("\n📊 Size Comparison:");
    println!("   Original size:  {} bytes ({:.2} KB)", 
        original_analysis.file_size, original_analysis.file_size as f64 / 1024.0);
    println!("   Converted size: {} bytes ({:.2} KB)", 
        converted_analysis.file_size, converted_analysis.file_size as f64 / 1024.0);
    
    let reduction = 100.0 * (1.0 - converted_analysis.file_size as f64 / original_analysis.file_size as f64);
    println!("   Size reduction: {:.2}%", reduction);

    // Load images for quality comparison
    let orig_img = load_image_safe(original)?;
    let conv_img = load_image_safe(converted)?;
    
    println!("\n📏 Quality Metrics:");
    if let Some(psnr) = calculate_psnr(&orig_img, &conv_img) {
        if psnr.is_infinite() {
            println!("   PSNR: ∞ dB (Identical - mathematically lossless)");
        } else {
            println!("   PSNR: {:.2} dB ({})", psnr, psnr_quality_description(psnr));
        }
    }
    
    if let Some(ssim) = calculate_ssim(&orig_img, &conv_img) {
        println!("   SSIM: {:.6} ({})", ssim, ssim_quality_description(ssim));
    }

    println!("\n✅ Verification complete");

    Ok(())
}

/// Load image safely, handling JXL via external decoder if needed
fn load_image_safe(path: &PathBuf) -> anyhow::Result<image::DynamicImage> {
    // Check extension
    let is_jxl = path.extension()
        .map(|e| e.to_string_lossy().to_lowercase() == "jxl")
        .unwrap_or(false);
        
    if is_jxl {
        use std::process::Command;
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let temp_path = std::env::temp_dir().join(format!("imgquality_verify_{}.png", timestamp));
        
        // Decode JXL to PNG using djxl
        let status = Command::new("djxl")
            .arg(path)
            .arg(&temp_path)
            .status()
            .map_err(|e| anyhow::anyhow!("Failed to execute djxl: {}", e))?;
            
        if !status.success() {
            return Err(anyhow::anyhow!("djxl failed to decode JXL file"));
        }
        
        // Load the temp PNG
        let img = image::open(&temp_path).map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            anyhow::anyhow!("Failed to open decoded PNG: {}", e)
        })?;
        
        // Cleanup
        let _ = std::fs::remove_file(&temp_path);
        
        Ok(img)
    } else {
        Ok(image::open(path)?)
    }
}

fn print_analysis_human(analysis: &imgquality::ImageAnalysis) {
    println!("\n📊 Image Quality Analysis Report");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📁 File: {}", analysis.file_path);
    println!("📷 Format: {} {}", analysis.format, 
        if analysis.is_lossless { "(Lossless)" } else { "(Lossy)" });
    println!("📐 Dimensions: {}x{}", analysis.width, analysis.height);
    println!("💾 Size: {} bytes ({:.2} KB)", 
        analysis.file_size, 
        analysis.file_size as f64 / 1024.0);
    println!("🎨 Bit depth: {}-bit {}", analysis.color_depth, analysis.color_space);
    if analysis.has_alpha {
        println!("🔍 Alpha channel: Yes");
    }
    if analysis.is_animated {
        println!("🎬 Animated: Yes");
    }
    
    // Quality analysis section
    println!("\n📈 Quality Analysis");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔒 Compression: {}", if analysis.is_lossless { "Lossless ✓" } else { "Lossy" });
    println!("📊 Entropy:   {:.2} ({})", 
        analysis.features.entropy,
        if analysis.features.entropy > 7.0 { "High complexity" } 
        else if analysis.features.entropy > 5.0 { "Medium complexity" } 
        else { "Low complexity" });
    println!("📦 Compression ratio:   {:.1}%", analysis.features.compression_ratio * 100.0);
    
    // JPEG specific analysis with enhanced details
    if let Some(ref jpeg) = analysis.jpeg_analysis {
        println!("\n🎯 JPEGQuality Analysis (精度: ±1)");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("📊 Estimated quality: Q={} ({})", jpeg.estimated_quality, jpeg.quality_description);
        println!("🎯 Confidence:   {:.1}%", jpeg.confidence * 100.0);
        println!("📋 Quantization table:   {}", 
            if jpeg.is_standard_table { "IJG Standard ✓" } else { "Custom" });
        
        // Show both luma and chroma quality if available
        if let Some(chroma_q) = jpeg.chrominance_quality {
            println!("🔬 Luma quality: Q={} (SSE: {:.1})", jpeg.luminance_quality, jpeg.luminance_sse);
            if let Some(chroma_sse) = jpeg.chrominance_sse {
                println!("🔬 Chroma quality: Q={} (SSE: {:.1})", chroma_q, chroma_sse);
            }
        } else {
            println!("🔬 Luma SSE:  {:.1}", jpeg.luminance_sse);
        }
        
        // Show encoder hint if detected
        if let Some(ref encoder) = jpeg.encoder_hint {
            println!("🏭 Encoder:   {}", encoder);
        }
        
        if jpeg.is_high_quality_original {
            println!("✨ Assessment: High quality original");
        }
    }
    
    // Legacy PSNR/SSIM
    if let Some(psnr) = analysis.psnr {
        println!("\n📐 Estimated metrics");
        println!("   PSNR: {:.2} dB", psnr);
        if let Some(ssim) = analysis.ssim {
            println!("   SSIM: {:.4}", ssim);
        }
    }
}

fn print_recommendation_human(rec: &imgquality::UpgradeRecommendation) {
    println!("\n💡 JXL Format Recommendation");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    if rec.recommended_format == rec.current_format {
        println!("ℹ️  {}", rec.reason);
    } else {
        println!("✅ {} → {}", rec.current_format, rec.recommended_format);
        println!("📝 原因: {}", rec.reason);
        println!("🎯 质量: {}", rec.quality_preservation);
        if rec.expected_size_reduction > 0.0 {
            println!("💾 预期减少: {:.1}%", rec.expected_size_reduction);
        }
        if !rec.command.is_empty() {
            println!("⚙️  命令: {}", rec.command);
        }
    }
}

/// Smart auto-convert a single file based on format detection
fn auto_convert_single_file(
    input: &PathBuf,
    output_dir: Option<&PathBuf>,
    force: bool,
    delete_original: bool,
    lossless: bool,
) -> anyhow::Result<()> {
    use imgquality::lossless_converter::{
        convert_to_jxl, convert_jpeg_to_jxl,
        convert_to_av1_mp4, convert_to_av1_mp4_lossless,
        ConvertOptions,
    };
    
    let analysis = analyze_image(input)?;
    
    let options = ConvertOptions {
        force,
        output_dir: output_dir.cloned(),
        delete_original,
    };
    
    // Smart conversion based on format and lossless status
    let result = match (analysis.format.as_str(), analysis.is_lossless, analysis.is_animated) {
        // Modern Formats Logic (WebP, AVIF, HEIC)
        // Rule: Avoid generational loss. 
        // - If Lossy: SKIP (don't recompress lossy to lossy/jxl)
        // - If Lossless: CONVERT to JXL (better compression)
        ("WebP", true, false) | ("AVIF", true, false) | ("HEIC", true, false) | ("HEIF", true, false) => {
            println!("🔄 Modern Lossless→JXL: {}", input.display());
            convert_to_jxl(input, &options, 0.0)? // Mathematical lossless
        }
        ("WebP", false, _) | ("AVIF", false, _) | ("HEIC", false, _) | ("HEIF", false, _) => {
            println!("⏭️ Skipping modern lossy format (avoid generation loss): {}", input.display());
            return Ok(());
        }

        // JPEG → JXL lossless transcode
        ("JPEG", _, false) => {
            println!("🔄 JPEG→JXL lossless transcode: {}", input.display());
            convert_jpeg_to_jxl(input, &options)?
        }
        // Legacy Static lossless (PNG, TIFF, BMP etc) → JXL
        (_, true, false) => {
            println!("🔄 Legacy Lossless→JXL: {}", input.display());
            convert_to_jxl(input, &options, 0.0)?
        }
        // Animated lossless → AV1 MP4 (only if >=3 seconds)
        (_, true, true) => {
            // Check duration - only convert animations >=3 seconds
            let duration = analysis.duration_secs.unwrap_or(0.0);
            if duration < 3.0 {
                println!("⏭️ Skipping short animation ({:.1}s < 3s): {}", duration, input.display());
                return Ok(());
            }
            
            if lossless {
                println!("🔄 Animated lossless→AV1 MP4 (LOSSLESS, {:.1}s): {}", duration, input.display());
                convert_to_av1_mp4_lossless(input, &options)?
            } else {
                println!("🔄 Animated lossless→AV1 MP4 ({:.1}s): {}", duration, input.display());
                convert_to_av1_mp4(input, &options)?
            }
        }
        // Animated lossy → skip (unless lossless mode AND >=3 seconds)
        (_, false, true) => {
            let duration = analysis.duration_secs.unwrap_or(0.0);
            if lossless && duration >= 3.0 {
                println!("🔄 Animated lossy→AV1 MP4 (LOSSLESS, {:.1}s): {}", duration, input.display());
                convert_to_av1_mp4_lossless(input, &options)?
            } else if duration < 3.0 {
                println!("⏭️ Skipping short animation ({:.1}s < 3s): {}", duration, input.display());
                return Ok(());
            } else {
                println!("⏭️ Skipping animated lossy: {}", input.display());
                return Ok(());
            }
        }
        // Legacy Static lossy (non-JPEG, non-Modern) → JXL (Quality 100/Lossy - VarDCT)
        // This handles cases like BMP (if not detected as lossless somehow) or other obscure formats
        (format, false, false) => {
             // Redundant safecheck for WebP/AVIF/HEIC just in case pattern matching missed
            if format == "WebP" || format == "AVIF" || format == "HEIC" || format == "HEIF" {
                println!("⏭️ Skipping modern lossy format: {}", input.display());
                return Ok(());
            }
            
            println!("🔄 Legacy Lossy→JXL (Quality 100): {}", input.display());
            convert_to_jxl(input, &options, 0.1)?
        }
    };
    
    if result.skipped {
        println!("⏭️ {}", result.message);
    } else if let Some(reduction) = result.size_reduction {
        println!("✅ {} (reduced {:.1}%)", result.message, reduction);
    }
    
    Ok(())
}

/// Smart auto-convert a directory with parallel processing
fn auto_convert_directory(
    input: &PathBuf,
    output_dir: Option<&PathBuf>,
    force: bool,
    recursive: bool,
    delete_original: bool,
    lossless: bool,
) -> anyhow::Result<()> {
    let image_extensions = ["png", "jpg", "jpeg", "webp", "gif", "tiff", "tif", "heic", "avif"];
    
    let walker = if recursive {
        WalkDir::new(input).follow_links(true)
    } else {
        WalkDir::new(input).max_depth(1)
    };

    // Collect all file paths first for parallel processing
    let files: Vec<PathBuf> = walker
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            if let Some(ext) = e.path().extension() {
                image_extensions.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str())
            } else {
                false
            }
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let total = files.len();
    println!("📂 Found {} files to process (parallel mode)", total);
    if lossless {
        println!("⚠️  Mathematical lossless mode: ENABLED (VERY SLOW!)");
    }

    // Atomic counters for thread-safe counting  
    let success = AtomicUsize::new(0);
    let skipped = AtomicUsize::new(0);
    let failed = AtomicUsize::new(0);

    // Process files in parallel using rayon
    files.par_iter().for_each(|path| {
        match auto_convert_single_file(path, output_dir, force, delete_original, lossless) {
            Ok(_) => { success.fetch_add(1, Ordering::Relaxed); }
            Err(e) => {
                let msg = e.to_string();
                if msg.contains("Skipped") || msg.contains("skip") {
                    skipped.fetch_add(1, Ordering::Relaxed);
                } else {
                    eprintln!("❌ Conversion failed {}: {}", path.display(), e);
                    failed.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    });

    let success_count = success.load(Ordering::Relaxed);
    let skipped_count = skipped.load(Ordering::Relaxed);
    let failed_count = failed.load(Ordering::Relaxed);

    println!("\n{}", "=".repeat(60));
    println!("✅ Auto-conversion complete: {} succeeded, {} skipped, {} failed", 
        success_count, skipped_count, failed_count);

    Ok(())
}

