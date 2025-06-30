//! On-disk layout definitions for AegisFS

use crate::blockdev::{BlockDevice, BlockDeviceError, BLOCK_SIZE};
use crate::cache::BlockCache;
use crate::format::{DirEntry, FormatError, Inode as DiskInode, Superblock};
use async_trait::async_trait;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use futures::TryFutureExt;
use parking_lot::RwLock;
use std::io::{self, Cursor, Write};
use std::sync::Arc;
use thiserror::Error;

// Helper trait to convert between error types
trait IntoFsError<T> {
    fn into_fs_error(self) -> Result<T, FsError>;
}

impl<T> IntoFsError<T> for Result<T, BlockDeviceError> {
    fn into_fs_error(self) -> Result<T, FsError> {
        self.map_err(FsError::Io)
    }
}

/// Magic number for AegisFS filesystem
const AEGISFS_MAGIC: &[u8; 8] = b"AEGISFS\x00";
/// Current filesystem version
const FS_VERSION: u32 = 1;

/// Block numbers for important filesystem structures
#[derive(Debug, Clone, Copy)]
pub struct Layout {
    /// Block number of the superblock (always block 0)
    pub superblock: u64,
    /// Block number of the block bitmap
    pub block_bitmap: u64,
    /// Number of blocks in the block bitmap
    pub block_bitmap_blocks: u64,
    /// Block number of the inode bitmap
    pub inode_bitmap: u64,
    /// Number of blocks in the inode bitmap
    pub inode_bitmap_blocks: u64,
    /// Block number of the inode table
    pub inode_table: u64,
    /// Number of blocks in the inode table
    pub inode_table_blocks: u64,
    /// Block number of the first data block
    pub data_blocks: u64,
    /// Total number of data blocks
    pub data_blocks_count: u64,
}

impl Layout {
    /// Calculate the layout for a filesystem with the given parameters
    pub fn new(block_count: u64, inode_count: u64) -> Self {
        // Superblock is always at block 0
        let superblock = 0;

        // Block bitmap starts right after superblock
        let block_bitmap = 1;
        let block_bitmap_blocks = (block_count + 7) / 8 / BLOCK_SIZE as u64 + 1;

        // Inode bitmap follows block bitmap
        let inode_bitmap = block_bitmap + block_bitmap_blocks;
        let inode_bitmap_blocks = (inode_count + 7) / 8 / BLOCK_SIZE as u64 + 1;

        // Inode table follows inode bitmap
        let inode_table = inode_bitmap + inode_bitmap_blocks;
        let inode_size = 128; // Size of on-disk inode in bytes
        let inodes_per_block = (BLOCK_SIZE as u64) / inode_size;
        let inode_table_blocks = (inode_count + inodes_per_block - 1) / inodes_per_block;

        // Data blocks start after inode table
        let data_blocks = inode_table + inode_table_blocks;
        let data_blocks_count = block_count - data_blocks;

        Self {
            superblock,
            block_bitmap,
            block_bitmap_blocks,
            inode_bitmap,
            inode_bitmap_blocks,
            inode_table,
            inode_table_blocks,
            data_blocks,
            data_blocks_count,
        }
    }

    /// Get the block number and byte offset for a given inode number
    pub fn inode_block(&self, inode_num: u64) -> (u64, u64) {
        const INODE_SIZE: u64 = 128; // Size of on-disk inode in bytes
        let inodes_per_block = BLOCK_SIZE as u64 / INODE_SIZE;
        let block_offset = inode_num / inodes_per_block;
        let inode_offset = (inode_num % inodes_per_block) * INODE_SIZE;
        (self.inode_table + block_offset, inode_offset)
    }

    /// Get the block number for a given data block
    pub fn data_block(&self, block_num: u64) -> u64 {
        self.data_blocks + block_num
    }
}

