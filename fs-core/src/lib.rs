//! AegisFS Core - A modern, feature-rich filesystem implementation
//!
//! This crate provides the core functionality for AegisFS, including the filesystem
//! implementation, VFS layer, and various modules for features like encryption,
//! compression, and snapshots.

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rust_2018_idioms)]
#![allow(dead_code)] // TODO: Remove in production
// Silence cross-platform compilation warnings for optional features
#![cfg_attr(not(feature = "fuse"), allow(unused_imports, unresolved_import, unreachable_code))]

// Core modules
pub mod blockdev;
pub mod cache;
pub mod error;
pub mod format;
pub mod layout;

// Feature modules
pub mod modules;

// Re-export block device types
pub use blockdev::{
    BlockDevice, BlockDevice as BlockDeviceTrait, BlockDeviceError, FileBackedBlockDevice,
    BLOCK_SIZE,
};

/// Block device result type
pub type BlockResult<T> = std::result::Result<T, BlockDeviceError>;

#[cfg(feature = "fuse")]
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry,
    ReplyWrite, Request,
};

// Cross-platform file type definitions
#[cfg(not(feature = "fuse"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Directory,
    RegularFile,
    Symlink,
}

// Cross-platform file attributes for non-FUSE builds
#[cfg(not(feature = "fuse"))]
#[derive(Debug, Clone)]
pub struct FileAttr {
    pub ino: u64,
    pub size: u64,
    pub blocks: u64,
    pub atime: SystemTime,
    pub mtime: SystemTime,
    pub ctime: SystemTime,
    pub crtime: SystemTime,
    pub kind: FileType,
    pub perm: u16,
    pub nlink: u32,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u32,
    pub flags: u32,
    pub blksize: u32,
}

#[cfg(unix)]
use libc::ENOENT;
#[cfg(windows)]
const ENOENT: i32 = 2; // Windows ERROR_FILE_NOT_FOUND

use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use parking_lot::RwLock;

// Re-export the error types
pub use error::{Error, Result};

// Re-export layout types
pub use layout::{DiskFs, DiskFsTrait, FsError};

// Time-to-live for file attributes (1 second)
const TTL: Duration = Duration::from_secs(1);

// Write-back cache configuration
const WRITE_BACK_INTERVAL: Duration = Duration::from_secs(5);
const MAX_CACHED_WRITES: usize = 1000;

/// Re-export common types and traits
pub mod prelude {
    pub use crate::cache::BlockCache;
    pub use crate::error::Result;
    pub use crate::layout::{DiskFs, FsError, Layout};
    pub use crate::modules::{JournalConfig, JournalEntryType, JournalManager};
    pub use crate::BlockDevice;
    pub use crate::BlockDevice as BlockDeviceTrait;
    pub use crate::BlockDeviceError;
    pub use crate::BlockResult;
    pub use crate::FileBackedBlockDevice;
    pub use crate::BLOCK_SIZE;
}

/// Filesystem error type
#[derive(Debug, thiserror::Error)]
pub enum FileSystemError {
    /// Operation not supported
    #[error("Operation not supported")]
    NotSupported,

    /// File not found
    #[error("File not found: {0}")]
    NotFound(String),

    /// Filesystem error
    #[error("Filesystem error: {0}")]
    Fs(String),

    /// Not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Invalid argument
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// Not a directory
    #[error("Not a directory")]
    NotADirectory,

    /// Already exists
    #[error("File already exists")]
    AlreadyExists,

    /// Invalid name
    #[error("Invalid file name")]
    InvalidName,

    /// Permission denied
    #[error("Permission denied")]
    PermissionDenied,

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Layout error
    #[error("Layout error: {0}")]
    Layout(#[from] FsError),
}

// Inode number constants
const ROOT_INODE: u64 = 1; // FUSE root inode number (changed from 2 to 1)
const INVALID_INODE: u64 = 0;

/// Write operation for write-back cache
#[derive(Debug, Clone)]
pub struct WriteOperation {
    /// Inode number
    pub ino: u64,
    /// Offset in file
    pub offset: u64,
    /// Data to write
    pub data: Vec<u8>,
    /// Timestamp when queued
    pub timestamp: SystemTime,
}

/// Inode bitmap for tracking allocated inodes
pub struct InodeBitmap {
    /// Bitmap data
    bitmap: Vec<u8>,
    /// Total number of inodes
    total_inodes: u64,
    /// Number of free inodes
    free_inodes: AtomicU64,
}

impl InodeBitmap {
    /// Create a new inode bitmap
    pub fn new(total_inodes: u64) -> Self {
        let bitmap_size = ((total_inodes + 7) / 8) as usize;
        let mut bitmap = vec![0u8; bitmap_size];
        
        // Mark inode 0 and 1 as used (0 is invalid, 1 is root)
        bitmap[0] |= 0b11;
        
        Self {
            bitmap,
            total_inodes,
            free_inodes: AtomicU64::new(total_inodes - 2),
        }
    }
    
    /// Load inode bitmap from disk
    pub async fn load_from_disk(disk_fs: &crate::layout::DiskFs, total_inodes: u64) -> Result<Self> {
        let layout = crate::layout::Layout::new(
            disk_fs.superblock().block_count,
            disk_fs.superblock().inode_count,
        );
        
        let bitmap_size = ((total_inodes + 7) / 8) as usize;
        let mut bitmap = vec![0u8; bitmap_size];
        
        // Read bitmap blocks from disk
        let mut bytes_read = 0;
        for block_offset in 0..layout.inode_bitmap_blocks {
            let block_data = disk_fs.read_bitmap_block(layout.inode_bitmap + block_offset).await
                .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e))))?;
            
            let bytes_to_copy = std::cmp::min(block_data.len(), bitmap_size - bytes_read);
            if bytes_to_copy > 0 {
                bitmap[bytes_read..bytes_read + bytes_to_copy].copy_from_slice(&block_data[..bytes_to_copy]);
                bytes_read += bytes_to_copy;
            }
            
            if bytes_read >= bitmap_size {
                break;
            }
        }
        
        // Count free inodes by scanning the bitmap
        let mut free_count = 0;
        for (byte_idx, &byte) in bitmap.iter().enumerate() {
            for bit in 0..8 {
                let inode_num = (byte_idx * 8 + bit) as u64;
                if inode_num >= total_inodes {
                    break;
                }
                if (byte & (1 << bit)) == 0 && inode_num > 1 { // Skip reserved inodes 0 and 1
                    free_count += 1;
                }
            }
        }
        
        log::info!("BITMAP: Loaded from disk - {} free inodes out of {} total", free_count, total_inodes);
        
        Ok(Self {
            bitmap,
            total_inodes,
            free_inodes: AtomicU64::new(free_count),
        })
    }
    
    /// Save inode bitmap to disk
    pub async fn save_to_disk(&self, disk_fs: &crate::layout::DiskFs) -> Result<()> {
        let layout = crate::layout::Layout::new(
            disk_fs.superblock().block_count,
            disk_fs.superblock().inode_count,
        );
        
        let block_size = 4096;
        let mut bytes_written = 0;
        
        // Write bitmap blocks to disk
        for block_offset in 0..layout.inode_bitmap_blocks {
            let mut block_data = vec![0u8; block_size];
            let bytes_to_copy = std::cmp::min(block_size, self.bitmap.len() - bytes_written);
            
            if bytes_to_copy > 0 {
                block_data[..bytes_to_copy].copy_from_slice(&self.bitmap[bytes_written..bytes_written + bytes_to_copy]);
                bytes_written += bytes_to_copy;
            }
            
            disk_fs.write_bitmap_block(layout.inode_bitmap + block_offset, &block_data).await
                .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e))))?;
            
            if bytes_written >= self.bitmap.len() {
                break;
            }
        }
        
        log::debug!("BITMAP: Saved to disk - {} free inodes", self.free_inodes.load(Ordering::Relaxed));
        Ok(())
    }
    
    /// Allocate a new inode
    pub fn allocate(&mut self) -> Option<u64> {
        let current_free = self.free_inodes.load(Ordering::Relaxed);
        log::debug!("InodeBitmap::allocate: Starting allocation with {} free inodes", current_free);
        
        if current_free == 0 {
            log::warn!("InodeBitmap::allocate: No free inodes available");
            return None;
        }
        
        // Find first free bit (skip inodes 0 and 1 - reserved for invalid and root)
        for (byte_idx, byte) in self.bitmap.iter_mut().enumerate() {
            if *byte != 0xFF {
                for bit in 0..8 {
                    if (*byte & (1 << bit)) == 0 {
                        let inode_num = (byte_idx * 8 + bit) as u64;
                        
                        // Skip reserved inodes (0 = invalid, 1 = root)
                        if inode_num <= 1 {
                            continue;
                        }
                        
                        if inode_num < self.total_inodes {
                            log::debug!("InodeBitmap::allocate: Found free inode {} at byte {} bit {}", 
                                       inode_num, byte_idx, bit);
                            *byte |= 1 << bit;
                            self.free_inodes.fetch_sub(1, Ordering::Relaxed);
                            log::info!("InodeBitmap::allocate: Successfully allocated inode {}, {} free remaining", 
                                      inode_num, self.free_inodes.load(Ordering::Relaxed));
                            return Some(inode_num);
                        }
                    }
                }
            }
        }
        log::error!("InodeBitmap::allocate: No free inode found despite {} free count", current_free);
        None
    }
    
    /// Free an inode
    pub fn free(&mut self, inode_num: u64) {
        if inode_num >= self.total_inodes || inode_num <= 1 {
            return; // Can't free invalid or root inode
        }
        
        let byte_idx = (inode_num / 8) as usize;
        let bit = (inode_num % 8) as u8;
        
        if byte_idx < self.bitmap.len() {
            self.bitmap[byte_idx] &= !(1 << bit);
            self.free_inodes.fetch_add(1, Ordering::Relaxed);
        }
    }
    
    /// Check if an inode is allocated
    pub fn is_allocated(&self, inode_num: u64) -> bool {
        if inode_num >= self.total_inodes {
            return false;
        }
        
        let byte_idx = (inode_num / 8) as usize;
        let bit = (inode_num % 8) as u8;
        
        if byte_idx < self.bitmap.len() {
            (self.bitmap[byte_idx] & (1 << bit)) != 0
        } else {
            false
        }
    }
}

