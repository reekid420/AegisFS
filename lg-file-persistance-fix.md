# Large File Persistence Fix Plan for AegisFS

## Executive Summary

This document outlines a comprehensive plan to fix the large file writing issues in AegisFS by implementing proper block bitmap management. The current implementation uses a naive static counter for block allocation, which causes failures when writing large files or after multiple file operations.

---

## Problem Statement

### Current Issues
1. **Static Block Allocation**: Uses `static mut NEXT_BLOCK: u64 = 1` that increments forever
2. **No Block Bitmap**: Unlike inode allocation, there's no bitmap to track free/used blocks
3. **No Block Reuse**: Deleted file blocks are never freed or reused
4. **Persistence Issues**: Block allocation state is not persisted across mounts
5. **Test Failures**: 2MB file creation fails due to running out of blocks

### Root Cause Analysis
```rust
// Current problematic implementation in fs-core/src/layout.rs
async fn allocate_data_block(&mut self) -> Result<u64, FsError> {
    // TODO: Implement proper block bitmap management
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
```

---

## Proposed Solution

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Superblock                            │
│  - block_count, free_blocks tracking                        │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                     Block Bitmap                             │
│  - Bit array tracking free/used blocks                      │
│  - Persisted to disk at layout.block_bitmap                 │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                     Inode Bitmap                             │
│  - Already implemented and working                           │
└─────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────┐
│                    Block Allocator                           │
│  - Manages block bitmap                                      │
│  - Allocates/frees blocks                                   │
│  - Integrates with DiskFs                                   │
└─────────────────────────────────────────────────────────────┘
```

### Detailed Design

#### 1. BlockBitmap Structure
```rust
pub struct BlockBitmap {
    /// Bitmap data
    bitmap: Vec<u8>,
    /// Total number of blocks
    total_blocks: u64,
    /// Number of free blocks
    free_blocks: AtomicU64,
    /// Starting block for data blocks
    data_blocks_start: u64,
    /// Number of data blocks
    data_blocks_count: u64,
}
```

#### 2. Key Methods
- `new()` - Create bitmap for formatting
- `load_from_disk()` - Load existing bitmap
- `save_to_disk()` - Persist bitmap
- `allocate()` - Find and allocate free block
- `free()` - Mark block as free
- `is_allocated()` - Check block status

---

## Implementation Plan

### Phase 1: Core Block Bitmap Implementation (2-3 days)

#### 1.1 Create BlockBitmap Structure
- [ ] Create `fs-core/src/block_bitmap.rs`
- [ ] Implement BlockBitmap struct with atomic operations
- [ ] Add allocation/deallocation methods
- [ ] Add persistence methods (load/save)

#### 1.2 Update Layout Structure
- [ ] Ensure Layout properly calculates block bitmap location
- [ ] Verify block bitmap size calculations
- [ ] Update format operation to initialize block bitmap

#### 1.3 Integrate with DiskFs
- [ ] Add `block_bitmap: Arc<RwLock<BlockBitmap>>` to DiskFs
- [ ] Replace static counter with bitmap allocation
- [ ] Load block bitmap during filesystem mount
- [ ] Save block bitmap during unmount/sync

### Phase 2: File Operation Integration (2-3 days)

#### 2.1 Update Block Allocation
- [ ] Modify `allocate_data_block()` to use BlockBitmap
- [ ] Handle allocation failures gracefully
- [ ] Update free block count in superblock

#### 2.2 Implement Block Deallocation
- [ ] Create `deallocate_data_block()` method
- [ ] Free blocks when files are deleted
- [ ] Free blocks when files are truncated
- [ ] Handle indirect blocks properly

#### 2.3 Update File Deletion
- [ ] Modify `unlink()` to free file blocks
- [ ] Modify `rmdir()` to free directory blocks
- [ ] Ensure proper cleanup on errors

### Phase 3: Persistence & Recovery (1-2 days)

#### 3.1 Bitmap Persistence
- [ ] Save bitmap during deferred flush
- [ ] Add bitmap to fsync operations
- [ ] Ensure atomic updates with inode bitmap

#### 3.2 Recovery & Consistency
- [ ] Implement bitmap consistency checks
- [ ] Add recovery for corrupted bitmaps
- [ ] Verify block counts match superblock

### Phase 4: Testing & Validation (2-3 days)

#### 4.1 Unit Tests
- [ ] Test bitmap allocation/deallocation
- [ ] Test persistence across mounts
- [ ] Test edge cases (full filesystem, etc.)

#### 4.2 Integration Tests
- [ ] Test large file creation (2MB+)
- [ ] Test file deletion and block reuse
- [ ] Test concurrent operations
- [ ] Test filesystem full scenarios

#### 4.3 Performance Tests
- [ ] Benchmark allocation performance
- [ ] Test fragmentation handling
- [ ] Verify no regression in small files

---

## File Changes Required

### New Files
1. `fs-core/src/block_bitmap.rs` - Block bitmap implementation

### Modified Files
1. `fs-core/src/lib.rs` - Add block bitmap module
2. `fs-core/src/layout.rs` - Update DiskFs and allocation
3. `fs-core/src/format/mod.rs` - Initialize bitmap during format
4. `fs-core/Cargo.toml` - Add any new dependencies

### Key Code Changes

#### In `layout.rs`:
```rust
pub struct DiskFs {
    device: Arc<dyn BlockDevice>,
    cache: BlockCache,
    layout: Layout,
    superblock: Superblock,
    block_bitmap: Arc<RwLock<BlockBitmap>>, // NEW
}