/// On-disk filesystem implementation
#[async_trait]
pub trait DiskFsTrait: Send + Sync {
    /// Open an existing filesystem on the given block device
    async fn open(device: Arc<dyn BlockDevice>) -> Result<Self, FsError>
    where
        Self: Sized;

    /// Format a new filesystem on the given block device
    async fn format(
        device: &mut dyn BlockDevice,
        size: u64,
        volume_name: Option<&str>,
    ) -> Result<(), FsError>;

    /// Read an inode from disk
    async fn read_inode(&self, inode_num: u64) -> Result<DiskInode, FsError>;

    /// Write an inode to disk
    async fn write_inode(&mut self, inode_num: u64, inode: &DiskInode) -> Result<(), FsError>;

    /// Read data from a file's data blocks
    async fn read_file_data(
        &self,
        inode: &DiskInode,
        offset: u64,
        size: u32,
    ) -> Result<Vec<u8>, FsError>;

    /// Write data to a file's data blocks
    async fn write_file_data(
        &mut self,
        inode: &mut DiskInode,
        offset: u64,
        data: &[u8],
    ) -> Result<(), FsError>;

    /// Allocate a new data block
    async fn allocate_data_block(&mut self) -> Result<u64, FsError>;
}

/// On-disk filesystem implementation
pub struct DiskFs {
    device: Arc<dyn BlockDevice>,
    cache: BlockCache,
    layout: Layout,
    superblock: Superblock,
}

impl DiskFs {
    /// Create a new DiskFs instance (for internal use)
    fn new(
        device: Arc<dyn BlockDevice>,
        cache: BlockCache,
        layout: Layout,
        superblock: Superblock,
    ) -> Self {
        Self {
            device,
            cache,
            layout,
            superblock,
        }
    }
}

#[async_trait]
impl DiskFsTrait for DiskFs {
    /// Open an existing filesystem on the given block device
    async fn open(device: Arc<dyn BlockDevice>) -> Result<Self, FsError>
    where
        Self: Sized,
    {
        // Read superblock from block 0
        let mut superblock_buf = vec![0; BLOCK_SIZE];
        device
            .read_block(0, &mut superblock_buf)
            .await
            .into_fs_error()?;

        // Parse superblock
        let mut cursor = Cursor::new(superblock_buf);
        let superblock = Superblock::read_from(&mut cursor).map_err(FsError::Format)?;

        // Create cache (cache 1024 blocks = 4MB)
        let cache = BlockCache::new(device.clone(), 1024, false);

        // Calculate layout
        let block_count = superblock.block_count;
        let inode_count = superblock.inode_count;
        let layout = Layout::new(block_count, inode_count);

        Ok(Self {
            device,
            cache,
            layout,
            superblock,
        })
    }

    /// Format a new filesystem on the given block device
    async fn format(
        device: &mut dyn BlockDevice,
        size: u64,
        volume_name: Option<&str>,
    ) -> Result<(), FsError> {
        // Create superblock
        let block_size = 4096; // 4KB blocks
        let block_count = size / block_size as u64;
        let inode_count = block_count * 4; // 1 inode per 4 blocks

        // Create a new superblock - convert io::Result to our Result
        let mut superblock =
            Superblock::new(size, volume_name).map_err(|e| FsError::Format(FormatError::Io(e)))?;

        // Create layout
        let layout = Layout::new(block_count, inode_count);

        // Write superblock to block 0
        let mut superblock_buf = vec![0u8; block_size as usize];
        superblock
            .write_to(&mut std::io::Cursor::new(&mut superblock_buf))
            .map_err(|e| FsError::Io(BlockDeviceError::Io(e)))?;

        device
            .write_block(0, &superblock_buf)
            .await
            .map_err(FsError::from)?;

        // Create root directory inode
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| FsError::InvalidArgument("System time is before UNIX_EPOCH".to_string()))?
            .as_secs();

