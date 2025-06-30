//! Journaling module for AegisFS
//!
//! This module implements write-ahead logging (WAL) for crash consistency.
//! All filesystem modifications are logged before being applied to ensure
//! atomic operations and crash recovery.

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::blockdev::{BlockDevice, BlockDeviceError};
use crate::error::Result;

/// Journal entry types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum JournalEntryType {
    /// Start of a transaction
    TransactionStart = 1,
    /// End of a transaction
    TransactionEnd = 2,
    /// Metadata update
    MetadataUpdate = 3,
    /// Data write
    DataWrite = 4,
    /// Inode update
    InodeUpdate = 5,
    /// Directory entry update
    DirEntryUpdate = 6,
    /// Block allocation
    BlockAlloc = 7,
    /// Block deallocation
    BlockDealloc = 8,
    /// Checkpoint marker
    Checkpoint = 9,
}

/// Journal entry header
#[derive(Debug, Clone)]
pub struct JournalEntryHeader {
    /// Entry type
    pub entry_type: JournalEntryType,
    /// Transaction ID
    pub transaction_id: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Data length
    pub data_length: u32,
    /// Checksum of the entry
    pub checksum: u32,
}

impl JournalEntryHeader {
    /// Size of the header in bytes
    pub const SIZE: usize = 32;

    /// Serialize the header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(Self::SIZE);
        buf.write_u32::<LittleEndian>(self.entry_type as u32)
            .unwrap();
        buf.write_u64::<LittleEndian>(self.transaction_id).unwrap();
        buf.write_u64::<LittleEndian>(self.timestamp).unwrap();
        buf.write_u32::<LittleEndian>(self.data_length).unwrap();
        buf.write_u32::<LittleEndian>(self.checksum).unwrap();
        // Pad to SIZE bytes
        buf.resize(Self::SIZE, 0);
        buf
    }

    /// Deserialize the header from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(crate::error::Error::InvalidArgument);
        }

        let mut cursor = std::io::Cursor::new(data);
        let entry_type_raw = cursor.read_u32::<LittleEndian>()?;
        let entry_type = match entry_type_raw {
            1 => JournalEntryType::TransactionStart,
            2 => JournalEntryType::TransactionEnd,
            3 => JournalEntryType::MetadataUpdate,
            4 => JournalEntryType::DataWrite,
            5 => JournalEntryType::InodeUpdate,
            6 => JournalEntryType::DirEntryUpdate,
            7 => JournalEntryType::BlockAlloc,
            8 => JournalEntryType::BlockDealloc,
            9 => JournalEntryType::Checkpoint,
            _ => return Err(crate::error::Error::InvalidArgument),
        };

        Ok(Self {
            entry_type,
            transaction_id: cursor.read_u64::<LittleEndian>()?,
            timestamp: cursor.read_u64::<LittleEndian>()?,
            data_length: cursor.read_u32::<LittleEndian>()?,
            checksum: cursor.read_u32::<LittleEndian>()?,
        })
    }
}

/// Journal entry containing header and data
#[derive(Debug, Clone)]
pub struct JournalEntry {
    /// Entry header
    pub header: JournalEntryHeader,
    /// Entry data
    pub data: Vec<u8>,
}

impl JournalEntry {
    /// Create a new journal entry
    pub fn new(entry_type: JournalEntryType, transaction_id: u64, data: Vec<u8>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let checksum = crc32fast::hash(&data);

        let header = JournalEntryHeader {
            entry_type,
            transaction_id,
            timestamp,
            data_length: data.len() as u32,
            checksum,
        };

        Self { header, data }
    }

    /// Serialize the entire entry to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = self.header.to_bytes();
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Verify the checksum of the entry
    pub fn verify_checksum(&self) -> bool {
        crc32fast::hash(&self.data) == self.header.checksum
    }
}

/// Transaction state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction is active
    Active,
    /// Transaction is being committed
    Committing,
    /// Transaction is committed
    Committed,
    /// Transaction was aborted
    Aborted,
}

/// A filesystem transaction
#[derive(Debug)]
pub struct Transaction {
    /// Transaction ID
    pub id: u64,
    /// Transaction state
    pub state: TransactionState,
    /// Journal entries in this transaction
    pub entries: Vec<JournalEntry>,
    /// Start timestamp
    pub start_time: SystemTime,
}