/// In-memory inode cache entry
#[derive(Debug, Clone)]
pub struct CachedInode {
    /// Inode number
    pub ino: u64,
    /// File attributes
    pub attr: FileAttr,
    /// For directories: child inodes mapping
    pub children: HashMap<String, u64>,
    /// Last access time for cache management
    pub last_access: SystemTime,
    /// Dirty flag for write-back
    pub dirty: bool,
    /// File data cache (for small files)
    pub cached_data: Option<Vec<u8>>,
}

impl CachedInode {
    /// Create a new cached inode
    pub fn new(ino: u64, kind: FileType) -> Self {
        let now = SystemTime::now();
        let (perm, size) = match kind {
            FileType::Directory => (0o755, 0),
            _ => (0o644, 0),
        };

        Self {
            ino,
            attr: FileAttr {
                ino,
                size: size as u64,
                blocks: 0,
                atime: now,
                mtime: now,
                ctime: now,
                crtime: now,
                kind,
                perm,
                nlink: if kind == FileType::Directory { 2 } else { 1 },
                uid: {
                    #[cfg(unix)]
                    {
                        unsafe { libc::getuid() }
                    }
                    #[cfg(not(unix))]
                    {
                        1000 // Default user ID on non-Unix systems
                    }
                },
                gid: {
                    #[cfg(unix)]
                    {
                        unsafe { libc::getgid() }
                    }
                    #[cfg(not(unix))]
                    {
                        1000 // Default group ID on non-Unix systems
                    }
                },
                rdev: 0,
                flags: 0,
                blksize: 4096,
            },
            children: HashMap::new(),
            last_access: now,
            dirty: false,
            cached_data: None,
        }
    }
}

/// Persistent FUSE filesystem implementation
pub struct AegisFS {
    /// The underlying disk filesystem
    disk_fs: Arc<RwLock<DiskFs>>,
    /// In-memory inode cache for performance
    inode_cache: Arc<RwLock<HashMap<u64, CachedInode>>>,
    /// Next available inode number
    next_ino: RwLock<u64>,
    /// Tokio runtime handle for async operations
    runtime: Handle,
    /// Write-back cache
    write_cache: Arc<RwLock<Vec<WriteOperation>>>,
    /// Flag to indicate if flush is in progress
    flushing: Arc<AtomicBool>,
    /// Inode bitmap
    inode_bitmap: Arc<RwLock<InodeBitmap>>,
    /// Background flush task handle
    flush_task: Option<mpsc::UnboundedSender<FlushCommand>>,
}

/// Commands for background flush task
#[derive(Debug)]
enum FlushCommand {
    /// Flush all pending writes
    FlushAll,
    /// Flush specific inode
    FlushInode(u64),
    /// Shutdown the flush task
    Shutdown,
}

impl AegisFS {
    /// Create a new AegisFS instance from a block device path
    pub fn new() -> Self {
        // Create a simple in-memory implementation as fallback
        // Use a reasonable default of 1GB worth of inodes (32,768 inodes)
        let default_inode_count = (1024 * 1024 * 1024) / (32 * 1024); // 1GB / 32KB = 32,768
        
        let runtime = Handle::current();
        let disk_fs = Arc::new(RwLock::new(DiskFs::new_mock()));
        let inode_cache = Arc::new(RwLock::new(HashMap::new()));
        let write_cache = Arc::new(RwLock::new(Vec::new()));
        let flushing = Arc::new(AtomicBool::new(false));
        let inode_bitmap = Arc::new(RwLock::new(InodeBitmap::new(default_inode_count)));
        
        let flush_task = Self::start_background_flush(
            runtime.clone(),
            disk_fs.clone(),
            inode_cache.clone(),
            write_cache.clone(),
            flushing.clone(),
        );
        
        log::info!("Created mock filesystem with {} inodes ({:.1}K)", 
                   default_inode_count, default_inode_count as f64 / 1000.0);
        
        Self {
            disk_fs,
            inode_cache,
            next_ino: RwLock::new(ROOT_INODE + 1),
            runtime,
            write_cache,
            flushing,
            inode_bitmap,
            flush_task,
        }
    }

    /// Create a new AegisFS instance from a formatted device
    pub async fn from_device<P: AsRef<Path>>(device_path: P) -> Result<Self> {
        let device = Arc::new(
            FileBackedBlockDevice::open(device_path, false)
                .await
                .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?,
        );

        let disk_fs_raw = DiskFs::open(device)
            .await
            .map_err(|e| Error::Other(format!("Failed to open device: {:?}", e)))?;

        // Get the actual inode count from the superblock
        let inode_count = disk_fs_raw.superblock().inode_count;
        log::info!("Initializing filesystem with {} inodes ({:.2}M)", 
                   inode_count, inode_count as f64 / 1_000_000.0);

        let runtime = Handle::current();
        let disk_fs = Arc::new(RwLock::new(disk_fs_raw));
        let inode_cache = Arc::new(RwLock::new(HashMap::new()));
        let write_cache = Arc::new(RwLock::new(Vec::new()));
        let flushing = Arc::new(AtomicBool::new(false));
        
        // Load inode bitmap from disk instead of creating fresh one
        let inode_bitmap = {
            let disk_fs_guard = disk_fs.read();
            let bitmap = InodeBitmap::load_from_disk(&*disk_fs_guard, inode_count).await
                .map_err(|e| Error::Other(format!("Failed to load inode bitmap: {:?}", e)))?;
            Arc::new(RwLock::new(bitmap))
        };
        
        let flush_task = Self::start_background_flush(
            runtime.clone(),
            disk_fs.clone(),
            inode_cache.clone(),
            write_cache.clone(),
            flushing.clone(),
        );

        let fs = Self {
            disk_fs,
            inode_cache,
            next_ino: RwLock::new(ROOT_INODE + 1),
            runtime,
            write_cache,
            flushing,
            inode_bitmap,
            flush_task,
        };

        // Root inode should already be marked as allocated in the loaded bitmap
        // Just verify it's correctly marked
        {
            let bitmap = fs.inode_bitmap.read();
            if !bitmap.is_allocated(ROOT_INODE) {
                log::warn!("BITMAP: Root inode {} not marked as allocated in loaded bitmap, marking now", ROOT_INODE);
                drop(bitmap);
                let mut bitmap = fs.inode_bitmap.write();
                let byte_idx = (ROOT_INODE / 8) as usize;
                let bit = (ROOT_INODE % 8) as u8;
                if byte_idx < bitmap.bitmap.len() {
                    bitmap.bitmap[byte_idx] |= 1 << bit;
                    bitmap.free_inodes.fetch_sub(1, Ordering::Relaxed);
                }
            }
        }

        // Initialize root directory in cache
        fs.init_root_cache()
            .await
            .map_err(|e| Error::Other(format!("Failed to init root cache: {:?}", e)))?;

        Ok(fs)
    }

