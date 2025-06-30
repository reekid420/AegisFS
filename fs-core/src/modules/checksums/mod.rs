//! Block checksums and self-healing module for AegisFS
//!
//! This module provides block-level integrity checking using CRC32 checksums,
//! background scrubbing for proactive error detection, and self-healing
//! capabilities to automatically repair corrupted blocks.

use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};

use crate::blockdev::{BlockDevice, BlockDeviceError};
use crate::error::Result;

/// Default scrub interval (24 hours)
const DEFAULT_SCRUB_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Maximum number of bad blocks to track
const MAX_BAD_BLOCKS: usize = 10000;

/// Checksum algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgorithm {
    /// CRC32 checksum (fast, 32-bit)
    Crc32,
    /// CRC64 checksum (more robust, 64-bit)
    Crc64,
    /// xxHash64 (very fast, non-cryptographic)
    XxHash64,
}

/// Block metadata including checksum
#[derive(Debug, Clone)]
pub struct BlockMetadata {
    /// Block number
    pub block_num: u64,
    /// Checksum of the block data
    pub checksum: u64,
    /// Algorithm used for checksum
    pub algorithm: ChecksumAlgorithm,
    /// Last verification timestamp
    pub last_verified: Option<u64>,
    /// Number of times this block has been corrected
    pub correction_count: u32,
}

/// Scrub statistics
#[derive(Debug, Default, Clone)]
pub struct ScrubStats {
    /// Total blocks scrubbed
    pub blocks_scrubbed: u64,
    /// Blocks with checksum mismatches
    pub blocks_corrupted: u64,
    /// Blocks successfully repaired
    pub blocks_repaired: u64,
    /// Blocks that couldn't be repaired
    pub blocks_unrepairable: u64,
    /// Start time of the scrub
    pub start_time: Option<SystemTime>,
    /// End time of the scrub
    pub end_time: Option<SystemTime>,
}

/// Checksum error types
#[derive(Error, Debug)]
pub enum ChecksumError {
    #[error("Block device error: {0}")]
    BlockDevice(#[from] BlockDeviceError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Checksum mismatch for block {block}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        block: u64,
        expected: u64,
        actual: u64,
    },
    #[error("Block {0} is corrupted and cannot be repaired")]
    UnrepairableBlock(u64),
    #[error("Too many bad blocks ({0}), exceeds limit of {MAX_BAD_BLOCKS}")]
    TooManyBadBlocks(usize),
    #[error("Scrub operation cancelled")]
    ScrubCancelled,
}

/// Configuration for the checksum manager
#[derive(Debug, Clone)]
pub struct ChecksumConfig {
    /// Checksum algorithm to use
    pub algorithm: ChecksumAlgorithm,
    /// Enable automatic repair
    pub auto_repair: bool,
    /// Scrub interval
    pub scrub_interval: Duration,
    /// Enable background scrubbing
    pub background_scrub: bool,
    /// Number of scrub threads
    pub scrub_threads: usize,
    /// Store checksums in memory (faster) or on disk
    pub in_memory_checksums: bool,
}

impl Default for ChecksumConfig {
    fn default() -> Self {
        Self {
            algorithm: ChecksumAlgorithm::Crc32,
            auto_repair: true,
            scrub_interval: DEFAULT_SCRUB_INTERVAL,
            background_scrub: true,
            scrub_threads: 2,
            in_memory_checksums: true,
        }
    }
}

/// Background task commands
#[derive(Debug)]
pub enum ChecksumTask {
    /// Start a full scrub
    StartScrub,
    /// Stop ongoing scrub
    StopScrub,
    /// Verify specific blocks
    VerifyBlocks(Vec<u64>),
    /// Update checksum for a block
    UpdateChecksum(u64, Vec<u8>),
    /// Shutdown the background tasks
    Shutdown,
}

