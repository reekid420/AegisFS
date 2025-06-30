//! Snapshot module for AegisFS
//!
//! This module provides point-in-time snapshot functionality using Copy-on-Write (CoW)
//! for metadata and data blocks. Snapshots are space-efficient and instantaneous.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::blockdev::{BlockDevice, BlockDeviceError};
use crate::error::Result;
use crate::format::Inode;

/// Maximum number of snapshots supported
const MAX_SNAPSHOTS: usize = 256;

/// Snapshot metadata version
const SNAPSHOT_VERSION: u32 = 1;

/// Snapshot state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotState {
    /// Snapshot is being created
    Creating,
    /// Snapshot is active and can be used
    Active,
    /// Snapshot is marked for deletion
    Deleting,
    /// Snapshot has been deleted
    Deleted,
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Unique snapshot ID
    pub id: u64,
    /// Snapshot name
    pub name: String,
    /// Parent snapshot ID (0 for root)
    pub parent_id: u64,
    /// Creation timestamp
    pub created_at: u64,
    /// Snapshot state
    pub state: SnapshotState,
    /// Root inode number at snapshot time
    pub root_inode: u64,
    /// Number of blocks referenced
    pub block_count: u64,
    /// Space used by this snapshot (excluding shared blocks)
    pub exclusive_space: u64,
    /// User-defined tags
    pub tags: HashMap<String, String>,
}

/// Block reference tracking for CoW
#[derive(Debug, Clone)]
pub struct BlockReference {
    /// Block number
    pub block_num: u64,
    /// Reference count (how many snapshots reference this block)
    pub ref_count: u32,
    /// Snapshots that reference this block
    pub snapshots: HashSet<u64>,
}

/// Copy-on-Write operation
#[derive(Debug)]
pub struct CowOperation {
    /// Original block number
    pub original_block: u64,
    /// New block number (allocated for CoW)
    pub new_block: u64,
    /// Snapshot ID that triggered the CoW
    pub snapshot_id: u64,
    /// Timestamp of the operation
    pub timestamp: u64,
}

/// Snapshot error types
#[derive(Error, Debug)]
pub enum SnapshotError {
    #[error("Block device error: {0}")]
    BlockDevice(#[from] BlockDeviceError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Snapshot not found: {0}")]
    SnapshotNotFound(u64),
    #[error("Snapshot name already exists: {0}")]
    SnapshotNameExists(String),
    #[error("Maximum number of snapshots ({MAX_SNAPSHOTS}) reached")]
    TooManySnapshots,
    #[error("Snapshot is not in valid state for operation: {0:?}")]
    InvalidSnapshotState(SnapshotState),
    #[error("Block {0} is already referenced by snapshot {1}")]
    BlockAlreadyReferenced(u64, u64),
    #[error("Cannot delete snapshot with children")]
    SnapshotHasChildren,
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Snapshot configuration
#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Enable automatic snapshots
    pub auto_snapshot: bool,
    /// Auto-snapshot interval (in seconds)
    pub auto_interval: u64,
    /// Maximum number of auto-snapshots to keep
    pub auto_max_count: usize,
    /// Enable compression for snapshot metadata
    pub compress_metadata: bool,
    /// Reserved space for snapshots (percentage of total space)
    pub reserved_space_percent: u8,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            auto_snapshot: false,
            auto_interval: 3600, // 1 hour
            auto_max_count: 24,  // Keep 24 hourly snapshots
            compress_metadata: true,
            reserved_space_percent: 20,
        }
    }
}