impl Transaction {
    /// Create a new transaction
    pub fn new(id: u64) -> Self {
        Self {
            id,
            state: TransactionState::Active,
            entries: Vec::new(),
            start_time: SystemTime::now(),
        }
    }

    /// Add an entry to the transaction
    pub fn add_entry(&mut self, entry_type: JournalEntryType, data: Vec<u8>) {
        let entry = JournalEntry::new(entry_type, self.id, data);
        self.entries.push(entry);
    }
}

/// Journal error types
#[derive(Error, Debug)]
pub enum JournalError {
    #[error("Block device error: {0}")]
    BlockDevice(#[from] BlockDeviceError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Transaction not found: {0}")]
    TransactionNotFound(u64),
    #[error("Transaction already committed: {0}")]
    TransactionAlreadyCommitted(u64),
    #[error("Journal is full")]
    JournalFull,
    #[error("Corrupt journal entry")]
    CorruptEntry,
    #[error("Invalid journal format")]
    InvalidFormat,
}

/// Journal manager configuration
#[derive(Debug, Clone)]
pub struct JournalConfig {
    /// Maximum number of concurrent transactions
    pub max_transactions: usize,
    /// Journal size in blocks
    pub journal_size: u64,
    /// Checkpoint interval in transactions
    pub checkpoint_interval: u64,
    /// Enable journal compression
    pub compress: bool,
}

impl Default for JournalConfig {
    fn default() -> Self {
        Self {
            max_transactions: 256,
            journal_size: 8192, // 32MB with 4KB blocks
            checkpoint_interval: 100,
            compress: false,
        }
    }
}

/// Journal manager for handling transactions and write-ahead logging
pub struct JournalManager {
    /// Block device for journal storage
    device: Arc<dyn BlockDevice>,
    /// Journal configuration
    config: JournalConfig,
    /// Next transaction ID
    next_transaction_id: AtomicU64,
    /// Active transactions
    active_transactions: RwLock<HashMap<u64, Arc<Mutex<Transaction>>>>,
    /// Journal write position
    write_position: AtomicU64,
    /// Journal read position (for recovery)
    read_position: AtomicU64,
    /// Background task sender
    task_sender: Option<mpsc::UnboundedSender<JournalTask>>,
}

/// Background journal tasks
#[derive(Debug)]
pub enum JournalTask {
    /// Flush journal to disk
    Flush,
    /// Create a checkpoint
    Checkpoint,
    /// Shutdown the journal
    Shutdown,
}

impl JournalManager {
    /// Create a new journal manager
    pub fn new(device: Arc<dyn BlockDevice>, config: JournalConfig) -> Self {
        Self {
            device,
            config,
            next_transaction_id: AtomicU64::new(1),
            active_transactions: RwLock::new(HashMap::new()),
            write_position: AtomicU64::new(0),
            read_position: AtomicU64::new(0),
            task_sender: None,
        }
    }

    /// Initialize the journal manager
    pub async fn init(&mut self) -> Result<()> {
        // Start background task handler
        let (sender, mut receiver) = mpsc::unbounded_channel();
        self.task_sender = Some(sender);

        let device = self.device.clone();
        tokio::spawn(async move {
            while let Some(task) = receiver.recv().await {
                match task {
                    JournalTask::Flush => {
                        // Flush journal to disk
                        if let Err(e) = device.sync().await {
                            log::error!("Failed to flush journal: {}", e);
                        }
                    }
                    JournalTask::Checkpoint => {
                        // Create checkpoint
                        log::info!("Creating journal checkpoint");
                    }
                    JournalTask::Shutdown => {
                        log::info!("Shutting down journal background tasks");
                        break;
                    }
                }
            }
        });

        // Perform recovery if needed
        self.recover().await?;

        Ok(())
    }

    /// Start a new transaction
    pub fn begin_transaction(&self) -> Result<u64> {
        let transaction_id = self.next_transaction_id.fetch_add(1, Ordering::SeqCst);
        let transaction = Arc::new(Mutex::new(Transaction::new(transaction_id)));

        // Check if we've reached the maximum number of transactions
        let mut active = self.active_transactions.write();
        if active.len() >= self.config.max_transactions {
            return Err(crate::error::Error::Other(
                "Too many active transactions".to_string(),
            ));
        }

        active.insert(transaction_id, transaction);

        log::debug!("Started transaction {}", transaction_id);
        Ok(transaction_id)
    }