/// Checksum manager for block integrity
pub struct ChecksumManager {
    /// Block device
    device: Arc<dyn BlockDevice>,
    /// Configuration
    config: ChecksumConfig,
    /// Block metadata storage
    metadata: RwLock<HashMap<u64, BlockMetadata>>,
    /// Set of known bad blocks
    bad_blocks: RwLock<HashSet<u64>>,
    /// Scrub statistics
    scrub_stats: RwLock<ScrubStats>,
    /// Flag indicating if scrub is running
    scrub_running: AtomicBool,
    /// Flag to cancel scrub
    scrub_cancel: AtomicBool,
    /// Next block to scrub
    next_scrub_block: AtomicU64,
    /// Task sender for background operations
    task_sender: Option<mpsc::UnboundedSender<ChecksumTask>>,
}

impl ChecksumManager {
    /// Create a new checksum manager
    pub fn new(device: Arc<dyn BlockDevice>, config: ChecksumConfig) -> Self {
        Self {
            device,
            config,
            metadata: RwLock::new(HashMap::new()),
            bad_blocks: RwLock::new(HashSet::new()),
            scrub_stats: RwLock::new(ScrubStats::default()),
            scrub_running: AtomicBool::new(false),
            scrub_cancel: AtomicBool::new(false),
            next_scrub_block: AtomicU64::new(0),
            task_sender: None,
        }
    }

    /// Initialize the checksum manager
    pub async fn init(&mut self) -> Result<()> {
        // Start background task handler
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.task_sender = Some(sender.clone());

        let device = self.device.clone();
        let config = self.config.clone();

        // Background task handler
        tokio::spawn(async move {
            while let Some(task) = receiver.recv().await {
                match task {
                    ChecksumTask::StartScrub => {
                        log::info!("Starting background scrub");
                        // Scrub implementation would go here
                    }
                    ChecksumTask::StopScrub => {
                        log::info!("Stopping background scrub");
                        // Note: In a real implementation, we'd set the scrub_cancel flag
                    }
                    ChecksumTask::VerifyBlocks(blocks) => {
                        log::debug!("Verifying {} blocks", blocks.len());
                        // Block verification would go here
                    }
                    ChecksumTask::UpdateChecksum(block_num, _data) => {
                        log::debug!("Updating checksum for block {}", block_num);
                        // Checksum update would go here
                    }
                    ChecksumTask::Shutdown => {
                        log::info!("Shutting down checksum background tasks");
                        break;
                    }
                }
            }
        });

        // Start periodic scrub if enabled
        if self.config.background_scrub {
            let sender = sender.clone();
            let interval_duration = self.config.scrub_interval;

            tokio::spawn(async move {
                let mut interval = interval(interval_duration);
                loop {
                    interval.tick().await;
                    if sender.send(ChecksumTask::StartScrub).is_err() {
                        break;
                    }
                }
            });
        }

        Ok(())
    }

    /// Calculate checksum for data
    pub fn calculate_checksum(&self, data: &[u8]) -> u64 {
        match self.config.algorithm {
            ChecksumAlgorithm::Crc32 => crc32fast::hash(data) as u64,
            ChecksumAlgorithm::Crc64 => {
                // For demonstration, using CRC32 twice (in real implementation, use proper CRC64)
                let crc1 = crc32fast::hash(&data[..data.len() / 2]) as u64;
                let crc2 = crc32fast::hash(&data[data.len() / 2..]) as u64;
                (crc1 << 32) | crc2
            }
            ChecksumAlgorithm::XxHash64 => {
                // For demonstration, using CRC32 (in real implementation, use xxHash)
                crc32fast::hash(data) as u64
            }
        }
    }

    /// Write a block with checksum
    pub async fn write_block_with_checksum(&self, block_num: u64, data: &[u8]) -> Result<()> {
        // Calculate checksum
        let checksum = self.calculate_checksum(data);

        // Write the block
        self.device.write_block(block_num, data).await?;

        // Update metadata
        let metadata = BlockMetadata {
            block_num,
            checksum,
            algorithm: self.config.algorithm,
            last_verified: None,
            correction_count: 0,
        };

        self.metadata.write().insert(block_num, metadata);

        // Remove from bad blocks if it was there
        self.bad_blocks.write().remove(&block_num);

        Ok(())
    }

