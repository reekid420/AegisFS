//! Block cache implementation for AegisFS

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use parking_lot::RwLock;
use lru::LruCache;
use async_trait::async_trait;
use crate::blockdev::{BlockDevice, BlockDeviceError, Result, BLOCK_SIZE};
use thiserror::Error;
use std::io;
use arrayref::array_ref;
use crate::blockdev::FileBackedBlockDevice;
use tempfile::tempdir;

/// Error type for cache operations
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Block device error: {0}")]
    BlockDevice(#[from] BlockDeviceError),
    #[error("Cache is full")]
    CacheFull,
    #[error("Invalid block number: {0}")]
    InvalidBlockNumber(u64),
}

impl From<CacheError> for BlockDeviceError {
    fn from(err: CacheError) -> Self {
        match err {
            CacheError::BlockDevice(e) => e,
            CacheError::CacheFull => BlockDeviceError::Io(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "Cache is full"
            )),
            CacheError::InvalidBlockNumber(n) => BlockDeviceError::InvalidBlockNumber(n),
        }
    }
}

/// A cached block with metadata
struct CachedBlock {
    data: Box<[u8; BLOCK_SIZE]>,
    dirty: bool,
}

/// A block cache that maintains a fixed-size in-memory cache of blocks
pub struct BlockCache {
    device: Arc<dyn BlockDevice>,
    cache: RwLock<LruCache<u64, CachedBlock>>,
    write_through: bool,
}

impl BlockCache {
    /// Create a new block cache with the given capacity (in number of blocks)
    pub fn new(device: Arc<dyn BlockDevice>, capacity: usize, write_through: bool) -> Self {
        Self {
            device,
            cache: RwLock::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
            write_through,
        }
    }
    
    /// Read a block from the cache or device
    pub async fn read_block(&self, block_num: u64, buf: &mut [u8]) -> Result<()> {
        if block_num >= self.device.block_count() {
            return Err(BlockDeviceError::InvalidBlockNumber(block_num));
        }

        if buf.len() != BLOCK_SIZE {
            return Err(BlockDeviceError::InvalidBlockSize(buf.len()));
        }

        // Check cache first with a read lock
        {
            let cache = self.cache.read();
            if let Some(block) = cache.peek(&block_num) {
                buf.copy_from_slice(&*block.data);
                return Ok(());
            }
        }

        // If not in cache, read from device and cache it
        let mut block = [0u8; BLOCK_SIZE];
        self.device.read_block(block_num, &mut block).await?;
        
        // Insert into cache with a write lock
        let cached_block = CachedBlock {
            data: Box::new(block),
            dirty: false,
        };
        
        // We need to handle cache access carefully to avoid holding the lock across await
        let existing_block = {
            let mut cache = self.cache.write();
            if let Some(existing) = cache.peek(&block_num) {
                // Another thread inserted it while we were reading
                Some(existing.data.clone())
            } else {
                // Not in cache, insert our value
                cache.push(block_num, cached_block);
                None
            }
        };
        
        if let Some(data) = existing_block {
            // Use the existing data
            buf.copy_from_slice(&*data);
        } else {
            // Use the data we read
            buf.copy_from_slice(&block);
        }
        
        Ok(())
    }
    
    /// Write a block to the cache (and device if write-through)
    pub async fn write_block(&self, block_num: u64, data: &[u8]) -> Result<()> {
        if block_num >= self.device.block_count() {
            return Err(BlockDeviceError::InvalidBlockNumber(block_num));
        }

        if data.len() != BLOCK_SIZE {
            return Err(BlockDeviceError::InvalidBlockSize(data.len()));
        }
        
        let block_data = *array_ref!(data, 0, BLOCK_SIZE);
        
        if self.write_through {
            // Write directly to device in write-through mode
            self.device.write_block(block_num, &block_data).await?;
        }
        
        // Update cache with a write lock
        let cached_block = CachedBlock {
            data: Box::new(block_data),
            dirty: !self.write_through,
        };
        
        // We need to handle eviction carefully to avoid holding the lock across an await point
        let maybe_evicted = {
            let mut cache = self.cache.write();
            cache.push(block_num, cached_block)
        };
        
        // If we evicted a dirty block, write it back
        if let Some((evicted_block_num, evicted)) = maybe_evicted {
            if evicted.dirty && !self.write_through {
                // Write the evicted block back to the device outside the lock
                self.device.write_block(evicted_block_num, &*evicted.data).await?;
            }
        }
        
        Ok(())
    }
    
    /// Flush all dirty blocks to disk
    pub async fn flush(&self) -> Result<()> {
        // First collect all dirty blocks with a read lock
        let dirty_blocks: Vec<_> = {
            let cache = self.cache.read();
            cache.iter()
                .filter(|(_, block)| block.dirty)
                .map(|(block_num, block)| (*block_num, block.data.clone()))
                .collect()
        };
        
        let mut write_errors = Vec::new();
        
        // Write all dirty blocks to disk
        for (block_num, data) in dirty_blocks {
            if let Err(e) = self.device.write_block(block_num, &*data).await {
                write_errors.push((block_num, e));
            } else {
                // Mark as clean with a write lock
                let mut cache = self.cache.write();
                if let Some(block) = cache.get_mut(&block_num) {
                    block.dirty = false;
                }
            }
        }
        
        if !write_errors.is_empty() {
            return Err(BlockDeviceError::Io(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to write {} blocks", write_errors.len()),
            )));
        }
        