/// Snapshot manager for handling filesystem snapshots
pub struct SnapshotManager {
    /// Block device
    device: Arc<dyn BlockDevice>,
    /// Configuration
    config: SnapshotConfig,
    /// Next snapshot ID
    next_snapshot_id: AtomicU64,
    /// Active snapshots
    snapshots: RwLock<BTreeMap<u64, SnapshotMetadata>>,
    /// Snapshot name to ID mapping
    name_to_id: RwLock<HashMap<String, u64>>,
    /// Block references for CoW
    block_refs: RwLock<HashMap<u64, BlockReference>>,
    /// Pending CoW operations
    pending_cow: RwLock<Vec<CowOperation>>,
    /// Total blocks available
    total_blocks: u64,
    /// Free blocks for allocation
    free_blocks: AtomicU64,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(device: Arc<dyn BlockDevice>, config: SnapshotConfig) -> Self {
        let total_blocks = device.block_count();
        let reserved = (total_blocks * config.reserved_space_percent as u64) / 100;

        Self {
            device,
            config,
            next_snapshot_id: AtomicU64::new(1),
            snapshots: RwLock::new(BTreeMap::new()),
            name_to_id: RwLock::new(HashMap::new()),
            block_refs: RwLock::new(HashMap::new()),
            pending_cow: RwLock::new(Vec::new()),
            total_blocks,
            free_blocks: AtomicU64::new(total_blocks - reserved),
        }
    }

    /// Initialize the snapshot manager
    pub async fn init(&mut self) -> Result<()> {
        // Load existing snapshots from disk
        self.load_snapshots().await?;

        // Start auto-snapshot timer if enabled
        if self.config.auto_snapshot {
            self.start_auto_snapshot().await?;
        }

        Ok(())
    }

    /// Create a new snapshot
    pub async fn create_snapshot(&self, name: &str, tags: HashMap<String, String>) -> Result<u64> {
        // Check if we've reached the maximum
        if self.snapshots.read().len() >= MAX_SNAPSHOTS {
            return Err(crate::error::Error::Other("Too many snapshots".to_string()));
        }

        // Check if name already exists
        if self.name_to_id.read().contains_key(name) {
            return Err(crate::error::Error::Other(format!(
                "Snapshot '{}' already exists",
                name
            )));
        }

        // Generate new snapshot ID
        let snapshot_id = self.next_snapshot_id.fetch_add(1, Ordering::SeqCst);

        // Find parent snapshot (latest active snapshot)
        let parent_id = self
            .snapshots
            .read()
            .values()
            .rev()
            .find(|s| s.state == SnapshotState::Active)
            .map(|s| s.id)
            .unwrap_or(0);

        // Create snapshot metadata
        let metadata = SnapshotMetadata {
            id: snapshot_id,
            name: name.to_string(),
            parent_id,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            state: SnapshotState::Creating,
            root_inode: 2, // TODO: Get actual root inode from filesystem
            block_count: 0,
            exclusive_space: 0,
            tags,
        };

        // Add to snapshots
        self.snapshots.write().insert(snapshot_id, metadata.clone());
        self.name_to_id
            .write()
            .insert(name.to_string(), snapshot_id);

        // Mark snapshot as active
        self.activate_snapshot(snapshot_id).await?;

        // Persist snapshot metadata
        self.save_snapshot_metadata(&metadata).await?;

        log::info!("Created snapshot '{}' with ID {}", name, snapshot_id);
        Ok(snapshot_id)
    }

    /// Activate a snapshot (mark as active)
    async fn activate_snapshot(&self, snapshot_id: u64) -> Result<()> {
        let mut snapshots = self.snapshots.write();
        if let Some(snapshot) = snapshots.get_mut(&snapshot_id) {
            snapshot.state = SnapshotState::Active;
            Ok(())
        } else {
            Err(crate::error::Error::Other(format!(
                "Snapshot {} not found",
                snapshot_id
            )))
        }
    }

