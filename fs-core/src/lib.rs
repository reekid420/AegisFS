//! AegisFS Core - A modern, feature-rich filesystem implementation
//!
//! This crate provides the core functionality for AegisFS, including the filesystem
//! implementation, VFS layer, and various modules for features like encryption,
//! compression, and snapshots.

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rust_2018_idioms)]
#![allow(dead_code)] // TODO: Remove in production

// Re-export the format module for use in binaries
pub mod format;
pub mod error;

use std::path::Path;
use std::time::{Duration, SystemTime};
use std::ffi::OsStr;
use std::collections::HashMap;
use parking_lot::RwLock;
use fuser::{
    FileAttr, FileType, Filesystem, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEntry, ReplyWrite, Request,
};
use libc::ENOENT;

// Re-export the error types
pub use error::{Error, Result};

// Time-to-live for file attributes (1 second)
const TTL: Duration = Duration::from_secs(1);

/// Re-export common types and traits
pub mod prelude {
    pub use crate::error::Result;
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
}

// Inode number constants
const ROOT_INODE: u64 = 1;
const INVALID_INODE: u64 = 0;

/// File attributes for the filesystem
#[derive(Debug, Clone)]
pub struct Inode {
    /// Inode number
    pub ino: u64,
    /// Parent inode number
    pub parent: u64,
    /// Name of the entry
    pub name: String,
    /// File attributes
    pub attr: FileAttr,
    /// File content (for regular files)
    pub data: Vec<u8>,
    /// Children inodes (for directories)
    pub children: HashMap<String, u64>,
}

impl Inode {
    /// Create a new inode
    pub fn new(ino: u64, parent: u64, name: &str, kind: FileType) -> Self {
        let now = SystemTime::now();

        let (perm, size) = match kind {
            FileType::Directory => (0o755, 0),
            _ => (0o644, 0),
        };

        Self {
            ino,
            parent,
            name: name.to_string(),
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
                nlink: 1,
                uid: unsafe { libc::getuid() },
                gid: unsafe { libc::getgid() },
                rdev: 0,
                flags: 0,
                blksize: 4096,
            },
            data: Vec::new(),
            children: HashMap::new(),
        }
    }
}

/// Virtual File System (VFS) implementation
pub struct VFS {
    /// Inode counter
    next_ino: RwLock<u64>,
    /// Inode storage
    inodes: RwLock<HashMap<u64, Inode>>,
}

impl VFS {
    /// Create a new VFS instance
    pub fn new() -> Self {
        let vfs = Self {
            next_ino: RwLock::new(ROOT_INODE + 1),
            inodes: RwLock::new(HashMap::new()),
        };
        
        // Create root directory
        let root = Inode::new(ROOT_INODE, ROOT_INODE, "/", FileType::Directory);
        vfs.inodes.write().insert(ROOT_INODE, root);
        
        vfs
    }
    
    /// Get the next available inode number
    fn next_ino(&self) -> u64 {
        let mut next = self.next_ino.write();
        let ino = *next;
        *next += 1;
        ino
    }
    
    /// Lookup an inode by its number
    pub fn get_inode(&self, ino: u64) -> Option<Inode> {
        self.inodes.read().get(&ino).cloned()
    }
    
    /// Get a mutable reference to an inode by its number
    pub fn get_inode_mut(&self, ino: u64) -> Option<Inode> {
        self.inodes.read().get(&ino).cloned()
    }
    
    /// Update an inode
    pub fn update_inode<T, F>(&self, ino: u64, f: F) -> Option<T>
    where
        F: FnOnce(&mut Inode) -> T,
    {
        let mut inodes = self.inodes.write();
        if let Some(inode) = inodes.get_mut(&ino) {
            Some(f(inode))
        } else {
            None
        }
    }
    
    /// Lookup an inode by its parent and name
    pub fn lookup(&self, parent: u64, name: &OsStr) -> Option<Inode> {
        let inodes = self.inodes.read();
        inodes.get(&parent).and_then(|parent_inode| {
            parent_inode.children.get(name.to_str()?)
                .and_then(|ino| inodes.get(ino).cloned())
        })
    }
    
