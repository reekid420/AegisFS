//! AegisFS feature modules
//! 
//! This module contains optional filesystem features that can be enabled
//! or disabled based on configuration and feature flags.

pub mod journaling;
pub mod checksums;
pub mod snapshot;

// Re-export journaling types
pub use journaling::{JournalManager, JournalConfig, JournalEntryType, Transaction, TransactionState};

// Re-export checksum types
pub use checksums::{ChecksumManager, ChecksumConfig, ChecksumAlgorithm, ScrubStats};

// Re-export snapshot types
pub use snapshot::{SnapshotManager, SnapshotConfig, SnapshotState, SnapshotMetadata, SnapshotStats}; 