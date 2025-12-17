//! Database migration from Reth MDBX to TrieDB.
//!
//! Provides a [`Migrator`] struct for copying account and storage state
//! from a Reth MDBX database into a TrieDB database.
//!
//! # Features
//!
//! - **Batched commits**: Configurable batch size to avoid memory pressure
//! - **Progress tracking**: Periodic logging of migration progress
//! - **Resume capability**: Can resume from a checkpoint if interrupted

use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use alloy_primitives::Address;
use libmdbx::{Database, DatabaseOptions, Mode, NoWriteMap};
use triedb::{
    database::Database as TrieDatabase,
    path::{AddressPath, StoragePath},
    transaction::{RW, Transaction as TrieTransaction},
};

use crate::reth::{RethAccount, decode_storage_entry};

/// Default batch size for commits.
pub const DEFAULT_BATCH_SIZE: u64 = 100_000;

/// Default interval for progress logging (number of entries between logs).
pub const DEFAULT_LOG_INTERVAL: u64 = 100_000;

/// Interval between checkpoint saves (in seconds).
const CHECKPOINT_INTERVAL_SECS: u64 = 30;

/// Maximum number of tables to open in the source MDBX database.
const MAX_MDBX_TABLES: u64 = 256;

/// Configuration for the migrator.
#[derive(Debug, Clone)]
pub struct MigratorConfig {
    /// Number of entries to process before committing a batch.
    pub batch_size: u64,
    /// Number of entries between progress log messages.
    pub log_interval: u64,
    /// Path to checkpoint file for resume capability.
    /// If None, checkpoints are disabled.
    pub checkpoint_path: Option<PathBuf>,
}

impl Default for MigratorConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
            log_interval: DEFAULT_LOG_INTERVAL,
            checkpoint_path: None,
        }
    }
}

impl MigratorConfig {
    /// Create a new config with the given batch size.
    #[must_use]
    pub fn with_batch_size(mut self, batch_size: u64) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Create a new config with the given log interval.
    #[must_use]
    pub fn with_log_interval(mut self, log_interval: u64) -> Self {
        self.log_interval = log_interval;
        self
    }

    /// Create a new config with checkpointing enabled at the given path.
    #[must_use]
    pub fn with_checkpoint_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.checkpoint_path = Some(path.into());
        self
    }
}

/// Checkpoint data for resuming a migration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Last successfully migrated account address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_account: Option<Address>,
    /// Last successfully migrated storage address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_storage_address: Option<Address>,
    /// Whether account migration is complete.
    #[serde(default)]
    pub accounts_complete: bool,
    /// Whether storage migration is complete.
    #[serde(default)]
    pub storage_complete: bool,
    /// Running stats.
    #[serde(default)]
    pub stats: MigrationStats,
}

impl Checkpoint {
    /// Load checkpoint from a JSON file.
    pub fn load(path: &Path) -> Option<Self> {
        let file = fs::File::open(path).ok()?;
        serde_json::from_reader(file).ok()
    }

    /// Save checkpoint to a JSON file atomically.
    ///
    /// Writes to a temporary file first, then atomically renames to avoid
    /// corruption if interrupted mid-write.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        use std::io::Write;

        let tmp_path = path.with_extension("checkpoint.tmp");
        let mut file = fs::File::create(&tmp_path)?;
        serde_json::to_writer_pretty(&mut file, self)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }
}

/// Migrates account and storage state from Reth MDBX to TrieDB.
pub struct Migrator {
    /// Source Reth MDBX database.
    source: Database<NoWriteMap>,
    /// Destination TrieDB database.
    destination: TrieDatabase,
    /// Migration configuration.
    config: MigratorConfig,
}

impl std::fmt::Debug for Migrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migrator")
            .field("source", &"Database<NoWriteMap>")
            .field("destination", &"TrieDatabase")
            .field("config", &self.config)
            .finish()
    }
}

/// Statistics from a migration run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationStats {
    /// Number of accounts migrated.
    pub accounts_migrated: u64,
    /// Number of storage slots migrated.
    pub storage_slots_migrated: u64,
    /// Number of accounts with errors.
    pub account_errors: u64,
    /// Number of storage entries with errors.
    pub storage_errors: u64,
}

