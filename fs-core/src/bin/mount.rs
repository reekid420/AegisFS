//! A simple FUSE-based mount tool for AegisFS

use anyhow::{anyhow, Context};
use clap::Parser;
use fuser::MountOption;
use log::{error, info, warn};

use std::fs::OpenOptions;
use std::io::Read;
use std::path::PathBuf;

#[cfg(feature = "fuse")]
use aegisfs::{format::FormatError, AegisFS};

/// Command-line arguments for the mount tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Device or image file to mount
    source: PathBuf,

    /// Mount point
    mountpoint: PathBuf,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command-line arguments
    let args = Args::parse();

    // Initialize logging
    env_logger::Builder::new()
        .filter_level(if args.debug {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        })
        .init();

    info!("AegisFS v{} starting...", env!("CARGO_PKG_VERSION"));

    // Check if source device exists and is accessible
    let mut source_file = OpenOptions::new()
        .read(true)
        .open(&args.source)
        .with_context(|| format!("Failed to open source device: {}", args.source.display()))?;

    // Check if the device is formatted with AegisFS
    let mut magic = [0u8; 8];
    if let Err(e) = source_file.read_exact(&mut magic) {
        return Err(anyhow!(
            "Failed to read from source device: {}. Is it a valid block device or file?",
            e
        ));
    }

    if &magic != b"AEGISFS\x00" {
        let source = args.source.display();
        return Err(anyhow!(
            "Source device is not formatted with AegisFS.\n\nPlease format it first using:\n    cargo run --bin format -- {} <size_in_gb>\n\nExample for a 3GB filesystem:\n    cargo run --bin format -- {} 3",
            source, source
        ));
    }

    // Check if mountpoint exists and is a directory
    let mountpoint = args
        .mountpoint
        .canonicalize()
        .with_context(|| format!("Failed to access mountpoint: {}", args.mountpoint.display()))?;

    if !mountpoint.is_dir() {
        return Err(anyhow!("Mountpoint must be a directory"));
    }

    info!(
        "Mounting AegisFS from '{}' to '{}'",
        args.source.display(),
        mountpoint.display()
    );

    // Create a new filesystem instance
    let fs = AegisFS::from_device(&args.source).await.with_context(|| {
        format!(
            "Failed to open AegisFS on device: {}",
            args.source.display()
        )
    })?;

    // Prepare mount options
    let options = vec![
        MountOption::FSName("aegisfs".to_string()),
        MountOption::AutoUnmount,
        MountOption::AllowOther,
        MountOption::NoExec,
    ];

    info!("Mounting AegisFS at {:?}", mountpoint);

    // Mount the filesystem in the current thread (blocking)
    info!("Mounting AegisFS at {:?}", mountpoint);

    #[cfg(not(feature = "fuse"))]
    {
        error!("FUSE support not enabled. Rebuild with --features fuse");
        return Err(anyhow!("FUSE support not enabled"));
    }

    #[cfg(feature = "fuse")]
    {
        // Set up signal handler for clean unmount
        ctrlc::set_handler(move || {
            info!("Unmounting filesystem...");
            // The AutoUnmount option should handle the actual unmounting
            std::process::exit(0);
        })
        .context("Failed to set Ctrl+C handler")?;

        info!("Filesystem mounted at {:?}", mountpoint);
        info!("Press Ctrl+C to unmount");

        // This will block until the filesystem is unmounted
        match fuser::mount2(fs, &mountpoint, &options) {
            Ok(_) => {
                info!("Filesystem unmounted successfully");
                Ok(())
            }
            Err(e) => {
                error!("Mount error: {}", e);
                Err(anyhow!("Failed to mount: {}", e))
            }
        }
    }

    // The mount function will block until unmounted
    // The ctrl-c handler will exit the process cleanly
}
