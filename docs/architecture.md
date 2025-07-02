# AegisFS Architecture

## Overview

AegisFS is a modern, modular filesystem implemented in Rust with a focus on safety, performance, and extensibility. The architecture is designed around a FUSE-based userspace implementation with pluggable modules for advanced features like journaling, snapshots, encryption, and compression.

## System Architecture

### High-Level Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        User Applications                        │
└─────────────────────────┬───────────────────────────────────────┘
                          │ POSIX File System Interface
┌─────────────────────────▼───────────────────────────────────────┐
│                    Linux Kernel VFS                            │
└─────────────────────────┬───────────────────────────────────────┘
                          │ FUSE Protocol
┌─────────────────────────▼───────────────────────────────────────┐
│                     AegisFS (Userspace)                        │
│  ┌─────────────────┬─────────────────┬─────────────────────────┐ │
│  │   FUSE Layer    │  Module System  │    Management APIs      │ │
│  │                 │                 │                         │ │
│  │ • File Ops      │ • Journaling    │ • CLI Interface         │ │
│  │ • Directory Ops │ • Snapshots     │ • GUI Interface         │ │
│  │ • Metadata      │ • Checksums     │ • REST APIs             │ │
│  │ • Caching       │ • Encryption    │                         │ │
│  └─────────────────┴─────────────────┴─────────────────────────┘ │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │              Block Device Abstraction                     │   │
│  │  • File-backed devices  • Real block devices             │   │
│  │  • NVMe/SSD support    • Cross-platform I/O             │   │
│  └───────────────────────────────────────────────────────────┘   │
└─────────────────────────┬───────────────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────────────┐
│              Physical Storage (Files/Block Devices)             │
└─────────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. FUSE Filesystem Layer (`fs-core/src/lib.rs`)

The main filesystem implementation provides:

#### **AegisFS Struct**
- **Primary Interface**: Implements the `fuser::Filesystem` trait
- **Async Runtime**: Uses Tokio for non-blocking I/O operations
- **Memory Management**: LRU cache for inodes with write-back strategy

#### **Key Operations**
```rust
impl Filesystem for AegisFS {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry);
    fn getattr(&mut self, req: &Request, ino: u64, fh: Option<u64>, reply: ReplyAttr);
    fn create(&mut self, req: &Request, parent: u64, name: &OsStr, mode: u32, 
              flags: u32, umask: i32, reply: ReplyCreate);
    fn read(&mut self, req: &Request, ino: u64, fh: u64, offset: i64, 
            size: u32, flags: i32, lock_owner: Option<u64>, reply: ReplyData);
    fn write(&mut self, req: &Request, ino: u64, fh: u64, offset: i64, 
             data: &[u8], write_flags: u32, flags: i32, 
             lock_owner: Option<u64>, reply: ReplyWrite);
    // ... additional operations
}
```

#### **Data Persistence Strategy**
- **Write-back Cache**: 5-second flush interval for optimal performance
- **Small File Optimization**: Files ≤4KB cached entirely in memory
- **Immediate Sync**: Critical operations trigger instant disk writes
- **Error Recovery**: 3x retry logic with exponential backoff

### 2. Block Device Abstraction (`fs-core/src/blockdev/`)

#### **Unified Interface**
```rust
#[async_trait]
pub trait BlockDevice: Send + Sync {
    async fn read_block(&self, block_num: u64) -> Result<Vec<u8>, BlockDeviceError>;
    async fn write_block(&self, block_num: u64, data: &[u8]) -> Result<(), BlockDeviceError>;
    async fn flush(&self) -> Result<(), BlockDeviceError>;
    async fn size(&self) -> Result<u64, BlockDeviceError>;
}
```

#### **Implementation Types**
- **FileBackedBlockDevice**: For development and testing with regular files
- **Real Block Device Support**: Direct access to NVMe, SSD, HDD devices
- **Cross-platform**: Windows, macOS, and Linux implementations

### 3. On-Disk Format (`fs-core/src/format/` and `fs-core/src/layout.rs`)

