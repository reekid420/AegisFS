//! AegisFS Command Line Interface
//! 
//! Unified CLI tool for managing AegisFS filesystems.

use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{info, LevelFilter};

mod commands;

/// AegisFS - Advanced Filesystem with Encryption, Snapshots, and Data Integrity
#[derive(Parser)]
#[command(
    name = "aegisfs",
    about = "AegisFS command-line interface",
    version = env!("CARGO_PKG_VERSION"),
    author = "AegisFS Contributors"
)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Enable debug output
    #[arg(short, long, global = true)]
    debug: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Format a device with AegisFS
    Format(commands::format::FormatArgs),
    
    /// Mount an AegisFS filesystem
    Mount(commands::mount::MountArgs),
    
    /// Manage snapshots
    Snapshot(commands::snapshot::SnapshotArgs),
    
    /// Check and repair filesystem integrity
    Scrub(commands::scrub::ScrubArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity level
    let log_level = if cli.debug {
        LevelFilter::Debug
    } else if cli.verbose {
        LevelFilter::Info
    } else {
        LevelFilter::Warn
    };

    env_logger::Builder::new()
        .filter_level(log_level)
        .init();

    info!("AegisFS CLI v{} starting...", env!("CARGO_PKG_VERSION"));

    // Execute the appropriate command
    match cli.command {
        Commands::Format(args) => commands::format::run(args).await,
        Commands::Mount(args) => commands::mount::run(args).await,
        Commands::Snapshot(args) => commands::snapshot::run(args).await,
        Commands::Scrub(args) => commands::scrub::run(args).await,
    }
} 