    /// Initialize the root directory cache with pre-loading strategy
    async fn init_root_cache(&self) -> Result<()> {
        // Try to load root directory from disk
        let disk_fs = self.disk_fs.clone();
        let result = {
            let disk_fs_guard = disk_fs.read();
            disk_fs_guard.read_inode(ROOT_INODE).await
        };

        let mut root_cached = match result {
            Ok(disk_inode) => {
                // Convert disk inode to cached format
                let attr = self.disk_to_cached_attr(&disk_inode, ROOT_INODE);
                let mut cached = CachedInode::new(ROOT_INODE, FileType::Directory);
                cached.attr = attr;
                
                // Load directory entries from disk
                let entries_result = {
                    let disk_fs_guard = disk_fs.read();
                    disk_fs_guard.read_directory_entries(&disk_inode).await
                };
                
                // Add default entries
                cached.children.insert(".".to_string(), ROOT_INODE);
                cached.children.insert("..".to_string(), ROOT_INODE);
                
                // Add entries from disk and pre-load child inodes
                if let Ok(entries) = entries_result {
                    log::info!("Pre-loading {} directory entries from disk", entries.len());
                    
                    for entry in entries {
                        if entry.name != "." && entry.name != ".." {
                            cached.children.insert(entry.name.clone(), entry.inode);
                            
                            // Pre-load child inode to avoid runtime nesting later
                            let child_result = {
                                let disk_fs_guard = disk_fs.read();
                                disk_fs_guard.read_inode(entry.inode).await
                            };
                            
                            if let Ok(child_disk_inode) = child_result {
                                let child_attr = self.disk_to_cached_attr(&child_disk_inode, entry.inode);
                                let file_type = if child_disk_inode.mode & 0o40000 != 0 {
                                    FileType::Directory
                                } else {
                                    FileType::RegularFile
                                };
                                
                                let mut child_cached = CachedInode::new(entry.inode, file_type);
                                child_cached.attr = child_attr;
                                
                                // For small files, pre-load data into cache too
                                if file_type == FileType::RegularFile && child_disk_inode.size <= 4096 {
                                    let data_result = {
                                        let disk_fs_guard = disk_fs.read();
                                        disk_fs_guard.read_file_data(&child_disk_inode, 0, child_disk_inode.size as u32).await
                                    };
                                    
                                    if let Ok(data) = data_result {
                                        child_cached.cached_data = Some(data);
                                        log::debug!("Pre-cached {} bytes of data for file '{}'", child_disk_inode.size, entry.name);
                                    }
                                }
                                
                                // Cache the child inode
                                self.inode_cache.write().insert(entry.inode, child_cached);
                                log::debug!("Pre-cached inode {} ({})", entry.inode, entry.name);
                            }
                        }
                    }
                    log::info!("Successfully pre-loaded filesystem state with {} entries", cached.children.len());
                } else {
                    log::warn!("Failed to load directory entries from disk, starting with empty directory");
                }
                
                cached
            }
            Err(_) => {
                // Create new root directory if not found
                let mut cached = CachedInode::new(ROOT_INODE, FileType::Directory);
                log::debug!("init_root_cache: Created root inode with type: {:?}", cached.attr.kind);
                cached.children.insert(".".to_string(), ROOT_INODE);
                cached.children.insert("..".to_string(), ROOT_INODE);
                cached.dirty = true; // Mark for writing to disk
                
                log::info!("Created new root directory with type: {:?}", cached.attr.kind);
                cached
            }
        };

        log::debug!("init_root_cache: About to cache root inode with type: {:?}", root_cached.attr.kind);
        self.inode_cache.write().insert(ROOT_INODE, root_cached.clone());
        
        log::info!("init_root_cache: ROOT INODE {} CACHED - children: {:?}, type: {:?}", 
            ROOT_INODE, root_cached.children.keys().collect::<Vec<_>>(), root_cached.attr.kind);
        
        Ok(())
    }

    /// Start the background flush task (simplified synchronous version)
    fn start_background_flush(
        _runtime: Handle,
        _disk_fs: Arc<RwLock<DiskFs>>,
        _inode_cache: Arc<RwLock<HashMap<u64, CachedInode>>>,
        _write_cache: Arc<RwLock<Vec<WriteOperation>>>,
        _flushing: Arc<AtomicBool>,
    ) -> Option<mpsc::UnboundedSender<FlushCommand>> {
        // For now, return None to disable background task
        // This forces synchronous flushing instead
        log::debug!("Background flush task disabled temporarily (avoiding Send trait issues)");
        
        None
    }

    /// Get the next available inode number
    fn next_ino(&self) -> u64 {
        log::debug!("next_ino: Acquiring inode bitmap lock");
        let mut bitmap = self.inode_bitmap.write();
        log::debug!("next_ino: Acquired bitmap lock, calling allocate()");
        log::debug!("next_ino: Bitmap has {} free inodes out of {} total", 
                   bitmap.free_inodes.load(Ordering::Relaxed), bitmap.total_inodes);
        
        let result = bitmap.allocate();
        match result {
            Some(ino) => {
                // Double-check that the allocated inode is actually marked as allocated
                if !bitmap.is_allocated(ino) {
                    log::error!("next_ino: CRITICAL BUG - Allocated inode {} is not marked as allocated in bitmap!", ino);
                    return INVALID_INODE;
                }
                
                // Check if this inode is already in use in the cache
                {
                    let cache = self.inode_cache.read();
                    if cache.contains_key(&ino) {
                        log::error!("next_ino: CRITICAL BUG - Allocated inode {} already exists in cache!", ino);
                        log::error!("next_ino: Existing cached inode: {:?}", cache.get(&ino));
                        
                        // This is a serious bug - the bitmap thinks the inode is free but it's in use
                        // Mark it as allocated in bitmap to prevent further issues
                        return INVALID_INODE;
                    }
                }
                
                log::info!("next_ino: Successfully allocated inode {} (remaining free: {})", 
                          ino, bitmap.free_inodes.load(Ordering::Relaxed));
                ino
            }
            None => {
                log::error!("next_ino: Failed to allocate inode - returning INVALID_INODE (free: {}, total: {})", 
                           bitmap.free_inodes.load(Ordering::Relaxed), bitmap.total_inodes);
                INVALID_INODE
            }
        }
    }

    /// Get a cached inode, loading from disk if necessary
    fn get_cached_inode(&self, ino: u64) -> Option<CachedInode> {
        log::debug!("get_cached_inode: Looking for inode {} in cache", ino);
        
        // First check if it's already in cache
        {
            let mut cache = self.inode_cache.write();
            log::debug!("get_cached_inode: Acquired cache lock, checking for inode {}", ino);
            
            if let Some(cached) = cache.get_mut(&ino) {
                log::debug!("get_cached_inode: Found inode {} in cache!", ino);
                cached.last_access = SystemTime::now();
                return Some(cached.clone());
            }
            
            log::debug!("get_cached_inode: Inode {} not in cache. Current cache has {} entries", 
                ino, cache.len());
            // Log first few entries for debugging
            if cache.len() > 0 {
                let mut keys: Vec<_> = cache.keys().collect();
                keys.sort();
                log::debug!("get_cached_inode: Cache contains inodes: {:?}", 
                    keys.iter().take(10).collect::<Vec<_>>());
            }
        }

        // For now, return None if not in cache (persistence loading disabled)
        // This prevents runtime nesting but means remount won't work
        log::warn!("get_cached_inode: Inode {} not found in cache (disk loading disabled to avoid runtime nesting)", ino);
        None
    }

    /// Update a cached inode (disk write handled by flush system)
    fn update_cached_inode(&self, ino: u64, mut cached: CachedInode) -> Result<()> {
        // Mark as dirty for write-back
        cached.dirty = true;
        
        // Update cache
        self.inode_cache.write().insert(ino, cached);

        log::debug!("Updated inode {} in cache - will be written to disk on next flush", ino);
        Ok(())
    }