    /// Delete a snapshot
    pub async fn delete_snapshot(&self, snapshot_id: u64) -> Result<()> {
        // Check if snapshot exists
        let snapshot = self
            .snapshots
            .read()
            .get(&snapshot_id)
            .cloned()
            .ok_or_else(|| {
                crate::error::Error::Other(format!("Snapshot {} not found", snapshot_id))
            })?;

        // Check if snapshot has children
        let has_children = self
            .snapshots
            .read()
            .values()
            .any(|s| s.parent_id == snapshot_id && s.state != SnapshotState::Deleted);

        if has_children {
            return Err(crate::error::Error::Other(
                "Cannot delete snapshot with children".to_string(),
            ));
        }

        // Mark snapshot for deletion
        {
            let mut snapshots = self.snapshots.write();
            if let Some(s) = snapshots.get_mut(&snapshot_id) {
                s.state = SnapshotState::Deleting;
            }
        }

        // Remove block references
        self.cleanup_snapshot_blocks(snapshot_id).await?;

        // Remove from name mapping
        self.name_to_id.write().remove(&snapshot.name);

        // Remove the snapshot completely
        {
            let mut snapshots = self.snapshots.write();
            snapshots.remove(&snapshot_id);
        }

        log::info!("Deleted snapshot '{}' (ID: {})", snapshot.name, snapshot_id);
        Ok(())
    }

    /// List all active snapshots
    pub fn list_snapshots(&self) -> Vec<SnapshotMetadata> {
        self.snapshots
            .read()
            .values()
            .filter(|s| s.state == SnapshotState::Active)
            .cloned()
            .collect()
    }

    /// Get snapshot by ID
    pub fn get_snapshot(&self, snapshot_id: u64) -> Option<SnapshotMetadata> {
        self.snapshots.read().get(&snapshot_id).cloned()
    }

    /// Get snapshot by name
    pub fn get_snapshot_by_name(&self, name: &str) -> Option<SnapshotMetadata> {
        if let Some(&id) = self.name_to_id.read().get(name) {
            self.get_snapshot(id)
        } else {
            None
        }
    }

    /// Mark a block as referenced by a snapshot (for CoW)
    pub fn reference_block(&self, block_num: u64, snapshot_id: u64) -> Result<()> {
        let mut block_refs = self.block_refs.write();

        let entry = block_refs
            .entry(block_num)
            .or_insert_with(|| BlockReference {
                block_num,
                ref_count: 0,
                snapshots: HashSet::new(),
            });

        if !entry.snapshots.insert(snapshot_id) {
            return Err(crate::error::Error::Other(format!(
                "Block {} already referenced by snapshot {}",
                block_num, snapshot_id
            )));
        }

        entry.ref_count += 1;
        Ok(())
    }

    /// Check if a block needs CoW before modification
    pub fn needs_cow(&self, block_num: u64) -> bool {
        self.block_refs
            .read()
            .get(&block_num)
            .map(|ref_info| ref_info.ref_count > 1)
            .unwrap_or(false)
    }