    /// Add an entry to a transaction
    pub fn add_entry(
        &self,
        transaction_id: u64,
        entry_type: JournalEntryType,
        data: Vec<u8>,
    ) -> Result<()> {
        let active = self.active_transactions.read();
        let transaction = active.get(&transaction_id).ok_or_else(|| {
            crate::error::Error::Other(format!("Transaction {} not found", transaction_id))
        })?;

        let mut tx = transaction.lock();
        if tx.state != TransactionState::Active {
            return Err(crate::error::Error::Other(format!(
                "Transaction {} is not active",
                transaction_id
            )));
        }

        tx.add_entry(entry_type, data);
        Ok(())
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self, transaction_id: u64) -> Result<()> {
        // Get the transaction
        let transaction = {
            let active = self.active_transactions.read();
            active
                .get(&transaction_id)
                .ok_or_else(|| {
                    crate::error::Error::Other(format!("Transaction {} not found", transaction_id))
                })?
                .clone()
        };

        // Mark as committing
        {
            let mut tx = transaction.lock();
            if tx.state != TransactionState::Active {
                return Err(crate::error::Error::Other(format!(
                    "Transaction {} is not active",
                    transaction_id
                )));
            }
            tx.state = TransactionState::Committing;
        }

        // Write transaction start marker
        let start_entry =
            JournalEntry::new(JournalEntryType::TransactionStart, transaction_id, vec![]);
        self.write_entry(&start_entry).await?;

        // Write all entries
        {
            let tx = transaction.lock();
            for entry in &tx.entries {
                self.write_entry(entry).await?;
            }
        }

        // Write transaction end marker
        let end_entry = JournalEntry::new(JournalEntryType::TransactionEnd, transaction_id, vec![]);
        self.write_entry(&end_entry).await?;

        // Flush to disk
        self.device.sync().await?;

        // Mark as committed and remove from active transactions
        {
            let mut tx = transaction.lock();
            tx.state = TransactionState::Committed;
        }

        let mut active = self.active_transactions.write();
        active.remove(&transaction_id);

        log::debug!("Committed transaction {}", transaction_id);
        Ok(())
    }

    /// Abort a transaction
    pub fn abort_transaction(&self, transaction_id: u64) -> Result<()> {
        let mut active = self.active_transactions.write();
        if let Some(transaction) = active.remove(&transaction_id) {
            let mut tx = transaction.lock();
            tx.state = TransactionState::Aborted;
            log::debug!("Aborted transaction {}", transaction_id);
        }
        Ok(())
    }

    /// Write a journal entry to disk
    async fn write_entry(&self, entry: &JournalEntry) -> Result<()> {
        let entry_bytes = entry.to_bytes();
        let blocks_needed = (entry_bytes.len() + 4095) / 4096; // Round up to block size

        let write_pos = self
            .write_position
            .fetch_add(blocks_needed as u64, Ordering::SeqCst);

        // Check if journal is full
        if write_pos + blocks_needed as u64 > self.config.journal_size {
            return Err(crate::error::Error::Other("Journal is full".to_string()));
        }

        // Write the entry
        let mut block_data = vec![0u8; blocks_needed * 4096];
        block_data[..entry_bytes.len()].copy_from_slice(&entry_bytes);

        for i in 0..blocks_needed {
            let block_num = write_pos + i as u64;
            let block_start = i * 4096;
            let block_end = std::cmp::min(block_start + 4096, block_data.len());

            self.device
                .write_block(block_num, &block_data[block_start..block_end])
                .await?;
        }

        Ok(())
    }