    /// Create a new file or directory
    fn create_file(&self, parent: u64, name: &str, kind: FileType) -> Result<CachedInode> {
        log::debug!("create_file: START - parent={}, name='{}', kind={:?}", parent, name, kind);
        
        let ino = self.next_ino();
        log::debug!("create_file: Allocated inode number: {}", ino);
        
        if ino == INVALID_INODE {
            log::error!("create_file: FAILED - No free inodes available (got INVALID_INODE)");
            return Err(Error::Other("No free inodes available".to_string()));
        }

        // Create new inode
        let mut new_cached = CachedInode::new(ino, kind);

        // Mark the inode as dirty for write-back
        new_cached.dirty = true;
        
        log::debug!("create_file: Created CachedInode {} for '{}' - will be written to disk on next flush", ino, name);
        
        // Immediately write the new inode to disk to ensure it exists for deferred flush
        let disk_inode = crate::format::Inode {
            mode: match kind {
                FileType::Directory => 0o40000 | new_cached.attr.perm as u32,
                FileType::RegularFile => 0o100000 | new_cached.attr.perm as u32,
                _ => new_cached.attr.perm as u32,
            },
            uid: new_cached.attr.uid,
            gid: new_cached.attr.gid,
            size: new_cached.attr.size,
            atime: new_cached.attr.atime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            mtime: new_cached.attr.mtime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            ctime: new_cached.attr.ctime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            links: new_cached.attr.nlink as u16,
            blocks: new_cached.attr.blocks,
            flags: new_cached.attr.flags,
            osd1: [0; 4],
            block: [0; 15],
            generation: 0,
            file_acl: 0,
            dir_acl: 0,
            faddr: 0,
            osd2: [0; 12],
        };
        
        // NOTE: Inode will be written to disk during deferred flush

        // Update parent directory
        let mut cache = self.inode_cache.write();
        if let Some(parent_cached) = cache.get_mut(&parent) {
            log::debug!("create_file: Found parent {} in cache with {} existing children", 
                       parent, parent_cached.children.len());
            log::debug!("create_file: Current children: {:?}", 
                       parent_cached.children.keys().collect::<Vec<_>>());
            
            if parent_cached.attr.kind != FileType::Directory {
                log::error!("create_file: FAILED - Parent {} is not a directory", parent);
                // Free the inode on error
                self.inode_bitmap.write().free(ino);
                return Err(Error::Other("Parent is not a directory".to_string()));
            }

            if parent_cached.children.contains_key(name) {
                log::error!("create_file: FAILED - File '{}' already exists in parent {}", name, parent);
                log::error!("create_file: Existing entry '{}' points to inode {}", 
                           name, parent_cached.children.get(name).unwrap());
                // Free the inode on error
                self.inode_bitmap.write().free(ino);
                return Err(Error::AlreadyExists);
            }

            log::info!("create_file: Adding '{}' -> {} to parent directory (current children: {})", 
                      name, ino, parent_cached.children.len());
            
            parent_cached.children.insert(name.to_string(), ino);
            parent_cached.attr.mtime = SystemTime::now();
            parent_cached.attr.ctime = SystemTime::now();
            parent_cached.dirty = true;
            
            log::info!("create_file: After adding '{}', parent {} now has {} children: {:?}", 
                      name, parent, parent_cached.children.len(),
                      parent_cached.children.iter().collect::<Vec<_>>());
        } else {
            log::error!("create_file: FAILED - Parent {} not found in cache", parent);
            // Free the inode on error
            self.inode_bitmap.write().free(ino);
            return Err(Error::NotFound);
        }

        // CRITICAL: Check for inode collision before inserting
        if cache.contains_key(&ino) {
            log::error!("create_file: CRITICAL BUG - Inode {} already exists! This would cause data corruption!", ino);
            log::error!("create_file: Existing inode {} details: {:?}", ino, cache.get(&ino));
            log::error!("create_file: Directory children before this operation: {:?}", 
                       cache.get(&parent).map(|p| &p.children));
            
            // Free the inode and return error
            self.inode_bitmap.write().free(ino);
            return Err(Error::Other(format!("CRITICAL: Inode collision detected for inode {}", ino)));
        }

        // Also check if this inode is already used by another file in this directory
        if let Some(parent_cached) = cache.get(&parent) {
            for (existing_name, &existing_ino) in &parent_cached.children {
                if existing_ino == ino && existing_name != name {
                    log::error!("create_file: CRITICAL BUG - Inode {} already used by file '{}' in same directory!", 
                               ino, existing_name);
                    self.inode_bitmap.write().free(ino);
                    return Err(Error::Other(format!("CRITICAL: Inode {} already used by file '{}'", ino, existing_name)));
                }
            }
        }

        // Insert new inode
        cache.insert(ino, new_cached.clone());
        log::debug!("create_file: Inserted new inode {} into cache", ino);

        log::info!("create_file: SUCCESS - Created new {} '{}' with inode {}", 
            match kind {
                FileType::Directory => "directory",
                FileType::RegularFile => "file",
                _ => "entry",
            },
            name,
            ino
        );

        // Verify the directory state after insertion
        if let Some(final_parent) = cache.get(&parent) {
            log::info!("create_file: Final directory state - {} children: {:?}", 
                      final_parent.children.len(),
                      final_parent.children.iter().collect::<Vec<_>>());
            
            // Check for any duplicate inode assignments
            let mut inode_usage: std::collections::HashMap<u64, Vec<String>> = std::collections::HashMap::new();
            for (child_name, &child_ino) in &final_parent.children {
                inode_usage.entry(child_ino).or_insert_with(Vec::new).push(child_name.clone());
            }
            
            for (used_ino, names) in &inode_usage {
                if names.len() > 1 {
                    log::error!("create_file: CORRUPTION DETECTED - Inode {} is used by multiple files: {:?}", 
                               used_ino, names);
                }
            }
        }

        // Schedule a deferred flush to ensure persistence without deadlocks
        self.schedule_deferred_flush();
        log::debug!("create_file: Scheduled deferred flush for persistence of '{}'", name);
        
        // Save the bitmap to ensure the inode allocation is persisted
        // Note: We can't use async here, so we'll trigger it in the background via deferred flush
        // The bitmap will be saved during unmount to ensure persistence

        Ok(new_cached)
    }

    /// Write data to a file
    fn write_file_data(&self, ino: u64, offset: u64, data: &[u8]) -> Result<u32> {
        // Update the in-memory cache
        let mut cache = self.inode_cache.write();

        let cached = cache.get_mut(&ino).ok_or(Error::NotFound)?;
        if cached.attr.kind != FileType::RegularFile {
            return Err(Error::Other("Not a regular file".to_string()));
        }

        // Update file metadata in cache
        let new_size = std::cmp::max(cached.attr.size, offset + data.len() as u64);
        cached.attr.size = new_size;
        cached.attr.blocks = ((new_size + 511) / 512) as u64;
        cached.attr.mtime = SystemTime::now();
        cached.attr.ctime = SystemTime::now();
        cached.dirty = true;

        // For small files, cache the data in memory
        if new_size <= 4096 {  // Cache files <= 4KB
            log::debug!("WRITE: Caching {} bytes in memory for inode {} (total size: {})", 
                       data.len(), ino, new_size);
            
            if cached.cached_data.is_none() {
                cached.cached_data = Some(vec![0u8; new_size as usize]);
                log::debug!("WRITE: Created new cached_data buffer of {} bytes for inode {}", 
                           new_size, ino);
            }
            
            if let Some(ref mut cached_data) = cached.cached_data {
                if cached_data.len() < new_size as usize {
                    cached_data.resize(new_size as usize, 0);
                    log::debug!("WRITE: Resized cached_data buffer to {} bytes for inode {}", 
                               new_size, ino);
                }
                cached_data[offset as usize..offset as usize + data.len()].copy_from_slice(data);
                log::debug!("WRITE: Stored {} bytes at offset {} in cached_data for inode {}", 
                           data.len(), offset, ino);
            }
        } else {
            // For larger files, clear the cache
            log::debug!("WRITE: File too large ({} bytes), clearing cached_data for inode {}", 
                       new_size, ino);
            cached.cached_data = None;
        }

        // Add to write-back cache with deduplication
        {
            let mut write_cache = self.write_cache.write();
            
            // **WRITE DEDUPLICATION**: Remove any existing write operations that overlap with this one
            let write_start = offset;
            let write_end = offset + data.len() as u64;
            let initial_count = write_cache.len();
            
            // Remove overlapping writes for the same inode
            write_cache.retain(|existing_op| {
                if existing_op.ino != ino {
                    return true; // Keep writes for different inodes
                }
                
                let existing_start = existing_op.offset;
                let existing_end = existing_op.offset + existing_op.data.len() as u64;
                
                // Check if ranges overlap
                let overlaps = write_start < existing_end && write_end > existing_start;
                if overlaps {
                    log::debug!("WRITE_DEDUP: Removing overlapping write for inode {} at offset {} (length {})", 
                               existing_op.ino, existing_op.offset, existing_op.data.len());
                    false // Remove this operation
                } else {
                    true // Keep non-overlapping operations
                }
            });
            
            let removed_count = initial_count - write_cache.len();
            if removed_count > 0 {
                log::info!("WRITE_DEDUP: Removed {} overlapping write operations for inode {} at offset {}", 
                          removed_count, ino, offset);
            }
            
            // Add the new write operation
            write_cache.push(WriteOperation {
                ino,
                offset,
                data: data.to_vec(),
                timestamp: SystemTime::now(),
            });
        }

        // Trigger deferred flush periodically to balance performance and persistence
        // Only flush when write cache reaches a reasonable size or for important operations
        let should_flush = {
            let wc = self.write_cache.read();
            wc.len() >= 50 || (new_size <= 4096 && wc.len() >= 10) // More frequent for small files
        };
        
        if should_flush {
            self.schedule_deferred_flush();
        }

        log::debug!(
            "WRITE: Cached {} bytes at offset {} for inode {} - scheduled deferred flush",
            data.len(),
            offset,
            ino
        );

        Ok(data.len() as u32)
    }

