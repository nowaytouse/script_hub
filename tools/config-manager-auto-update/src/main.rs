mod cli;
mod config;
mod interactive;
mod subscription;
mod types;
mod updater;

use clap::Parser;
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Sing-box Configuration and Core Auto-Update Tool - Pure Rust CLI Version
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Interactive menu mode
    #[arg(short, long)]
    interactive: bool,

    /// Config file path
    #[arg(short, long, default_value = "config.json")]
    config: PathBuf,

    /// Execute update once and exit (non-interactive mode)
    #[arg(short, long)]
    once: bool,

    /// Show version information
    #[arg(short, long)]
    version: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Show version information
    if args.version {
        println!("Sing-box Manager v{}", VERSION);
        println!("Pure Rust CLI Version");
        println!("Platform: {}/{}", std::env::consts::OS, std::env::consts::ARCH);
        return;
    }

    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    // Interactive mode
    if args.interactive {
        let cli = crate::interactive::InteractiveCLI::new(args.config);
        if let Err(e) = cli.run().await {
            eprintln!("Program error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Standard CLI mode
    if let Err(e) = cli::run_cli(args.config, args.once).await {
        eprintln!("Program error: {}", e);
        std::process::exit(1);
    }
}