        // Sync the device to ensure all writes are persisted
        self.device.sync().await
    }
    
    /// Clear the entire cache, writing back any dirty blocks
    pub async fn clear(&self) -> Result<()> {
        self.flush().await?;
        self.cache.write().clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::cell::RefCell;
    use async_trait::async_trait;
    use crate::blockdev::BlockDevice;
    use crate::blockdev::FileBackedBlockDevice;
    use std::io;
    use futures::executor::block_on;
    use tempfile::tempdir;
    
    struct MockBlockDevice {
        blocks: std::collections::HashMap<u64, [u8; BLOCK_SIZE]>,
    }
    
    impl MockBlockDevice {
        fn new() -> Self {
            Self {
                blocks: std::collections::HashMap::new(),
            }
        }
    }
    
    #[async_trait]
    impl BlockDevice for MockBlockDevice {
        async fn read_block(&self, block_num: u64, buf: &mut [u8]) -> Result<()> {
            if block_num >= 1024 {
                return Err(BlockDeviceError::InvalidBlockNumber(block_num));
            }
            buf.fill(0);
            Ok(())
        }
        
        async fn write_block(&self, block_num: u64, data: &[u8]) -> Result<()> {
            if block_num >= 1024 {
                return Err(BlockDeviceError::InvalidBlockNumber(block_num));
            }
            Ok(())
        }
        
        fn block_count(&self) -> u64 {
            1024
        }
        
        fn block_size(&self) -> usize {
            BLOCK_SIZE
        }
        
        async fn sync(&self) -> Result<()> {
            Ok(())
        }
        
        async fn close(&mut self) -> Result<()> {
            Ok(())
        }
        
        fn is_read_only(&self) -> bool {
            false
        }
    }
    
    #[tokio::test]
    async fn test_cache_read_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_cache");
        
        // Create a test device
        let device = Arc::new(FileBackedBlockDevice::create(&path, 2 * BLOCK_SIZE as u64).await.unwrap());
        
        // Create a cache with capacity for 2 blocks
        let cache = BlockCache::new(device.clone(), 2, false);
        
        // Write test data
        let test_data1 = [0xAA; BLOCK_SIZE];
        let test_data2 = [0x55; BLOCK_SIZE];
        
        // Write blocks
        cache.write_block(0, &test_data1).await.unwrap();
        cache.write_block(1, &test_data2).await.unwrap();
        
        // Read back and verify
        let mut read_buf1 = [0u8; BLOCK_SIZE];
        let mut read_buf2 = [0u8; BLOCK_SIZE];
        cache.read_block(0, &mut read_buf1).await.unwrap();
        cache.read_block(1, &mut read_buf2).await.unwrap();
        
        assert_eq!(&read_buf1, &test_data1);
        assert_eq!(&read_buf2, &test_data2);
        
        // Flush and verify data was written to device
        cache.flush().await.unwrap();
        
        let mut buf = [0u8; BLOCK_SIZE];
        device.read_block(0, &mut buf).await.unwrap();
        assert_eq!(&buf, &test_data1);
        
        device.read_block(1, &mut buf).await.unwrap();
        assert_eq!(&buf, &test_data2);
    }
    
    #[tokio::test]
    async fn test_cache_eviction() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_eviction");
        
        // Create a test device with 4 blocks
        let device = Arc::new(FileBackedBlockDevice::create(&path, 4 * BLOCK_SIZE as u64).await.unwrap());
        
        // Create a cache with capacity for 2 blocks
        let cache = BlockCache::new(device, 2, false);
        
        // Write 3 blocks (should evict the first one)
        let test_data = [
            [0x11; BLOCK_SIZE],
            [0x22; BLOCK_SIZE],
            [0x33; BLOCK_SIZE],
        ];
        
        for (i, data) in test_data.iter().enumerate() {
            cache.write_block(i as u64, data).await.unwrap();
        }
        
        // First block should be evicted
        let mut read_buf = [0u8; BLOCK_SIZE];
        assert!(cache.read_block(0, &mut read_buf).await.is_err());
        
        // Second and third blocks should be in cache
        let mut read_buf2 = [0u8; BLOCK_SIZE];
        let mut read_buf3 = [0u8; BLOCK_SIZE];
        cache.read_block(1, &mut read_buf2).await.unwrap();
        cache.read_block(2, &mut read_buf3).await.unwrap();
        
        assert_eq!(&read_buf2, &test_data[1]);
        assert_eq!(&read_buf3, &test_data[2]);
    }
}
