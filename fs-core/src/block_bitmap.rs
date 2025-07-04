//! Block bitmap implementation for AegisFS
//!
//! This module provides block-level allocation tracking using a persistent bitmap
//! to replace the naive static counter approach. Supports block allocation,
//! deallocation, and persistence across mounts.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;

use crate::blockdev::{BlockDevice, BlockDeviceError, BLOCK_SIZE};
use crate::layout::{FsError, Layout};

/// Error type for block bitmap operations
#[derive(Error, Debug)]
pub enum BlockBitmapError {
    #[error("Block device error: {0}")]
    BlockDevice(#[from] BlockDeviceError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid block number: {0}")]
    InvalidBlockNumber(u64),
    #[error("Block {0} is already allocated")]
    BlockAlreadyAllocated(u64),
    #[error("Block {0} is already free")]
    BlockAlreadyFree(u64),
    #[error("No free blocks available")]
    NoFreeBlocks,
    #[error("Bitmap is full")]
    BitmapFull,
}

/// Block bitmap for tracking allocated/free data blocks
pub struct BlockBitmap {
    /// Bitmap data (each bit represents one block)
    bitmap: Vec<u8>,
    /// Total number of blocks that can be tracked
    total_blocks: u64,
    /// Number of free blocks
    free_blocks: AtomicU64,
    /// Starting block number for data blocks
    data_blocks_start: u64,
    /// Number of data blocks
    data_blocks_count: u64,
}

impl BlockBitmap {
    /// Create a new block bitmap for formatting
    pub fn new(total_blocks: u64, data_blocks_start: u64, data_blocks_count: u64) -> Self {
        let bitmap_size = ((data_blocks_count + 7) / 8) as usize;
        let bitmap = vec![0u8; bitmap_size];
        
        Self {
            bitmap,
            total_blocks,
            free_blocks: AtomicU64::new(data_blocks_count),
            data_blocks_start,
            data_blocks_count,
        }
    }

    /// Load block bitmap from disk
    pub async fn load_from_disk(
        device: Arc<dyn BlockDevice>,
        layout: &Layout,
    ) -> Result<Self, BlockBitmapError> {
        let bitmap_size = ((layout.data_blocks_count + 7) / 8) as usize;
        let mut bitmap = vec![0u8; bitmap_size];
        
        // Read bitmap blocks from disk
        let mut bytes_read = 0;
        for block_offset in 0..layout.block_bitmap_blocks {
            let block_num = layout.block_bitmap + block_offset;
            let mut block_data = vec![0u8; BLOCK_SIZE];
            device.read_block(block_num, &mut block_data).await?;
            
            let bytes_to_copy = std::cmp::min(block_data.len(), bitmap_size - bytes_read);
            if bytes_to_copy > 0 {
                bitmap[bytes_read..bytes_read + bytes_to_copy]
                    .copy_from_slice(&block_data[..bytes_to_copy]);
                bytes_read += bytes_to_copy;
            }
            
            if bytes_read >= bitmap_size {
                break;
            }
        }
        
        // Count free blocks by scanning the bitmap
        let mut free_count = 0;
        for (byte_idx, &byte) in bitmap.iter().enumerate() {
            for bit in 0..8 {
                let block_idx = (byte_idx * 8 + bit) as u64;
                if block_idx >= layout.data_blocks_count {
                    break;
                }
                if (byte & (1 << bit)) == 0 {
                    free_count += 1;
                }
            }
        }
        
        log::info!(
            "BLOCK_BITMAP: Loaded from disk - {} free blocks out of {} total data blocks",
            free_count,
            layout.data_blocks_count
        );
        
        Ok(Self {
            bitmap,
            total_blocks: layout.data_blocks_count,
            free_blocks: AtomicU64::new(free_count),
            data_blocks_start: layout.data_blocks,
            data_blocks_count: layout.data_blocks_count,
        })
    }

    /// Save block bitmap to disk
    pub async fn save_to_disk(
        &self,
        device: Arc<dyn BlockDevice>,
        layout: &Layout,
    ) -> Result<(), BlockBitmapError> {
        let mut bytes_written = 0;
        
        // Write bitmap blocks to disk
        for block_offset in 0..layout.block_bitmap_blocks {
            let block_num = layout.block_bitmap + block_offset;
            let mut block_data = vec![0u8; BLOCK_SIZE];
            
            let bytes_to_copy = std::cmp::min(BLOCK_SIZE, self.bitmap.len() - bytes_written);
            if bytes_to_copy > 0 {
                block_data[..bytes_to_copy]
                    .copy_from_slice(&self.bitmap[bytes_written..bytes_written + bytes_to_copy]);
                bytes_written += bytes_to_copy;
            }
            
            device.write_block(block_num, &block_data).await?;
            
            if bytes_written >= self.bitmap.len() {
                break;
            }
        }
        
        log::debug!(
            "BLOCK_BITMAP: Saved to disk - {} free blocks",
            self.free_blocks.load(Ordering::Relaxed)
        );
        Ok(())
    }