    /// Create a new file or directory
    pub fn create(
        &self,
        parent: u64,
        name: &OsStr,
        kind: FileType,
    ) -> crate::error::Result<Inode> {
        let name = name.to_str().ok_or_else(|| FileSystemError::InvalidName)
            .map_err(|e| crate::error::Error::from(e))?;
        
        let mut inodes = self.inodes.write();
        
        // Check if parent exists and is a directory
        let parent_inode = inodes.get_mut(&parent)
            .ok_or_else(|| crate::error::Error::Other("Parent not found".to_string()))?;
            
        if parent_inode.attr.kind != FileType::Directory {
            return Err(FileSystemError::NotADirectory.into());
        }
        
        // Check if name already exists
        if parent_inode.children.contains_key(name) {
            return Err(FileSystemError::AlreadyExists.into());
        }
        
        // Create new inode
        let ino = self.next_ino();
        let inode = Inode::new(ino, parent, name, kind);
        
        // Update parent's children
        parent_inode.children.insert(name.to_string(), ino);
        
        // Store the new inode
        inodes.insert(ino, inode.clone());
        
        Ok(inode)
    }
}



/// Filesystem implementation
pub struct AegisFS {
    vfs: VFS,
}

impl AegisFS {
    /// Create a new AegisFS instance
    pub fn new() -> Self {
        Self {
            vfs: VFS::new(),
        }
    }
    
    /// Initialize the filesystem on the given device
    pub fn format_device<P: AsRef<Path>>(_device: P) -> crate::error::Result<()> {
        // TODO: Implement device formatting
        Ok(())
    }
}

impl Filesystem for AegisFS {
    fn lookup(&mut self, _req: &Request<'_>, parent: u64, name: &OsStr, reply: ReplyEntry) {
        if let Some(inode) = self.vfs.lookup(parent, name) {
            reply.entry(&TTL, &inode.attr, 0);
        } else {
            reply.error(ENOENT);
        }
    }

