//! Batch driver module.
//!
//! This module contains the `BatchDriver` which coordinates when batches should be
//! submitted based on size, time, and block count criteria.

use std::{collections::VecDeque, path::PathBuf, time::Instant};

/// A batch ready for submission.
#[derive(Clone, Debug)]
pub struct PendingBatch {
    /// Batch sequence number.
    pub batch_number: u64,
    /// Blocks in this batch.
    pub blocks: Vec<montana_pipeline::L2BlockData>,
    /// Total uncompressed size in bytes.
    pub uncompressed_size: usize,
}

/// Drives batching decisions: when to cut a batch, when to wait.
#[derive(Debug)]
pub struct BatchDriver {
    /// Configuration.
    config: crate::BatcherConfig,
    /// Pending blocks to batch.
    pending_blocks: VecDeque<montana_pipeline::L2BlockData>,
    /// Current accumulated size in bytes.
    current_size: usize,
    /// Last submission time.
    last_submission: Instant,
    /// Next batch number.
    batch_number: u64,
    /// The checkpoint state.
    checkpoint: montana_checkpoint::Checkpoint,
    /// Path to save checkpoints (None means no persistence).
    checkpoint_path: Option<PathBuf>,
}

impl BatchDriver {
    /// Creates a new batch driver with the provided configuration.
    pub fn new(config: crate::BatcherConfig) -> Self {
        Self {
            config,
            pending_blocks: VecDeque::new(),
            current_size: 0,
            last_submission: Instant::now(),
            batch_number: 0,
            checkpoint: montana_checkpoint::Checkpoint::default(),
            checkpoint_path: None,
        }
    }

    /// Adds blocks to the pending queue and updates current size.
    pub fn add_blocks(&mut self, blocks: Vec<montana_pipeline::L2BlockData>) {
        for block in blocks {
            // Calculate the size contribution of this block's transactions
            let block_size: usize = block.transactions.iter().map(|tx| tx.len()).sum();
            self.current_size += block_size;
            self.pending_blocks.push_back(block);
        }
    }

    /// Determines if a batch should be submitted based on configured criteria.
    pub fn should_submit(&self) -> bool {
        self.size_ready() || self.time_ready() || self.blocks_ready()
    }

    /// Builds a batch if submission criteria are met.
    ///
    /// Returns `Some(PendingBatch)` if `should_submit()` is true, draining pending blocks
    /// up to the configured limits and incrementing the batch number. Returns `None` otherwise.
    pub fn build_batch(&mut self) -> Option<PendingBatch> {
        if !self.should_submit() {
            return None;
        }

        // Drain pending blocks up to max_blocks_per_batch
        let mut blocks = Vec::new();
        let max_blocks = self.config.max_blocks_per_batch as usize;

        while !self.pending_blocks.is_empty() && blocks.len() < max_blocks {
            if let Some(block) = self.pending_blocks.pop_front() {
                blocks.push(block);
            }
        }

        // Recalculate current_size based on remaining blocks
        self.current_size = self
            .pending_blocks
            .iter()
            .flat_map(|block| &block.transactions)
            .map(|tx| tx.len())
            .sum();

        let batch = PendingBatch {
            batch_number: self.batch_number,
            uncompressed_size: blocks
                .iter()
                .flat_map(|block| &block.transactions)
                .map(|tx| tx.len())
                .sum(),
            blocks,
        };

        self.batch_number += 1;
        self.last_submission = Instant::now();

        Some(batch)
    }

    /// Returns the number of pending blocks waiting to be batched.
    pub fn pending_count(&self) -> usize {
        self.pending_blocks.len()
    }

    /// Returns the current accumulated size in bytes.
    pub const fn current_size(&self) -> usize {
        self.current_size
    }

    /// Checks if the batch size threshold has been reached.
    const fn size_ready(&self) -> bool {
        self.current_size >= self.config.min_batch_size
    }

    /// Checks if the time threshold for submission has been reached.
    fn time_ready(&self) -> bool {
        self.last_submission.elapsed() >= self.config.batch_interval
    }

    /// Checks if the maximum blocks per batch threshold has been reached.
    fn blocks_ready(&self) -> bool {
        self.pending_blocks.len() >= self.config.max_blocks_per_batch as usize
    }

    /// Loads a checkpoint from the specified path and sets the checkpoint path.
    ///
    /// If a checkpoint exists at the path, it will be loaded and the batch driver will
    /// resume from that state. If no checkpoint exists, a new one will be created.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the checkpoint file
    ///
    /// # Errors
    ///
    /// Returns [`montana_checkpoint::CheckpointError`] if there's an error loading the checkpoint.
    pub fn with_checkpoint(
        mut self,
        path: PathBuf,
    ) -> Result<Self, montana_checkpoint::CheckpointError> {
        match montana_checkpoint::Checkpoint::load(&path)? {
            Some(checkpoint) => {
                tracing::info!(
                    "Resuming from checkpoint at {:?}, last batch submitted: {}",
                    path,
                    checkpoint.last_batch_submitted
                );
                self.checkpoint = checkpoint;
            }
            None => {
                tracing::info!("No checkpoint found at {:?}, starting fresh", path);
            }
        }
        self.checkpoint_path = Some(path);
        Ok(self)
    }

