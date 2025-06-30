//! Format command for creating AegisFS filesystems

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use log::info;
use std::io::Write;
use std::path::PathBuf;

use aegisfs::format;

/// Format a device with AegisFS
#[derive(Parser, Debug)]
#[command(about = "Format a device with AegisFS")]
pub struct FormatArgs {
    /// Device or image file to format
    pub device: PathBuf,

    /// Size of the partition in GB (default: 3)
    #[arg(short, long, default_value_t = 3)]
    pub size: u64,

    /// Force formatting without confirmation
    #[arg(short, long)]
    pub force: bool,
}

pub async fn run(args: FormatArgs) -> Result<()> {
    info!("AegisFS Format Tool v{}", env!("CARGO_PKG_VERSION"));

    // Check if device exists and is writable
    if !args.device.exists() {
        return Err(anyhow!("Device {:?} does not exist", args.device));
    }

    // Get device size - different methods for files vs block devices
    let device_size = if args.device.is_file() {
        // For regular files, use filesystem metadata
        let metadata = std::fs::metadata(&args.device)
            .with_context(|| format!("Failed to get file metadata: {:?}", args.device))?;
        
        info!("Detected regular file: {}", args.device.display());
        metadata.len()
    } else {
        // For block devices, use blockdev command
        info!("Detected block device: {}", args.device.display());
        let device_size_output = std::process::Command::new("blockdev")
            .args(["--getsize64", args.device.to_str().unwrap()])
            .output()
            .with_context(|| {
                format!(
                    "Failed to get device size using blockdev: {:?}",
                    args.device
                )
            })?;

        if !device_size_output.status.success() {
            return Err(anyhow!(
                "Failed to get device size: {}",
                String::from_utf8_lossy(&device_size_output.stderr)
            ));
        }

        String::from_utf8(device_size_output.stdout)
            .with_context(|| "Failed to parse blockdev output")?
            .trim()
            .parse::<u64>()
            .with_context(|| "Failed to parse device size as number")?
    };

    // Convert device size to GiB for display (using floating point for precision)
    let gibibyte = 1024u64 * 1024 * 1024; // 1 GiB in bytes (2^30)
    let device_size_gib = device_size as f64 / gibibyte as f64;

    // Use the exact device size if no size is specified
    let partition_size = if args.size == 0 {
        info!(
            "Using exact device size: {} bytes ({:.2} GiB)",
            device_size, device_size_gib
        );
        device_size
    } else {
        // If size is specified, use it but ensure it doesn't exceed device size
        // Calculate requested size in GiB (2^30 bytes)
        let requested_size = args
            .size
            .checked_mul(gibibyte)
            .ok_or_else(|| anyhow!("Requested size is too large"))?;

        info!(
            "Device size: {} bytes ({:.2} GiB)",
            device_size, device_size_gib
        );
        info!(
            "Requested size: {} bytes ({} GiB)",
            requested_size, args.size
        );

        // Allow a small tolerance (1MB) for devices that are slightly smaller than requested
        const TOLERANCE: u64 = 1024 * 1024; // 1MB tolerance

        if requested_size > device_size {
            if requested_size - device_size <= TOLERANCE {
                info!(
                    "Device is slightly smaller than requested ({} bytes), using full device size",
                    requested_size - device_size
                );
                // Use the full device size since it's within tolerance
                device_size
            } else {
                return Err(anyhow!(
                    "Requested size ({} GiB = {} bytes) exceeds device size ({:.2} GiB = {} bytes)",
                    args.size,
                    requested_size,
                    device_size_gib,
                    device_size
                ));
            }
        } else {
            requested_size
        }
    };

    // Final sanity check (should never fail due to above logic)
    if partition_size > device_size {
        return Err(anyhow!(
            "Internal error: Calculated partition size ({} bytes) exceeds device size ({} bytes)",
            partition_size,
            device_size
        ));
    }

    info!(
        "Formatting with size: {} bytes ({:.2} GiB)",
        partition_size,
        partition_size as f64 / (1024.0 * 1024.0 * 1024.0)
    );

    // Confirm before formatting
    if !args.force {
        println!(
            "WARNING: This will format {} as an AegisFS partition ({} GB).",
            args.device.display(),
            args.size
        );
        println!("This operation will DESTROY ALL DATA on the device!");
        print!("Are you sure you want to continue? [y/N] ");
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Operation cancelled.");
            return Ok(());
        }
    }

    // Format the device
    info!(
        "Formatting {} as AegisFS ({} GB)...",
        args.device.display(),
        args.size
    );

    // Format the device with our filesystem
    format::format_device(&args.device, args.size, Some("AegisFS Volume"))
        .await
        .with_context(|| format!("Failed to format device: {}", args.device.display()))?;

    info!(
        "Successfully formatted {} as AegisFS",
        args.device.display()
    );

    Ok(())
} 