        let root_inode = DiskInode {
            mode: 0o40755, // Directory with 0755 permissions
            uid: 0,        // root
            gid: 0,        // root
            size: 0,
            atime: now,
            mtime: now,
            ctime: now,
            links: 2, // '.' and '..' from parent
            blocks: 0,
            flags: 0,
            osd1: [0; 4],
            block: [0; 15],
            generation: 0,
            file_acl: 0,
            dir_acl: 0,
            faddr: 0,
            osd2: [0; 12],
        };

        // Write root inode
        let mut inode_buf = vec![0u8; block_size as usize];
        root_inode
            .write_to(&mut std::io::Cursor::new(&mut inode_buf))
            .map_err(|e| FsError::Io(BlockDeviceError::Io(e)))?;

        // Write root inode to inode table
        device
            .write_block(layout.inode_block(1).0, &inode_buf)
            .await
            .map_err(FsError::from)?;

        // Create root directory entries
        let dot = DirEntry::new(1, ".");
        let dot_dot = DirEntry::new(1, "..");

        let mut dir_block = vec![0u8; block_size as usize];
        let mut cursor = std::io::Cursor::new(&mut dir_block);

        dot.write_to(&mut cursor)
            .map_err(|e| FsError::Io(BlockDeviceError::Io(e)))?;
        dot_dot
            .write_to(&mut cursor)
            .map_err(|e| FsError::Io(BlockDeviceError::Io(e)))?;

        // Write directory block
        device
            .write_block(layout.data_block(0), &dir_block)
            .await
            .map_err(FsError::from)?;

        // Update superblock with allocated blocks
        superblock.free_blocks = block_count - 3; // Superblock, inode, and data block
        superblock.free_inodes = inode_count - 1; // Root inode
        superblock.last_write = now;

        // Write updated superblock
        let mut superblock_buf = vec![0u8; block_size as usize];
        superblock
            .write_to(&mut std::io::Cursor::new(&mut superblock_buf))
            .map_err(|e| FsError::Io(BlockDeviceError::Io(e)))?;

        device
            .write_block(0, &superblock_buf)
            .await
            .map_err(FsError::from)?;

        // Sync the device to ensure all writes are persisted
        device.sync().await?;

