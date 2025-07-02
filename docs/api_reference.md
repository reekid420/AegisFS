# AegisFS API Reference

This document provides comprehensive API documentation for AegisFS, covering the core library, CLI interface, and module APIs.

## 📚 Table of Contents

- [Core Library API](#core-library-api)
- [CLI Interface](#cli-interface)
- [Module APIs](#module-apis)
- [Block Device API](#block-device-api)
- [Examples](#examples)

## 🚀 Core Library API

### AegisFS Struct

The main filesystem implementation that provides FUSE integration.

```rust
pub struct AegisFS {
    disk_fs: Arc<RwLock<DiskFs>>,
    inode_cache: Arc<RwLock<HashMap<u64, CachedInode>>>,
    next_ino: RwLock<u64>,
    runtime: Handle,
    write_cache: Arc<RwLock<Vec<WriteOperation>>>,
    flushing: Arc<AtomicBool>,
    inode_bitmap: Arc<RwLock<InodeBitmap>>,
    flush_task: Option<mpsc::UnboundedSender<FlushCommand>>,
}
```

#### Constructor Methods

```rust
impl AegisFS {
    /// Create a new AegisFS instance (mock/in-memory)
    pub fn new() -> Self;
    
    /// Create an AegisFS instance from a formatted device
    pub async fn from_device<P: AsRef<Path>>(device_path: P) -> Result<Self>;
}
```

#### Core Operations

```rust
impl AegisFS {
    /// Get a cached inode by number
    fn get_cached_inode(&self, ino: u64) -> Option<CachedInode>;
    
    /// Update an inode in the cache
    fn update_cached_inode(&self, ino: u64, cached: CachedInode) -> Result<()>;
    
    /// Create a new file or directory
    fn create_file(&self, parent: u64, name: &str, kind: FileType) -> Result<CachedInode>;
    
    /// Write data to a file
    fn write_file_data(&self, ino: u64, offset: u64, data: &[u8]) -> Result<u32>;
    
    /// Read data from a file
    fn read_file_data(&self, ino: u64, offset: u64, size: u32) -> Result<Vec<u8>>;
    
    /// Flush pending writes to disk
    fn flush_writes(&self) -> Result<()>;
    
    /// Schedule a deferred flush (avoids deadlocks)
    fn schedule_deferred_flush(&self);
}
```

### FUSE Interface

AegisFS implements the `fuser::Filesystem` trait for FUSE integration:

```rust
impl Filesystem for AegisFS {
    /// Look up a directory entry by name
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry);
    
    /// Get file attributes for an inode
    fn getattr(&mut self, req: &Request, ino: u64, fh: Option<u64>, reply: ReplyAttr);
    
    /// Create a new file
    fn create(&mut self, req: &Request, parent: u64, name: &OsStr, mode: u32, 
              flags: u32, umask: i32, reply: ReplyCreate);
    
    /// Read data from a file
    fn read(&mut self, req: &Request, ino: u64, fh: u64, offset: i64, 
            size: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyData);
    
    /// Write data to a file
    fn write(&mut self, req: &Request, ino: u64, fh: u64, offset: i64, 
             data: &[u8], write_flags: u32, flags: i32, 
             lock_owner: Option<u64>, reply: ReplyWrite);
    
    /// Read directory entries
    fn readdir(&mut self, req: &Request, ino: u64, fh: u64, 
               offset: i64, reply: ReplyDirectory);
    
    /// Create a directory
    fn mkdir(&mut self, req: &Request, parent: u64, name: &OsStr, 
             mode: u32, umask: u32, reply: ReplyEntry);
    
    /// Set file attributes
    fn setattr(&mut self, req: &Request, ino: u64, mode: Option<u32>, 
               uid: Option<u32>, gid: Option<u32>, size: Option<u64>, 
               // ... additional parameters
               reply: ReplyAttr);
    
    /// Remove a file
    fn unlink(&mut self, req: &Request, parent: u64, name: &OsStr, 
              reply: fuser::ReplyEmpty);
    
    /// Remove a directory
    fn rmdir(&mut self, req: &Request, parent: u64, name: &OsStr, 
             reply: fuser::ReplyEmpty);
    
    /// Rename a file or directory
    fn rename(&mut self, req: &Request, parent: u64, name: &OsStr, 
              newparent: u64, newname: &OsStr, flags: u32, 
              reply: fuser::ReplyEmpty);
    
    /// Synchronize file data
    fn fsync(&mut self, req: &Request, ino: u64, fh: u64, 
             datasync: bool, reply: fuser::ReplyEmpty);
    
    /// Cleanup when filesystem is unmounted
    fn destroy(&mut self);
}
```

### Data Structures

#### CachedInode

Represents an in-memory inode with caching metadata:

```rust
#[derive(Debug, Clone)]
pub struct CachedInode {
    pub ino: u64,
    pub attr: FileAttr,
    pub children: HashMap<String, u64>,  // For directories
    pub last_access: SystemTime,
    pub dirty: bool,
    pub cached_data: Option<Vec<u8>>,    // For small files
}

impl CachedInode {
    /// Create a new cached inode
    pub fn new(ino: u64, kind: FileType) -> Self;
}
```

#### WriteOperation

Represents a pending write operation:

```rust
#[derive(Debug, Clone)]
pub struct WriteOperation {
    pub ino: u64,
    pub offset: u64,
    pub data: Vec<u8>,
    pub timestamp: SystemTime,
}
```

#### InodeBitmap

Manages inode allocation:

```rust
pub struct InodeBitmap {
    bitmap: Vec<u8>,
    total_inodes: u64,
    free_inodes: AtomicU64,
}

impl InodeBitmap {
    /// Create a new inode bitmap
    pub fn new(total_inodes: u64) -> Self;
    
    /// Load bitmap from disk
    pub async fn load_from_disk(disk_fs: &DiskFs, total_inodes: u64) -> Result<Self>;
    
    /// Save bitmap to disk
    pub async fn save_to_disk(&self, disk_fs: &DiskFs) -> Result<()>;
    
    /// Allocate a new inode
    pub fn allocate(&mut self) -> Option<u64>;
    
    /// Free an inode
    pub fn free(&mut self, inode_num: u64);
    
    /// Check if an inode is allocated
    pub fn is_allocated(&self, inode_num: u64) -> bool;
}
```

## 💻 CLI Interface

### Main Command Structure

```bash
aegisfs [GLOBAL_OPTIONS] <SUBCOMMAND> [SUBCOMMAND_OPTIONS]
```

#### Global Options

- `-v, --verbose`: Enable verbose output
- `-d, --debug`: Enable debug output
- `-h, --help`: Show help information
- `-V, --version`: Show version information

### Format Command

Format a device or file with AegisFS.

```bash
aegisfs format <DEVICE> --size <SIZE_GB> [OPTIONS]
```

#### Arguments

- `<DEVICE>`: Path to device or file to format

#### Options

- `--size <SIZE_GB>`: Size in gigabytes (required)
- `--force`: Force formatting (destroys existing data)
- `--volume-name <NAME>`: Set volume name
- `--block-size <SIZE>`: Block size in bytes (default: 4096)

#### Examples

```bash
# Format a file
aegisfs format test.img --size 1

# Format a real device (WARNING: destroys data)
sudo aegisfs format /dev/sdX --size 100 --force

# Format with custom volume name
aegisfs format test.img --size 5 --volume-name "MyData"
```

### Mount Command

Mount an AegisFS filesystem via FUSE.

```bash
aegisfs mount <DEVICE> <MOUNTPOINT> [OPTIONS]
```

#### Arguments

- `<DEVICE>`: Path to formatted device or file
- `<MOUNTPOINT>`: Directory to mount the filesystem

#### Options

- `--read-only`: Mount in read-only mode
- `--debug`: Enable FUSE debug output
- `--auto-unmount`: Automatically unmount on exit

#### Examples

```bash
# Mount a filesystem
mkdir testmnt
aegisfs mount test.img testmnt

# Mount with debug output
aegisfs mount test.img testmnt --debug

# Mount read-only
aegisfs mount test.img testmnt --read-only
```

### Snapshot Command

Manage filesystem snapshots.

```bash
aegisfs snapshot <DEVICE> <OPERATION> [OPTIONS]
```

#### Suboperations

##### Create Snapshot

```bash
aegisfs snapshot <DEVICE> create <NAME> [OPTIONS]
```

- `<NAME>`: Snapshot name

Options:
- `--description <DESC>`: Snapshot description

##### List Snapshots

```bash
aegisfs snapshot <DEVICE> list [OPTIONS]
```

Options:
- `--json`: Output in JSON format
- `--verbose`: Show detailed information

##### Delete Snapshot

```bash
aegisfs snapshot <DEVICE> delete <NAME_OR_ID> [OPTIONS]
```

- `<NAME_OR_ID>`: Snapshot name or ID

Options:
- `--force`: Force deletion without confirmation

##### Rollback to Snapshot

```bash
aegisfs snapshot <DEVICE> rollback <NAME_OR_ID> [OPTIONS]
```

- `<NAME_OR_ID>`: Snapshot name or ID to rollback to

Options:
- `--force`: Force rollback without confirmation

##### Show Statistics

```bash
aegisfs snapshot <DEVICE> stats
```

#### Examples

```bash
# Create a snapshot
aegisfs snapshot test.img create "before-update"

# List all snapshots
aegisfs snapshot test.img list

# List with JSON output
aegisfs snapshot test.img list --json

# Delete a snapshot
aegisfs snapshot test.img delete "before-update"

# Rollback to a snapshot
aegisfs snapshot test.img rollback "before-update"

# Show snapshot statistics
aegisfs snapshot test.img stats
```

### Scrub Command

Check and repair filesystem integrity.

```bash
aegisfs scrub <DEVICE> [OPTIONS]
```

#### Arguments

- `<DEVICE>`: Path to device or file to check

#### Options

- `--fix`: Attempt to fix errors found
- `--deep`: Perform deep integrity checking
- `--progress`: Show progress information

#### Examples

```bash
# Check filesystem integrity
aegisfs scrub test.img

# Check and fix errors
aegisfs scrub test.img --fix

# Deep integrity check with progress
aegisfs scrub test.img --deep --progress
```

## 🔧 Module APIs

### Journaling Module

Transaction-based journaling for data consistency.

```rust
use aegisfs::modules::journaling::*;

pub struct JournalManager {
    log_file: Arc<dyn BlockDevice>,
    current_transaction: Arc<RwLock<Option<Transaction>>>,
    commit_queue: Arc<RwLock<VecDeque<Transaction>>>,
}

impl JournalManager {
    /// Create a new journal manager
    pub fn new(log_device: Arc<dyn BlockDevice>) -> Self;
    
    /// Start a new transaction
    pub async fn begin_transaction(&self) -> Result<u64>;
    
    /// Add an operation to the current transaction
    pub async fn log_operation(&self, entry: JournalEntry) -> Result<()>;
    
    /// Commit the current transaction
    pub async fn commit_transaction(&self, transaction_id: u64) -> Result<()>;
    
    /// Abort the current transaction
    pub async fn abort_transaction(&self, transaction_id: u64) -> Result<()>;
    
    /// Recover from journal after crash
    pub async fn recover(&self) -> Result<()>;
}

pub struct Transaction {
    pub id: u64,
    pub operations: Vec<JournalEntry>,
    pub state: TransactionState,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Active,
    Committed,
    Aborted,
}

pub struct JournalEntry {
    pub header: JournalEntryHeader,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalEntryType {
    Write,
    InodeUpdate,
    DirectoryUpdate,
    BlockAllocation,
}
```

### Snapshot Module

Copy-on-write snapshot management.

```rust
use aegisfs::modules::snapshot::*;

pub struct SnapshotManager {
    snapshots: Arc<RwLock<Vec<SnapshotMetadata>>>,
    cow_blocks: Arc<RwLock<HashMap<u64, u64>>>,
    metadata_file: PathBuf,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    pub fn new(metadata_path: PathBuf) -> Self;
    
    /// Create a new snapshot
    pub async fn create_snapshot(&mut self, name: String, description: Option<String>) -> Result<u64>;
    
    /// Delete a snapshot
    pub async fn delete_snapshot(&mut self, id: u64) -> Result<()>;
    
    /// List all snapshots
    pub fn list_snapshots(&self) -> Vec<SnapshotMetadata>;
    
    /// Get snapshot by ID or name
    pub fn get_snapshot(&self, identifier: &str) -> Option<SnapshotMetadata>;
    
    /// Rollback to a snapshot
    pub async fn rollback_to_snapshot(&mut self, id: u64) -> Result<()>;
    
    /// Handle copy-on-write for a block
    pub async fn cow_block(&mut self, block_num: u64) -> Result<u64>;
    
    /// Get snapshot statistics
    pub fn get_stats(&self) -> SnapshotStats;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub id: u64,
    pub name: String,
    pub description: Option<String>,
    pub timestamp: SystemTime,
    pub root_inode: u64,
    pub state: SnapshotState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotState {
    Active,
    Deleted,
    Corrupted,
}

#[derive(Debug, Clone)]
pub struct SnapshotStats {
    pub total_snapshots: usize,
    pub active_snapshots: usize,
    pub total_cow_blocks: usize,
    pub disk_usage: u64,
}
```

### Checksum Module

Data integrity verification and self-healing.

```rust
use aegisfs::modules::checksums::*;

pub struct ChecksumManager {
    algorithm: ChecksumAlgorithm,
    block_checksums: Arc<RwLock<HashMap<u64, [u8; 32]>>>,
    scrub_stats: Arc<RwLock<ScrubStats>>,
}

impl ChecksumManager {
    /// Create a new checksum manager
    pub fn new(algorithm: ChecksumAlgorithm) -> Self;
    
    /// Calculate checksum for data
    pub fn calculate_checksum(&self, data: &[u8]) -> [u8; 32];
    
    /// Verify data against stored checksum
    pub async fn verify_block(&self, block_num: u64, data: &[u8]) -> Result<bool>;
    
    /// Update checksum for a block
    pub async fn update_checksum(&mut self, block_num: u64, data: &[u8]) -> Result<()>;
    
    /// Verify multiple blocks
    pub async fn verify_blocks(&self, blocks: &[(u64, Vec<u8>)]) -> Result<Vec<bool>>;
    
    /// Perform background scrubbing
    pub async fn scrub_filesystem(&mut self, block_device: &dyn BlockDevice) -> Result<ScrubStats>;
    
    /// Get scrubbing statistics
    pub fn get_scrub_stats(&self) -> ScrubStats;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumAlgorithm {
    CRC32,
    SHA256,
    Blake3,
}

#[derive(Debug, Default, Clone)]
pub struct ScrubStats {
    pub blocks_checked: u64,
    pub errors_found: u64,
    pub errors_fixed: u64,
    pub start_time: Option<SystemTime>,
    pub end_time: Option<SystemTime>,
}
```

## 💾 Block Device API

Abstract interface for block devices (files and real devices).

```rust
use aegisfs::blockdev::*;

#[async_trait]
pub trait BlockDevice: Send + Sync {
    /// Read a block from the device
    async fn read_block(&self, block_num: u64) -> Result<Vec<u8>, BlockDeviceError>;
    
    /// Write a block to the device
    async fn write_block(&self, block_num: u64, data: &[u8]) -> Result<(), BlockDeviceError>;
    
    /// Flush all pending writes
    async fn flush(&self) -> Result<(), BlockDeviceError>;
    
    /// Get the total size of the device
    async fn size(&self) -> Result<u64, BlockDeviceError>;
    
    /// Get the block size
    fn block_size(&self) -> u32;
    
    /// Check if the device is read-only
    fn is_read_only(&self) -> bool;
}

/// File-backed block device implementation
pub struct FileBackedBlockDevice {
    file: Arc<Mutex<File>>,
    block_size: u32,
    read_only: bool,
}

impl FileBackedBlockDevice {
    /// Open a file as a block device
    pub async fn open<P: AsRef<Path>>(path: P, read_only: bool) -> Result<Self, BlockDeviceError>;
    
    /// Create a new file-backed device
    pub async fn create<P: AsRef<Path>>(path: P, size: u64) -> Result<Self, BlockDeviceError>;
}

#[derive(Error, Debug)]
pub enum BlockDeviceError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Block out of range: {0}")]
    BlockOutOfRange(u64),
    
    #[error("Invalid block size: {0}")]
    InvalidBlockSize(u32),
    
    #[error("Device is read-only")]
    ReadOnly,
    
    #[error("Device not found: {0}")]
    DeviceNotFound(String),
}

pub type BlockResult<T> = std::result::Result<T, BlockDeviceError>;
```

### On-Disk Format API

```rust
use aegisfs::format::*;

/// Superblock structure
#[derive(Debug)]
pub struct Superblock {
    pub magic: [u8; 8],           // "AEGISFS\0"
    pub version: u32,             // Filesystem version
    pub size: u64,                // Total size in bytes
    pub block_size: u32,          // Block size (4096 bytes)
    pub block_count: u64,         // Total blocks
    pub free_blocks: u64,         // Available blocks
    pub inode_count: u64,         // Total inodes
    pub free_inodes: u64,         // Available inodes
    pub root_inode: u64,          // Root directory inode
    pub last_mount: u64,          // Last mount timestamp
    pub last_write: u64,          // Last write timestamp
    pub uuid: [u8; 16],           // Filesystem UUID
    pub volume_name: [u8; 64],    // Human-readable name
}

impl Superblock {
    /// Create a new superblock
    pub fn new(size: u64, volume_name: Option<&str>) -> io::Result<Self>;
    
    /// Write superblock to storage
    pub fn write_to<W: Write + Seek>(&self, writer: &mut W) -> io::Result<()>;
    
    /// Read superblock from storage
    pub fn read_from<R: Read + Seek>(reader: &mut R) -> Result<Self, FormatError>;
}

/// On-disk inode structure
#[derive(Debug, Clone)]
pub struct Inode {
    pub mode: u32,              // File type and permissions
    pub uid: u32,               // Owner user ID
    pub gid: u32,               // Owner group ID
    pub size: u64,              // File size in bytes
    pub atime: u64,             // Last access time
    pub mtime: u64,             // Last modification time
    pub ctime: u64,             // Creation time
    pub links: u16,             // Hard link count
    pub blocks: u64,            // Allocated blocks
    pub flags: u32,             // File flags
    pub block: [u64; 8],        // Direct block pointers
}

impl Inode {
    /// Write inode to buffer
    pub fn write_to<W: Write>(&self, buf: &mut W) -> io::Result<()>;
}

/// Directory entry structure
#[derive(Debug)]
pub struct DirEntry {
    pub inode: u64,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    pub name: String,
}

impl DirEntry {
    /// Create a new directory entry
    pub fn new(inode: u64, name: &str) -> Self;
    
    /// Write directory entry to storage
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()>;
    
    /// Read directory entry from storage
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Self>;
}

/// Format a device with AegisFS
pub async fn format_device<P: AsRef<Path>>(
    device_path: P,
    size_gb: u64,
    volume_name: Option<&str>,
) -> Result<(), FormatError>;
```

## 📋 Examples

### Basic Filesystem Operations

```rust
use aegisfs::prelude::*;
use tempfile::TempDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary filesystem
    let temp_dir = TempDir::new()?;
    let device_path = temp_dir.path().join("test.img");
    
    // Format the device
    aegisfs::format::format_device(&device_path, 1, Some("TestFS")).await?;
    
    // Create filesystem instance
    let mut fs = AegisFS::from_device(&device_path).await?;
    
    // The filesystem is now ready for FUSE mounting
    // In practice, you would pass this to fuser::mount()
    
    Ok(())
}
```

### Snapshot Management

```rust
use aegisfs::modules::snapshot::*;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let metadata_path = PathBuf::from("snapshots.json");
    let mut snapshot_manager = SnapshotManager::new(metadata_path);
    
    // Create a snapshot
    let snapshot_id = snapshot_manager
        .create_snapshot("backup-001".to_string(), Some("Before update".to_string()))
        .await?;
    
    println!("Created snapshot with ID: {}", snapshot_id);
    
    // List snapshots
    let snapshots = snapshot_manager.list_snapshots();
    for snapshot in snapshots {
        println!("Snapshot: {} ({})", snapshot.name, snapshot.id);
    }
    
    // Get statistics
    let stats = snapshot_manager.get_stats();
    println!("Total snapshots: {}", stats.total_snapshots);
    
    Ok(())
}
```

### Block Device Usage

```rust
use aegisfs::blockdev::*;

#[tokio::main]
async fn main() -> Result<(), BlockDeviceError> {
    // Create a file-backed block device
    let device = FileBackedBlockDevice::create("test.img", 1024 * 1024).await?;
    
    // Write some data
    let test_data = vec![0x42; 4096];
    device.write_block(0, &test_data).await?;
    
    // Read it back
    let read_data = device.read_block(0).await?;
    assert_eq!(test_data, read_data);
    
    // Flush to ensure data is written
    device.flush().await?;
    
    println!("Block device test completed successfully");
    
    Ok(())
}
```

### Journaling Example

```rust
use aegisfs::modules::journaling::*;
use aegisfs::blockdev::*;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a journal device
    let journal_device = Arc::new(
        FileBackedBlockDevice::create("journal.log", 100 * 1024 * 1024).await?
    );
    
    let journal_manager = JournalManager::new(journal_device);
    
    // Begin a transaction
    let txn_id = journal_manager.begin_transaction().await?;
    
    // Log some operations
    let entry = JournalEntry {
        header: JournalEntryHeader {
            entry_type: JournalEntryType::Write,
            transaction_id: txn_id,
            timestamp: std::time::SystemTime::now(),
            data_len: 100,
        },
        data: vec![0; 100],
    };
    
    journal_manager.log_operation(entry).await?;
    
    // Commit the transaction
    journal_manager.commit_transaction(txn_id).await?;
    
    println!("Transaction {} committed successfully", txn_id);
    
    Ok(())
}
```

---

This API reference provides comprehensive documentation for all public interfaces in AegisFS. For more examples and usage patterns, see the tests in the `fs-core/tests/` directory.