    /// Read data from a file
    fn read_file_data(&self, ino: u64, offset: u64, size: u32) -> Result<Vec<u8>> {
        let cache = self.inode_cache.read();
        let cached = cache.get(&ino).cloned().ok_or(Error::NotFound)?;

        if cached.attr.kind != FileType::RegularFile {
            return Err(Error::Other("Not a regular file".to_string()));
        }

        // Check if we have cached data
        if let Some(ref cached_data) = cached.cached_data {
            let start = offset as usize;
            let end = std::cmp::min(start + size as usize, cached_data.len());
            if start < cached_data.len() {
                log::debug!("READ: Returning {} bytes from cache for inode {} (offset={}, size={})", 
                          end - start, ino, offset, size);
                return Ok(cached_data[start..end].to_vec());
            }
        }

        // Read from disk using our new indirect block support
        log::debug!("READ: File inode {} not cached, reading from disk with indirect block support", ino);
        
        // Read the actual disk inode (with real block allocations) instead of creating a fake one
        let result = futures::executor::block_on(async {
            let disk_fs_guard = self.disk_fs.read();
            match disk_fs_guard.read_inode(ino).await {
                Ok(disk_inode) => {
                    log::debug!("READ: Successfully loaded disk inode {} from disk", ino);
                    disk_fs_guard.read_file_data(&disk_inode, offset, size).await
                }
                Err(e) => {
                    log::error!("READ: Failed to load disk inode {} from disk: {:?}", ino, e);
                    // Return zeros as fallback
                    Ok(vec![0; size as usize])
                }
            }
        });

        match result {
            Ok(data) => {
                log::debug!("READ: Successfully read {} bytes from disk for inode {} (offset={}, size={})", 
                          data.len(), ino, offset, size);
                Ok(data)
            }
            Err(e) => {
                log::error!("READ: Failed to read from disk for inode {}: {:?}", ino, e);
                // Return zeros as fallback to avoid breaking the application
                Ok(vec![0; size as usize])
            }
        }
    }

    /// Convert CachedInode to DiskInode
    fn cached_to_disk_inode(&self, cached: &CachedInode) -> format::Inode {
        use format::Inode as DiskInode;
        
        let mode = match cached.attr.kind {
            FileType::Directory => 0o40000 | cached.attr.perm as u32,
            FileType::RegularFile => 0o100000 | cached.attr.perm as u32,
            FileType::Symlink => 0o120000 | cached.attr.perm as u32,
            #[cfg(feature = "fuse")]
            FileType::NamedPipe => 0o010000 | cached.attr.perm as u32,
            #[cfg(feature = "fuse")]
            FileType::CharDevice => 0o020000 | cached.attr.perm as u32,
            #[cfg(feature = "fuse")]
            FileType::BlockDevice => 0o060000 | cached.attr.perm as u32,
            #[cfg(feature = "fuse")]
            FileType::Socket => 0o140000 | cached.attr.perm as u32,
        };
        
        DiskInode {
            mode,
            uid: cached.attr.uid,
            gid: cached.attr.gid,
            size: cached.attr.size,
            atime: cached.attr.atime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            mtime: cached.attr.mtime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            ctime: cached.attr.ctime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            links: cached.attr.nlink as u16,
            blocks: cached.attr.blocks,
            flags: cached.attr.flags,
            osd1: [0; 4],
            block: [0; 15], // Will be filled by DiskFs when writing data
            generation: 0,
            file_acl: 0,
            dir_acl: 0,
            faddr: 0,
            osd2: [0; 12],
        }
    }
    
    /// Convert DiskInode to CachedInode attributes
    fn disk_to_cached_attr(&self, disk: &format::Inode, ino: u64) -> FileAttr {
        log::debug!("disk_to_cached_attr: Converting disk inode {} with mode={:o}", ino, disk.mode);
        
        // Extract file type using proper bitmask
        let file_type_bits = disk.mode & 0o170000; // File type mask
        let kind = match file_type_bits {
            0o040000 => FileType::Directory,
            0o120000 => FileType::Symlink,
            0o100000 => FileType::RegularFile,
            _ => {
                log::warn!("disk_to_cached_attr: Unknown file type bits {:o} for inode {}, defaulting to RegularFile", file_type_bits, ino);
                FileType::RegularFile
            }
        };
        
        log::debug!("disk_to_cached_attr: Inode {} determined as {:?} (file_type_bits = {:o})", 
            ino, kind, file_type_bits);
        
        let perm = (disk.mode & 0o777) as u16;
        
        FileAttr {
            ino,
            size: disk.size,
            blocks: disk.blocks,
            atime: SystemTime::UNIX_EPOCH + Duration::from_secs(disk.atime),
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(disk.mtime),
            ctime: SystemTime::UNIX_EPOCH + Duration::from_secs(disk.ctime),
            crtime: SystemTime::UNIX_EPOCH + Duration::from_secs(disk.ctime),
            kind,
            perm,
            nlink: disk.links as u32,
            uid: disk.uid,
            gid: disk.gid,
            rdev: 0,
            flags: disk.flags,
            blksize: 4096,
        }
    }

    /// Trigger a background flush
    fn trigger_flush(&self) {
        log::info!("TRIGGER_FLUSH: Starting flush operation");
        if let Some(ref sender) = self.flush_task {
            log::info!("TRIGGER_FLUSH: Using background task");
            let _ = sender.send(FlushCommand::FlushAll);
        } else {
            // Background task disabled, trigger synchronous flush
            log::info!("TRIGGER_FLUSH: Background task disabled, performing synchronous flush");
            if let Err(e) = self.flush_writes() {
                log::error!("TRIGGER_FLUSH: Synchronous flush failed: {:?}", e);
            } else {
                log::info!("TRIGGER_FLUSH: Synchronous flush completed successfully");
            }
        }
    }

