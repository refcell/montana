//! Derivation runner implementation.

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use database::Database;
use montana_pipeline::{CompressedBatch, Compressor, L1BatchSource};
use primitives::OpBlockBatch;
use tokio::time::sleep;
use vm::BlockExecutor;

use crate::{DerivationConfig, DerivationError, DerivationMetrics};

/// Derivation runner that polls for batches and derives L2 blocks.
///
/// The runner continuously polls a batch source for new compressed batches,
/// decompresses them, deserializes the `OpBlockBatch`, and tracks metrics
/// about the derivation process.
///
/// The database is generic, allowing different storage backends to be used
/// (e.g., TrieDatabase with RocksDB for production, or in-memory for testing).
pub struct DerivationRunner<S, C, DB>
where
    S: L1BatchSource,
    C: Compressor,
    DB: Database,
{
    /// The batch source to poll for new batches.
    source: S,
    /// The compressor for decompressing batches.
    compressor: C,
    /// Block executor for processing derived blocks.
    /// Currently dead code - execution is disabled until ready.
    #[allow(dead_code)]
    executor: BlockExecutor<DB>,
    /// Configuration.
    config: DerivationConfig,
    /// Accumulated metrics.
    metrics: DerivationMetrics,
    /// Batch submission timestamps for latency tracking.
    submission_times: std::collections::HashMap<u64, Instant>,
    /// Batch-to-block-count mapping for computing block numbers.
    /// Maps batch_number to the number of blocks in that batch.
    batch_block_counts: std::collections::HashMap<u64, u64>,
    /// The last finalized block number.
    last_finalized_block: u64,
    /// Whether the runner is paused.
    paused: bool,
    /// The checkpoint state.
    checkpoint: montana_checkpoint::Checkpoint,
    /// Path to save checkpoints (None means no persistence).
    checkpoint_path: Option<PathBuf>,
}

