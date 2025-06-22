//! AegisFS on-disk format implementation

use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::Path;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use thiserror::Error;

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
            root_inode: 2, // Inode 2 is traditionally root
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
        let padding = (self.block_size as u64 - (pos % self.block_size as u64)) % self.block_size as u64;
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

/// Format a block device or file as an AegisFS filesystem
pub fn format_device<P: AsRef<Path>>(
    device_path: P,
    size_gb: u64,
    volume_name: Option<&str>,
) -> Result<(), FormatError> {
    let size = size_gb * 1024 * 1024 * 1024; // Convert GB to bytes
    
    // Open the device/file in read-write mode
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(device_path.as_ref())
        .map_err(|e| FormatError::Io(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to open device {:?}: {}", device_path.as_ref(), e)
        )))?;
    
    // Get the device path as a string for blockdev command
    let device_path = device_path.as_ref().to_string_lossy().to_string();
    
    // Get device size using blockdev command for accurate size
    let output = std::process::Command::new("blockdev")
        .args(["--getsize64", &device_path])
        .output()
        .map_err(|e| FormatError::Io(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to get device size using blockdev: {}", e)
        )))?;
        
    if !output.status.success() {
        return Err(FormatError::Io(io::Error::new(
            io::ErrorKind::Other,
            format!("blockdev command failed: {}", String::from_utf8_lossy(&output.stderr))
        )));
    }
    
    let device_size = String::from_utf8(output.stdout)
        .map_err(|_| FormatError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "Failed to parse blockdev output as UTF-8"
        )))?
        .trim()
        .parse::<u64>()
        .map_err(|e| FormatError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Failed to parse device size: {}", e)
        )))?;
    
    // Only set length if it's a regular file, not a block device
    let metadata = file.metadata().map_err(|e| FormatError::Io(io::Error::new(
        io::ErrorKind::Other,
        format!("Failed to get device metadata: {}", e)
    )))?;
    
    if metadata.file_type().is_file() {
        file.set_len(size).map_err(|e| FormatError::Io(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to set file size: {}", e)
        )))?;
    } else if device_size < size {
        // For block devices, verify the device is large enough, with a small tolerance (1MB)
        const TOLERANCE: u64 = 1024 * 1024; // 1MB tolerance
        
        if size - device_size > TOLERANCE {
            return Err(FormatError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Device size ({} bytes) is too small for requested size ({} bytes). Difference exceeds tolerance of {} bytes",
                    device_size, size, TOLERANCE
                )
            )));
        }
        
        // If we get here, the device is slightly smaller than requested but within tolerance
        // We'll use the actual device size instead of the requested size
        log::info!(
            "Using device size ({} bytes) which is slightly smaller than requested ({} bytes) but within tolerance",
            device_size, size
        );
        
        // Update the size to match the actual device size
        let size = device_size;
    }
    
    // Create and write superblock
    let mut superblock = Superblock::new(size, volume_name)?;
    superblock.write_to(&mut file)?;
    
    // Calculate block counts and positions
    let block_size = superblock.block_size as u64;
    let blocks_per_group = 8 * block_size; // 1 bit per block
    
    // Block allocation bitmap (1 bit per block, rounded up to nearest block)
    let bitmap_blocks = (superblock.block_count + blocks_per_group - 1) / blocks_per_group;
    let inode_table_blocks = (superblock.inode_count * 128 + block_size - 1) / block_size; // 128 bytes per inode
    
    // Initialize block allocation bitmap
    file.seek(SeekFrom::Start(block_size))?; // Skip superblock
    
    // Mark superblock and bitmaps as used
    let mut used_blocks = 1 + bitmap_blocks; // Superblock + bitmaps
    let mut bitmap = vec![0u8; (bitmap_blocks * block_size) as usize];
    
    // Mark used blocks
    for i in 0..used_blocks {
        let byte = (i / 8) as usize;
        let bit = i % 8;
        if byte < bitmap.len() {
            bitmap[byte] |= 1 << bit;
        }
    }
    
    // Write bitmap with better error context
    file.write_all(&bitmap).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to write bitmap at position {}: {}", 
                   block_size, e)
        ))
    })?;
    
    // Initialize inode table
    let inode_table_start = block_size * (1 + bitmap_blocks);
    file.seek(SeekFrom::Start(inode_table_start))?;
    
    // Create root inode (inode 2)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let root_inode = Inode {
        mode: 0o40755, // Directory with 755 permissions
        uid: 0,        // root
        gid: 0,        // root
        size: block_size, // Minimum size for a directory
        atime: now,
        mtime: now,
        ctime: now,
        links: 2,      // . and ..
        blocks: 1,      // 1 block allocated
        flags: 0,
        osd1: [0; 4],
        block: [0; 15], // Will be filled with block pointers
        generation: 0,
        file_acl: 0,
        dir_acl: 0,
        faddr: 0,
        osd2: [0; 12],
    };
    
    // Write root inode (inode 2)
    let mut inode_buf = vec![0u8; 128]; // 128 bytes per inode
    root_inode.write_to(&mut &mut inode_buf[..]).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to serialize root inode: {}", e)
        ))
    })?;
    
    file.write_all(&inode_buf).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to write root inode at position {}: {}", 
                   inode_table_start, e)
        ))
    })?;
    
    // Initialize root directory
    let root_dir_block = inode_table_start + inode_table_blocks * block_size;
    file.seek(SeekFrom::Start(root_dir_block)).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to seek to root directory block at position {}: {}", 
                   root_dir_block, e)
        ))
    })?;
    
    // Write . and .. directory entries
    let dot = DirEntry::new(2, ".");
    let dotdot = DirEntry::new(2, "..");
    
    let mut dir_block = vec![0u8; block_size as usize];
    let mut cursor = std::io::Cursor::new(&mut dir_block[..]);
    
    // Write . entry
    dot.write_to(&mut cursor).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to write '.' directory entry: {}", e)
        ))
    })?;
    
    // Write .. entry
    dotdot.write_to(&mut cursor).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to write '..' directory entry: {}", e)
        ))
    })?;
    
    // Write directory block to disk
    file.write_all(&dir_block).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to write root directory block at position {}: {}", 
                   root_dir_block, e)
        ))
    })?;
    
    // Update superblock with used blocks
    superblock.free_blocks = superblock.block_count - used_blocks - inode_table_blocks - 1; // -1 for root dir block
    superblock.free_inodes = superblock.inode_count - 1; // -1 for root inode
    
    // Write updated superblock
    file.seek(SeekFrom::Start(0)).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to seek to superblock at position 0: {}", e)
        ))
    })?;
    
    superblock.write_to(&mut file).map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to write updated superblock: {}", e)
        ))
    })?;
    
    // Flush all changes to disk
    file.sync_all().map_err(|e| {
        FormatError::Io(io::Error::new(
            e.kind(),
            format!("Failed to sync filesystem changes to disk: {}", e)
        ))
    })?;
    
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