    /// Recover from journal after a crash
    async fn recover(&self) -> Result<()> {
        log::info!("Starting journal recovery");

        let mut recovered_transactions = 0;
        let mut read_pos = self.read_position.load(Ordering::SeqCst);

        while read_pos < self.write_position.load(Ordering::SeqCst) {
            // Read journal entry header
            let mut header_data = vec![0u8; 4096];
            self.device.read_block(read_pos, &mut header_data).await?;

            // Parse header
            let header = match JournalEntryHeader::from_bytes(&header_data) {
                Ok(h) => h,
                Err(_) => {
                    log::warn!(
                        "Corrupt journal entry at block {}, stopping recovery",
                        read_pos
                    );
                    break;
                }
            };

            // Read entry data if present
            let mut entry_data = Vec::new();
            if header.data_length > 0 {
                let data_blocks = (header.data_length as usize + 4095) / 4096;
                for i in 1..=data_blocks {
                    let mut block_data = vec![0u8; 4096];
                    self.device
                        .read_block(read_pos + i as u64, &mut block_data)
                        .await?;
                    entry_data.extend_from_slice(&block_data);
                }
                entry_data.truncate(header.data_length as usize);
            }

            // Create and verify entry
            let entry = JournalEntry {
                header,
                data: entry_data,
            };

            if !entry.verify_checksum() {
                log::warn!("Corrupt journal entry checksum at block {}", read_pos);
                break;
            }

            // Process the entry for recovery
            match entry.header.entry_type {
                JournalEntryType::TransactionStart => {
                    log::debug!("Found transaction start: {}", entry.header.transaction_id);
                }
                JournalEntryType::TransactionEnd => {
                    log::debug!("Found transaction end: {}", entry.header.transaction_id);
                    recovered_transactions += 1;
                }
                _ => {
                    // Process other entry types as needed
                }
            }

            // Move to next entry
            let entry_blocks = (JournalEntryHeader::SIZE + entry.data.len() + 4095) / 4096;
            read_pos += entry_blocks as u64;
        }

        self.read_position.store(read_pos, Ordering::SeqCst);
        log::info!(
            "Journal recovery complete. Recovered {} transactions",
            recovered_transactions
        );

        Ok(())
    }

    /// Create a checkpoint
    pub async fn checkpoint(&self) -> Result<()> {
        let checkpoint_entry = JournalEntry::new(
            JournalEntryType::Checkpoint,
            0, // Checkpoint doesn't belong to a transaction
            vec![],
        );

        self.write_entry(&checkpoint_entry).await?;
        self.device.sync().await?;

        log::info!("Created journal checkpoint");
        Ok(())
    }

    /// Shutdown the journal manager
    pub async fn shutdown(&mut self) -> Result<()> {
        // Commit any remaining active transactions
        let active_ids: Vec<u64> = {
            let active = self.active_transactions.read();
            active.keys().cloned().collect()
        };

        for transaction_id in active_ids {
            log::warn!(
                "Aborting uncommitted transaction {} during shutdown",
                transaction_id
            );
            self.abort_transaction(transaction_id)?;
        }

        // Send shutdown signal to background tasks
        if let Some(sender) = &self.task_sender {
            let _ = sender.send(JournalTask::Shutdown);
        }

        // Final sync
        self.device.sync().await?;

        log::info!("Journal manager shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockdev::FileBackedBlockDevice;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_journal_entry_serialization() {
        let entry = JournalEntry::new(JournalEntryType::MetadataUpdate, 123, b"test data".to_vec());

        let bytes = entry.to_bytes();
        assert!(bytes.len() >= JournalEntryHeader::SIZE + 9);
        assert!(entry.verify_checksum());
    }

    #[tokio::test]
    async fn test_transaction_lifecycle() {
        let temp_file = NamedTempFile::new().unwrap();
        let device = Arc::new(
            FileBackedBlockDevice::create(temp_file.path(), 1024 * 1024)
                .await
                .unwrap(),
        );

        let mut journal = JournalManager::new(device, JournalConfig::default());
        journal.init().await.unwrap();

        // Begin transaction
        let tx_id = journal.begin_transaction().unwrap();
        assert!(tx_id > 0);

        // Add entries
        journal
            .add_entry(
                tx_id,
                JournalEntryType::MetadataUpdate,
                b"metadata".to_vec(),
            )
            .unwrap();
        journal
            .add_entry(tx_id, JournalEntryType::DataWrite, b"data".to_vec())
            .unwrap();

        // Commit transaction
        journal.commit_transaction(tx_id).await.unwrap();

        // Shutdown
        journal.shutdown().await.unwrap();
    }
}
