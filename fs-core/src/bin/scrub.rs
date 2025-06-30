//! AegisFS scrub CLI tool for filesystem verification and repair

use clap::Parser;
use log::{error, info, warn};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use aegisfs::blockdev::FileBackedBlockDevice;
use aegisfs::modules::{ChecksumConfig, ChecksumManager};

#[derive(Parser)]
#[command(author, version, about = "AegisFS filesystem scrub tool", long_about = None)]
struct Cli {
    /// Device path to scrub
    device: PathBuf,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Only verify, don't attempt repairs
    #[arg(short = 'n', long = "dry-run")]
    dry_run: bool,

    /// Force scrub even if one is already running
    #[arg(short, long)]
    force: bool,

    /// Number of threads to use for scrubbing
    #[arg(short = 't', long = "threads", default_value = "2")]
    threads: usize,

    /// Show statistics only, don't perform scrub
    #[arg(short = 's', long = "stats")]
    stats_only: bool,

    /// Stop an ongoing scrub
    #[arg(long = "stop")]
    stop: bool,

    /// Clear the bad blocks list
    #[arg(long = "clear-bad-blocks")]
    clear_bad_blocks: bool,

    /// List known bad blocks
    #[arg(short = 'l', long = "list-bad-blocks")]
    list_bad_blocks: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();

    // Open device
    let device = Arc::new(
        FileBackedBlockDevice::open(&cli.device, cli.dry_run) // Read-only if dry-run
            .await
            .map_err(|e| format!("Failed to open device: {}", e))?,
    );

    // Configure checksum manager
    let mut config = ChecksumConfig::default();
    config.auto_repair = !cli.dry_run;
    config.scrub_threads = cli.threads;

    let mut manager = ChecksumManager::new(device, config);
    manager
        .init()
        .await
        .map_err(|e| format!("Failed to initialize checksum manager: {}", e))?;

    // Handle various operations
    if cli.stop {
        info!("Stopping ongoing scrub...");
        manager.shutdown().await?;
        info!("Scrub stopped");
        return Ok(());
    }

    if cli.clear_bad_blocks {
        let bad_blocks = manager.get_bad_blocks();
        if bad_blocks.is_empty() {
            println!("No bad blocks to clear");
        } else {
            println!("Clearing {} bad blocks...", bad_blocks.len());
            for block in bad_blocks {
                manager.clear_bad_block(block);
            }
            println!("Bad blocks list cleared");
        }
        return Ok(());
    }

    if cli.list_bad_blocks {
        let bad_blocks = manager.get_bad_blocks();
        if bad_blocks.is_empty() {
            println!("No bad blocks found");
        } else {
            println!("Bad blocks ({} total):", bad_blocks.len());
            for block in bad_blocks {
                println!("  Block {}", block);
            }
        }
        return Ok(());
    }

    if cli.stats_only {
        let stats = manager.get_scrub_stats();
        print_stats(&stats);
        return Ok(());
    }

    // Perform the scrub
    info!("Starting filesystem scrub on {:?}", cli.device);
    if cli.dry_run {
        info!("Running in dry-run mode - no repairs will be attempted");
    }

    let start_time = Instant::now();

    match manager.scrub_all().await {
        Ok(stats) => {
            let duration = start_time.elapsed();

            info!("Scrub completed in {:.2} seconds", duration.as_secs_f64());
            print_stats(&stats);

            // Check results
            if stats.blocks_corrupted == 0 {
                println!("\n✓ Filesystem is healthy - no errors found");
            } else if stats.blocks_unrepairable > 0 {
                error!(
                    "\n✗ Filesystem has {} unrepairable blocks!",
                    stats.blocks_unrepairable
                );
                println!("  Data loss may have occurred. Consider restoring from backup.");
                std::process::exit(1);
            } else if stats.blocks_corrupted > stats.blocks_repaired {
                warn!("\n⚠ Filesystem has errors that were not fully repaired");
                println!(
                    "  {} blocks corrupted, {} repaired",
                    stats.blocks_corrupted, stats.blocks_repaired
                );
                std::process::exit(2);
            } else {
                info!("\n✓ All errors were successfully repaired");
            }
        }
        Err(e) => {
            error!("Scrub failed: {}", e);
            std::process::exit(3);
        }
    }

    // Shutdown
    manager.shutdown().await?;
    Ok(())
}

fn print_stats(stats: &aegisfs::modules::ScrubStats) {
    println!("\nScrub Statistics:");
    println!("  Blocks scrubbed:     {}", stats.blocks_scrubbed);
    println!("  Blocks corrupted:    {}", stats.blocks_corrupted);
    println!("  Blocks repaired:     {}", stats.blocks_repaired);
    println!("  Blocks unrepairable: {}", stats.blocks_unrepairable);

    if let (Some(start), Some(end)) = (stats.start_time, stats.end_time) {
        let duration = end.duration_since(start).unwrap_or_default();
        println!(
            "  Duration:            {:.2} seconds",
            duration.as_secs_f64()
        );

        if stats.blocks_scrubbed > 0 && duration.as_secs() > 0 {
            let blocks_per_sec = stats.blocks_scrubbed as f64 / duration.as_secs_f64();
            let mb_per_sec = (blocks_per_sec * 4096.0) / (1024.0 * 1024.0); // Assuming 4KB blocks
            println!(
                "  Throughput:          {:.2} MB/s ({:.0} blocks/s)",
                mb_per_sec, blocks_per_sec
            );
        }
    }

    if stats.blocks_corrupted > 0 {
        let error_rate = (stats.blocks_corrupted as f64 / stats.blocks_scrubbed as f64) * 100.0;
        println!("  Error rate:          {:.4}%", error_rate);
    }
}
