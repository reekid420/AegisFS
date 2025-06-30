# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

AegisFS is a modern, feature-rich filesystem implementation built in Rust. It's designed as a FUSE-based filesystem with planned kernel module support. The project is structured around a modular architecture with optional features for encryption, compression, journaling, and snapshots.

## Build Commands

### Primary Build Commands
- `cargo build` - Build the core library
- `cargo build --release` - Release build
- `cargo test` - Run all tests
- `cargo bench` - Run benchmarks

### Binary Targets
- `cargo run --bin aegisfs-format -- <device> <size>` - Format a device with AegisFS
- `cargo run --bin aegisfs-mount -- <device> <mountpoint>` - Mount an AegisFS filesystem

### Feature Flags
- `cargo build --features encryption` - Build with encryption support
- `cargo build --features compression` - Build with compression support
- `cargo build --features fuse` - Build with FUSE support (default)
- `cargo build --no-default-features` - Build without default features

## Architecture Overview

### Core Components

**Block Device Layer** (`src/blockdev/`):
- `BlockDevice` trait - Abstraction for storage devices
- `FileBackedBlockDevice` - File-based block device implementation
- Async I/O with block-level operations

**Cache Layer** (`src/cache.rs`):
- `BlockCache` - LRU cache for block device operations
- Thread-safe with parking_lot RwLock
- Configurable cache size and write-through/write-back modes

**On-Disk Layout** (`src/layout.rs`):
- `Layout` struct - Calculates filesystem layout (superblock, bitmaps, inode table, data blocks)
- `DiskFs` - On-disk filesystem implementation with async trait
- Superblock, inode, and directory entry management

**Format Layer** (`src/format/`):
- On-disk data structure definitions
- Superblock, inode, and directory entry serialization/deserialization
- Magic number validation and versioning

**VFS Layer** (`src/lib.rs`):
- `AegisFS` - Main FUSE filesystem implementation
- `VFS` - Virtual filesystem abstraction
- `Inode` - In-memory inode representation
- Complete FUSE operations: lookup, getattr, readdir, create, write, read, mkdir, unlink, rmdir, rename, setattr

**Error Handling** (`src/error.rs`):
- Unified error types for the entire filesystem
- Conversion between different error types (I/O, filesystem, format errors)

### Modular Architecture

The filesystem is designed with optional modules in `src/modules/`:
- `journaling/` - Transaction support (planned)
- `snapshot/` - Point-in-time snapshots (planned)
- `encryption/` - AES-GCM encryption (planned)
- `compression/` - LZ4/ZSTD compression (planned)
- `checksums/` - Block-level integrity checking (planned)
- `tiering/` - Hot/cold data tiering (planned)
- `dedupe/` - Data deduplication (planned)
- `audit_logs/` - Audit logging (planned)
- `metrics/` - Performance metrics (planned)

## Common Development Issues

### Compilation Errors to Fix

The codebase currently has several compilation errors that need to be addressed:

1. **Visibility Qualifiers in Traits** (`src/layout.rs:146,172,264,282`):
   - Remove `pub` qualifiers from trait method implementations
   - Trait items always share the visibility of their trait

2. **Missing Dependencies**:
   - `array_ref` macro not in scope (`src/cache.rs:120`)
   - Add `arrayref = "0.3"` to Cargo.toml or use alternative approach

3. **Missing Imports** (`src/layout.rs:199,331`):
   - Import `Inode` from the correct module
   - Import `FormatError` from `crate::format`

4. **Trait Method Implementations**:
   - `DiskInode::read_from` method not found
   - Need to implement serialization traits for `Inode` struct

5. **Borrowing Issues**:
   - Read-only lock cannot be borrowed as mutable (`src/cache.rs:71`)
   - Use write lock when modification is needed

### Testing Strategy

- Unit tests for each module
- Integration tests for filesystem operations
- Property-based testing for filesystem consistency
- Performance benchmarks using `criterion`
- Test with various block device sizes and configurations

### Code Patterns

- Async/await throughout for non-blocking I/O
- Error handling with `thiserror` and `anyhow`
- Thread-safe operations using `parking_lot` locks
- Feature-gated compilation for optional modules
- Trait-based abstractions for extensibility

## Development Workflow

1. **Before Making Changes**: Always run `cargo check` to identify compilation issues
2. **Testing**: Run specific tests with `cargo test <test_name>`
3. **Formatting**: Code uses `rustfmt` - run `cargo fmt` before committing
4. **Linting**: Use `cargo clippy` for additional code quality checks
5. **Documentation**: Update docs with `cargo doc --open`

## Current Status

The project is in **Phase 1** of development according to the roadmap:
- ✅ Core FUSE filesystem implementation
- ✅ Basic file operations (create, read, write, delete)
- ✅ Directory operations (mkdir, rmdir, list)
- ✅ Format and mount tools
- 🚧 Need to fix compilation errors before proceeding
- 🚧 Journaling, snapshots, and encryption modules are stubs

Next priorities:
1. Fix compilation errors
2. Complete block cache implementation  
3. Implement journaling for crash consistency
4. Add snapshot functionality
5. Performance optimization and benchmarking