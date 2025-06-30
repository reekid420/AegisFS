//! AegisFS snapshot management CLI tool

use clap::{Parser, Subcommand};
use log::{error, info};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aegisfs::blockdev::FileBackedBlockDevice;
use aegisfs::modules::{SnapshotManager, SnapshotConfig};

#[derive(Parser)]
#[command(author, version, about = "AegisFS snapshot management tool", long_about = None)]
struct Cli {
    /// Device path to operate on
    device: PathBuf,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();

    // Initialize snapshot manager
    let device = Arc::new(
        FileBackedBlockDevice::open(&cli.device, false)  // Open in read-write mode
            .await
            .map_err(|e| format!("Failed to open device: {}", e))?
    );

    let mut manager = SnapshotManager::new(device, SnapshotConfig::default());
    manager.init().await
        .map_err(|e| format!("Failed to initialize snapshot manager: {}", e))?;

    match cli.command {
        Commands::Create { name, description, tags } => {
            info!("Creating snapshot '{}'", name);

            // Parse tags
            let mut tag_map = HashMap::new();
            for tag in tags {
                let parts: Vec<&str> = tag.split('=').collect();
                if parts.len() == 2 {
                    tag_map.insert(parts[0].to_string(), parts[1].to_string());
                } else {
                    error!("Invalid tag format: {}. Use key=value", tag);
                    return Err("Invalid tag format".into());
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

        Commands::List { long } => {
            let snapshots = manager.list_snapshots();
            
            if snapshots.is_empty() {
                println!("No snapshots found");
                return Ok(());
            }

            if long {
                println!("{:<10} {:<20} {:<20} {:<15} {:<10} {:<10}", 
                    "ID", "NAME", "CREATED", "STATE", "BLOCKS", "SPACE");
                println!("{}", "-".repeat(85));

                for snap in snapshots {
                    let created = chrono::DateTime::<chrono::Utc>::from_timestamp(snap.created_at as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "Unknown".to_string());

                    println!("{:<10} {:<20} {:<20} {:<15} {:<10} {:<10}",
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

        Commands::Delete { snapshot } => {
            // Try to parse as ID first
            let snapshot_id = if let Ok(id) = snapshot.parse::<u64>() {
                id
            } else {
                // Look up by name
                if let Some(snap) = manager.get_snapshot_by_name(&snapshot) {
                    snap.id
                } else {
                    error!("Snapshot '{}' not found", snapshot);
                    return Err("Snapshot not found".into());
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

        Commands::Rollback { snapshot, force } => {
            // Try to parse as ID first
            let snapshot_id = if let Ok(id) = snapshot.parse::<u64>() {
                id
            } else {
                // Look up by name
                if let Some(snap) = manager.get_snapshot_by_name(&snapshot) {
                    snap.id
                } else {
                    error!("Snapshot '{}' not found", snapshot);
                    return Err("Snapshot not found".into());
                }
            };

            if !force {
                println!("WARNING: Rolling back to a snapshot will discard all changes made after the snapshot was created.");
                println!("Are you sure you want to rollback to snapshot {}? (yes/no)", snapshot);
                
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

        Commands::Stats => {
            let stats = manager.get_snapshot_stats();
            
            println!("Snapshot Statistics:");
            println!("  Total snapshots:     {}", stats.total_snapshots);
            println!("  Active snapshots:    {}", stats.active_snapshots);
            println!("  Blocks referenced:   {}", stats.total_blocks_referenced);
            println!("  Total space used:    {}", format_bytes(stats.total_space_used));
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