#### **Superblock Structure**
```rust
pub struct Superblock {
    pub magic: [u8; 8],           // "AEGISFS\0"
    pub version: u32,             // Filesystem version
    pub size: u64,                // Total size in bytes
    pub block_size: u32,          // Block size (4096 bytes)
    pub block_count: u64,         // Total blocks
    pub free_blocks: u64,         // Available blocks
    pub inode_count: u64,         // Total inodes
    pub free_inodes: u64,         // Available inodes
    pub root_inode: u64,          // Root directory inode (1)
    pub last_mount: u64,          // Last mount timestamp
    pub last_write: u64,          // Last write timestamp
    pub uuid: [u8; 16],           // Filesystem UUID
    pub volume_name: [u8; 64],    // Human-readable name
}
```

#### **Disk Layout**
```
Block 0:    Superblock (4KB)
Block 1-N:  Inode Bitmap (tracks allocated inodes)
Block N+1-M: Inode Table (128-byte inodes)
Block M+1-P: Block Bitmap (tracks allocated data blocks)
Block P+1-End: Data Blocks (file/directory content)
```

#### **Inode Structure (128 bytes)**
```rust
pub struct Inode {
    pub mode: u32,              // File type and permissions
    pub uid: u32,               // Owner user ID
    pub gid: u32,               // Owner group ID
    pub size: u64,              // File size in bytes
    pub atime: u64,             // Last access time
    pub mtime: u64,             // Last modification time
    pub ctime: u64,             // Creation time
    pub links: u16,             // Hard link count
    pub blocks: u64,            // Allocated 512-byte blocks
    pub flags: u32,             // File flags
    pub block: [u64; 8],        // Direct block pointers (up to 32KB)
    // Additional fields for extended metadata
}
```

### 4. Caching System (`fs-core/src/cache.rs` and in-memory structures)

#### **Multi-Level Caching Strategy**

1. **Inode Cache** (`Arc<RwLock<HashMap<u64, CachedInode>>`)
   - Stores file metadata and small file data
   - LRU eviction policy
   - Write-back with periodic flush

2. **Directory Cache**
   - Parent-child relationship mapping
   - Accelerates path resolution
   - Persistent to disk

3. **Block Cache** (planned for larger files)
   - Page-based caching for large files
   - Integration with system page cache

#### **Write-Back Implementation**
```rust
struct WriteOperation {
    pub ino: u64,
    pub offset: u64,
    pub data: Vec<u8>,
    pub timestamp: SystemTime,
}

impl AegisFS {
    fn schedule_deferred_flush(&self) {
        // Avoid deadlock by using separate thread with 10ms delay
        // Allows current FUSE operation to complete first
    }
    
    fn flush_writes(&self) -> Result<()> {
        // Write cached data to disk
        // Update inode metadata
        // Clear dirty flags
    }
}
```

## Module System

### 1. Journaling Module (`fs-core/src/modules/journaling/`)

#### **Transaction Framework**
```rust
pub struct JournalManager {
    log_file: Arc<dyn BlockDevice>,
    current_transaction: Arc<RwLock<Option<Transaction>>>,
    commit_queue: Arc<RwLock<VecDeque<Transaction>>>,
}

pub struct Transaction {
    pub id: u64,
    pub operations: Vec<JournalEntry>,
    pub state: TransactionState,
    pub timestamp: SystemTime,
}
```

#### **Journal Entry Types**
- **Write Operations**: File data changes
- **Metadata Updates**: Inode modifications
- **Directory Changes**: File creation/deletion
- **Block Allocation**: Free space management

### 2. Snapshot Module (`fs-core/src/modules/snapshot/`)

#### **Copy-on-Write Implementation**
```rust
pub struct SnapshotManager {
    snapshots: Arc<RwLock<Vec<SnapshotMetadata>>>,
    cow_blocks: Arc<RwLock<HashMap<u64, u64>>>,
    metadata_file: PathBuf,
}

pub struct SnapshotMetadata {
    pub id: u64,
    pub name: String,
    pub timestamp: SystemTime,
    pub root_inode: u64,
    pub state: SnapshotState,
}
```

#### **Snapshot Operations**
- **Create**: Capture current filesystem state
- **List**: Show all available snapshots
- **Delete**: Remove snapshot and free CoW blocks
- **Rollback**: Restore filesystem to snapshot state

### 3. Checksum Module (`fs-core/src/modules/checksums/`)