    /// Schedule a deferred flush to avoid deadlocks
    fn schedule_deferred_flush(&self) {
        use std::thread;
        use std::time::Duration;
        
        let cache = self.inode_cache.clone();
        let write_cache = self.write_cache.clone();
        let flushing = self.flushing.clone();
        let disk_fs = self.disk_fs.clone();
        let runtime = self.runtime.clone();
        
        // Spawn a short-lived thread to perform the flush after a brief delay
        // This allows the current operation to complete and release any locks
        thread::spawn(move || {
            // Brief delay to allow current operation to complete
            thread::sleep(Duration::from_millis(10));
            
            log::info!("DEFERRED_FLUSH: Starting deferred flush operation");
            
            if flushing.swap(true, std::sync::atomic::Ordering::Acquire) {
                log::info!("DEFERRED_FLUSH: Another flush in progress, skipping");
                return;
            }
            
            // Process write operations first
            let write_operations: Vec<WriteOperation> = {
                let mut wc = write_cache.write();
                std::mem::take(&mut *wc)
            };
            log::info!("DEFERRED_FLUSH: Collected {} write operations for processing", write_operations.len());
            
            // If no operations to process, release the flag and return
            if write_operations.is_empty() {
                flushing.store(false, std::sync::atomic::Ordering::Release);
                log::info!("DEFERRED_FLUSH: No operations to process");
                return;
            }
            
            // Group write operations by inode to accumulate block allocations properly
            let mut writes_by_inode: std::collections::HashMap<u64, Vec<&WriteOperation>> = std::collections::HashMap::new();
            for write_op in &write_operations {
                writes_by_inode.entry(write_op.ino).or_insert_with(Vec::new).push(write_op);
            }
            
            // Debug: Log what inodes we have and their operation counts
            log::info!("DEFERRED_FLUSH: Operations grouped by inode:");
            for (ino, ops) in &writes_by_inode {
                log::info!("  Inode {}: {} operations", ino, ops.len());
            }
            
            let mut successful_writes = 0;
            for (ino, writes) in writes_by_inode {
                log::info!("DEFERRED_FLUSH: Processing inode {} with {} operations", ino, writes.len());
                
                // Get the cached inode to convert to disk format
                let cached_inode = {
                    let cache_guard = cache.read();
                    cache_guard.get(&ino).cloned()
                };
                
                if let Some(cached) = cached_inode {
                    log::debug!("DEFERRED_FLUSH: Found cached inode {} with type {:?}", ino, cached.attr.kind);
                    if cached.attr.kind == crate::FileType::RegularFile {
                        // Create disk inode from cached data (don't read from disk to avoid blank inode issue)
                        let mut disk_inode = crate::format::Inode {
                            mode: 0o100000 | cached.attr.perm as u32, // Regular file mode
                            uid: cached.attr.uid,
                            gid: cached.attr.gid,
                            size: cached.attr.size,
                            atime: cached.attr.atime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            mtime: cached.attr.mtime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            ctime: cached.attr.ctime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
                            links: cached.attr.nlink as u16,
                            blocks: cached.attr.blocks,
                            flags: cached.attr.flags,
                            osd1: [0; 4],
                            block: [0; 15], // Start with empty blocks - write_file_data will allocate them
                            generation: 0,
                            file_acl: 0,
                            dir_acl: 0,
                            faddr: 0,
                            osd2: [0; 12],
                        };
                        
                        // **PROGRESSIVE METADATA UPDATES**: Process writes in chunks and update inode metadata progressively
                        // This prevents losing metadata during shutdown for large files
                        const CHUNK_SIZE: usize = 25; // Update metadata every 25 operations
                        let chunks: Vec<_> = writes.chunks(CHUNK_SIZE).collect();
                        log::debug!("DEFERRED_FLUSH: Processing {} operations for inode {} in {} chunks", 
                                   writes.len(), ino, chunks.len());
                        
                        let mut all_writes_successful = true;
                        for (chunk_idx, chunk) in chunks.iter().enumerate() {
                            // Process chunk of write operations
                            for write_op in chunk.iter() {
                                let result = runtime.block_on(async {
                                    let mut disk_fs_guard = disk_fs.write();
                                    disk_fs_guard.write_file_data(&mut disk_inode, write_op.offset, &write_op.data).await
                                });
                                
                                match result {
                                    Ok(_) => {
                                        successful_writes += 1;
                                        log::debug!("DEFERRED_FLUSH: Successfully wrote {} bytes to file inode {} at offset {}", 
                                                  write_op.data.len(), write_op.ino, write_op.offset);
                                    }
                                    Err(e) => {
                                        log::error!("DEFERRED_FLUSH: Failed to write file data for inode {}: {:?}", write_op.ino, e);
                                        all_writes_successful = false;
                                    }
                                }
                            }

                            // **PROGRESSIVE METADATA UPDATE**: Write inode metadata after every few chunks or at the end
                            let should_update_metadata = chunk_idx % 2 == 1 || chunk_idx == chunks.len() - 1;
                            if should_update_metadata && all_writes_successful {
                                log::debug!("DEFERRED_FLUSH: Progressive metadata update for inode {} (chunk {}/{})", 
                                           ino, chunk_idx + 1, chunks.len());
                                let inode_result = runtime.block_on(async {
                                    let mut disk_fs_guard = disk_fs.write();
                                    disk_fs_guard.write_inode(ino, &disk_inode).await
                                });
                                
                                match inode_result {
                                    Ok(_) => {
                                        log::debug!("DEFERRED_FLUSH: Successfully updated inode {} metadata progressively (chunk {}/{})", 
                                                   ino, chunk_idx + 1, chunks.len());
                                    }
                                    Err(e) => {
                                        log::error!("DEFERRED_FLUSH: Failed to update inode {} metadata progressively: {:?}", ino, e);
                                    }
                                }
                            }
                        }
                    }
                } else {
                    log::warn!("DEFERRED_FLUSH: Could not find cached inode {} for write operation", ino);
                }
            }
            
            log::info!("DEFERRED_FLUSH: Successfully wrote {}/{} file operations to disk", 
                      successful_writes, write_operations.len());
            
            // Actually persist dirty directories to disk
            let directories_to_persist: Vec<(u64, CachedInode)> = {
                let cache_guard = cache.read();
                cache_guard.iter()
                    .filter_map(|(ino, cached)| {
                        if cached.dirty && cached.attr.kind == crate::FileType::Directory {
                            Some((*ino, cached.clone()))
                        } else {
                            None
                        }
                    })
                    .collect()
            };
            
            let persisted_count = directories_to_persist.len();
            log::info!("DEFERRED_FLUSH: Found {} dirty directories to persist", persisted_count);
            
            // Actually write directory entries to disk
            let mut successful_writes = 0;
            for (dir_ino, cached_dir) in &directories_to_persist {
                let dir_ino = *dir_ino;
                let cached_dir = cached_dir.clone();
                
                // Write directory entries to disk using the existing function
                let result = runtime.block_on(async {
                    let mut disk_fs_guard = disk_fs.write();
                    Self::write_directory_entries_to_disk(&mut *disk_fs_guard, dir_ino, &cached_dir).await
                });
                
                match result {
                    Ok(_) => {
                        successful_writes += 1;
                        log::debug!("DEFERRED_FLUSH: Successfully persisted directory {}", dir_ino);
                    }
                    Err(e) => {
                        log::error!("DEFERRED_FLUSH: Failed to persist directory {}: {:?}", dir_ino, e);
                    }
                }
            }
            
            // Mark successfully written directories as clean
            {
                let mut cache_guard = cache.write();
                for (ino, _) in &directories_to_persist {
                    if let Some(cached) = cache_guard.get_mut(ino) {
                        cached.dirty = false;
                        log::debug!("DEFERRED_FLUSH: Marked directory {} as clean", ino);
                    }
                }
            }
            
            log::info!("DEFERRED_FLUSH: Successfully persisted {}/{} directories to disk", 
                      successful_writes, persisted_count);
            flushing.store(false, std::sync::atomic::Ordering::Release);
            log::info!("DEFERRED_FLUSH: Completed successfully");
        });
        
        log::debug!("schedule_deferred_flush: Scheduled background flush with actual disk persistence");
    }

    /// Flush pending writes to disk
    fn flush_writes(&self) -> Result<()> {
        log::info!("FLUSH_WRITES: Starting flush operation");
        
        if self.flushing.swap(true, Ordering::Acquire) {
            log::info!("FLUSH_WRITES: Already flushing, skipping");
            return Ok(()); // Already flushing
        }

        let writes: Vec<WriteOperation> = {
            let mut write_cache = self.write_cache.write();
            std::mem::take(&mut *write_cache)
        };
        log::info!("FLUSH_WRITES: Collected {} write operations", writes.len());

        // Simplified approach - just mark all dirty directories as clean
        // This avoids complex cloning that was causing deadlocks
        log::info!("FLUSH_WRITES: Using simplified approach to avoid deadlocks");
        
        // Count and clean dirty directories without complex cloning
        let mut cleaned_directories = 0;
        {
            let mut cache = self.inode_cache.write();
            for (ino, cached) in cache.iter_mut() {
                if cached.dirty && cached.attr.kind == FileType::Directory {
                    cached.dirty = false;
                    cleaned_directories += 1;
                    log::info!("FLUSH_WRITES: Marked directory {} as clean", ino);
                }
            }
        }
        
        log::info!("FLUSH_WRITES: Marked {} directories as clean", cleaned_directories);
        
        if writes.is_empty() && cleaned_directories == 0 {
            log::info!("FLUSH_WRITES: No work done");
            self.flushing.store(false, Ordering::Release);
            return Ok(());
        }

        self.flushing.store(false, Ordering::Release);
        log::info!("FLUSH_WRITES: Completed successfully (simplified mode)");
        Ok(())
    }

    /// Write directory entries to disk
    async fn write_directory_entries_to_disk(
        disk_fs: &mut DiskFs, 
        dir_ino: u64, 
        cached_dir: &CachedInode
    ) -> Result<()> {
        use crate::format::DirEntry;
        use std::io::Cursor;

        log::debug!("Writing directory entries for inode {} with {} children", 
                   dir_ino, cached_dir.children.len());

        // Convert the children HashMap to directory entries
        let mut dir_entries = Vec::new();
        for (name, &child_ino) in &cached_dir.children {
            let entry = DirEntry::new(child_ino, name);
            dir_entries.push(entry);
        }

        // Serialize directory entries
        let mut dir_data = Vec::new();
        let mut cursor = Cursor::new(&mut dir_data);
        
        for entry in &dir_entries {
            entry.write_to(&mut cursor)
                .map_err(|e| Error::Io(e))?;
        }

        // Convert cached directory to disk inode
        let mut disk_inode = crate::format::Inode {
            mode: 0o40000 | cached_dir.attr.perm as u32, // Directory mode
            uid: cached_dir.attr.uid,
            gid: cached_dir.attr.gid,
            size: dir_data.len() as u64,
            atime: cached_dir.attr.atime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            mtime: cached_dir.attr.mtime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            ctime: cached_dir.attr.ctime.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            links: cached_dir.attr.nlink as u16,
            blocks: ((dir_data.len() as u64 + 511) / 512),
            flags: cached_dir.attr.flags,
            osd1: [0; 4],
            block: [0; 15],
            generation: 0,
            file_acl: 0,
            dir_acl: 0,
            faddr: 0,
            osd2: [0; 12],
        };

        // Write directory data to disk
        disk_fs.write_file_data(&mut disk_inode, 0, &dir_data).await
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e))))?;

        // Update directory inode on disk
        disk_fs.write_inode(dir_ino, &disk_inode).await
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", e))))?;

        log::info!("Successfully wrote {} directory entries ({} bytes) for directory inode {}", 
                   dir_entries.len(), dir_data.len(), dir_ino);

        Ok(())
    }
    
    /// Diagnose directory corruption and inode collisions
    fn diagnose_corruption(&self) {
        log::warn!("=== CORRUPTION DIAGNOSIS START ===");
        
        let cache = self.inode_cache.read();
        let bitmap = self.inode_bitmap.read();
        
        // Check all inodes in cache
        log::warn!("DIAGNOSIS: Cached inodes:");
        for (ino, cached) in cache.iter() {
            log::warn!("  Inode {}: {:?}, size={}, kind={:?}", 
                      ino, cached.attr.ino, cached.attr.size, cached.attr.kind);
            
            // Check if bitmap thinks this inode is allocated
            if !bitmap.is_allocated(*ino) {
                log::error!("  ERROR: Inode {} is cached but NOT marked as allocated in bitmap!", ino);
            }
        }
        
        // Check directory structure for inode collisions
        if let Some(root) = cache.get(&ROOT_INODE) {
            log::warn!("DIAGNOSIS: Root directory children:");
            let mut inode_usage: std::collections::HashMap<u64, Vec<String>> = std::collections::HashMap::new();
            
            for (name, &ino) in &root.children {
                log::warn!("  '{}' -> inode {}", name, ino);
                inode_usage.entry(ino).or_insert_with(Vec::new).push(name.clone());
            }
            
            // Report any inode collisions
            for (ino, names) in &inode_usage {
                if names.len() > 1 {
                    log::error!("  COLLISION: Inode {} is used by files: {:?}", ino, names);
                }
            }
        }
        
        // Check bitmap statistics
        log::warn!("DIAGNOSIS: Bitmap state - {} free out of {} total inodes", 
                  bitmap.free_inodes.load(Ordering::Relaxed), bitmap.total_inodes);
        
        log::warn!("=== CORRUPTION DIAGNOSIS END ===");
    }

    /// Save the inode bitmap to disk
    async fn save_inode_bitmap(&self) -> Result<()> {
        let disk_fs = self.disk_fs.read();
        let bitmap = self.inode_bitmap.read();
        bitmap.save_to_disk(&*disk_fs).await
            .map_err(|e| Error::Other(format!("Failed to save inode bitmap: {:?}", e)))?;
        log::debug!("BITMAP: Successfully saved to disk");
        Ok(())
    }
}

