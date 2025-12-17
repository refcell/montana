#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Checkpoint state for node resumption.
///
/// Tracks progress across various node operations to enable resumption after restart
/// and prevent duplicate work.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Last successfully submitted batch number (sequencer).
    pub last_batch_submitted: u64,
    /// Last successfully derived batch number (validator).
    pub last_batch_derived: u64,
    /// Last executed block number.
    pub last_block_executed: u64,
    /// L2 block number we've synced to.
    pub synced_to_block: u64,
    /// Unix timestamp of last update.
    pub updated_at: u64,
}

impl Checkpoint {
    /// Creates a new empty checkpoint.
    ///
    /// All fields are initialized to zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a checkpoint from the specified file path.
    ///
    /// Returns `Ok(None)` if the file doesn't exist, or `Ok(Some(checkpoint))` if successfully loaded.
    ///
    /// # Errors
    ///
    /// Returns [`CheckpointError::Io`] if there's an error reading the file (other than not found).
    /// Returns [`CheckpointError::Serialize`] if the file contents cannot be deserialized.
    pub fn load(path: &Path) -> Result<Option<Self>, CheckpointError> {
        match fs::read_to_string(path) {
            Ok(contents) => {
                let checkpoint = serde_json::from_str(&contents)?;
                Ok(Some(checkpoint))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(CheckpointError::Io(e)),
        }
    }

    /// Atomically saves the checkpoint to the specified file path.
    ///
    /// Uses a temporary file and atomic rename to ensure data integrity.
    ///
    /// # Errors
    ///
    /// Returns [`CheckpointError::Io`] if there's an error writing the file.
    /// Returns [`CheckpointError::Serialize`] if the checkpoint cannot be serialized.
    pub fn save(&self, path: &Path) -> Result<(), CheckpointError> {
        let json = serde_json::to_string_pretty(self)?;

        // Write to temporary file in the same directory
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, json)?;

        // Atomically rename to final location
        fs::rename(&temp_path, path)?;

        Ok(())
    }

    /// Updates the timestamp to the current time.
    ///
    /// Sets `updated_at` to the current Unix timestamp in seconds.
    pub fn touch(&mut self) {
        self.updated_at =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    }

    /// Checks if a batch should be skipped based on the checkpoint state.
    ///
    /// Returns `true` if the batch number has already been submitted (batch_number <= last_batch_submitted).
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number to check
    pub const fn should_skip_batch(&self, batch_number: u64) -> bool {
        batch_number <= self.last_batch_submitted
    }

    /// Records that a batch has been successfully submitted.
    ///
    /// Updates `last_batch_submitted` if the provided batch number is greater than the current value.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number that was submitted
    pub const fn record_batch_submitted(&mut self, batch_number: u64) {
        if batch_number > self.last_batch_submitted {
            self.last_batch_submitted = batch_number;
        }
    }

    /// Records that a batch has been successfully derived.
    ///
    /// Updates `last_batch_derived` if the provided batch number is greater than the current value.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number that was derived
    pub const fn record_batch_derived(&mut self, batch_number: u64) {
        if batch_number > self.last_batch_derived {
            self.last_batch_derived = batch_number;
        }
    }

    /// Records that a block has been successfully executed.
    ///
    /// Updates `last_block_executed` if the provided block number is greater than the current value.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number that was executed
    pub const fn record_block_executed(&mut self, block_number: u64) {
        if block_number > self.last_block_executed {
            self.last_block_executed = block_number;
        }
    }

    /// Records sync progress to a block.
    ///
    /// Updates `synced_to_block` if the provided block number is greater than the current value.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number we've synced to
    pub const fn record_sync_progress(&mut self, block_number: u64) {
        if block_number > self.synced_to_block {
            self.synced_to_block = block_number;
        }
    }
}