    /// Checks if a batch should be skipped based on the checkpoint state.
    ///
    /// Returns `true` if the batch has already been submitted according to the checkpoint.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number to check
    pub const fn should_skip_batch(&self, batch_number: u64) -> bool {
        self.checkpoint.should_skip_batch(batch_number)
    }

    /// Records that a batch has been successfully submitted.
    ///
    /// Updates the checkpoint state, touches the timestamp, and saves to disk if a
    /// checkpoint path is configured.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number that was submitted
    ///
    /// # Errors
    ///
    /// Returns [`montana_checkpoint::CheckpointError`] if there's an error saving the checkpoint.
    pub fn record_batch_submitted(
        &mut self,
        batch_number: u64,
    ) -> Result<(), montana_checkpoint::CheckpointError> {
        self.checkpoint.record_batch_submitted(batch_number);
        self.checkpoint.touch();

        if let Some(path) = &self.checkpoint_path {
            self.checkpoint.save(path)?;
            tracing::info!(
                "Recorded batch {} as submitted, checkpoint saved to {:?}",
                batch_number,
                path
            );
        } else {
            tracing::info!(
                "Recorded batch {} as submitted (no checkpoint persistence)",
                batch_number
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rstest::rstest;

    use super::*;

    fn default_config() -> crate::BatcherConfig {
        crate::BatcherConfig {
            min_batch_size: 1000,
            batch_interval: Duration::from_secs(10),
            max_blocks_per_batch: 5,
            max_batch_size: 128 * 1024,
            use_blobs: true,
            max_blob_fee_per_gas: 10_000_000_000,
            max_blobs_per_tx: 6,
            num_confirmations: 10,
            resubmission_timeout: Duration::from_secs(48),
            network_timeout: Duration::from_secs(10),
            max_l1_drift: 30,
            sequencing_window: 3600,
            safe_confirmations: 12,
        }
    }

    #[test]
    fn test_new_creates_empty_driver() {
        let config = default_config();
        let driver = BatchDriver::new(config);

        assert_eq!(driver.pending_count(), 0);
        assert_eq!(driver.current_size(), 0);
        assert_eq!(driver.batch_number, 0);
    }

    #[test]
    fn test_add_blocks_increases_pending_count_and_size() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        let blocks = vec![
            montana_pipeline::L2BlockData {
                timestamp: 1000,
                transactions: vec![montana_pipeline::Bytes::from(vec![1, 2, 3, 4, 5])],
            },
            montana_pipeline::L2BlockData {
                timestamp: 2000,
                transactions: vec![montana_pipeline::Bytes::from(vec![6, 7, 8])],
            },
        ];

        driver.add_blocks(blocks);

        assert_eq!(driver.pending_count(), 2);
        assert_eq!(driver.current_size(), 8); // 5 + 3 bytes
    }

    #[test]
    fn test_should_submit_returns_false_when_empty() {
        let config = default_config();
        let driver = BatchDriver::new(config);

        assert!(!driver.should_submit());
    }

    #[test]
    fn test_should_submit_returns_true_when_size_threshold_met() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        let blocks = vec![montana_pipeline::L2BlockData {
            timestamp: 1000,
            transactions: vec![montana_pipeline::Bytes::from(vec![0; 1001])],
        }];

        driver.add_blocks(blocks);

        assert!(driver.should_submit());
    }

    #[test]
    fn test_should_submit_returns_true_when_block_count_threshold_met() {
        let mut config = default_config();
        config.min_batch_size = 10000; // Set high so we don't trigger size_ready
        let mut driver = BatchDriver::new(config);

        let mut blocks = Vec::new();
        for i in 0..5 {
            blocks.push(montana_pipeline::L2BlockData {
                timestamp: 1000 + i,
                transactions: vec![montana_pipeline::Bytes::from(vec![i as u8; 10])],
            });
        }

        driver.add_blocks(blocks);

        assert!(driver.should_submit());
    }

    #[test]
    fn test_should_submit_returns_true_when_time_threshold_met() {
        let mut config = default_config();
        config.batch_interval = Duration::from_secs(0); // Instantly ready
        let driver = BatchDriver::new(config);

        assert!(driver.should_submit());
    }

    #[test]
    fn test_build_batch_returns_none_when_not_ready() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        let blocks = vec![montana_pipeline::L2BlockData {
            timestamp: 1000,
            transactions: vec![montana_pipeline::Bytes::from(vec![1, 2, 3])],
        }];

        driver.add_blocks(blocks);

        assert!(driver.build_batch().is_none());
    }