    /// Perform Copy-on-Write for a block
    pub async fn copy_on_write(&self, block_num: u64) -> Result<u64> {
        if !self.needs_cow(block_num) {
            return Ok(block_num);
        }

        // Allocate new block
        let new_block = self.allocate_block()?;

        // Copy data from original to new block
        let mut buffer = vec![0u8; 4096]; // Assuming 4KB blocks
        self.device.read_block(block_num, &mut buffer).await?;
        self.device.write_block(new_block, &buffer).await?;

        // Record CoW operation
        let cow_op = CowOperation {
            original_block: block_num,
            new_block,
            snapshot_id: 0, // TODO: Get current snapshot context
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.pending_cow.write().push(cow_op);

        log::debug!("CoW: copied block {} to {}", block_num, new_block);
        Ok(new_block)
    }

    /// Allocate a new block
    fn allocate_block(&self) -> Result<u64> {
        let free = self.free_blocks.fetch_sub(1, Ordering::SeqCst);
        if free == 0 {
            self.free_blocks.fetch_add(1, Ordering::SeqCst);
            return Err(crate::error::Error::Other(
                "No free blocks available".to_string(),
            ));
        }

        // In a real implementation, this would use a proper block allocator
        Ok(self.total_blocks - free)
    }

    /// Rollback to a snapshot
    pub async fn rollback_to_snapshot(&self, snapshot_id: u64) -> Result<()> {
        // Verify snapshot exists and is active
        let snapshot = self.get_snapshot(snapshot_id).ok_or_else(|| {
            crate::error::Error::Other(format!("Snapshot {} not found", snapshot_id))
        })?;

        if snapshot.state != SnapshotState::Active {
            return Err(crate::error::Error::Other(format!(
                "Snapshot {} is not active",
                snapshot_id
            )));
        }

        log::info!(
            "Rolling back to snapshot '{}' (ID: {})",
            snapshot.name,
            snapshot_id
        );

        // In a real implementation, this would:
        // 1. Flush all pending writes
        // 2. Update filesystem metadata to point to snapshot's root
        // 3. Invalidate caches
        // 4. Notify VFS layer

        Ok(())
    }

    /// Clean up blocks for a deleted snapshot
    async fn cleanup_snapshot_blocks(&self, snapshot_id: u64) -> Result<()> {
        let mut block_refs = self.block_refs.write();
        let mut blocks_to_remove = Vec::new();

        for (block_num, ref_info) in block_refs.iter_mut() {
            if ref_info.snapshots.remove(&snapshot_id) {
                ref_info.ref_count -= 1;
                if ref_info.ref_count == 0 {
                    blocks_to_remove.push(*block_num);
                    self.free_blocks.fetch_add(1, Ordering::SeqCst);
                }
            }
        }

        // Remove blocks with no references
        for block in blocks_to_remove {
            block_refs.remove(&block);
        }

        Ok(())
    }

    /// Load snapshots from disk
    async fn load_snapshots(&self) -> Result<()> {
        log::info!("Loading snapshots from disk");

        // For now, use a simple JSON file approach
        // In production, this would be integrated with the block device metadata
        let snapshot_file = "/tmp/aegisfs_snapshots.json";

        if let Ok(contents) = tokio::fs::read_to_string(snapshot_file).await {
            if let Ok(saved_snapshots) = serde_json::from_str::<Vec<SnapshotMetadata>>(&contents) {
                let mut snapshots = self.snapshots.write();
                let mut name_to_id = self.name_to_id.write();
                let mut max_id = 0u64;

                for snapshot in saved_snapshots {
                    if snapshot.state == SnapshotState::Active {
                        max_id = max_id.max(snapshot.id);
                        name_to_id.insert(snapshot.name.clone(), snapshot.id);
                        snapshots.insert(snapshot.id, snapshot);
                    }
                }

                // Update next snapshot ID
                self.next_snapshot_id.store(max_id + 1, Ordering::SeqCst);

                log::info!("Loaded {} snapshots from disk", snapshots.len());
            }
        } else {
            log::info!("No existing snapshots found");
        }

        Ok(())
    }

    /// Save snapshot metadata to disk
    async fn save_snapshot_metadata(&self, metadata: &SnapshotMetadata) -> Result<()> {
        log::debug!("Saving snapshot metadata for ID {}", metadata.id);

        // Collect all active snapshots
        let snapshots: Vec<SnapshotMetadata> = self
            .snapshots
            .read()
            .values()
            .filter(|s| s.state == SnapshotState::Active)
            .cloned()
            .collect();

        // Serialize to JSON
        let json_data = serde_json::to_string_pretty(&snapshots).map_err(|e| {
            crate::error::Error::Other(format!("Failed to serialize snapshots: {}", e))
        })?;

        // Write to file
        let snapshot_file = "/tmp/aegisfs_snapshots.json";
        tokio::fs::write(snapshot_file, json_data)
            .await
            .map_err(|e| crate::error::Error::Io(e))?;

        log::debug!("Successfully saved snapshot metadata to {}", snapshot_file);
        Ok(())
    }

    /// Start automatic snapshot timer
    async fn start_auto_snapshot(&self) -> Result<()> {
        log::info!(
            "Starting automatic snapshot timer (interval: {}s)",
            self.config.auto_interval
        );
        // In a real implementation, this would start a background task
        Ok(())
    }

    /// Get snapshot statistics
    pub fn get_snapshot_stats(&self) -> SnapshotStats {
        let snapshots = self.snapshots.read();
        let block_refs = self.block_refs.read();

        SnapshotStats {
            total_snapshots: snapshots.len(),
            active_snapshots: snapshots
                .values()
                .filter(|s| s.state == SnapshotState::Active)
                .count(),
            total_blocks_referenced: block_refs.len(),
            total_space_used: block_refs.len() as u64 * 4096, // Assuming 4KB blocks
            cow_operations_pending: self.pending_cow.read().len(),
        }
    }
}

/// Snapshot statistics
#[derive(Debug, Clone)]
pub struct SnapshotStats {
    /// Total number of snapshots
    pub total_snapshots: usize,
    /// Number of active snapshots
    pub active_snapshots: usize,
    /// Total blocks referenced by snapshots
    pub total_blocks_referenced: usize,
    /// Total space used by snapshots
    pub total_space_used: u64,
    /// Number of pending CoW operations
    pub cow_operations_pending: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::FileBackedBlockDevice;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_create_snapshot() {
        let temp_file = NamedTempFile::new().unwrap();
        let device = Arc::new(
            FileBackedBlockDevice::create(temp_file.path(), 100 * 1024 * 1024) // 100MB
                .await
                .unwrap(),
        );

        let mut manager = SnapshotManager::new(device, SnapshotConfig::default());
        manager.init().await.unwrap();

        // Create a snapshot
        let mut tags = HashMap::new();
        tags.insert("type".to_string(), "manual".to_string());

        let snapshot_id = manager
            .create_snapshot("test-snapshot", tags)
            .await
            .unwrap();
        assert!(snapshot_id > 0);

        // Verify snapshot was created
        let snapshot = manager.get_snapshot(snapshot_id).unwrap();
        assert_eq!(snapshot.name, "test-snapshot");
        assert_eq!(snapshot.state, SnapshotState::Active);
    }