impl<S, C, DB> DerivationRunner<S, C, DB>
where
    S: L1BatchSource,
    C: Compressor,
    DB: Database,
{
    /// Creates a new derivation runner with the given executor.
    pub fn new(
        source: S,
        compressor: C,
        config: DerivationConfig,
        executor: BlockExecutor<DB>,
    ) -> Self {
        Self {
            source,
            compressor,
            executor,
            config,
            metrics: DerivationMetrics::default(),
            submission_times: std::collections::HashMap::new(),
            batch_block_counts: std::collections::HashMap::new(),
            last_finalized_block: 0,
            paused: false,
            checkpoint: montana_checkpoint::Checkpoint::default(),
            checkpoint_path: None,
        }
    }

    /// Creates a new derivation runner with a starting block number.
    ///
    /// Use this when resuming derivation from a non-zero block.
    pub fn with_start_block(
        source: S,
        compressor: C,
        config: DerivationConfig,
        executor: BlockExecutor<DB>,
        start_block: u64,
    ) -> Self {
        Self {
            source,
            compressor,
            executor,
            config,
            metrics: DerivationMetrics::default(),
            submission_times: std::collections::HashMap::new(),
            batch_block_counts: std::collections::HashMap::new(),
            last_finalized_block: start_block,
            paused: false,
            checkpoint: montana_checkpoint::Checkpoint::default(),
            checkpoint_path: None,
        }
    }

    /// Pause the runner.
    pub const fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume the runner.
    pub const fn resume(&mut self) {
        self.paused = false;
    }

    /// Check if the runner is paused.
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Toggle pause state.
    pub const fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Configures the runner to use checkpointing at the specified path.
    ///
    /// Loads existing checkpoint state from the path and configures automatic
    /// checkpoint saving after each batch derivation.
    pub fn with_checkpoint(
        mut self,
        path: PathBuf,
    ) -> Result<Self, montana_checkpoint::CheckpointError> {
        let checkpoint = montana_checkpoint::Checkpoint::load(&path)?.unwrap_or_default();
        tracing::info!(
            "Resuming from checkpoint at {:?}, last_batch_derived={}",
            path,
            checkpoint.last_batch_derived
        );
        self.checkpoint = checkpoint;
        self.checkpoint_path = Some(path);
        Ok(self)
    }

    /// Checks if a batch should be skipped because it was already derived.
    pub const fn should_skip_batch(&self, batch_number: u64) -> bool {
        // Skip if the checkpoint shows we've already processed a batch >= this one
        // A checkpoint with last_batch_derived = N means batch N was successfully derived
        // So we should skip any batch with number <= N
        // But the default value is 0, which means "no batches derived yet"
        // So we need to handle the special case where last_batch_derived is 0
        if self.checkpoint.last_batch_derived == 0 {
            false
        } else {
            batch_number <= self.checkpoint.last_batch_derived
        }
    }

    /// Records that a batch was derived and saves the checkpoint to disk if configured.
    pub fn record_batch_derived(
        &mut self,
        batch_number: u64,
    ) -> Result<(), montana_checkpoint::CheckpointError> {
        self.checkpoint.record_batch_derived(batch_number);
        self.checkpoint.touch();
        if let Some(path) = &self.checkpoint_path {
            self.checkpoint.save(path)?;
            tracing::debug!("Checkpoint saved to {:?}, batch={}", path, batch_number);
        }
        Ok(())
    }

    /// Record a batch submission time for latency tracking.
    ///
    /// This should be called when a batch is submitted, before it's derived.
    pub fn record_submission(&mut self, batch_number: u64, submit_time: Instant) {
        self.submission_times.insert(batch_number, submit_time);
    }

    /// Record the block count for a submitted batch.
    ///
    /// This should be called from the sequencer side when a batch is submitted,
    /// so that when the batch is later derived, we know how many blocks it contains.
    pub fn record_batch_block_count(&mut self, batch_number: u64, block_count: u64) {
        self.batch_block_counts.insert(batch_number, block_count);
    }

    /// Get the last finalized block number.
    pub const fn last_finalized_block(&self) -> u64 {
        self.last_finalized_block
    }

    /// Get a reference to the accumulated metrics.
    pub const fn metrics(&self) -> &DerivationMetrics {
        &self.metrics
    }

    /// Get a mutable reference to the accumulated metrics.
    pub const fn metrics_mut(&mut self) -> &mut DerivationMetrics {
        &mut self.metrics
    }

    /// Performs a single tick of the derivation loop.
    ///
    /// Attempts to fetch and derive one batch. Returns metrics if a batch was
    /// successfully derived, None if no batch is available, or an error if
    /// derivation fails.
    pub async fn tick(&mut self) -> Result<Option<DerivationMetrics>, DerivationError> {
        // Check if paused
        if self.paused {
            sleep(Duration::from_millis(100)).await;
            return Ok(None);
        }

        // Try to get the next batch
        let derive_time = Instant::now();
        let batch = match self.source.next_batch().await {
            Ok(Some(batch)) => batch,
            Ok(None) => {
                // No batch available, sleep and return None
                sleep(Duration::from_millis(self.config.poll_interval_ms)).await;
                return Ok(None);
            }
            Err(e) => {
                sleep(Duration::from_millis(self.config.poll_interval_ms)).await;
                return Err(DerivationError::SourceError(e.to_string()));
            }
        };

        // Derive the batch
        self.derive_batch(batch, derive_time).await
    }

    /// Derives a single batch and updates metrics.
    async fn derive_batch(
        &mut self,
        batch: CompressedBatch,
        derive_time: Instant,
    ) -> Result<Option<DerivationMetrics>, DerivationError> {
        let _compressed_size = batch.data.len();
        let batch_number = batch.batch_number;

        // Check if we should skip this batch
        if self.should_skip_batch(batch_number) {
            tracing::info!(
                "Skipping batch #{} as it was already derived (checkpoint: {})",
                batch_number,
                self.checkpoint.last_batch_derived
            );
            return Ok(None);
        }

        // Decompress the batch
        let decompressed = self.compressor.decompress(&batch.data).map_err(|e| {
            DerivationError::DecompressionFailed(format!(
                "Failed to decompress batch #{}: {}",
                batch_number, e
            ))
        })?;

        let decompressed_size = decompressed.len();

        // Deserialize the OpBlockBatch from the decompressed data to validate it
        let block_batch = OpBlockBatch::from_bytes(&decompressed).map_err(|e| {
            DerivationError::DecompressionFailed(format!(
                "Failed to deserialize OpBlockBatch from batch #{}: {}",
                batch_number, e
            ))
        })?;

        // Dead code: would execute blocks when enabled
        // TODO: When ready to enable execution, uncomment the loop below:
        // for block in block_batch.blocks {
        //     let result = self.executor.execute_block(block)?;
        //     tracing::info!(block = result.block_number, "Executed block");
        // }
        let _ = (&self.executor, &block_batch); // suppress unused warnings

        // Record the batch derivation in checkpoint
        self.record_batch_derived(batch_number)
            .map_err(|e| DerivationError::InternalError(format!("Checkpoint error: {}", e)))?;

        // Get the block count for this batch (if recorded by sequencer)
        // Default to 1 if not recorded (e.g., validator-only mode)
        let blocks_in_batch = self.batch_block_counts.remove(&batch_number).unwrap_or(1);

        // Compute the block range for this batch
        let first_block = self.last_finalized_block + 1;
        let last_block = self.last_finalized_block + blocks_in_batch;

        // Update the finalized head
        self.last_finalized_block = last_block;

        // Update metrics
        self.metrics.batches_derived += 1;
        self.metrics.blocks_derived += blocks_in_batch;
        self.metrics.bytes_decompressed += decompressed_size as u64;
        self.metrics.current_batch_number = batch_number;
        self.metrics.blocks_in_current_batch = blocks_in_batch;
        self.metrics.first_block_in_batch = first_block;
        self.metrics.last_block_in_batch = last_block;

        // Calculate latency if we have a submission timestamp
        if let Some(submit_time) = self.submission_times.remove(&batch_number) {
            let latency_ms = derive_time.duration_since(submit_time).as_millis() as u64;
            self.metrics.record_latency(latency_ms);
        }

        // Return a snapshot of current metrics
        Ok(Some(self.metrics.clone()))
    }

    /// Runs the derivation loop continuously until cancelled.
    ///
    /// This is a convenience method that runs tick() in a loop.
    pub async fn run(&mut self) -> Result<(), DerivationError> {
        loop {
            self.tick().await?;
        }
    }
}