/// Error type for migration operations.
#[derive(Debug)]
pub enum MigrationError {
    /// Error opening or reading from the source database.
    Source(String),
    /// Error opening or writing to the destination database.
    Destination(String),
    /// Error decoding Reth data.
    Decode(String),
    /// Error with checkpoint file.
    Checkpoint(String),
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(e) => write!(f, "source database error: {}", e),
            Self::Destination(e) => write!(f, "destination database error: {}", e),
            Self::Decode(e) => write!(f, "decode error: {}", e),
            Self::Checkpoint(e) => write!(f, "checkpoint error: {}", e),
        }
    }
}

impl std::error::Error for MigrationError {}

impl Migrator {
    /// Create a new migrator with the given source and destination database paths.
    ///
    /// # Errors
    /// Returns an error if either database cannot be opened.
    pub fn new(
        source_path: impl AsRef<Path>,
        destination_path: impl AsRef<Path>,
    ) -> Result<Self, MigrationError> {
        Self::with_config(source_path, destination_path, MigratorConfig::default())
    }

    /// Create a new migrator with custom configuration.
    ///
    /// # Errors
    /// Returns an error if either database cannot be opened.
    pub fn with_config(
        source_path: impl AsRef<Path>,
        destination_path: impl AsRef<Path>,
        config: MigratorConfig,
    ) -> Result<Self, MigrationError> {
        // Open source (Reth MDBX) in read-only mode
        let source_opts = DatabaseOptions {
            mode: Mode::ReadOnly,
            max_tables: Some(MAX_MDBX_TABLES),
            ..Default::default()
        };
        let source = Database::open_with_options(source_path.as_ref(), source_opts)
            .map_err(|e| MigrationError::Source(e.to_string()))?;

        // Open or create destination (TrieDB)
        // Note: TrieDB uses file-based storage, not directory-based.
        // It creates two files: <path> and <path>.meta
        let dest = destination_path.as_ref();
        let destination = if dest.exists() {
            TrieDatabase::open(dest)
        } else {
            // Ensure parent directory exists
            if let Some(parent) = dest.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    fs::create_dir_all(parent).map_err(|e| {
                        MigrationError::Destination(format!(
                            "failed to create parent directory: {e}"
                        ))
                    })?;
                }
            }
            TrieDatabase::create_new(dest)
        }
        .map_err(|e| MigrationError::Destination(format!("{e:?}")))?;

        Ok(Self { source, destination, config })
    }

    /// Load checkpoint if it exists.
    fn load_checkpoint(&self) -> Option<Checkpoint> {
        self.config.checkpoint_path.as_ref().and_then(|p| Checkpoint::load(p))
    }

    /// Save checkpoint.
    fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), MigrationError> {
        if let Some(path) = &self.config.checkpoint_path {
            checkpoint.save(path).map_err(|e| MigrationError::Checkpoint(e.to_string()))?;
        }
        Ok(())
    }

    /// Migrate all accounts from the source to destination.
    ///
    /// Supports resuming from a checkpoint if one exists.
    ///
    /// # Errors
    /// Returns an error if the migration fails.
    pub fn migrate_accounts(&self) -> Result<MigrationStats, MigrationError> {
        self.migrate_accounts_from(None)
    }

    /// Migrate accounts starting from the given address (exclusive).
    ///
    /// # Errors
    /// Returns an error if the migration fails.
    pub fn migrate_accounts_from(
        &self,
        resume_from: Option<Address>,
    ) -> Result<MigrationStats, MigrationError> {
        let mut stats = MigrationStats::default();
        let start_time = Instant::now();
        let mut last_checkpoint_time = Instant::now();
        let mut batch_count: u64 = 0;
        let mut last_address: Option<Address> = None;
        let mut skipping = resume_from.is_some();

        // Begin read transaction on source
        let src_txn =
            self.source.begin_ro_txn().map_err(|e| MigrationError::Source(e.to_string()))?;

        let table = src_txn
            .open_table(Some("PlainAccountState"))
            .map_err(|e| MigrationError::Source(e.to_string()))?;

        let mut cursor =
            src_txn.cursor(&table).map_err(|e| MigrationError::Source(e.to_string()))?;

        // Begin write transaction on destination
        let mut dst_txn =
            self.destination.begin_rw().map_err(|e| MigrationError::Destination(e.to_string()))?;

        // Iterate through all accounts
        for result in cursor.iter_start::<Vec<u8>, Vec<u8>>() {
            let (key, value) = result.map_err(|e| MigrationError::Source(e.to_string()))?;

            let address = Address::from_slice(&key);

            // Skip entries until we pass the resume point
            if skipping {
                if Some(address) == resume_from {
                    skipping = false;
                }
                continue;
            }

            match RethAccount::from_compact(&value) {
                Ok(reth_account) => {
                    let triedb_account = reth_account.to_triedb_account();
                    let address_path = AddressPath::for_address(address);

                    if let Err(e) = dst_txn.set_account(address_path, Some(triedb_account)) {
                        tracing::warn!(?address, error = %e, "failed to write account");
                        stats.account_errors += 1;
                    } else {
                        stats.accounts_migrated += 1;
                        last_address = Some(address);
                    }
                }
                Err(e) => {
                    tracing::warn!(?address, error = ?e, "failed to decode account");
                    stats.account_errors += 1;
                }
            }

            batch_count += 1;

            // Log progress
            if stats.accounts_migrated % self.config.log_interval == 0
                && stats.accounts_migrated > 0
            {
                let elapsed = start_time.elapsed().as_secs_f64();
                let rate = stats.accounts_migrated as f64 / elapsed;
                tracing::info!(
                    accounts = stats.accounts_migrated,
                    errors = stats.account_errors,
                    rate = format!("{:.0}/s", rate),
                    "account migration progress"
                );
            }

            // Commit batch
            if batch_count >= self.config.batch_size {
                dst_txn.commit().map_err(|e| MigrationError::Destination(e.to_string()))?;

                // Save checkpoint periodically
                if self.config.checkpoint_path.is_some()
                    && last_checkpoint_time.elapsed().as_secs() >= CHECKPOINT_INTERVAL_SECS
                {
                    let checkpoint = Checkpoint {
                        last_account: last_address,
                        accounts_complete: false,
                        stats: stats.clone(),
                        ..Default::default()
                    };
                    self.save_checkpoint(&checkpoint)?;
                    last_checkpoint_time = Instant::now();
                    tracing::debug!(accounts = stats.accounts_migrated, "saved checkpoint");
                }

                tracing::debug!(accounts = stats.accounts_migrated, "committed batch");

                // Start new transaction
                dst_txn = self
                    .destination
                    .begin_rw()
                    .map_err(|e| MigrationError::Destination(e.to_string()))?;
                batch_count = 0;
            }
        }

        // Commit final batch
        dst_txn.commit().map_err(|e| MigrationError::Destination(e.to_string()))?;

        let elapsed = start_time.elapsed();
        tracing::info!(
            accounts = stats.accounts_migrated,
            errors = stats.account_errors,
            elapsed_secs = elapsed.as_secs(),
            "account migration complete"
        );

        Ok(stats)
    }

    /// Migrate all storage slots from the source to destination.
    ///
    /// # Errors
    /// Returns an error if the migration fails.
    pub fn migrate_storage(&self) -> Result<MigrationStats, MigrationError> {
        self.migrate_storage_from(None)
    }

    /// Migrate storage starting from the given address (exclusive).
    ///
    /// Note: Resume is at address-level granularity. If interrupted mid-address,
    /// all slots for that address will be reprocessed on resume.
    ///
    /// # Errors
    /// Returns an error if the migration fails.
    pub fn migrate_storage_from(
        &self,
        resume_from: Option<Address>,
    ) -> Result<MigrationStats, MigrationError> {
        let mut stats = MigrationStats::default();
        let start_time = Instant::now();
        let mut last_checkpoint_time = Instant::now();
        let mut batch_count: u64 = 0;
        // Track last processed address for checkpoints (assigned in loop before first use).
        // Allow the initial None assignment warning - it's a valid initialization pattern.
        #[allow(unused_assignments)]
        let mut last_address = Option::<Address>::None;

        // Begin read transaction on source
        let src_txn =
            self.source.begin_ro_txn().map_err(|e| MigrationError::Source(e.to_string()))?;

        let table = src_txn
            .open_table(Some("PlainStorageState"))
            .map_err(|e| MigrationError::Source(e.to_string()))?;

        let mut cursor =
            src_txn.cursor(&table).map_err(|e| MigrationError::Source(e.to_string()))?;

        // Begin write transaction on destination
        let mut dst_txn =
            self.destination.begin_rw().map_err(|e| MigrationError::Destination(e.to_string()))?;

        // Use single-cursor iteration for DupSort tables.
        // Start with first entry, then use next_nodup() to skip to next address.
        let mut item = cursor
            .first::<Vec<u8>, Vec<u8>>()
            .map_err(|e| MigrationError::Source(e.to_string()))?;

        while let Some((key, first_value)) = item {
            let address = Address::from_slice(&key);

            // Skip entries until we pass the resume point
            if let Some(resume_addr) = resume_from {
                if address <= resume_addr {
                    item = cursor
                        .next_nodup::<Vec<u8>, Vec<u8>>()
                        .map_err(|e| MigrationError::Source(e.to_string()))?;
                    continue;
                }
            }

            last_address = Some(address);

            // Process first value for this address
            self.process_storage_entry(&mut dst_txn, address, &first_value, &mut stats);
            batch_count += 1;

            // Process remaining duplicates for this address
            while let Some((_, dup_value)) = cursor
                .next_dup::<Vec<u8>, Vec<u8>>()
                .map_err(|e| MigrationError::Source(e.to_string()))?
            {
                self.process_storage_entry(&mut dst_txn, address, &dup_value, &mut stats);
                batch_count += 1;
            }

            // Log progress after processing each address
            if stats.storage_slots_migrated % self.config.log_interval == 0
                && stats.storage_slots_migrated > 0
            {
                let elapsed = start_time.elapsed().as_secs_f64();
                let rate = stats.storage_slots_migrated as f64 / elapsed;
                tracing::info!(
                    slots = stats.storage_slots_migrated,
                    errors = stats.storage_errors,
                    rate = format!("{:.0}/s", rate),
                    "storage migration progress"
                );
            }

            // Commit batch after completing all slots for an address
            if batch_count >= self.config.batch_size {
                dst_txn.commit().map_err(|e| MigrationError::Destination(e.to_string()))?;

                // Save checkpoint periodically
                if self.config.checkpoint_path.is_some()
                    && last_checkpoint_time.elapsed().as_secs() >= CHECKPOINT_INTERVAL_SECS
                {
                    let checkpoint = Checkpoint {
                        last_storage_address: last_address,
                        storage_complete: false,
                        stats: stats.clone(),
                        ..Default::default()
                    };
                    self.save_checkpoint(&checkpoint)?;
                    last_checkpoint_time = Instant::now();
                    tracing::debug!(slots = stats.storage_slots_migrated, "saved checkpoint");
                }

                tracing::debug!(slots = stats.storage_slots_migrated, "committed batch");

                dst_txn = self
                    .destination
                    .begin_rw()
                    .map_err(|e| MigrationError::Destination(e.to_string()))?;
                batch_count = 0;
            }

            // Move to next address (skips any remaining dups we've already processed)
            item = cursor
                .next_nodup::<Vec<u8>, Vec<u8>>()
                .map_err(|e| MigrationError::Source(e.to_string()))?;
        }

        // Commit final batch
        dst_txn.commit().map_err(|e| MigrationError::Destination(e.to_string()))?;

        let elapsed = start_time.elapsed();
        tracing::info!(
            slots = stats.storage_slots_migrated,
            errors = stats.storage_errors,
            elapsed_secs = elapsed.as_secs(),
            "storage migration complete"
        );

        Ok(stats)
    }

    /// Process a single storage entry.
    fn process_storage_entry(
        &self,
        dst_txn: &mut TrieTransaction<&TrieDatabase, RW>,
        address: Address,
        value: &[u8],
        stats: &mut MigrationStats,
    ) {
        match decode_storage_entry(value) {
            Ok(entry) => {
                let storage_path = StoragePath::for_address_and_slot(address, entry.slot);

                if let Err(e) = dst_txn.set_storage_slot(storage_path, Some(entry.value)) {
                    tracing::warn!(?address, slot = ?entry.slot, error = %e, "failed to write storage");
                    stats.storage_errors += 1;
                } else {
                    stats.storage_slots_migrated += 1;
                }
            }
            Err(e) => {
                tracing::warn!(?address, error = ?e, "failed to decode storage entry");
                stats.storage_errors += 1;
            }
        }
    }

    /// Migrate all data (accounts and storage) from source to destination.
    ///
    /// Automatically resumes from checkpoint if one exists.
    ///
    /// # Errors
    /// Returns an error if the migration fails.
    pub fn migrate_all(&self) -> Result<MigrationStats, MigrationError> {
        // Load checkpoint if exists
        let checkpoint = self.load_checkpoint().unwrap_or_default();

        let mut total_stats = checkpoint.stats.clone();

        // Migrate accounts if not complete
        if !checkpoint.accounts_complete {
            tracing::info!(
                resume_from = ?checkpoint.last_account,
                "starting account migration"
            );

            let account_stats = self.migrate_accounts_from(checkpoint.last_account)?;
            total_stats.accounts_migrated += account_stats.accounts_migrated;
            total_stats.account_errors += account_stats.account_errors;

            // Update checkpoint
            if self.config.checkpoint_path.is_some() {
                let new_checkpoint = Checkpoint {
                    accounts_complete: true,
                    stats: total_stats.clone(),
                    ..Default::default()
                };
                self.save_checkpoint(&new_checkpoint)?;
            }

            tracing::info!(
                accounts = total_stats.accounts_migrated,
                errors = total_stats.account_errors,
                "account migration complete"
            );
        } else {
            tracing::info!("accounts already migrated, skipping");
        }

        // Migrate storage if not complete
        if !checkpoint.storage_complete {
            tracing::info!(
                resume_from = ?checkpoint.last_storage_address,
                "starting storage migration"
            );

            let storage_stats = self.migrate_storage_from(checkpoint.last_storage_address)?;
            total_stats.storage_slots_migrated += storage_stats.storage_slots_migrated;
            total_stats.storage_errors += storage_stats.storage_errors;

            // Update checkpoint
            if self.config.checkpoint_path.is_some() {
                let new_checkpoint = Checkpoint {
                    accounts_complete: true,
                    storage_complete: true,
                    stats: total_stats.clone(),
                    ..Default::default()
                };
                self.save_checkpoint(&new_checkpoint)?;
            }

            tracing::info!(
                slots = total_stats.storage_slots_migrated,
                errors = total_stats.storage_errors,
                "storage migration complete"
            );
        } else {
            tracing::info!("storage already migrated, skipping");
        }

        Ok(total_stats)
    }

    /// Close the migrator and release database handles.
    ///
    /// # Errors
    /// Returns an error if the databases cannot be closed cleanly.
    pub fn close(self) -> Result<(), MigrationError> {
        self.destination.close().map_err(|e| MigrationError::Destination(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrate_reth_to_triedb() -> eyre::Result<()> {
        // Path relative to manifest directory (crate root)
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let source_path = manifest_dir.join("../../../static/reth/db");

        let temp_dir = tempfile::tempdir()?;
        let dest_path = temp_dir.path().join("triedb");
        let checkpoint_path = temp_dir.path().join("migration.checkpoint");

        // Create config with small batch size for testing
        let config = MigratorConfig::default()
            .with_batch_size(500) // Small batch to test batching
            .with_log_interval(100)
            .with_checkpoint_path(&checkpoint_path);

        // Create the migrator with config
        let migrator = Migrator::with_config(&source_path, &dest_path, config)?;

        // Run the migration
        let stats = migrator.migrate_all()?;

        // Verify some data was migrated
        assert!(stats.accounts_migrated > 0, "should migrate at least one account");
        assert!(stats.storage_slots_migrated > 0, "should migrate at least one storage slot");

        // Close the migrator
        migrator.close()?;

        // Verify checkpoint file was created
        assert!(checkpoint_path.exists(), "checkpoint file should exist");

        // Verify checkpoint shows completion
        let checkpoint = Checkpoint::load(&checkpoint_path).expect("should load checkpoint");
        assert!(checkpoint.accounts_complete, "accounts should be marked complete");
        assert!(checkpoint.storage_complete, "storage should be marked complete");

        // Reopen the TrieDB and verify the state root
        let triedb = TrieDatabase::open(&dest_path)
            .map_err(|e| eyre::eyre!("failed to reopen triedb: {:?}", e))?;
        let state_root = triedb.state_root();

        let expected_state_root = alloy_primitives::b256!(
            "b2afcb88cd1d0ab228f0415d99b0fb90a18e8515daf5eb31f55b5c4697e18328"
        );
        assert_eq!(state_root, expected_state_root, "state root mismatch after migration");

        triedb.close().map_err(|e| eyre::eyre!("failed to close triedb: {:?}", e))?;

        Ok(())
    }
}