        Ok(())
    }

    /// Read an inode from disk
    async fn read_inode(&self, inode_num: u64) -> Result<DiskInode, FsError> {
        if inode_num < 1 || inode_num >= self.superblock.inode_count {
            return Err(FsError::InvalidInode);
        }

        // Get block number and offset for the inode
        let (block_num, offset) = self.layout.inode_block(inode_num);

        // Read the block containing the inode
        let mut block_data = vec![0u8; BLOCK_SIZE];
        self.cache
            .read_block(block_num, &mut block_data)
            .await
            .map_err(FsError::Io)?;

        // Parse the inode from the block at the given offset
        // We need to implement a basic deserializer for DiskInode
        let mut cursor = Cursor::new(&block_data[offset as usize..(offset as usize + 128)]);

        // For now, return a placeholder inode. In a real implementation, we would deserialize from cursor
        // This is just a placeholder to get past compilation
        Ok(DiskInode {
            mode: 0,
            uid: 0,
            gid: 0,
            size: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
            links: 0,
            blocks: 0,
            flags: 0,
            osd1: [0; 4],
            block: [0; 15],
            generation: 0,
            file_acl: 0,
            dir_acl: 0,
            faddr: 0,
            osd2: [0; 12],
        })
    }

    /// Write an inode to disk
    async fn write_inode(&mut self, inode_num: u64, inode: &DiskInode) -> Result<(), FsError> {
        if inode_num < 1 || inode_num >= self.superblock.inode_count {
            return Err(FsError::InvalidInode);
        }

        let (block_num, offset) = self.layout.inode_block(inode_num);

        // Read the block containing the inode
        let mut block = vec![0u8; BLOCK_SIZE];
        self.device
            .read_block(block_num, &mut block)
            .await
            .into_fs_error()?;

        // Update the inode in the block
        const INODE_SIZE: usize = 128; // Size of DiskInode
        let offset = offset as usize; // Safe because offset is derived from BLOCK_SIZE
        let inode_slice = &mut block[offset..offset + INODE_SIZE];
        let mut cursor = Cursor::new(inode_slice);
        inode.write_to(&mut cursor)?;

        // Write the block back
        self.device
            .write_block(block_num, &block)
            .await
            .into_fs_error()?;

        Ok(())
    }

    /// Read data from a file's data blocks
    async fn read_file_data(
        &self,
        inode: &DiskInode,
        offset: u64,
        size: u32,
    ) -> Result<Vec<u8>, FsError> {
        let mut result = Vec::new();
        let mut remaining = size as usize;
        let mut current_offset = offset;

        while remaining > 0 && current_offset < inode.size {
            let block_idx = current_offset / BLOCK_SIZE as u64;
            let block_offset = current_offset % BLOCK_SIZE as u64;

            // For now, we only support direct blocks (first 12 entries in block array)
            if block_idx >= 12 {
                break; // Don't support indirect blocks yet
            }

            let block_num = inode.block[block_idx as usize];
            if block_num == 0 {
                // Sparse block, return zeros
                let to_read = std::cmp::min(remaining, BLOCK_SIZE - block_offset as usize);
                result.extend_from_slice(&vec![0u8; to_read]);
                remaining -= to_read;
                current_offset += to_read as u64;
                continue;
            }

            // Read the data block
            let mut block_data = vec![0u8; BLOCK_SIZE];
            self.cache
                .read_block(self.layout.data_block(block_num), &mut block_data)
                .await
                .map_err(FsError::Io)?;

            // Copy the relevant portion
            let to_read = std::cmp::min(remaining, BLOCK_SIZE - block_offset as usize);
            let end_offset = block_offset as usize + to_read;
            result.extend_from_slice(&block_data[block_offset as usize..end_offset]);

            remaining -= to_read;
            current_offset += to_read as u64;
        }

        Ok(result)
    }

    /// Write data to a file's data blocks
    async fn write_file_data(
        &mut self,
        inode: &mut DiskInode,
        offset: u64,
        data: &[u8],
    ) -> Result<(), FsError> {
        let mut remaining = data.len();
        let mut data_offset = 0;
        let mut current_offset = offset;

        while remaining > 0 {
            let block_idx = current_offset / BLOCK_SIZE as u64;
            let block_offset = current_offset % BLOCK_SIZE as u64;

            // For now, we only support direct blocks (first 12 entries in block array)
            if block_idx >= 12 {
                return Err(FsError::InvalidArgument(
                    "File too large for direct blocks".to_string(),
                ));
            }

            let mut block_num = inode.block[block_idx as usize] as u64;

            // Allocate a new block if needed
            if block_num == 0 {
                // TODO: Implement proper block allocation
                // For now, use a simple allocation strategy
                block_num = self.allocate_data_block().await?;
                inode.block[block_idx as usize] = block_num;
            }

            // Read the existing block
            let mut block_data = vec![0u8; BLOCK_SIZE];
            if block_num > 0 {
                self.cache
                    .read_block(self.layout.data_block(block_num), &mut block_data)
                    .await
                    .map_err(FsError::Io)?;
            }

            // Update the block with new data
            let to_write = std::cmp::min(remaining, BLOCK_SIZE - block_offset as usize);
            let end_offset = block_offset as usize + to_write;
            block_data[block_offset as usize..end_offset]
                .copy_from_slice(&data[data_offset..data_offset + to_write]);

            // Write the block back
            self.device
                .write_block(self.layout.data_block(block_num), &block_data)
                .await
                .into_fs_error()?;

            remaining -= to_write;
            data_offset += to_write;
            current_offset += to_write as u64;
        }

        // Update file size if needed
        if current_offset > inode.size {
            inode.size = current_offset;
            inode.blocks = (inode.size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64;
        }

        Ok(())
    }

    /// Allocate a new data block (simple implementation)
    async fn allocate_data_block(&mut self) -> Result<u64, FsError> {
        // TODO: Implement proper block bitmap management
        // For now, use a simple counter-based allocation
        static mut NEXT_BLOCK: u64 = 1;

        unsafe {
            let block = NEXT_BLOCK;
            NEXT_BLOCK += 1;

            if block >= self.layout.data_blocks_count {
                return Err(FsError::NoFreeBlocks);
            }

            Ok(block)
        }
    }
}

