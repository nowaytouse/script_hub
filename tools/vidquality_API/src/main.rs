use clap::{Parser, Subcommand, ValueEnum};
use tracing::{info, error};
use std::path::PathBuf;
use vidquality::{detect_video, simple_convert, smart_convert, determine_strategy, ConversionConfig};

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

    /// Convert video with intelligent strategy selection
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
    },

    /// Simple mode: Lossless→FFV1, Others→AV1
    Simple {
        /// Input video file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output directory
        #[arg(short, long)]
        output: Option<PathBuf>,
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

        Commands::Auto { input, output, force, delete_original } => {
            let config = ConversionConfig {
                output_dir: output,
                simple_mode: false,
                force,
                delete_original,
                preserve_metadata: true,
            };
            
            let result = smart_convert(&input, &config)?;
            
            info!("");
            info!("📊 Conversion Summary:");
            info!("   Input:  {} ({} bytes)", result.input_path, result.input_size);
            info!("   Output: {} ({} bytes)", result.output_path, result.output_size);
            info!("   Ratio:  {:.1}%", result.size_ratio * 100.0);
        }

        Commands::Simple { input, output } => {
            info!("🎬 Simple Mode Conversion");
            info!("   Lossless sources → FFV1 MKV (archival)");
            info!("   Lossy sources → AV1 MP4 (high quality)");
            info!("");
            
            let result = simple_convert(&input, output.as_deref())?;
            
            info!("");
            info!("✅ Complete!");
            info!("   Output: {}", result.output_path);
        }

        Commands::Strategy { input } => {
            let detection = detect_video(&input)?;
            let strategy = determine_strategy(&detection);
            
            println!("\n🎯 Recommended Strategy");
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📁 File: {}", input.display());
            println!("🎬 Codec: {} ({})", detection.codec.as_str(), detection.compression.as_str());
            println!("");
            println!("💡 Target: {}", strategy.target.as_str());
            println!("📝 Reason: {}", strategy.reason);
            println!("");
            println!("⚙️  Command:");
            println!("   {}", strategy.command);
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