    #[test]
    fn test_build_batch_returns_batch_when_ready() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        let blocks = vec![montana_pipeline::L2BlockData {
            timestamp: 1000,
            transactions: vec![montana_pipeline::Bytes::from(vec![0; 1001])],
        }];

        driver.add_blocks(blocks);

        let batch = driver.build_batch();

        assert!(batch.is_some());
        let pending = batch.unwrap();
        assert_eq!(pending.batch_number, 0);
        assert_eq!(pending.blocks.len(), 1);
        assert_eq!(pending.uncompressed_size, 1001);
    }

    #[test]
    fn test_build_batch_clears_state_when_ready() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        let blocks = vec![montana_pipeline::L2BlockData {
            timestamp: 1000,
            transactions: vec![montana_pipeline::Bytes::from(vec![0; 1001])],
        }];

        driver.add_blocks(blocks);
        driver.build_batch();

        assert_eq!(driver.pending_count(), 0);
        assert_eq!(driver.current_size(), 0);
    }

    #[test]
    fn test_batch_number_increments_after_each_build() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        for i in 0..3 {
            let blocks = vec![montana_pipeline::L2BlockData {
                timestamp: 1000 + i,
                transactions: vec![montana_pipeline::Bytes::from(vec![0; 1001])],
            }];

            driver.add_blocks(blocks);
            let batch = driver.build_batch();

            assert!(batch.is_some());
            assert_eq!(batch.unwrap().batch_number, i as u64);
        }

        assert_eq!(driver.batch_number, 3);
    }

    #[test]
    fn test_build_batch_respects_max_blocks_per_batch() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        // Add 10 blocks
        let blocks: Vec<_> = (0..10)
            .map(|i| montana_pipeline::L2BlockData {
                timestamp: 1000 + i,
                transactions: vec![montana_pipeline::Bytes::from(vec![0; 200])],
            })
            .collect();

        driver.add_blocks(blocks);

        let batch = driver.build_batch();
        assert!(batch.is_some());

        let pending = batch.unwrap();
        assert_eq!(pending.blocks.len(), 5); // max_blocks_per_batch is 5
        assert_eq!(driver.pending_count(), 5); // 5 blocks remaining
    }

    #[test]
    fn test_pending_count_returns_correct_count() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        assert_eq!(driver.pending_count(), 0);

        let blocks = vec![
            montana_pipeline::L2BlockData {
                timestamp: 1000,
                transactions: vec![montana_pipeline::Bytes::from(vec![1, 2])],
            },
            montana_pipeline::L2BlockData {
                timestamp: 2000,
                transactions: vec![montana_pipeline::Bytes::from(vec![3, 4])],
            },
        ];

        driver.add_blocks(blocks);
        assert_eq!(driver.pending_count(), 2);
    }

    #[test]
    fn test_current_size_returns_correct_size() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        assert_eq!(driver.current_size(), 0);

        let blocks = vec![montana_pipeline::L2BlockData {
            timestamp: 1000,
            transactions: vec![
                montana_pipeline::Bytes::from(vec![0; 100]),
                montana_pipeline::Bytes::from(vec![0; 200]),
            ],
        }];

        driver.add_blocks(blocks);
        assert_eq!(driver.current_size(), 300);
    }

    #[rstest]
    #[case(100, false)]
    #[case(999, false)]
    #[case(1000, true)]
    #[case(5000, true)]
    fn test_size_ready_threshold(#[case] size: usize, #[case] expected: bool) {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        let blocks = vec![montana_pipeline::L2BlockData {
            timestamp: 1000,
            transactions: vec![montana_pipeline::Bytes::from(vec![0; size])],
        }];

        driver.add_blocks(blocks);

        assert_eq!(driver.size_ready(), expected);
    }

    #[test]
    fn test_blocks_ready_threshold() {
        let config = default_config();
        let mut driver = BatchDriver::new(config);

        // Add 4 blocks (below threshold)
        let blocks: Vec<_> = (0..4)
            .map(|i| montana_pipeline::L2BlockData {
                timestamp: 1000 + i,
                transactions: vec![montana_pipeline::Bytes::from(vec![0; 10])],
            })
            .collect();

        driver.add_blocks(blocks);
        assert!(!driver.blocks_ready());

        // Add 1 more to reach 5 (threshold)
        let block = montana_pipeline::L2BlockData {
            timestamp: 2000,
            transactions: vec![montana_pipeline::Bytes::from(vec![0; 10])],
        };

        driver.add_blocks(vec![block]);
        assert!(driver.blocks_ready());
    }
}