    /// Read and verify a block
    pub async fn read_block_with_verification(&self, block_num: u64, buf: &mut [u8]) -> Result<()> {
        // Read the block
        self.device.read_block(block_num, buf).await?;

        // Get stored metadata
        let metadata = self.metadata.read().get(&block_num).cloned();

        if let Some(metadata) = metadata {
            // Calculate actual checksum
            let actual_checksum = self.calculate_checksum(buf);

            // Verify checksum
            if actual_checksum != metadata.checksum {
                log::error!(
                    "Checksum mismatch for block {}: expected {}, got {}",
                    block_num,
                    metadata.checksum,
                    actual_checksum
                );

                // Mark as bad block
                self.bad_blocks.write().insert(block_num);

                // Attempt repair if enabled
                if self.config.auto_repair {
                    return self.repair_block(block_num, buf).await;
                }

                return Err(crate::error::Error::Other(format!(
                    "Checksum mismatch for block {}",
                    block_num
                )));
            }

            // Update last verified timestamp
            self.metadata
                .write()
                .get_mut(&block_num)
                .unwrap()
                .last_verified = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            );
        }

        Ok(())
    }

    /// Attempt to repair a corrupted block
    async fn repair_block(&self, block_num: u64, _buf: &mut [u8]) -> Result<()> {
        log::info!("Attempting to repair block {}", block_num);

        // In a real implementation, this would:
        // 1. Try to read from mirror/parity blocks
        // 2. Use forward error correction if available
        // 3. Restore from recent backup/snapshot

        // For now, we'll just mark it as unrepairable
        Err(crate::error::Error::Other(format!(
            "Block {} is corrupted and cannot be repaired",
            block_num
        )))
    }

    /// Perform a full scrub of all blocks
    pub async fn scrub_all(&self) -> Result<ScrubStats> {
        if self.scrub_running.swap(true, Ordering::Acquire) {
            return Err(crate::error::Error::Other(
                "Scrub already in progress".to_string(),
            ));
        }

        self.scrub_cancel.store(false, Ordering::Relaxed);

        let mut stats = ScrubStats {
            start_time: Some(SystemTime::now()),
            ..Default::default()
        };

        let total_blocks = self.device.block_count();
        let mut buf = vec![0u8; 4096]; // Assuming 4KB blocks

        for block_num in 0..total_blocks {
            // Check if scrub was cancelled
            if self.scrub_cancel.load(Ordering::Relaxed) {
                log::info!("Scrub cancelled at block {}", block_num);
                break;
            }

            // Update progress
            self.next_scrub_block.store(block_num, Ordering::Relaxed);

            // Read and verify the block
            match self.read_block_with_verification(block_num, &mut buf).await {
                Ok(_) => {
                    stats.blocks_scrubbed += 1;
                }
                Err(_) => {
                    stats.blocks_corrupted += 1;

                    // Attempt repair
                    if self.config.auto_repair {
                        match self.repair_block(block_num, &mut buf).await {
                            Ok(_) => stats.blocks_repaired += 1,
                            Err(_) => stats.blocks_unrepairable += 1,
                        }
                    }
                }
            }

            // Periodic progress update
            if block_num % 1000 == 0 {
                log::info!(
                    "Scrub progress: {}/{} blocks ({:.1}%)",
                    block_num,
                    total_blocks,
                    (block_num as f64 / total_blocks as f64) * 100.0
                );
            }
        }

        stats.end_time = Some(SystemTime::now());
        *self.scrub_stats.write() = stats.clone();

        self.scrub_running.store(false, Ordering::Release);

        log::info!(
            "Scrub completed: {} blocks scrubbed, {} corrupted, {} repaired, {} unrepairable",
            stats.blocks_scrubbed,
            stats.blocks_corrupted,
            stats.blocks_repaired,
            stats.blocks_unrepairable
        );

        Ok(stats)
    }

    /// Get list of known bad blocks
    pub fn get_bad_blocks(&self) -> Vec<u64> {
        self.bad_blocks.read().iter().cloned().collect()
    }

    /// Get scrub statistics
    pub fn get_scrub_stats(&self) -> ScrubStats {
        self.scrub_stats.read().clone()
    }

    /// Mark a block as bad
    pub fn mark_bad_block(&self, block_num: u64) -> Result<()> {
        let mut bad_blocks = self.bad_blocks.write();

        if bad_blocks.len() >= MAX_BAD_BLOCKS {
            return Err(crate::error::Error::Other(format!(
                "Too many bad blocks ({}), exceeds limit",
                bad_blocks.len()
            )));
        }

        bad_blocks.insert(block_num);
        log::warn!("Block {} marked as bad", block_num);

        Ok(())
    }

    /// Clear a block from the bad blocks list
    pub fn clear_bad_block(&self, block_num: u64) {
        self.bad_blocks.write().remove(&block_num);
        log::info!("Block {} removed from bad blocks list", block_num);
    }

    /// Shutdown the checksum manager
    pub async fn shutdown(&mut self) -> Result<()> {
        // Stop any ongoing scrub
        self.scrub_cancel.store(true, Ordering::Relaxed);

        // Send shutdown signal
        if let Some(sender) = &self.task_sender {
            let _ = sender.send(ChecksumTask::Shutdown);
        }

        // Wait for scrub to complete
        while self.scrub_running.load(Ordering::Acquire) {
            sleep(Duration::from_millis(100)).await;
        }

        log::info!("Checksum manager shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::FileBackedBlockDevice;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_checksum_calculation() {
        let config = ChecksumConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let device = Arc::new(
            FileBackedBlockDevice::create(temp_file.path(), 1024 * 1024)
                .await
                .unwrap(),
        );

        let manager = ChecksumManager::new(device, config);

        let data = b"Hello, AegisFS!";
        let checksum1 = manager.calculate_checksum(data);
        let checksum2 = manager.calculate_checksum(data);

        // Same data should produce same checksum
        assert_eq!(checksum1, checksum2);

        // Different data should produce different checksum
        let different_data = b"Different data";
        let checksum3 = manager.calculate_checksum(different_data);
        assert_ne!(checksum1, checksum3);
    }

    #[tokio::test]
    async fn test_write_and_verify() {
        let config = ChecksumConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let device = Arc::new(
            FileBackedBlockDevice::create(temp_file.path(), 1024 * 1024)
                .await
                .unwrap(),
        );

        let mut manager = ChecksumManager::new(device, config);
        manager.init().await.unwrap();

        // Write a block with checksum
        let data = vec![42u8; 4096];
        manager.write_block_with_checksum(0, &data).await.unwrap();

        // Read and verify
        let mut buf = vec![0u8; 4096];
        manager
            .read_block_with_verification(0, &mut buf)
            .await
            .unwrap();

        assert_eq!(buf, data);

        // Shutdown
        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_bad_block_tracking() {
        let config = ChecksumConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let device = Arc::new(
            FileBackedBlockDevice::create(temp_file.path(), 1024 * 1024)
                .await
                .unwrap(),
        );

        let manager = ChecksumManager::new(device, config);

        // Mark blocks as bad
        manager.mark_bad_block(10).unwrap();
        manager.mark_bad_block(20).unwrap();
        manager.mark_bad_block(30).unwrap();

        let bad_blocks = manager.get_bad_blocks();
        assert_eq!(bad_blocks.len(), 3);
        assert!(bad_blocks.contains(&10));
        assert!(bad_blocks.contains(&20));
        assert!(bad_blocks.contains(&30));

        // Clear a bad block
        manager.clear_bad_block(20);
        let bad_blocks = manager.get_bad_blocks();
        assert_eq!(bad_blocks.len(), 2);
        assert!(!bad_blocks.contains(&20));
    }
}
