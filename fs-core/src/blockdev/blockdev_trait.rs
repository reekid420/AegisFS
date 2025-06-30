//! Block device trait definitions for AegisFS

use std::io::{self, Read, Seek, Write};
use thiserror::Error;

/// Block size in bytes (4KB)
pub const BLOCK_SIZE: usize = 4096;

/// Error type for block device operations
#[derive(Error, Debug)]
pub enum BlockDeviceError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Invalid block number: {0}")]
    InvalidBlockNumber(u64),
    #[error("Invalid block size: {0} (expected {BLOCK_SIZE})")]
    InvalidBlockSize(usize),
    #[error("Device is read-only")]
    ReadOnly,
    #[error("Device is not open")]
    DeviceNotOpen,
    #[error("Device is already closed")]
    DeviceClosed,
}

/// Result type for block device operations
pub type Result<T> = std::result::Result<T, BlockDeviceError>;

/// Trait for block device operations
#[async_trait::async_trait]
pub trait BlockDevice: Send + Sync + 'static {
    /// Read a block from the device
    async fn read_block(&self, block_num: u64, buf: &mut [u8]) -> Result<()>;
    
    /// Write a block to the device
    async fn write_block(&self, block_num: u64, data: &[u8]) -> Result<()>;
    
    /// Get the total number of blocks in the device
    fn block_count(&self) -> u64;
    
    /// Get the block size in bytes
    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }
    
    /// Sync any pending writes to the device
    async fn sync(&self) -> Result<()>;
    
    /// Close the device
    async fn close(&mut self) -> Result<()>;
    
    /// Check if the device is read-only
    fn is_read_only(&self) -> bool {
        false
    }
}