async fn allocate_data_block(&mut self) -> Result<u64, FsError> {
    let mut bitmap = self.block_bitmap.write();
    match bitmap.allocate() {
        Some(block) => {
            // Update superblock free blocks
            self.superblock.free_blocks -= 1;
            Ok(block)
        }
        None => Err(FsError::NoFreeBlocks)
    }
}

async fn deallocate_data_block(&mut self, block_num: u64) -> Result<u64, FsError> {
    let mut bitmap = self.block_bitmap.write();
    bitmap.free(block_num);
    self.superblock.free_blocks += 1;
    Ok(())
}
```

---

## Testing Strategy

### Test Cases
1. **Large File Test**: Create, write, read 2MB file
2. **Block Reuse Test**: Create/delete files, verify blocks reused
3. **Persistence Test**: Write files, unmount, remount, verify
4. **Full Filesystem Test**: Fill filesystem, verify proper errors
5. **Concurrent Test**: Multiple threads writing large files

### Test Script Updates
```bash
# Update quick_test.sh to:
# 1. Create multiple large files
# 2. Delete some files
# 3. Create new files in freed space
# 4. Verify all operations succeed
```

---

## Risk Mitigation

### Potential Risks
1. **Bitmap Corruption**: Implement checksums for bitmap blocks
2. **Performance Impact**: Use efficient bit operations, cache hot blocks
3. **Backward Compatibility**: Version check in superblock
4. **Concurrency Issues**: Proper locking around bitmap operations

### Rollback Plan
- Keep existing static counter as fallback
- Add feature flag for new allocator
- Extensive testing before removing old code

---

## Success Criteria

1. ✅ 2MB file creation succeeds consistently
2. ✅ Blocks are properly reused after deletion
3. ✅ Bitmap persists across mounts
4. ✅ No performance regression for small files
5. ✅ All existing tests continue to pass
6. ✅ New tests for large files pass

---

## Timeline

- **Week 1**: Phase 1 & 2 implementation
- **Week 2**: Phase 3 & 4, testing and refinement
- **Total**: ~2 weeks for complete implementation

---

## Alternative Quick Fix (Not Recommended)

For immediate testing only:
```rust
// Reset counter on each mount (loses allocation state)
static mut NEXT_BLOCK: u64 = 1;

// Or increase filesystem size
aegisfs format /dev/nvme0n1p6 --size 10  // 10GB instead of 3GB
```

**Note**: This is only for testing and doesn't solve the fundamental issue. 

## Update (Implemented)

### Key Fixes Added
1. **Double Indirect Block Support**
   * Extended `get_file_block`, `set_file_block`, and `free_inode_blocks` in `fs-core/src/layout.rs` to handle a second level of indirection.
   * New constants `DOUBLE_INDIRECT_START` and `DOUBLE_INDIRECT_RANGE` define address space.  With 4 KiB blocks and 8-byte pointers this increases single-file capacity to ≈ 1 TB.
2. **Large-File Loops Updated**
   * `read_file_data`, `write_file_data`, and directory helpers now iterate over `DIRECT + SINGLE + DOUBLE` ranges.
3. **Safe Deallocation**
   * Comprehensive free routine walks double-indirect hierarchy and releases all nested blocks.
4. **Non-interactive Formatting**
   * `scripts/quick-deploy.sh` now passes `--force` to `aegisfs format` so automated runs don't block for confirmation.

The allocator/bitmap work from previous phases remains active; combined with the new addressing logic AegisFS can now create and verify files ≥ 1 GiB (tested with `dd if=/dev/urandom bs=1M count=1024`).

--- 