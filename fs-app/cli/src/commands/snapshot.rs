//! Snapshot command for managing AegisFS snapshots

use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{error, info};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aegisfs::blockdev::FileBackedBlockDevice;
use aegisfs::modules::{SnapshotConfig, SnapshotManager};

/// Manage snapshots
#[derive(Parser)]
#[command(about = "Manage AegisFS snapshots")]
pub struct SnapshotArgs {
    /// Device path to operate on
    pub device: PathBuf,

    #[command(subcommand)]
    pub command: SnapshotCommands,
}

#[derive(Subcommand)]
pub enum SnapshotCommands {
    /// Create a new snapshot
    Create {
        /// Name for the snapshot
        name: String,

        /// Optional description/tag
        #[arg(short, long)]
        description: Option<String>,

        /// Additional tags in key=value format
        #[arg(short = 't', long = "tag")]
        tags: Vec<String>,
    },

    /// List all snapshots
    List {
        /// Show detailed information
        #[arg(short, long)]
        long: bool,
    },

    /// Delete a snapshot
    Delete {
        /// Name or ID of the snapshot to delete
        snapshot: String,
    },

    /// Rollback to a snapshot
    Rollback {
        /// Name or ID of the snapshot to rollback to
        snapshot: String,

        /// Force rollback without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Show snapshot statistics
    Stats,
}

pub async fn run(args: SnapshotArgs) -> Result<()> {
    // Initialize snapshot manager
    let device = Arc::new(
        FileBackedBlockDevice::open(&args.device, false) // Open in read-write mode
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open device: {}", e))?,
    );

    let mut manager = SnapshotManager::new(device, SnapshotConfig::default());
    manager
        .init()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize snapshot manager: {}", e))?;

    match args.command {
        SnapshotCommands::Create {
            name,
            description,
            tags,
        } => {
            info!("Creating snapshot '{}'", name);

            // Parse tags
            let mut tag_map = HashMap::new();
            for tag in tags {
                let parts: Vec<&str> = tag.split('=').collect();
                if parts.len() == 2 {
                    tag_map.insert(parts[0].to_string(), parts[1].to_string());
                } else {
                    error!("Invalid tag format: {}. Use key=value", tag);
                    return Err(anyhow::anyhow!("Invalid tag format"));
                }
            }

            if let Some(desc) = description {
                tag_map.insert("description".to_string(), desc);
            }

            match manager.create_snapshot(&name, tag_map).await {
                Ok(id) => {
                    info!("Successfully created snapshot '{}' with ID {}", name, id);
                }
                Err(e) => {
                    error!("Failed to create snapshot: {}", e);
                    return Err(e.into());
                }
            }
        }

        SnapshotCommands::List { long } => {
            let snapshots = manager.list_snapshots();

            if snapshots.is_empty() {
                println!("No snapshots found");
                return Ok(());
            }

            if long {
                println!(
                    "{:<10} {:<20} {:<20} {:<15} {:<10} {:<10}",
                    "ID", "NAME", "CREATED", "STATE", "BLOCKS", "SPACE"
                );
                println!("{}", "-".repeat(85));

                for snap in snapshots {
                    let created =
                        chrono::DateTime::<chrono::Utc>::from_timestamp(snap.created_at as i64, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Unknown".to_string());

                    println!(
                        "{:<10} {:<20} {:<20} {:<15} {:<10} {:<10}",
                        snap.id,
                        snap.name,
                        created,
                        format!("{:?}", snap.state),
                        snap.block_count,
                        format_bytes(snap.exclusive_space)
                    );
                }
            } else {
                println!("Snapshots:");
                for snap in snapshots {
                    println!("  {} - {}", snap.id, snap.name);
                }
            }
        }

        SnapshotCommands::Delete { snapshot } => {
            // Try to parse as ID first
            let snapshot_id = if let Ok(id) = snapshot.parse::<u64>() {
                id
            } else {
                // Look up by name
                if let Some(snap) = manager.get_snapshot_by_name(&snapshot) {
                    snap.id
                } else {
                    error!("Snapshot '{}' not found", snapshot);
                    return Err(anyhow::anyhow!("Snapshot not found"));
                }
            };

            info!("Deleting snapshot ID {}", snapshot_id);
            match manager.delete_snapshot(snapshot_id).await {
                Ok(_) => {
                    info!("Successfully deleted snapshot");
                }
                Err(e) => {
                    error!("Failed to delete snapshot: {}", e);
                    return Err(e.into());
                }
            }
        }

        SnapshotCommands::Rollback { snapshot, force } => {
            // Try to parse as ID first
            let snapshot_id = if let Ok(id) = snapshot.parse::<u64>() {
                id
            } else {
                // Look up by name
                if let Some(snap) = manager.get_snapshot_by_name(&snapshot) {
                    snap.id
                } else {
                    error!("Snapshot '{}' not found", snapshot);
                    return Err(anyhow::anyhow!("Snapshot not found"));
                }
            };

            if !force {
                println!("WARNING: Rolling back to a snapshot will discard all changes made after the snapshot was created.");
                println!(
                    "Are you sure you want to rollback to snapshot {}? (yes/no)",
                    snapshot
                );

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;

                if !input.trim().eq_ignore_ascii_case("yes") {
                    println!("Rollback cancelled");
                    return Ok(());
                }
            }

            info!("Rolling back to snapshot ID {}", snapshot_id);
            match manager.rollback_to_snapshot(snapshot_id).await {
                Ok(_) => {
                    info!("Successfully rolled back to snapshot");
                }
                Err(e) => {
                    error!("Failed to rollback: {}", e);
                    return Err(e.into());
                }
            }
        }

        SnapshotCommands::Stats => {
            let stats = manager.get_snapshot_stats();

            println!("Snapshot Statistics:");
            println!("  Total snapshots:     {}", stats.total_snapshots);
            println!("  Active snapshots:    {}", stats.active_snapshots);
            println!("  Blocks referenced:   {}", stats.total_blocks_referenced);
            println!(
                "  Total space used:    {}",
                format_bytes(stats.total_space_used)
            );
            println!("  Pending CoW ops:     {}", stats.cow_operations_pending);
        }
    }

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
} 