//! Block device I/O operations for AegisFS

mod blockdev_trait;

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::Mutex;

// Re-export the block device trait and related types
pub use self::blockdev_trait::{BlockDevice, BlockDeviceError, Result, BLOCK_SIZE};

/// A block device that is backed by a file on the filesystem
#[derive(Debug)]
pub struct FileBackedBlockDevice {
    file: Mutex<Option<File>>,
    path: PathBuf,
    size: u64,
    block_count: u64,
    read_only: bool,
}

impl FileBackedBlockDevice {
    /// Create a new file-backed block device
    pub async fn create(path: impl AsRef<Path>, size: u64) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)
            .await?;

        // Set file length
        file.set_len(size).await?;

        let block_count = size / BLOCK_SIZE as u64;

        Ok(Self {
            file: Mutex::new(Some(file)),
            path,
            size,
            block_count,
            read_only: false,
        })
    }

    /// Get the size of a block device using platform-specific methods
    fn get_block_device_size(path: &Path) -> Result<u64> {
        #[cfg(unix)]
        {
            Self::get_block_device_size_unix(path)
        }
        #[cfg(windows)]
        {
            Self::get_block_device_size_windows(path)
        }
    }

    /// Unix-specific block device size detection
    #[cfg(unix)]
    fn get_block_device_size_unix(path: &Path) -> Result<u64> {
        use std::fs::File as StdFile;
        use std::os::unix::fs::FileTypeExt;
        use std::os::unix::io::AsRawFd;

        // Check if it's a block device first
        let metadata = std::fs::metadata(path)?;
        if !metadata.file_type().is_block_device() {
            return Ok(metadata.len());
        }

        let file = StdFile::open(path)?;
        let fd = file.as_raw_fd();

        // Use ioctl to get block device size
        // BLKGETSIZE64 = 0x80081272 on Linux
        const BLKGETSIZE64: libc::c_ulong = 0x80081272;

        let mut size: u64 = 0;
        let result = unsafe { libc::ioctl(fd, BLKGETSIZE64, &mut size as *mut u64) };

        if result == -1 {
            return Err(BlockDeviceError::Io(std::io::Error::last_os_error()));
        }

        Ok(size)
    }

    /// Windows-specific block device size detection
    #[cfg(windows)]
    fn get_block_device_size_windows(path: &Path) -> Result<u64> {
        use std::fs::File as StdFile;
        use std::os::windows::io::AsRawHandle;
        use winapi::um::fileapi::GetFileSizeEx;
        use winapi::um::winnt::LARGE_INTEGER;

        let metadata = std::fs::metadata(path)?;
        
        // For regular files, just return the file size
        if metadata.is_file() {
            return Ok(metadata.len());
        }

        // For block devices on Windows, we need to use different APIs
        let file = StdFile::open(path)?;
        let handle = file.as_raw_handle();

        let mut size: LARGE_INTEGER = unsafe { std::mem::zeroed() };
        
        unsafe {
            if GetFileSizeEx(handle as _, &mut size) != 0 {
                Ok(*size.QuadPart() as u64)
            } else {
                // Fallback to regular file size
                Ok(metadata.len())
            }
        }
    }

    /// Open an existing file-backed block device
    pub async fn open(path: impl AsRef<Path>, read_only: bool) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(!read_only)
            .open(&path)
            .await?;

        // Get the actual size (handles both files and block devices)
        let size = Self::get_block_device_size(&path)?;
        let block_count = size / BLOCK_SIZE as u64;

        Ok(Self {
            file: Mutex::new(Some(file)),
            path,
            size,
            block_count,
            read_only,
        })
    }

    /// Get the total size of the device in bytes
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get the number of blocks in the device
    pub fn block_count(&self) -> u64 {
        self.block_count
    }

    /// Check if the device is read-only
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
}

#[async_trait]
impl BlockDevice for FileBackedBlockDevice {
    async fn read_block(&self, block_num: u64, buf: &mut [u8]) -> Result<()> {
        if block_num >= self.block_count {
            return Err(BlockDeviceError::InvalidBlockNumber(block_num));
        }

        if buf.len() != BLOCK_SIZE {
            return Err(BlockDeviceError::InvalidBlockSize(buf.len()));
        }

        let offset = block_num * BLOCK_SIZE as u64;
        let mut file_guard = self.file.lock().await;

        if let Some(file) = &mut *file_guard {
            file.seek(SeekFrom::Start(offset)).await?;
            file.read_exact(buf).await?;
            Ok(())
        } else {
            Err(BlockDeviceError::DeviceClosed)
        }
    }

    async fn write_block(&self, block_num: u64, data: &[u8]) -> Result<()> {
        if self.read_only {
            return Err(BlockDeviceError::ReadOnly);
        }

        if block_num >= self.block_count {
            return Err(BlockDeviceError::InvalidBlockNumber(block_num));
        }

        if data.len() != BLOCK_SIZE {
            return Err(BlockDeviceError::InvalidBlockSize(data.len()));
        }

        let offset = block_num * BLOCK_SIZE as u64;
        let mut file_guard = self.file.lock().await;

        if let Some(file) = &mut *file_guard {
            file.seek(SeekFrom::Start(offset)).await?;
            file.write_all(data).await?;
            file.flush().await?;
            Ok(())
        } else {
            Err(BlockDeviceError::DeviceClosed)
        }
    }

    fn block_count(&self) -> u64 {
        self.block_count
    }

    async fn sync(&self) -> Result<()> {
        let mut file_guard = self.file.lock().await;

        if let Some(file) = &mut *file_guard {
            file.sync_all().await?;
            Ok(())
        } else {
            Err(BlockDeviceError::DeviceClosed)
        }
    }

    async fn close(&mut self) -> Result<()> {
        let mut file_guard = self.file.lock().await;

        if file_guard.take().is_some() {
            Ok(())
        } else {
            Err(BlockDeviceError::DeviceClosed)
        }
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn block_size(&self) -> usize {
        BLOCK_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_block_device_operations() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_device.bin");

        // Create a new block device
        let device = FileBackedBlockDevice::create(&file_path, 4096 * 8)
            .await
            .unwrap();

        // Test writing and reading a block
        let test_data = [0xAAu8; 4096];
        device.write_block(0, &test_data).await.unwrap();

        let mut read_buf = [0u8; 4096];
        device.read_block(0, &mut read_buf).await.unwrap();
        assert_eq!(test_data, read_buf);

        // Test reading/writing multiple blocks
        for i in 1..8 {
            let data = [i as u8; 4096];
            device.write_block(i, &data).await.unwrap();

            let mut read_data = [0u8; 4096];
            device.read_block(i, &mut read_data).await.unwrap();
            assert_eq!(data, read_data);
        }
    }

    #[tokio::test]
    async fn test_read_only() {
        // Create a temporary directory for testing
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_read_only.bin");

        // Create a new block device
        let device = FileBackedBlockDevice::create(&file_path, 4096)
            .await
            .unwrap();

        // Write some data
        let test_data = [0x55u8; 4096];
        device.write_block(0, &test_data).await.unwrap();

        // Reopen as read-only
        let read_only_device = FileBackedBlockDevice::open(&file_path, true).await.unwrap();

        // Verify we can read
        let mut read_buf = [0u8; 4096];
        read_only_device.read_block(0, &mut read_buf).await.unwrap();
        assert_eq!(test_data, read_buf);

        // Verify we can't write
        let write_result = read_only_device.write_block(0, &[0u8; 4096]).await;
        assert!(matches!(write_result, Err(BlockDeviceError::ReadOnly)));
    }
}
