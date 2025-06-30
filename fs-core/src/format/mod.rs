//! AegisFS on-disk format implementation

use async_trait::async_trait;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

use crate::blockdev::BlockDevice;
use crate::layout::DiskFsTrait;

/// Magic number for AegisFS filesystem
const AEGISFS_MAGIC: &[u8; 8] = b"AEGISFS\x00";
/// Current filesystem version
const FS_VERSION: u32 = 1;

/// Filesystem metadata stored at the beginning of the partition
/// On-disk inode structure
#[derive(Debug, Clone)]
pub struct Inode {
    /// File mode and type
    pub mode: u32,
    /// User ID of owner
    pub uid: u32,
    /// Group ID of owner
    pub gid: u32,
    /// Size in bytes
    pub size: u64,
    /// Last access time
    pub atime: u64,
    /// Last modification time
    pub mtime: u64,
    /// Creation time
    pub ctime: u64,
    /// Number of hard links
    pub links: u16,
    /// Number of 512-byte blocks allocated
    pub blocks: u64,
    /// File flags
    pub flags: u32,
    /// OS specific value 1
    pub osd1: [u8; 4],
    /// Pointers to data blocks
    pub block: [u64; 15],
    /// File version (for NFS)
    pub generation: u32,
    /// File ACL
    pub file_acl: u32,
    /// Directory ACL or high 32 bits of size
    pub dir_acl: u32,
    /// Fragment address
    pub faddr: u32,
    /// OS specific value 2
    pub osd2: [u8; 12],
}

/// Directory entry structure
#[derive(Debug)]
pub struct DirEntry {
    /// Inode number
    pub inode: u64,
    /// Length of this entry
    pub rec_len: u16,
    /// Length of name
    pub name_len: u8,
    /// File type
    pub file_type: u8,
    /// File name (variable length, up to 255 bytes)
    pub name: String,
}

impl DirEntry {
    /// Create a new directory entry
    pub fn new(inode: u64, name: &str) -> Self {
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u8;
        let pad_len = (8 - ((8 + name_bytes.len() + 1) % 8)) % 8; // Align to 8 bytes
        let rec_len = 8 + name_bytes.len() as u16 + 1 + pad_len as u16;

        Self {
            inode,
            rec_len,
            name_len,
            file_type: 2, // Directory
            name: name.to_string(),
        }
    }

    /// Write directory entry to writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_u64::<LittleEndian>(self.inode)?;
        writer.write_u16::<LittleEndian>(self.rec_len)?;
        writer.write_u8(self.name_len)?;
        writer.write_u8(self.file_type)?;
        writer.write_all(self.name.as_bytes())?;
        writer.write_u8(0)?; // Null terminator

        // Write padding
        let pad_len = (self.rec_len - 8 - self.name_len as u16 - 1) as usize;
        if pad_len > 0 {
            writer.write_all(&vec![0u8; pad_len])?;
        }

        Ok(())
    }
}