impl<S, C, DB> std::fmt::Debug for DerivationRunner<S, C, DB>
where
    S: L1BatchSource,
    C: Compressor,
    DB: Database,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivationRunner")
            .field("config", &self.config)
            .field("metrics", &self.metrics)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use chainspec::BASE_MAINNET;
    use database::{CachedDatabase, RocksDbKvDatabase, TrieDatabase};
    use montana_harness::{AnvilState, DEFAULT_GENESIS_STATE};
    use tempfile::TempDir;
    use vm::BlockExecutor;

    use super::*;

    /// Type alias for the test database stack.
    type TestDb = CachedDatabase<TrieDatabase<RocksDbKvDatabase>>;

    /// Helper to create a real database and executor for tests.
    /// Returns the temp dir (to keep it alive) and the executor.
    fn create_test_executor() -> (TempDir, BlockExecutor<TestDb>) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let anvil_state =
            AnvilState::from_json(DEFAULT_GENESIS_STATE).expect("Failed to parse anvil state");
        let genesis = anvil_state.into_genesis();

        let kvdb_path = temp_dir.path().join("kvdb");
        let trie_path = temp_dir.path().join("triedb");

        let kvdb =
            RocksDbKvDatabase::open_or_create(&kvdb_path, &genesis).expect("Failed to create kvdb");
        let trie_db = TrieDatabase::open_or_create(&trie_path, &genesis, kvdb)
            .expect("Failed to create trie db");
        let cached_db = CachedDatabase::new(trie_db);
        let executor = BlockExecutor::new(cached_db, BASE_MAINNET);

        (temp_dir, executor)
    }

    // Mock batch source for testing
    struct MockBatchSource {
        batches: Vec<Option<CompressedBatch>>,
        index: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockBatchSource {
        fn new(batches: Vec<Option<CompressedBatch>>) -> Self {
            Self { batches, index: std::sync::Arc::new(std::sync::Mutex::new(0)) }
        }
    }

    #[async_trait::async_trait]
    impl L1BatchSource for MockBatchSource {
        async fn next_batch(
            &mut self,
        ) -> Result<Option<CompressedBatch>, montana_pipeline::SourceError> {
            let mut idx = self.index.lock().unwrap();
            if *idx >= self.batches.len() {
                return Ok(None);
            }
            let result = self.batches[*idx].clone();
            *idx += 1;
            Ok(result)
        }

        async fn l1_head(&self) -> Result<u64, montana_pipeline::SourceError> {
            Ok(0)
        }
    }

    // Mock compressor for testing that returns valid OpBlockBatch JSON
    struct MockCompressor;

    impl Compressor for MockCompressor {
        fn compress(&self, _data: &[u8]) -> Result<Vec<u8>, montana_pipeline::CompressionError> {
            unimplemented!("Not needed for derivation tests")
        }

        fn decompress(&self, _data: &[u8]) -> Result<Vec<u8>, montana_pipeline::CompressionError> {
            // Return a valid empty OpBlockBatch JSON
            let batch = OpBlockBatch::new(vec![]);
            Ok(batch.to_bytes().unwrap())
        }

        fn estimated_ratio(&self) -> f64 {
            0.5
        }
    }

    #[tokio::test]
    async fn test_runner_no_batches() {
        let (_temp_dir, executor) = create_test_executor();
        let source = MockBatchSource::new(vec![None]);
        let compressor = MockCompressor;
        let config = DerivationConfig::default();
        let mut runner = DerivationRunner::new(source, compressor, config, executor);

        let result = runner.tick().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_runner_single_batch() {
        let (_temp_dir, executor) = create_test_executor();

        // Create a valid OpBlockBatch and serialize it
        let block_batch = OpBlockBatch::new(vec![]);
        let serialized = block_batch.to_bytes().unwrap();

        let batch = CompressedBatch {
            batch_number: 0,
            data: serialized.clone(),
            block_count: 0,
            first_block: 0,
            last_block: 0,
        };
        let source = MockBatchSource::new(vec![Some(batch)]);
        let compressor = MockCompressor;
        let config = DerivationConfig::default();
        let mut runner = DerivationRunner::new(source, compressor, config, executor);

        let result = runner.tick().await;
        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert!(metrics.is_some());

        let metrics = metrics.unwrap();
        assert_eq!(metrics.batches_derived, 1);
    }

    #[test]
    fn test_runner_debug() {
        let (_temp_dir, executor) = create_test_executor();
        let source = MockBatchSource::new(vec![]);
        let compressor = MockCompressor;
        let config = DerivationConfig::default();
        let runner = DerivationRunner::new(source, compressor, config, executor);

        let debug_str = format!("{:?}", runner);
        assert!(debug_str.contains("DerivationRunner"));
    }

    #[test]
    fn test_runner_metrics_access() {
        let (_temp_dir, executor) = create_test_executor();
        let source = MockBatchSource::new(vec![]);
        let compressor = MockCompressor;
        let config = DerivationConfig::default();
        let mut runner = DerivationRunner::new(source, compressor, config, executor);

        // Test immutable access
        let metrics = runner.metrics();
        assert_eq!(metrics.batches_derived, 0);

        // Test mutable access
        let metrics_mut = runner.metrics_mut();
        metrics_mut.batches_derived = 10;
        assert_eq!(runner.metrics().batches_derived, 10);
    }
}
