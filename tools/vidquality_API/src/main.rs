use clap::{Parser, Subcommand, ValueEnum};
use tracing::info;
use std::path::PathBuf;
use vidquality::{detect_video, auto_convert, determine_strategy, ConversionConfig};

#[derive(Parser)]
#[command(name = "vidquality")]
#[command(version, about = "Video quality analyzer and format converter - FFV1 archival and AV1 compression", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze video properties
    Analyze {
        /// Input video file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output format
        #[arg(short, long, default_value = "human")]
        output: OutputFormat,
    },

    /// Auto mode: FFV1 for lossless, AV1 for lossy (intelligent selection)
    Auto {
        /// Input video file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Force overwrite existing files
        #[arg(short, long)]
        force: bool,

        /// Delete original after conversion
        #[arg(long)]
        delete_original: bool,

        /// Explore smaller size (try higher CRF if output > input)
        #[arg(long)]
        explore: bool,

        /// Use mathematical lossless AV1 (⚠️ VERY SLOW, huge files)
        #[arg(long)]
        lossless: bool,
    },

    /// Simple mode: ALL videos → AV1 MP4
    Simple {
        /// Input video file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Use mathematical lossless AV1 (⚠️ VERY SLOW, huge files)
        #[arg(long)]
        lossless: bool,
    },

    /// Show recommended strategy without converting
    Strategy {
        /// Input video file
        #[arg(value_name = "INPUT")]
        input: PathBuf,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    /// Human-readable output
    Human,
    /// JSON output
    Json,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { input, output } => {
            let result = detect_video(&input)?;
            
            match output {
                OutputFormat::Human => print_analysis_human(&result),
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
            }
        }

        Commands::Auto { input, output, force, delete_original, explore, lossless } => {
            let config = ConversionConfig {
                output_dir: output.clone(),
                force,
                delete_original,
                preserve_metadata: true,
                explore_smaller: explore,
                use_lossless: lossless,
            };
            
            info!("🎬 Auto Mode Conversion");
            info!("   Lossless sources → FFV1 MKV (archival)");
            info!("   Lossy sources → AV1 MP4 (high quality)");
            if lossless {
                info!("   ⚠️  Mathematical lossless AV1: ENABLED (VERY SLOW!)");
            }
            if explore {
                info!("   📊 Size exploration: ENABLED");
            }
            info!("");
            
            if input.is_dir() {
                // Directory processing
                let video_extensions = ["mp4", "mkv", "avi", "mov", "webm", "flv", "wmv", "m4v", "mpg", "mpeg", "ts", "mts"];
                
                let files: Vec<_> = std::fs::read_dir(&input)?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_file())
                    .filter(|e| {
                        if let Some(ext) = e.path().extension() {
                            video_extensions.contains(&ext.to_str().unwrap_or("").to_lowercase().as_str())
                        } else {
                            false
                        }
                    })
                    .map(|e| e.path())
                    .collect();
                
                info!("📂 Found {} video files to process", files.len());
                
                let mut success = 0;
                let mut failed = 0;
                
                for file in &files {
                    match auto_convert(file, &config) {
                        Ok(result) => {
                            info!("✅ {} → {} ({:.1}%)", 
                                file.file_name().unwrap_or_default().to_string_lossy(),
                                result.output_path,
                                result.size_ratio * 100.0
                            );
                            success += 1;
                        }
                        Err(e) => {
                            info!("❌ {} failed: {}", file.display(), e);
                            failed += 1;
                        }
                    }
                }
                
                info!("");
                info!("📊 Batch Summary: {} succeeded, {} failed", success, failed);
            } else {
                // Single file processing
                let result = auto_convert(&input, &config)?;
                
                info!("");
                info!("📊 Conversion Summary:");
                info!("   Input:  {} ({} bytes)", result.input_path, result.input_size);
                info!("   Output: {} ({} bytes)", result.output_path, result.output_size);
                info!("   Ratio:  {:.1}%", result.size_ratio * 100.0);
                if result.exploration_attempts > 0 {
                    info!("   🔍 Explored {} CRF values, final: CRF {}", result.exploration_attempts, result.final_crf);
                }
            }
        }

        Commands::Simple { input, output, lossless: _ } => {
            info!("🎬 Simple Mode Conversion");
            info!("   ⚠️  ALL videos → AV1 MP4 (MATHEMATICAL LOSSLESS - VERY SLOW!)");
            info!("   (Note: Simple mode now enforces lossless conversion by default)");
            info!("");
            
            let result = vidquality::simple_convert(&input, output.as_deref())?;
            
            info!("");
            info!("✅ Complete!");
            info!("   Output: {}", result.output_path);
            info!("   Size: {:.1}% of original", result.size_ratio * 100.0);
        }

        Commands::Strategy { input } => {
            let detection = detect_video(&input)?;
            let strategy = determine_strategy(&detection);
            
            println!("\n🎯 Recommended Strategy (Auto Mode)");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📁 File: {}", input.display());
            println!("🎬 Codec: {} ({})", detection.codec.as_str(), detection.compression.as_str());
            println!("");
            println!("💡 Target: {}", strategy.target.as_str());
            println!("📝 Reason: {}", strategy.reason);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        }
    }

    Ok(())
}

fn print_analysis_human(result: &vidquality::VideoDetectionResult) {
    println!("\n📊 Video Analysis Report");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📁 File: {}", result.file_path);
    println!("📦 Format: {}", result.format);
    println!("🎬 Codec: {} ({})", result.codec.as_str(), result.codec_long);
    println!("🔍 Compression: {}", result.compression.as_str());
    println!("");
    println!("📐 Resolution: {}x{}", result.width, result.height);
    println!("🎞️  Frames: {} @ {:.2} fps", result.frame_count, result.fps);
    println!("⏱️  Duration: {:.2}s", result.duration_secs);
    println!("🎨 Bit Depth: {}-bit", result.bit_depth);
    println!("🌈 Pixel Format: {}", result.pix_fmt);
    println!("");
    println!("💾 File Size: {} bytes", result.file_size);
    println!("📊 Bitrate: {} bps", result.bitrate);
    println!("🎵 Audio: {}", if result.has_audio { 
        result.audio_codec.as_deref().unwrap_or("yes") 
    } else { 
        "no" 
    });
    println!("");
    println!("⭐ Quality Score: {}/100", result.quality_score);
    println!("📦 Archival Candidate: {}", if result.archival_candidate { "✅ Yes" } else { "❌ No" });
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
}