impl Inode {
    /// Write inode to buffer (exactly 128 bytes)
    pub fn write_to<W: Write>(&self, buf: &mut W) -> io::Result<()> {
        // Calculate total size to ensure we write exactly 128 bytes
        // mode: 4, uid: 4, gid: 4, size: 8, atime: 8, mtime: 8, ctime: 8,
        // links: 2, blocks: 8, flags: 4, osd1: 4, block[15]: 15*8=120,
        // generation: 4, file_acl: 4, dir_acl: 4, faddr: 4, osd2: 12
        // Total: 4*4 + 8*5 + 2 + 8 + 4 + 4 + 120 + 4*4 + 12 = 16 + 40 + 2 + 8 + 4 + 4 + 120 + 16 + 12 = 222 bytes

        // But we only want to write 128 bytes, so we need to adjust the structure.
        // Let's create a fixed-size buffer and write to that first.
        let mut buffer = [0u8; 128];
        let mut cursor = std::io::Cursor::new(&mut buffer[..]);

        // Write fixed-size fields (4+4+4+8+8+8+8+2+8+4+4 = 62 bytes)
        cursor.write_u32::<LittleEndian>(self.mode)?;
        cursor.write_u32::<LittleEndian>(self.uid)?;
        cursor.write_u32::<LittleEndian>(self.gid)?;
        cursor.write_u64::<LittleEndian>(self.size)?;
        cursor.write_u64::<LittleEndian>(self.atime)?;
        cursor.write_u64::<LittleEndian>(self.mtime)?;
        cursor.write_u64::<LittleEndian>(self.ctime)?;
        cursor.write_u16::<LittleEndian>(self.links)?;
        cursor.write_u64::<LittleEndian>(self.blocks)?;
        cursor.write_u32::<LittleEndian>(self.flags)?;
        cursor.write_all(&self.osd1)?;

        // Write block pointers (truncate to fit in remaining space)
        let max_blocks = (128 - 62) / 8; // (128 - 62) / 8 = 8 blocks max
        for &block in self.block.iter().take(max_blocks) {
            cursor.write_u64::<LittleEndian>(block)?;
        }

        // Write the buffer to the actual writer
        buf.write_all(&buffer)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct Superblock {
    /// Magic number (AEGISFS\x00)
    pub magic: [u8; 8],
    /// Filesystem version
    pub version: u32,
    /// Size of the filesystem in bytes
    pub size: u64,
    /// Block size in bytes
    pub block_size: u32,
    /// Number of blocks in the filesystem
    pub block_count: u64,
    /// Number of free blocks
    pub free_blocks: u64,
    /// Inode count
    pub inode_count: u64,
    /// Number of free inodes
    pub free_inodes: u64,
    /// Root inode number
    pub root_inode: u64,
    /// Timestamp of last mount
    pub last_mount: u64,
    /// Timestamp of last write
    pub last_write: u64,
    /// Filesystem UUID
    pub uuid: [u8; 16],
    /// Volume name
    pub volume_name: [u8; 64],
}

impl Default for Superblock {
    fn default() -> Self {
        let mut uuid = [0u8; 16];
        getrandom::getrandom(&mut uuid).expect("Failed to generate UUID");

        Self {
            magic: *AEGISFS_MAGIC,
            version: FS_VERSION,
            size: 0,
            block_size: 4096, // 4KB blocks
            block_count: 0,
            free_blocks: 0,
            inode_count: 0,
            free_inodes: 0,
            root_inode: 1, // FUSE root inode number
            last_mount: 0,
            last_write: 0,
            uuid,
            volume_name: [0; 64],
        }
    }
}

/// Error type for filesystem formatting operations
#[derive(Error, Debug)]
pub enum FormatError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Invalid magic number")]
    InvalidMagic,
    #[error("Unsupported filesystem version: {0}")]
    UnsupportedVersion(u32),
    #[error("Invalid filesystem size")]
    InvalidSize,
}

impl Superblock {
    /// Size of the superblock in bytes
    pub const SIZE: usize = 8 + 4 + 8 + 4 + 8 + 8 + 8 + 8 + 8 + 8 + 16 + 64; // 140 bytes

    /// Create a new superblock for a filesystem of the given size
    pub fn new(size: u64, volume_name: Option<&str>) -> io::Result<Self> {
        let block_size = 4096; // 4KB blocks
        let block_count = size / block_size as u64;

        // Estimate inodes: 1 inode per 32KB
        let inode_count = size / (32 * 1024);

        let mut sb = Superblock {
            size,
            block_size,
            block_count,
            free_blocks: block_count - 1, // Reserve space for superblock
            inode_count,
            free_inodes: inode_count - 1, // Reserve root inode
            ..Default::default()
        };

        // Set volume name if provided
        if let Some(name) = volume_name {
            let name_bytes = name.as_bytes();
            let len = name_bytes.len().min(63);
            sb.volume_name[..len].copy_from_slice(&name_bytes[..len]);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        sb.last_mount = now;
        sb.last_write = now;

        Ok(sb)
    }

    /// Write the superblock to a writer
    pub fn write_to<W: Write + Seek>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_u32::<LittleEndian>(self.version)?;
        writer.write_u64::<LittleEndian>(self.size)?;
        writer.write_u32::<LittleEndian>(self.block_size)?;
        writer.write_u64::<LittleEndian>(self.block_count)?;
        writer.write_u64::<LittleEndian>(self.free_blocks)?;
        writer.write_u64::<LittleEndian>(self.inode_count)?;
        writer.write_u64::<LittleEndian>(self.free_inodes)?;
        writer.write_u64::<LittleEndian>(self.root_inode)?;
        writer.write_u64::<LittleEndian>(self.last_mount)?;
        writer.write_u64::<LittleEndian>(self.last_write)?;
        writer.write_all(&self.uuid)?;
        writer.write_all(&self.volume_name)?;

        // Pad to block size
        let pos = writer.stream_position()?;
        let padding =
            (self.block_size as u64 - (pos % self.block_size as u64)) % self.block_size as u64;
        writer.write_all(&vec![0u8; padding as usize])?;

        Ok(())
    }