    #[tokio::test]
    async fn test_list_snapshots() {
        let temp_file = NamedTempFile::new().unwrap();
        let device = Arc::new(
            FileBackedBlockDevice::create(temp_file.path(), 100 * 1024 * 1024)
                .await
                .unwrap(),
        );

        let mut manager = SnapshotManager::new(device, SnapshotConfig::default());
        manager.init().await.unwrap();

        // Create multiple snapshots
        for i in 1..=3 {
            manager
                .create_snapshot(&format!("snapshot-{}", i), HashMap::new())
                .await
                .unwrap();
        }

        // List snapshots
        let snapshots = manager.list_snapshots();
        assert_eq!(snapshots.len(), 3);
    }

    #[tokio::test]
    async fn test_copy_on_write() {
        let temp_file = NamedTempFile::new().unwrap();
        let device = Arc::new(
            FileBackedBlockDevice::create(temp_file.path(), 100 * 1024 * 1024)
                .await
                .unwrap(),
        );

        let manager = SnapshotManager::new(device, SnapshotConfig::default());

        // Reference a block from multiple snapshots
        manager.reference_block(10, 1).unwrap();
        manager.reference_block(10, 2).unwrap();

        // Check if CoW is needed
        assert!(manager.needs_cow(10));

        // Perform CoW
        let new_block = manager.copy_on_write(10).await.unwrap();
        assert_ne!(new_block, 10);
    }

    #[tokio::test]
    async fn test_delete_snapshot() {
        let temp_file = NamedTempFile::new().unwrap();
        let device = Arc::new(
            FileBackedBlockDevice::create(temp_file.path(), 100 * 1024 * 1024)
                .await
                .unwrap(),
        );

        let mut manager = SnapshotManager::new(device, SnapshotConfig::default());
        manager.init().await.unwrap();

        // Create and delete a snapshot
        let snapshot_id = manager
            .create_snapshot("temp-snapshot", HashMap::new())
            .await
            .unwrap();

        manager.delete_snapshot(snapshot_id).await.unwrap();

        // Verify snapshot is gone
        assert!(manager.get_snapshot(snapshot_id).is_none());
    }
}
