//! Mount command for mounting AegisFS filesystems via FUSE

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use fuser::MountOption;
use log::{error, info, warn};

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;

#[cfg(feature = "fuse")]
use aegisfs::AegisFS;

#[cfg(not(feature = "fuse"))]
compile_error!("FUSE feature is required for the mount command. Use --features fuse");

/// Check if a device is already mounted by reading /proc/mounts
fn is_device_mounted(device_path: &PathBuf) -> Result<Option<String>> {
    let mounts_file = File::open("/proc/mounts")
        .context("Failed to open /proc/mounts. Are you running on Linux?")?;
    
    let reader = BufReader::new(mounts_file);
    
    // Get the canonical path of the device to handle symlinks
    let canonical_device = device_path.canonicalize()
        .unwrap_or_else(|_| device_path.clone());
    
    info!("Checking for device mount: {} (canonical: {})", 
          device_path.display(), canonical_device.display());
    
    for line in reader.lines() {
        let line = line.context("Failed to read line from /proc/mounts")?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() >= 3 {
            let mounted_device = PathBuf::from(parts[0]);
            let mount_point = parts[1];
            let fs_type = parts[2];
            
            // For non-FUSE filesystems, check device path directly
            if fs_type != "fuse" {
                let mounted_canonical = mounted_device.canonicalize()
                    .unwrap_or_else(|_| mounted_device.clone());
                
                if mounted_device == *device_path || 
                   mounted_device == canonical_device ||
                   mounted_canonical == canonical_device {
                    info!("Found mounted device: {} at {}", mounted_device.display(), mount_point);
                    return Ok(Some(mount_point.to_string()));
                }
            } else if mounted_device.to_string_lossy() == "aegisfs" {
                // For FUSE AegisFS, we need a different approach
                // Check if we can create an exclusive lock on the device file
                if is_device_locked_by_another_process(device_path)? {
                    info!("Device {} appears to be in use by another AegisFS process (mounted at {})", 
                          device_path.display(), mount_point);
                    return Ok(Some(mount_point.to_string()));
                }
            }
        }
    }
    
    Ok(None)
}

/// Check if a device is locked by another process using lsof
fn is_device_locked_by_another_process(device_path: &PathBuf) -> Result<bool> {
    use std::process::Command;
    
    // Use lsof to check if any process is using this device
    let output = Command::new("lsof")
        .arg(device_path)
        .output();
        
    match output {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            info!("lsof output for {}: {}", device_path.display(), stdout.trim());
            // If lsof shows any output (besides header), the device is in use
            let lines: Vec<&str> = stdout.trim().lines().collect();
            Ok(lines.len() > 1) // More than just the header line
        },
        Err(e) => {
            info!("lsof command failed: {}, assuming device not in use", e);
            Ok(false) // lsof not available or failed, assume not in use
        }
    }
}

/// Check if a mountpoint is already in use
fn is_mountpoint_in_use(mountpoint: &PathBuf) -> Result<bool> {
    let mounts_file = File::open("/proc/mounts")
        .context("Failed to open /proc/mounts")?;
    
    let reader = BufReader::new(mounts_file);
    let canonical_mountpoint = mountpoint.canonicalize()
        .unwrap_or_else(|_| mountpoint.clone());
    
    for line in reader.lines() {
        let line = line.context("Failed to read line from /proc/mounts")?;
        let parts: Vec<&str> = line.split_whitespace().collect();
        
        if parts.len() >= 2 {
            let mount_point = PathBuf::from(parts[1]);
            let canonical_mount = mount_point.canonicalize()
                .unwrap_or(mount_point);
            
            if canonical_mount == canonical_mountpoint {
                return Ok(true);
            }
        }
    }
    
    Ok(false)
}

/// Mount an AegisFS filesystem
#[derive(Parser, Debug)]
#[command(about = "Mount an AegisFS filesystem via FUSE")]
pub struct MountArgs {
    /// Device or image file to mount
    pub source: PathBuf,

    /// Mount point
    pub mountpoint: PathBuf,
}

pub async fn run(args: MountArgs) -> Result<()> {
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
            "Source device is not formatted with AegisFS.\n\nPlease format it first using:\n    aegisfs format {} <size_in_gb>\n\nExample for a 3GB filesystem:\n    aegisfs format {} 3",
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

    // Check if the device is already mounted
    if let Some(existing_mount) = is_device_mounted(&args.source)? {
        return Err(anyhow!(
            "Device '{}' is already mounted at '{}'.\n\nTo unmount it first, use:\n    fusermount -u '{}'",
            args.source.display(),
            existing_mount,
            existing_mount
        ));
    }

    // Check if the mountpoint is already in use
    if is_mountpoint_in_use(&mountpoint)? {
        return Err(anyhow!(
            "Mountpoint '{}' is already in use by another filesystem.\n\nCheck current mounts with:\n    mount | grep '{}'",
            mountpoint.display(),
            mountpoint.display()
        ));
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
} 