    /// Read a superblock from a reader
    pub fn read_from<R: Read + Seek>(reader: &mut R) -> Result<Self, FormatError> {
        let mut magic = [0u8; 8];
        reader.read_exact(&mut magic)?;

        if &magic != AEGISFS_MAGIC {
            return Err(FormatError::InvalidMagic);
        }

        let version = reader.read_u32::<LittleEndian>()?;
        if version != FS_VERSION {
            return Err(FormatError::UnsupportedVersion(version));
        }

        let size = reader.read_u64::<LittleEndian>()?;
        let block_size = reader.read_u32::<LittleEndian>()?;
        let block_count = reader.read_u64::<LittleEndian>()?;
        let free_blocks = reader.read_u64::<LittleEndian>()?;
        let inode_count = reader.read_u64::<LittleEndian>()?;
        let free_inodes = reader.read_u64::<LittleEndian>()?;
        let root_inode = reader.read_u64::<LittleEndian>()?;
        let last_mount = reader.read_u64::<LittleEndian>()?;
        let last_write = reader.read_u64::<LittleEndian>()?;

        let mut uuid = [0u8; 16];
        reader.read_exact(&mut uuid)?;

        let mut volume_name = [0u8; 64];
        reader.read_exact(&mut volume_name)?;

        Ok(Self {
            magic,
            version,
            size,
            block_size,
            block_count,
            free_blocks,
            inode_count,
            free_inodes,
            root_inode,
            last_mount,
            last_write,
            uuid,
            volume_name,
        })
    }
}

/// Get the size of a block device using platform-specific methods
fn get_block_device_size<P: AsRef<Path>>(device_path: P) -> io::Result<u64> {
    #[cfg(unix)]
    {
        get_block_device_size_unix(device_path)
    }
    #[cfg(windows)]
    {
        get_block_device_size_windows(device_path)
    }
}

/// Unix-specific block device size detection
#[cfg(unix)]
fn get_block_device_size_unix<P: AsRef<Path>>(device_path: P) -> io::Result<u64> {
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    let file = File::open(device_path)?;
    let fd = file.as_raw_fd();

    // Use ioctl to get block device size
    // BLKGETSIZE64 = 0x80081272 on Linux
    const BLKGETSIZE64: libc::c_ulong = 0x80081272;

    let mut size: u64 = 0;
    let result = unsafe { libc::ioctl(fd, BLKGETSIZE64, &mut size as *mut u64) };

    if result == -1 {
        return Err(io::Error::last_os_error());
    }

    Ok(size)
}

/// Windows-specific block device size detection
#[cfg(windows)]
fn get_block_device_size_windows<P: AsRef<Path>>(device_path: P) -> io::Result<u64> {
    use std::fs::File;
    use std::os::windows::io::AsRawHandle;

    let metadata = std::fs::metadata(&device_path)?;
    
    // For regular files, just return the file size
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    // For block devices on Windows, we need to use different APIs
    let file = File::open(device_path)?;
    let handle = file.as_raw_handle();

    unsafe {
        use winapi::um::fileapi::GetFileSizeEx;
        use winapi::um::winnt::LARGE_INTEGER;

        let mut size: LARGE_INTEGER = std::mem::zeroed();
        
        if GetFileSizeEx(handle as _, &mut size) != 0 {
            Ok(*size.QuadPart() as u64)
        } else {
            // Fallback to regular file size
            Ok(metadata.len())
        }
    }
}

/// Format a block device with the AegisFS filesystem
pub async fn format_device<P: AsRef<Path>>(
    device_path: P,
    size_gb: u64,
    volume_name: Option<&str>,
) -> Result<(), FormatError> {
    use crate::blockdev::BlockDevice;
    use crate::layout::DiskFs;
    use std::sync::Arc;

    let mut size = size_gb * 1024 * 1024 * 1024; // Convert GB to bytes

    // Check if the target path exists and is a block device
    let path = device_path.as_ref();
    let metadata = std::fs::metadata(path).map_err(FormatError::Io)?;

    // For block devices, get the actual device size using platform-specific methods
    let is_block_device = {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            metadata.file_type().is_block_device()
        }
        #[cfg(windows)]
        {
            // On Windows, we typically work with files or volumes
            // Check if it's a volume (like \\.\C:) or a regular file
            let path_str = path.to_string_lossy();
            path_str.starts_with(r"\\.\") || !metadata.is_file()
        }
    };

    if is_block_device {
        let device_size = get_block_device_size(path)?;
        const TOLERANCE: u64 = 1024 * 1024; // 1MB tolerance

        log::info!("Block device size: {} bytes", device_size);

        if device_size < size && (size - device_size) > TOLERANCE {
            return Err(FormatError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Device size ({} bytes) is too small for requested size ({} bytes). Difference exceeds tolerance of {} bytes",
                    device_size, size, TOLERANCE
                )
            )));
        } else if device_size < size {
            // If we get here, the device is slightly smaller than requested but within tolerance
            // We'll use the actual device size instead of the requested size
            log::info!(
                "Using device size ({} bytes) which is slightly smaller than requested ({} bytes) but within tolerance",
                device_size, size
            );

            // Update the size to match the actual device size
            size = device_size;
        }

        // For block devices, always use the actual device size
        size = device_size;
        log::info!(
            "Using full block device size: {} bytes ({:.2} GiB)",
            size,
            size as f64 / (1024.0 * 1024.0 * 1024.0)
        );
    }

    // Create or open the device/file
    use crate::blockdev::FileBackedBlockDevice;
    let mut device = if path.exists() {
        FileBackedBlockDevice::open(device_path, false).await
    } else {
        FileBackedBlockDevice::create(device_path, size).await
    }
    .map_err(|e| {
        FormatError::Io(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to open/create device: {}", e),
        ))
    })?;

    // Format the device using DiskFs implementation
    let format_result = DiskFs::format(&mut device, size, volume_name).await;

    // Convert FsError to FormatError
    format_result.map_err(|e| {
        FormatError::Io(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to format device: {:?}", e),
        ))
    })?;

    log::info!(
        "Successfully formatted device with {}GB filesystem",
        size_gb
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_superblock_roundtrip() {
        let mut buffer = Cursor::new(Vec::new());

        // Create a test superblock with a volume name
        let volume_name = "testvol";
        let mut sb = Superblock::new(1024 * 1024 * 1024, Some(volume_name)).unwrap();

        // Write superblock to buffer
        sb.write_to(&mut buffer).unwrap();

        // Reset cursor to beginning for reading
        buffer.set_position(0);

        // Read it back
        let sb2 = Superblock::read_from(&mut buffer).unwrap();

        // Compare the important fields
        assert_eq!(sb.magic, sb2.magic, "Magic numbers should match");
        assert_eq!(sb.version, sb2.version, "Versions should match");
        assert_eq!(sb.size, sb2.size, "Sizes should match");
        assert_eq!(sb.block_size, sb2.block_size, "Block sizes should match");

        // Compare volume names as strings, handling null termination
        let vol1 = std::str::from_utf8(&sb.volume_name)
            .unwrap()
            .trim_end_matches('\0');
        let vol2 = std::str::from_utf8(&sb2.volume_name)
            .unwrap()
            .trim_end_matches('\0');

        assert_eq!(vol1, volume_name, "Original volume name should match");
        assert_eq!(vol2, volume_name, "Round-tripped volume name should match");
    }
}