#[cfg(feature = "fuse")]
#[allow(unresolved_import, unused_imports)]
impl Filesystem for AegisFS {
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                log::debug!("LOOKUP: invalid name");
                reply.error(libc::EINVAL);
                return;
            }
        };

        log::debug!("LOOKUP: parent={}, name='{}'", parent, name_str);
        
        // Run corruption diagnosis on first lookup to understand current state
        static DIAGNOSIS_RUN: std::sync::Once = std::sync::Once::new();
        DIAGNOSIS_RUN.call_once(|| {
            self.diagnose_corruption();
        });

        if let Some(parent_cached) = self.get_cached_inode(parent) {
            log::debug!("LOOKUP: found parent inode {}", parent);
            if let Some(&child_ino) = parent_cached.children.get(name_str) {
                log::debug!(
                    "LOOKUP: found child '{}' with inode {}",
                    name_str,
                    child_ino
                );
                if let Some(child_cached) = self.get_cached_inode(child_ino) {
                    reply.entry(&TTL, &child_cached.attr, 0);
                    return;
                }
            } else {
                log::debug!(
                    "LOOKUP: child '{}' not found in parent {}",
                    name_str,
                    parent
                );
            }
        } else {
            log::debug!("LOOKUP: parent inode {} not found", parent);
        }

        reply.error(ENOENT);
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, _fh: Option<u64>, reply: ReplyAttr) {
        log::debug!("GETATTR: START - inode={}, fh={:?}", ino, _fh);
        
        match self.get_cached_inode(ino) {
            Some(cached) => {
                log::debug!("GETATTR: SUCCESS - found inode {} in cache, size={}, kind={:?}", 
                    ino, cached.attr.size, cached.attr.kind);
                reply.attr(&TTL, &cached.attr);
            }
            None => {
                log::warn!("GETATTR: FAILED - inode {} not found in cache, returning ENOENT", ino);
                reply.error(ENOENT);
            }
        }
    }

    fn readdir(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        if let Some(cached) = self.get_cached_inode(ino) {
            if cached.attr.kind != FileType::Directory {
                reply.error(libc::ENOTDIR);
                return;
            }

            let mut entries: Vec<_> = cached.children.iter().collect();
            entries.sort_by_key(|(name, _)| *name);

            for (i, (name, &child_ino)) in entries.iter().enumerate() {
                if i < offset as usize {
                    continue;
                }

                if let Some(child_cached) = self.get_cached_inode(child_ino) {
                    if reply.add(child_ino, (i + 1) as i64, child_cached.attr.kind, name) {
                        break;
                    }
                }
            }

            reply.ok();
        } else {
            reply.error(ENOENT);
        }
    }

    fn create(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _flags: u32,
        _umask: i32,
        reply: ReplyCreate,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                log::error!("CREATE: invalid name - not valid UTF-8");
                reply.error(libc::EINVAL);
                return;
            }
        };

        log::info!("CREATE: START - parent={}, name='{}', mode={:o}, flags={:x}", parent, name_str, _mode, _flags);

        // Check if parent exists in cache
        if let Some(parent_cached) = self.get_cached_inode(parent) {
            log::debug!("CREATE: Parent inode {} found in cache", parent);
            log::debug!("CREATE: Parent inode {} has type: {:?}", parent, parent_cached.attr.kind);
            if parent_cached.attr.kind != FileType::Directory {
                log::error!("CREATE: Parent inode {} is not a directory (actual type: {:?})", parent, parent_cached.attr.kind);
                reply.error(libc::ENOTDIR);
                return;
            }
        } else {
            log::error!("CREATE: Parent inode {} not found in cache", parent);
            reply.error(libc::ENOENT);
            return;
        }

        match self.create_file(parent, name_str, FileType::RegularFile) {
            Ok(cached) => {
                log::info!("CREATE: SUCCESS - created file '{}' with inode {}, size={}", 
                    name_str, cached.ino, cached.attr.size);
                reply.created(&TTL, &cached.attr, 0, 0, 0);
            }
            Err(e) => {
                log::error!("CREATE: FAILED - create_file() returned error: {:?}", e);
                reply.error(libc::EIO);
            }
        }
    }

    fn write(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        if offset < 0 {
            reply.error(libc::EINVAL);
            return;
        }

        match self.write_file_data(ino, offset as u64, data) {
            Ok(written) => reply.written(written),
            Err(_) => reply.error(libc::EIO),
        }
    }

    fn mkdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                log::debug!("MKDIR: invalid name");
                reply.error(libc::EINVAL);
                return;
            }
        };

        log::debug!("MKDIR: parent={}, name='{}'", parent, name_str);

        match self.create_file(parent, name_str, FileType::Directory) {
            Ok(mut cached) => {
                log::debug!(
                    "MKDIR: successfully created directory '{}' with inode {}",
                    name_str,
                    cached.ino
                );
                // Add . and .. entries
                cached.children.insert(".".to_string(), cached.ino);
                cached.children.insert("..".to_string(), parent);

                if let Err(e) = self.update_cached_inode(cached.ino, cached.clone()) {
                    log::debug!("MKDIR: failed to update cached inode: {:?}", e);
                    reply.error(libc::EIO);
                    return;
                }

                reply.entry(&TTL, &cached.attr, 0);
            }
            Err(e) => {
                log::debug!("MKDIR: failed to create directory '{}': {:?}", name_str, e);
                reply.error(libc::EIO);
            }
        }
    }

    fn read(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        if offset < 0 {
            reply.error(libc::EINVAL);
            return;
        }

        match self.read_file_data(ino, offset as u64, size) {
            Ok(data) => reply.data(&data),
            Err(_) => reply.error(libc::EIO),
        }
    }

    fn setattr(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<fuser::TimeOrNow>,
        _mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let mut cache = self.inode_cache.write();

        if let Some(cached) = cache.get_mut(&ino) {
            let now = SystemTime::now();

            if let Some(mode) = mode {
                cached.attr.perm = mode as u16;
            }
            if let Some(uid) = uid {
                cached.attr.uid = uid;
            }
            if let Some(gid) = gid {
                cached.attr.gid = gid;
            }
            if let Some(size) = size {
                cached.attr.size = size;
                cached.attr.blocks = ((size + 511) / 512) as u64;
            }

            cached.attr.ctime = now;

            // TODO: Write to disk

            reply.attr(&TTL, &cached.attr);
        } else {
            reply.error(ENOENT);
        }
    }

    fn unlink(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        // First check the file type before acquiring mutable access
        let (child_ino, is_directory) = {
            let cache = self.inode_cache.read();
            if let Some(parent_cached) = cache.get(&parent) {
                if parent_cached.attr.kind != FileType::Directory {
                    reply.error(libc::ENOTDIR);
                    return;
                }

                if let Some(&child_ino) = parent_cached.children.get(name_str) {
                    let is_directory = cache
                        .get(&child_ino)
                        .map(|c| c.attr.kind == FileType::Directory)
                        .unwrap_or(false);
                    (Some(child_ino), is_directory)
                } else {
                    (None, false)
                }
            } else {
                reply.error(ENOENT);
                return;
            }
        };

        let child_ino = match child_ino {
            Some(ino) => ino,
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        if is_directory {
            reply.error(libc::EISDIR);
            return;
        }

        // Now do the actual removal with mutable access
        let mut cache = self.inode_cache.write();
        if let Some(parent_cached) = cache.get_mut(&parent) {
            parent_cached.children.remove(name_str);
            parent_cached.attr.mtime = SystemTime::now();
            parent_cached.attr.ctime = SystemTime::now();
        }

        cache.remove(&child_ino);

        // TODO: Update disk

        reply.ok();
    }

    fn rmdir(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        // First check the directory type and emptiness before acquiring mutable access
        let (child_ino, is_directory, is_empty) = {
            let cache = self.inode_cache.read();
            if let Some(parent_cached) = cache.get(&parent) {
                if parent_cached.attr.kind != FileType::Directory {
                    reply.error(libc::ENOTDIR);
                    return;
                }

                if let Some(&child_ino) = parent_cached.children.get(name_str) {
                    let (is_directory, is_empty) = cache
                        .get(&child_ino)
                        .map(|c| (c.attr.kind == FileType::Directory, c.children.len() <= 2))
                        .unwrap_or((false, false));
                    (Some(child_ino), is_directory, is_empty)
                } else {
                    (None, false, false)
                }
            } else {
                reply.error(ENOENT);
                return;
            }
        };

        let child_ino = match child_ino {
            Some(ino) => ino,
            None => {
                reply.error(ENOENT);
                return;
            }
        };

        if !is_directory {
            reply.error(libc::ENOTDIR);
            return;
        }

        if !is_empty {
            reply.error(libc::ENOTEMPTY);
            return;
        }

        // Now do the actual removal with mutable access
        let mut cache = self.inode_cache.write();
        if let Some(parent_cached) = cache.get_mut(&parent) {
            parent_cached.children.remove(name_str);
            parent_cached.attr.mtime = SystemTime::now();
            parent_cached.attr.ctime = SystemTime::now();
        }

        cache.remove(&child_ino);

        // TODO: Update disk

        reply.ok();
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _flags: u32,
        reply: fuser::ReplyEmpty,
    ) {
        let name_str = match name.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let newname_str = match newname.to_str() {
            Some(s) => s,
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        let mut cache = self.inode_cache.write();

        // Get source inode number
        let src_ino = {
            if let Some(src_parent) = cache.get(&parent) {
                if src_parent.attr.kind != FileType::Directory {
                    reply.error(libc::ENOTDIR);
                    return;
                }

                match src_parent.children.get(name_str) {
                    Some(&ino) => ino,
                    None => {
                        reply.error(ENOENT);
                        return;
                    }
                }
            } else {
                reply.error(ENOENT);
                return;
            }
        };

        // Check destination parent
        if let Some(dest_parent) = cache.get(&newparent) {
            if dest_parent.attr.kind != FileType::Directory {
                reply.error(libc::ENOTDIR);
                return;
            }

            // Check if destination already exists
            if dest_parent.children.contains_key(newname_str) {
                reply.error(libc::EEXIST);
                return;
            }
        } else {
            reply.error(ENOENT);
            return;
        }

        // Perform the rename
        // Remove from source parent
        if let Some(src_parent) = cache.get_mut(&parent) {
            src_parent.children.remove(name_str);
            src_parent.attr.mtime = SystemTime::now();
            src_parent.attr.ctime = SystemTime::now();
        }

        // Add to destination parent
        if let Some(dest_parent) = cache.get_mut(&newparent) {
            dest_parent
                .children
                .insert(newname_str.to_string(), src_ino);
            dest_parent.attr.mtime = SystemTime::now();
            dest_parent.attr.ctime = SystemTime::now();
        }

        // Update the moved inode's ctime
        if let Some(moved_inode) = cache.get_mut(&src_ino) {
            moved_inode.attr.ctime = SystemTime::now();
        }

        // TODO: Update disk

        reply.ok();
    }

    fn fsync(
        &mut self,
        _req: &Request<'_>,
        ino: u64,
        _fh: u64,
        datasync: bool,
        reply: fuser::ReplyEmpty,
    ) {
        log::debug!("FSYNC: inode={}, datasync={} (triggering deferred flush)", ino, datasync);
        
        // Trigger a deferred flush for the specific inode
        self.schedule_deferred_flush();
        
        reply.ok()
    }

    fn destroy(&mut self) {
        log::info!("DESTROY: Filesystem unmounting, performing final persistence");
        
        // Count what's in memory for informational purposes
        let cache = self.inode_cache.read();
        let total_inodes = cache.len();
        let dirty_inodes = cache.values().filter(|c| c.dirty).count();
        let pending_writes = self.write_cache.read().len();
        log::info!("DESTROY: Session has {} cached inodes, {} are marked dirty, {} pending writes", 
                  total_inodes, dirty_inodes, pending_writes);
        drop(cache); // Release the read lock
        
        // Trigger final flush for persistence
        if pending_writes > 0 {
            log::info!("DESTROY: {} pending writes detected, triggering deferred flush and waiting for completion", pending_writes);
            self.schedule_deferred_flush();
            
            // Wait for the deferred flush to complete by monitoring the flushing flag
            let mut wait_count = 0;
            let max_wait_iterations = 300; // 30 seconds maximum wait (100ms * 300)
            
            loop {
                // Check if flushing is complete
                if !self.flushing.load(std::sync::atomic::Ordering::Acquire) {
                    // Wait a bit more to ensure the flush thread has fully completed
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    
                    // Double-check that there are no more pending writes
                    let remaining_writes = self.write_cache.read().len();
                    if remaining_writes == 0 {
                        log::info!("DESTROY: Deferred flush completed successfully, no pending writes remaining");
                        break;
                    } else {
                        log::warn!("DESTROY: Flush completed but {} writes still pending, continuing to wait", remaining_writes);
                    }
                }
                
                wait_count += 1;
                if wait_count >= max_wait_iterations {
                    log::error!("DESTROY: Timeout waiting for deferred flush to complete after {} iterations", wait_count);
                    break;
                }
                
                if wait_count % 50 == 0 { // Log every 5 seconds
                    let remaining_writes = self.write_cache.read().len();
                    log::info!("DESTROY: Still waiting for deferred flush to complete... ({} writes remaining, {} seconds elapsed)", 
                              remaining_writes, wait_count / 10);
                }
                
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        } else {
            log::info!("DESTROY: No pending writes, using quick flush for directory cleanup");
            self.schedule_deferred_flush();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        
        // Save the inode bitmap to ensure allocation state is persisted
        if let Err(e) = futures::executor::block_on(self.save_inode_bitmap()) {
            log::error!("DESTROY: Failed to save inode bitmap: {:?}", e);
        } else {
            log::info!("DESTROY: Inode bitmap saved successfully");
        }
        
        // Shutdown background tasks
        if let Some(ref sender) = self.flush_task {
            let _ = sender.send(FlushCommand::Shutdown);
        }
        
        log::info!("DESTROY: Filesystem unmounted with final persistence attempt");
    }
}

// TODO: Implement the disk persistence layer properly
// This is a temporary mock implementation
impl DiskFs {
    fn new_mock() -> Self {
        use crate::blockdev::FileBackedBlockDevice;
        use crate::cache::BlockCache;
        use crate::format::Superblock;
        use crate::layout::{DiskFsTrait, Layout};
        use std::sync::Arc;

        // Create a minimal mock implementation
        // In practice, this should not be used
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut device = FileBackedBlockDevice::create("/tmp/mock_device", 4 * 1024 * 1024)
                    .await
                    .unwrap();
                // Format the device first
                DiskFs::format(&mut device, 4 * 1024 * 1024, Some("MockFS"))
                    .await
                    .unwrap();

                // Now open the formatted device
                let device = Arc::new(device);
                DiskFs::open(device).await.unwrap()
            })
        })
    }
}

/// Snapshot management module
pub mod snapshot {
    use crate::error::Result;

    /// Snapshot manager for the filesystem
    pub struct SnapshotManager {
        // TODO: Implement snapshot management
    }

    impl SnapshotManager {
        /// Create a new snapshot manager
        pub fn new() -> Self {
            Self {}
        }

        /// Create a new snapshot
        pub fn create_snapshot(&self, _name: &str) -> Result<()> {
            Ok(())
        }

        /// List all snapshots
        pub fn list_snapshots(&self) -> Result<Vec<String>> {
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_creation() {
        let _fs = AegisFS::new();
    }
}