    fn getattr(&mut self, _req: &Request<'_>, ino: u64, reply: ReplyAttr) {
        if let Some(inode) = self.vfs.get_inode(ino) {
            reply.attr(&TTL, &inode.attr);
        } else {
            reply.error(ENOENT);
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
        if let Some(inode) = self.vfs.get_inode(ino) {
            if inode.attr.kind != FileType::Directory {
                reply.error(libc::ENOTDIR);
                return;
            }

            // Add . and .. entries
            if offset <= 0 {
                if reply.add(ino, 1, FileType::Directory, ".") {
                    return;
                }
            }
            if offset <= 1 {
                if reply.add(inode.parent, 2, FileType::Directory, "..") {
                    return;
                }
            }

            // Add directory entries
            for (i, (name, &ino)) in inode.children.iter().enumerate() {
                if let Some(child) = self.vfs.get_inode(ino) {
                    let offset = i as i64 + 3; // +3 because of . and .. and 1-based index
                    if offset <= offset {
                        continue;
                    }
                    if reply.add(ino, offset, child.attr.kind, name) {
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
        match self.vfs.create(parent, name, FileType::RegularFile) {
            Ok(inode) => {
                reply.created(&TTL, &inode.attr, 0, 0, 0);
            }
            Err(_) => {
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
        // Get current time for mtime/ctime updates
        let now = SystemTime::now();
            
        // Get a mutable reference to the inode
        if let Some(mut inode) = self.vfs.get_inode_mut(ino) {
            // Ensure we're working with a regular file
            if inode.attr.kind != FileType::RegularFile {
                reply.error(libc::EISDIR);
                return;
            }
            
            let offset = offset as usize;
            let data_len = data.len();
            
            // Calculate new size and resize if needed
            let new_size = std::cmp::max(offset + data_len, inode.data.len());
            if inode.data.len() < new_size {
                inode.data.resize(new_size, 0);
            }
            
            // Write the data
            inode.data[offset..offset + data_len].copy_from_slice(data);
            
            // Update metadata
            inode.attr.size = new_size as u64;
            inode.attr.blocks = ((new_size + 511) / 512) as u64;
            inode.attr.mtime = SystemTime::now();
            inode.attr.ctime = SystemTime::now();
            
            // Update the inode in the VFS
            if let Some(()) = self.vfs.update_inode(ino, |i| *i = inode) {
                reply.written(data_len as u32);
                return;
            }
            
            reply.error(libc::EIO);
        } else {
            reply.error(libc::ENOENT);
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
        match self.vfs.create(parent, name, FileType::Directory) {
            Ok(inode) => {
                reply.entry(&TTL, &inode.attr, 0);
            }
            Err(_) => {
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
        let inodes = self.vfs.inodes.read();
        if let Some(inode) = inodes.get(&ino) {
            if inode.attr.kind != FileType::RegularFile {
                reply.error(libc::EISDIR);
                return;
            }
            
            let offset = offset as usize;
            let data = if offset < inode.data.len() {
                let end = std::cmp::min(offset + size as usize, inode.data.len());
                &inode.data[offset..end]
            } else {
                &[]
            };
            
            reply.data(data);
        } else {
            reply.error(ENOENT);
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
        atime: Option<fuser::TimeOrNow>,
        mtime: Option<fuser::TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        // Get current time as SystemTime for ctime
        let now_system = SystemTime::now();
        let now = now_system
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if let Some(mut inode) = self.vfs.get_inode_mut(ino) {
            // Update mode if provided
            if let Some(mode) = mode {
                inode.attr.perm = mode as u16;
            }
            
            // Update UID if provided
            if let Some(uid) = uid {
                inode.attr.uid = uid;
            }
            
            // Update GID if provided
            if let Some(gid) = gid {
                inode.attr.gid = gid;
            }
            
            // Update size if provided (truncate or extend the file)
            if let Some(new_size) = size {
                inode.data.resize(new_size as usize, 0);
                inode.attr.size = new_size;
                inode.attr.blocks = ((new_size + 511) / 512) as u64;
            }
            
            // Update access time
            if let Some(atime) = atime {
                inode.attr.atime = match atime {
                    fuser::TimeOrNow::SpecificTime(t) => t,
                    fuser::TimeOrNow::Now => SystemTime::now(),
                };
            }
            
            // Update modification time
            if let Some(mtime) = mtime {
                inode.attr.mtime = match mtime {
                    fuser::TimeOrNow::SpecificTime(t) => t,
                    fuser::TimeOrNow::Now => SystemTime::now(),
                };
            }
            
            // Update change time to now
            inode.attr.ctime = SystemTime::now();
            
            // Update the inode in the VFS
            if self.vfs.update_inode(ino, |i| *i = inode).is_some() {
                reply.attr(&TTL, &self.vfs.get_inode(ino).unwrap().attr);
                return;
            }
        }
        
        // If we get here, something went wrong
        reply.error(libc::EIO);
    }
    
    fn unlink(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        reply: fuser::ReplyEmpty,
    ) {
        // Convert name to string early to avoid borrowing issues
        let name = match name.to_str() {
            Some(name) => name.to_string(),
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        // First, get the child ino from the parent
        let child_ino = {
            let inodes = self.vfs.inodes.read();
            let parent_inode = match inodes.get(&parent) {
                Some(inode) => inode,
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            };
            *parent_inode.children.get(&name).unwrap_or(&0)
        };

        if child_ino == 0 {
            reply.error(libc::ENOENT);
            return;
        }

        // Check if it's a directory (can't unlink directories with unlink)
        {
            let inodes = self.vfs.inodes.read();
            if let Some(child_inode) = inodes.get(&child_ino) {
                if child_inode.attr.kind == FileType::Directory {
                    reply.error(libc::EISDIR);
                    return;
                }
            } else {
                reply.error(libc::ENOENT);
                return;
            }
        }

        // Remove from parent's children
        {
            let mut inodes = self.vfs.inodes.write();
            if let Some(parent_inode) = inodes.get_mut(&parent) {
                parent_inode.children.remove(&name);
            }
        }

        // Remove the inode itself
        {
            let mut inodes = self.vfs.inodes.write();
            inodes.remove(&child_ino);
        }

        reply.ok();
    }
    
    fn rmdir(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        reply: fuser::ReplyEmpty,
    ) {
        // Convert name to string early to avoid borrowing issues
        let name = match name.to_str() {
            Some(name) => name.to_string(),
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        // First, get the child ino from the parent
        let child_ino = {
            let inodes = self.vfs.inodes.read();
            let parent_inode = match inodes.get(&parent) {
                Some(inode) => inode,
                None => {
                    reply.error(libc::ENOENT);
                    return;
                }
            };
            *parent_inode.children.get(&name).unwrap_or(&0)
        };

        if child_ino == 0 {
            reply.error(libc::ENOENT);
            return;
        }

        // Check if it's a directory and empty
        {
            let inodes = self.vfs.inodes.read();
            if let Some(child_inode) = inodes.get(&child_ino) {
                if child_inode.attr.kind != FileType::Directory {
                    reply.error(libc::ENOTDIR);
                    return;
                }
                
                if !child_inode.children.is_empty() {
                    reply.error(libc::ENOTEMPTY);
                    return;
                }
            } else {
                reply.error(libc::ENOENT);
                return;
            }
        }


        // Remove from parent's children
        {
            let mut inodes = self.vfs.inodes.write();
            if let Some(parent_inode) = inodes.get_mut(&parent) {
                parent_inode.children.remove(&name);
            }
        }


        // Remove the inode itself
        let mut inodes = self.vfs.inodes.write();
        inodes.remove(&child_ino);
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
        // Convert names to strings early to avoid borrowing issues
        let name = match name.to_str() {
            Some(name) => name.to_string(),
            None => {
                reply.error(libc::EINVAL);
                return;
            }
        };
    
    let newname = match newname.to_str() {
        Some(name) => name.to_string(),
        None => {
            reply.error(libc::EINVAL);
            return;
        }
    };
    
    // Step 1: Get the source inode number and verify source parent exists
    let (src_ino, src_parent_children) = {
        let inodes = self.vfs.inodes.read();
        let src_parent = match inodes.get(&parent) {
            Some(p) => p,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        // Get the source inode number
        let src_ino = match src_parent.children.get(&name) {
            Some(ino) => *ino,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        (src_ino, src_parent.children.clone())
    };
    
    // Step 2: Check if destination parent exists and get the source inode
    let (mut src_inode, dst_parent_children) = {
        let inodes = self.vfs.inodes.read();
        
        // Check if destination parent exists
        let dst_parent = match inodes.get(&newparent) {
            Some(p) => p,
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        // Get the source inode
        let src_inode = match inodes.get(&src_ino) {
            Some(i) => i.clone(),
            None => {
                reply.error(libc::ENOENT);
                return;
            }
        };
        
        (src_inode, dst_parent.children.clone())
    };
    
    // Check if destination name already exists and handle it
    if let Some(&existing_ino) = dst_parent_children.get(&newname) {
        // If it's the same inode, we're done
        if existing_ino == src_ino {
            reply.ok();
            return;
        }
        
        // Otherwise, remove the existing entry
        let mut inodes = self.vfs.inodes.write();
        if let Some(dst_parent) = inodes.get_mut(&newparent) {
            dst_parent.children.remove(&newname);
        }
        inodes.remove(&existing_ino);
    }
    
    // Step 3: Update the inode's parent and name
    src_inode.parent = newparent;
    src_inode.name = newname.clone();
    
    // Step 4: Update the VFS
    let mut inodes = self.vfs.inodes.write();
    
    // Remove from source parent
    if let Some(src_parent) = inodes.get_mut(&parent) {
        src_parent.children.remove(&name);
    }
    
    // Add to destination parent
    if let Some(dst_parent) = inodes.get_mut(&newparent) {
        dst_parent.children.insert(newname, src_ino);
    }
    
    // Update the inode in the VFS
    inodes.insert(src_ino, src_inode);
    
    reply.ok();
}
}

/// Journaling module for transaction support
pub mod journaling {
    use super::error::Result;
    
    /// Journal manager for handling transactions
    pub struct JournalManager {
        // TODO: Implement journaling
    }
    
    impl JournalManager {
        /// Create a new journal manager
        pub fn new() -> Self {
            Self {}
        }
        
        /// Start a new transaction
        pub fn begin_transaction(&self) -> Result<()> {
            todo!("Journal transaction not implemented")
        }
        
        /// Commit the current transaction
        pub fn commit_transaction(&self) -> Result<()> {
            todo!("Commit transaction not implemented")
        }
    }
}

/// Snapshot module for point-in-time recovery
pub mod snapshot {
    use super::error::Result;
    
    /// Snapshot manager for handling filesystem snapshots
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
            todo!("Snapshot creation not implemented")
        }
        
        /// List all snapshots
        pub fn list_snapshots(&self) -> Result<Vec<String>> {
            todo!("Snapshot listing not implemented")
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_filesystem_creation() {
        let _fs = AegisFS::new();
        // Basic test to ensure the filesystem can be created
        assert!(true);
    }
}