    /// Allocate a free block
    pub fn allocate(&mut self) -> Option<u64> {
        let current_free = self.free_blocks.load(Ordering::Relaxed);
        log::debug!(
            "BlockBitmap::allocate: Starting allocation with {} free blocks",
            current_free
        );
        
        if current_free == 0 {
            log::warn!("BlockBitmap::allocate: No free blocks available");
            return None;
        }
        
        // Find first free bit
        for (byte_idx, byte) in self.bitmap.iter_mut().enumerate() {
            if *byte != 0xFF {
                for bit in 0..8 {
                    if (*byte & (1 << bit)) == 0 {
                        let block_idx = (byte_idx * 8 + bit) as u64;
                        
                        if block_idx < self.data_blocks_count {
                            log::debug!(
                                "BlockBitmap::allocate: Found free block {} at byte {} bit {}",
                                block_idx,
                                byte_idx,
                                bit
                            );
                            *byte |= 1 << bit;
                            self.free_blocks.fetch_sub(1, Ordering::Relaxed);
                            
                            let actual_block_num = self.data_blocks_start + block_idx;
                            log::info!(
                                "BlockBitmap::allocate: Successfully allocated data block {} (index {}), {} free remaining",
                                actual_block_num,
                                block_idx,
                                self.free_blocks.load(Ordering::Relaxed)
                            );
                            return Some(block_idx);
                        }
                    }
                }
            }
        }
        
        log::error!(
            "BlockBitmap::allocate: No free block found despite {} free count",
            current_free
        );
        None
    }

    /// Free a block
    pub fn free(&mut self, block_idx: u64) -> Result<(), BlockBitmapError> {
        if block_idx >= self.data_blocks_count {
            return Err(BlockBitmapError::InvalidBlockNumber(block_idx));
        }
        
        let byte_idx = (block_idx / 8) as usize;
        let bit = (block_idx % 8) as u8;
        
        if byte_idx >= self.bitmap.len() {
            return Err(BlockBitmapError::InvalidBlockNumber(block_idx));
        }
        
        // Check if block is already free
        if (self.bitmap[byte_idx] & (1 << bit)) == 0 {
            return Err(BlockBitmapError::BlockAlreadyFree(block_idx));
        }
        
        self.bitmap[byte_idx] &= !(1 << bit);
        self.free_blocks.fetch_add(1, Ordering::Relaxed);
        
        let actual_block_num = self.data_blocks_start + block_idx;
        log::info!(
            "BlockBitmap::free: Freed data block {} (index {}), {} free total",
            actual_block_num,
            block_idx,
            self.free_blocks.load(Ordering::Relaxed)
        );
        
        Ok(())
    }

    /// Check if a block is allocated
    pub fn is_allocated(&self, block_idx: u64) -> bool {
        if block_idx >= self.data_blocks_count {
            return false;
        }
        
        let byte_idx = (block_idx / 8) as usize;
        let bit = (block_idx % 8) as u8;
        
        if byte_idx >= self.bitmap.len() {
            return false;
        }
        
        (self.bitmap[byte_idx] & (1 << bit)) != 0
    }

    /// Get the number of free blocks
    pub fn free_blocks(&self) -> u64 {
        self.free_blocks.load(Ordering::Relaxed)
    }

    /// Get the total number of blocks
    pub fn total_blocks(&self) -> u64 {
        self.data_blocks_count
    }

    /// Initialize all blocks as free (for formatting)
    pub fn initialize_as_free(&mut self) {
        self.bitmap.fill(0);
        self.free_blocks.store(self.data_blocks_count, Ordering::Relaxed);
        log::info!(
            "BlockBitmap::initialize_as_free: Initialized {} blocks as free",
            self.data_blocks_count
        );
    }

    /// Get a reference to the bitmap data (for persistence)
    pub fn bitmap_data(&self) -> &[u8] {
        &self.bitmap
    }
}

impl std::fmt::Debug for BlockBitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockBitmap")
            .field("total_blocks", &self.total_blocks)
            .field("free_blocks", &self.free_blocks.load(Ordering::Relaxed))
            .field("data_blocks_start", &self.data_blocks_start)
            .field("data_blocks_count", &self.data_blocks_count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::FileBackedBlockDevice;
    use tempfile::tempdir;

    #[test]
    fn test_block_allocation() {
        let mut bitmap = BlockBitmap::new(1024, 100, 900);
        
        // Test allocation
        let block1 = bitmap.allocate().unwrap();
        assert_eq!(block1, 0);
        assert_eq!(bitmap.free_blocks(), 899);
        
        let block2 = bitmap.allocate().unwrap();
        assert_eq!(block2, 1);
        assert_eq!(bitmap.free_blocks(), 898);
        
        // Test that allocated blocks are marked as allocated
        assert!(bitmap.is_allocated(0));
        assert!(bitmap.is_allocated(1));
        assert!(!bitmap.is_allocated(2));
    }

    #[test]
    fn test_block_deallocation() {
        let mut bitmap = BlockBitmap::new(1024, 100, 900);
        
        // Allocate and then free
        let block = bitmap.allocate().unwrap();
        assert_eq!(bitmap.free_blocks(), 899);
        
        bitmap.free(block).unwrap();
        assert_eq!(bitmap.free_blocks(), 900);
        assert!(!bitmap.is_allocated(block));
    }

    #[test]
    fn test_block_reuse() {
        let mut bitmap = BlockBitmap::new(1024, 100, 900);
        
        // Allocate, free, then allocate again
        let block1 = bitmap.allocate().unwrap();
        bitmap.free(block1).unwrap();
        let block2 = bitmap.allocate().unwrap();
        
        // Should reuse the same block
        assert_eq!(block1, block2);
    }
} 