#### **Data Integrity Framework**
```rust
pub struct ChecksumManager {
    algorithm: ChecksumAlgorithm,
    block_checksums: Arc<RwLock<HashMap<u64, [u8; 32]>>>,
    scrub_stats: Arc<RwLock<ScrubStats>>,
}

pub enum ChecksumAlgorithm {
    CRC32,
    SHA256,
    Blake3,
}
```

#### **Self-Healing Operations**
- **Block Verification**: Check data integrity on read
- **Background Scrubbing**: Systematic integrity checking
- **Automatic Repair**: Restore corrupted blocks from redundancy

## Data Flow Architecture

### Read Path
```
Application
    ↓ POSIX read()
Linux Kernel VFS
    ↓ FUSE protocol
AegisFS::read()
    ↓ Check inode cache
Cached Data Available?
    ↓ YES: Return cached data
    ↓ NO: Continue to disk
Block Device Read
    ↓ Checksum verification
Decompress (if enabled)
    ↓ Decrypt (if enabled)
Return to Application
```

### Write Path
```
Application
    ↓ POSIX write()
Linux Kernel VFS
    ↓ FUSE protocol
AegisFS::write()
    ↓ Journal transaction start
Encrypt (if enabled)
    ↓ Compress (if enabled)
Write to Cache
    ↓ Mark dirty
Schedule Flush
    ↓ Background write to disk
Update checksums
    ↓ Journal transaction commit
Return success to Application
```

## Performance Optimizations

### 1. Asynchronous I/O
- **Tokio Runtime**: Non-blocking operations
- **Thread Pool**: Dedicated threads for disk I/O
- **Future Chaining**: Efficient async operation composition

### 2. Intelligent Caching
- **Small File Optimization**: Files ≤4KB fully cached
- **Write Coalescing**: Batch multiple writes
- **Read-ahead**: Predictive data loading (planned)

### 3. Lock Optimization
- **Fine-grained Locking**: Minimize contention
- **Reader-Writer Locks**: Allow concurrent reads
- **Lock-free Structures**: Where possible

## Security Architecture

### 1. Memory Safety
- **Rust Ownership**: Prevents buffer overflows and use-after-free
- **Bounds Checking**: Array access validation
- **Safe Concurrency**: Data race prevention

### 2. Input Validation
- **Path Sanitization**: Prevent directory traversal
- **Size Limits**: Prevent resource exhaustion
- **Type Checking**: Validate all user inputs

### 3. Permission Model
- **POSIX Compliance**: Standard Unix permissions
- **User/Group Mapping**: Proper ownership handling
- **Access Control**: Operation-level permission checks

## Cross-Platform Considerations

### 1. Build System
- **Cargo Features**: Platform-specific compilation
- **Conditional Compilation**: OS-specific code paths
- **Cross-compilation**: Multi-target support

### 2. FUSE Abstraction
- **Linux**: libfuse3 integration
- **macOS**: macFUSE support
- **Windows**: WinFsp compatibility (planned)

## Extensibility Framework

### 1. Plugin Architecture (Planned)
```rust
pub trait FilesystemPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn on_read(&self, ino: u64, data: &mut [u8]) -> Result<()>;
    fn on_write(&self, ino: u64, data: &[u8]) -> Result<()>;
    fn on_create(&self, parent: u64, name: &str) -> Result<()>;
}
```

### 2. Module Registration
- **Dynamic Loading**: Runtime plugin discovery
- **Configuration**: YAML/TOML-based module settings
- **Dependency Management**: Inter-module dependencies

## Future Enhancements

### 1. Kernel Module Port
- **Performance**: Direct kernel integration
- **Compatibility**: Native VFS integration
- **Security**: Kernel-level permission enforcement

### 2. Distributed Features
- **Replication**: Multi-node data redundancy
- **Clustering**: Scale-out filesystem
- **Cloud Integration**: Cloud storage backends

### 3. Advanced Analytics
- **Usage Patterns**: ML-based optimization
- **Predictive Caching**: Intelligent prefetching
- **Performance Metrics**: Real-time monitoring

---

This architecture provides a solid foundation for a modern, extensible filesystem while maintaining the safety guarantees of Rust and the flexibility of userspace implementation.
