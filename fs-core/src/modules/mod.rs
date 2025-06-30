//! AegisFS feature modules
//!
//! This module contains optional filesystem features that can be enabled
//! or disabled based on configuration and feature flags.

pub mod checksums;
pub mod journaling;
pub mod snapshot;

// Re-export journaling types
pub use journaling::{
    JournalConfig, JournalEntryType, JournalManager, Transaction, TransactionState,
};

// Re-export checksum types
pub use checksums::{ChecksumAlgorithm, ChecksumConfig, ChecksumManager, ScrubStats};

// Re-export snapshot types
pub use snapshot::{
    SnapshotConfig, SnapshotManager, SnapshotMetadata, SnapshotState, SnapshotStats,
};