/// Errors that can occur during checkpoint operations.
#[derive(Debug, Error)]
pub enum CheckpointError {
    /// I/O error occurred while reading or writing checkpoint file.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_new_checkpoint() {
        let checkpoint = Checkpoint::new();
        assert_eq!(checkpoint.last_batch_submitted, 0);
        assert_eq!(checkpoint.last_batch_derived, 0);
        assert_eq!(checkpoint.last_block_executed, 0);
        assert_eq!(checkpoint.synced_to_block, 0);
        assert_eq!(checkpoint.updated_at, 0);
    }

    #[test]
    fn test_load_nonexistent() {
        let result = Checkpoint::load(Path::new("/nonexistent/path/checkpoint.json"));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_save_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        let mut checkpoint = Checkpoint::new();
        checkpoint.last_batch_submitted = 42;
        checkpoint.last_batch_derived = 40;
        checkpoint.last_block_executed = 100;
        checkpoint.synced_to_block = 99;
        checkpoint.updated_at = 1234567890;

        // Save checkpoint
        checkpoint.save(path).unwrap();

        // Load checkpoint
        let loaded = Checkpoint::load(path).unwrap().unwrap();
        assert_eq!(loaded, checkpoint);
    }

    #[test]
    fn test_touch() {
        let mut checkpoint = Checkpoint::new();
        assert_eq!(checkpoint.updated_at, 0);

        checkpoint.touch();
        assert!(checkpoint.updated_at > 0);
    }

    #[test]
    fn test_should_skip_batch() {
        let mut checkpoint = Checkpoint::new();
        checkpoint.last_batch_submitted = 10;

        assert!(checkpoint.should_skip_batch(5));
        assert!(checkpoint.should_skip_batch(10));
        assert!(!checkpoint.should_skip_batch(11));
    }

    #[test]
    fn test_record_batch_submitted() {
        let mut checkpoint = Checkpoint::new();

        checkpoint.record_batch_submitted(10);
        assert_eq!(checkpoint.last_batch_submitted, 10);

        // Should update to higher value
        checkpoint.record_batch_submitted(20);
        assert_eq!(checkpoint.last_batch_submitted, 20);

        // Should not update to lower value
        checkpoint.record_batch_submitted(15);
        assert_eq!(checkpoint.last_batch_submitted, 20);
    }

    #[test]
    fn test_record_batch_derived() {
        let mut checkpoint = Checkpoint::new();

        checkpoint.record_batch_derived(10);
        assert_eq!(checkpoint.last_batch_derived, 10);

        checkpoint.record_batch_derived(20);
        assert_eq!(checkpoint.last_batch_derived, 20);

        checkpoint.record_batch_derived(15);
        assert_eq!(checkpoint.last_batch_derived, 20);
    }

    #[test]
    fn test_record_block_executed() {
        let mut checkpoint = Checkpoint::new();

        checkpoint.record_block_executed(100);
        assert_eq!(checkpoint.last_block_executed, 100);

        checkpoint.record_block_executed(200);
        assert_eq!(checkpoint.last_block_executed, 200);

        checkpoint.record_block_executed(150);
        assert_eq!(checkpoint.last_block_executed, 200);
    }

    #[test]
    fn test_record_sync_progress() {
        let mut checkpoint = Checkpoint::new();

        checkpoint.record_sync_progress(100);
        assert_eq!(checkpoint.synced_to_block, 100);

        checkpoint.record_sync_progress(200);
        assert_eq!(checkpoint.synced_to_block, 200);

        checkpoint.record_sync_progress(150);
        assert_eq!(checkpoint.synced_to_block, 200);
    }

    #[test]
    fn test_atomic_save() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Create initial checkpoint
        let mut checkpoint = Checkpoint::new();
        checkpoint.last_batch_submitted = 42;
        checkpoint.save(path).unwrap();

        // Update and save again
        checkpoint.last_batch_submitted = 100;
        checkpoint.save(path).unwrap();

        // Verify the latest data is present
        let loaded = Checkpoint::load(path).unwrap().unwrap();
        assert_eq!(loaded.last_batch_submitted, 100);
    }
}