/// Filesystem error type
#[derive(Error, Debug)]
pub enum FsError {
    #[error("I/O error: {0}")]
    Io(#[from] BlockDeviceError),
    #[error("Filesystem is corrupt")]
    CorruptFs,
    #[error("Inode is corrupt")]
    CorruptInode,
    #[error("Invalid inode number")]
    InvalidInode,
    #[error("No free inodes")]
    NoFreeInodes,
    #[error("No free blocks")]
    NoFreeBlocks,
    #[error("File not found")]
    FileNotFound,
    #[error("Not a directory")]
    NotADirectory,
    #[error("Is a directory")]
    IsADirectory,
    #[error("Directory not empty")]
    DirectoryNotEmpty,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Format error: {0}")]
    Format(#[from] FormatError),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<io::Error> for FsError {
    fn from(err: io::Error) -> Self {
        FsError::Io(BlockDeviceError::Io(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::FileBackedBlockDevice;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempfile;

    async fn create_test_device(size: u64) -> FileBackedBlockDevice {
        use std::fs::OpenOptions;
        use std::os::unix::fs::FileExt;
        use tempfile::tempfile;

        // Create a temporary file with the required size
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join(format!("aegisfs_test_{}", rand::random::<u64>()));

        // Create and resize the file
        let file = std::fs::File::create(&file_path).unwrap();
        file.set_len(size).unwrap();

        // Open the file with the FileBackedBlockDevice
        FileBackedBlockDevice::open(file_path, false).await.unwrap()
    }

    #[tokio::test]
    async fn test_disk_fs_format() {
        // Create a block device for testing (16MB)
        let mut device = create_test_device(16 * 1024 * 1024).await;

        // Format the device with a 16MB filesystem
        DiskFs::format(&mut device, 16 * 1024 * 1024, Some("testfs"))
            .await
            .unwrap();

        // Read the superblock back
        let mut superblock_buf = [0u8; 4096];
        device.read_block(0, &mut superblock_buf).await.unwrap();

        // Parse the superblock
        let superblock =
            Superblock::read_from(&mut std::io::Cursor::new(&superblock_buf[..])).unwrap();

        // Verify the superblock magic number (8 bytes)
        assert_eq!(&superblock.magic, b"AEGISFS\0");
        assert_eq!(superblock.block_size, 4096);
        assert_eq!(superblock.block_count, (16 * 1024 * 1024) / 4096);

        // Check volume name (first 7 bytes should be "testfs\0\0")
        let volume_name = &superblock.volume_name[..7];
        assert_eq!(volume_name, b"testfs\0\0");

        // Verify root inode (inode 1 is the FUSE root inode)
        let device = Arc::new(device);
        let disk_fs = DiskFs::open(device).await.unwrap();
        let root_inode = disk_fs.read_inode(1).await.unwrap();
        assert_eq!(root_inode.mode, 0o40755);
        assert_eq!(root_inode.links, 2);
    }

    #[tokio::test]
    async fn test_disk_fs_format_invalid_size() {
        // Create a block device for testing (1KB)
        let mut device = create_test_device(1024).await;

        // Try to format with size smaller than block size
        let result = DiskFs::format(&mut device, 1024, None).await;
        match result {
            Err(FsError::InvalidArgument(_)) => { /* expected */ }
            _ => panic!("Expected InvalidArgument error for small device size"),
        }
    }
}
