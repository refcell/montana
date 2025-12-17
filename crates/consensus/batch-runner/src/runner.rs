//! Batch submission runner implementation.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use montana_pipeline::{BatchSink, CompressedBatch, Compressor, L2BlockData};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::{
    BatchSubmissionConfig, BatchSubmissionError, BatchSubmissionMetrics, BlockSource,
    BlockSourceError,
};

/// Callback trait for batch submission events.
#[async_trait]
pub trait BatchSubmissionCallback: Send + Sync {
    /// Called when a batch is successfully submitted.
    async fn on_batch_submitted(
        &self,
        batch_number: u64,
        blocks_count: usize,
        original_size: usize,
        compressed_size: usize,
        tx_hash: [u8; 32],
    );

    /// Called when a batch submission fails.
    async fn on_batch_failed(&self, batch_number: u64, error: &BatchSubmissionError);

    /// Called when a new block is processed.
    async fn on_block_processed(&self, block_number: u64, tx_count: usize, size: usize);

    /// Called when the chain head is updated.
    async fn on_chain_head_updated(&self, head: u64);
}

/// Batch submission runner that orchestrates block streaming and batch submission.
pub struct BatchSubmissionRunner<S, C, K>
where
    S: BlockSource + Send + Sync,
    C: Compressor + Send + Sync,
    K: BatchSink + Send + Sync,
{
    /// Block source for fetching L2 blocks.
    source: S,
    /// Compressor for batch data.
    compressor: C,
    /// Batch sink for submission.
    sink: K,
    /// Configuration.
    config: BatchSubmissionConfig,
    /// Metrics.
    metrics: BatchSubmissionMetrics,
    /// Optional callback for events.
    callback: Option<Box<dyn BatchSubmissionCallback>>,
    /// Whether the runner is paused.
    paused: bool,
}

impl<S, C, K> BatchSubmissionRunner<S, C, K>
where
    S: BlockSource + Send + Sync,
    C: Compressor + Send + Sync,
    K: BatchSink + Send + Sync,
{
    /// Create a new batch submission runner.
    pub fn new(source: S, compressor: C, sink: K, config: BatchSubmissionConfig) -> Self {
        Self {
            source,
            compressor,
            sink,
            config,
            metrics: BatchSubmissionMetrics::new(),
            callback: None,
            paused: false,
        }
    }

    /// Set a callback for batch submission events.
    pub fn with_callback(mut self, callback: impl BatchSubmissionCallback + 'static) -> Self {
        self.callback = Some(Box::new(callback));
        self
    }

    /// Get a reference to the metrics.
    pub const fn metrics(&self) -> &BatchSubmissionMetrics {
        &self.metrics
    }

    /// Get a mutable reference to the metrics.
    pub fn metrics_mut(&mut self) -> &mut BatchSubmissionMetrics {
        &mut self.metrics
    }

    /// Pause the runner.
    pub fn pause(&mut self) {
        self.paused = true;
    }

    /// Resume the runner.
    pub fn resume(&mut self) {
        self.paused = false;
    }

    /// Check if the runner is paused.
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Toggle pause state.
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Run the batch submission loop starting from the given block number.
    ///
    /// This function will run indefinitely, streaming blocks from the source,
    /// accumulating them into batches, and submitting through the sink.
    pub async fn run(&mut self, start_block: u64) -> Result<(), BatchSubmissionError> {
        info!("Starting batch submission runner from block #{}", start_block);

        let mut current_block = start_block;
        let mut batch_number = 0u64;
        let mut pending_blocks: Vec<L2BlockData> = Vec::new();
        let mut pending_size = 0usize;

        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);

        loop {
            // Check if paused
            if self.paused {
                sleep(Duration::from_millis(100)).await;
                continue;
            }

            // Update chain head periodically (for TUI display)
            if let Ok(head) = self.source.get_head().await {
                if let Some(ref callback) = self.callback {
                    callback.on_chain_head_updated(head).await;
                }
            }

            // Try to fetch the next block
            match self.source.get_block(current_block).await {
                Ok(block) => {
                    let block_size: usize = block.transactions.iter().map(|tx| tx.0.len()).sum();
                    let tx_count = block.transactions.len();

                    debug!(
                        "Fetched block #{}: {} txs, {} bytes",
                        current_block, tx_count, block_size
                    );

                    // Notify callback
                    if let Some(ref callback) = self.callback {
                        callback.on_block_processed(current_block, tx_count, block_size).await;
                    }

                    pending_blocks.push(block);
                    pending_size += block_size;

                    // Check if we should submit a batch
                    let should_submit = pending_blocks.len() >= self.config.max_blocks_per_batch
                        || pending_size >= self.config.target_batch_size;

                    if should_submit && !pending_blocks.is_empty() {
                        let submission_result =
                            self.submit_batch(batch_number, &pending_blocks, pending_size).await;

                        match submission_result {
                            Ok(tx_hash) => {
                                info!(
                                    "Submitted batch #{}: {} blocks, {} bytes compressed",
                                    batch_number,
                                    pending_blocks.len(),
                                    self.metrics.bytes_compressed
                                );

                                // Notify callback
                                if let Some(ref callback) = self.callback {
                                    callback
                                        .on_batch_submitted(
                                            batch_number,
                                            pending_blocks.len(),
                                            pending_size,
                                            self.metrics.bytes_compressed as usize,
                                            tx_hash,
                                        )
                                        .await;
                                }
                            }
                            Err(e) => {
                                error!("Failed to submit batch #{}: {}", batch_number, e);

                                // Notify callback
                                if let Some(ref callback) = self.callback {
                                    callback.on_batch_failed(batch_number, &e).await;
                                }

                                // Return error if fatal
                                if e.is_fatal() {
                                    return Err(e);
                                }
                            }
                        }

                        batch_number += 1;

                        // Clear pending
                        pending_blocks.clear();
                        pending_size = 0;
                    }

                    current_block += 1;
                    self.metrics.latest_block = current_block;
                }
                Err(e) => {
                    // Block not available yet
                    if Self::is_block_not_found(&e) {
                        debug!("Block #{} not available yet, waiting...", current_block);
                    } else {
                        warn!("Error fetching block #{}: {:?}", current_block, e);
                    }
                    // Wait before retrying
                    sleep(poll_interval).await;
                }
            }
        }
    }

    /// Check if an error indicates block not found.
    fn is_block_not_found(error: &BlockSourceError) -> bool {
        matches!(error, BlockSourceError::BlockNotFound(_))
    }

    /// Submit a batch of blocks.
    async fn submit_batch(
        &mut self,
        batch_number: u64,
        blocks: &[L2BlockData],
        _original_size: usize,
    ) -> Result<[u8; 32], BatchSubmissionError> {
        let start_time = Instant::now();

        // Encode blocks into raw batch data
        let raw_data = Self::encode_blocks(blocks);
        let actual_size = raw_data.len();

        // Compress the batch
        let compressed = self
            .compressor
            .compress(&raw_data)
            .map_err(|e| BatchSubmissionError::CompressionFailed(e.to_string()))?;

        let compressed_size = compressed.len();
        let ratio = if actual_size > 0 { compressed_size as f64 / actual_size as f64 } else { 1.0 };

        debug!(
            "Compressed batch #{}: {} -> {} bytes ({:.1}%)",
            batch_number,
            actual_size,
            compressed_size,
            ratio * 100.0
        );

        // Create the batch
        let batch = CompressedBatch { batch_number, data: compressed };

        // Record submission time before submitting
        self.metrics.record_submission_time(batch_number, start_time);

        // Submit through the sink
        let receipt = self
            .sink
            .submit(batch)
            .await
            .map_err(|e| BatchSubmissionError::SinkError(e.to_string()))?;

        // Update metrics
        let elapsed = start_time.elapsed();
        self.metrics.batches_submitted += 1;
        self.metrics.blocks_processed += blocks.len() as u64;
        self.metrics.bytes_original += actual_size as u64;
        self.metrics.bytes_compressed += compressed_size as u64;
        self.metrics.record_latency(elapsed.as_millis() as u64);

        Ok(receipt.tx_hash)
    }

    /// Encode blocks into raw batch data.
    fn encode_blocks(blocks: &[L2BlockData]) -> Vec<u8> {
        let mut raw_batch = Vec::new();
        for block in blocks {
            for tx in &block.transactions {
                raw_batch.extend_from_slice(&tx.0);
            }
        }
        raw_batch
    }
}

impl<S, C, K> std::fmt::Debug for BatchSubmissionRunner<S, C, K>
where
    S: BlockSource + Send + Sync,
    C: Compressor + Send + Sync,
    K: BatchSink + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchSubmissionRunner")
            .field("config", &self.config)
            .field("metrics", &self.metrics)
            .field("paused", &self.paused)
            .field("has_callback", &self.callback.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use montana_pipeline::{CompressionError, SinkError, SubmissionReceipt};

    use super::*;

    // Mock block source for testing
    struct MockBlockSource {
        blocks: Vec<L2BlockData>,
        index: Arc<Mutex<usize>>,
    }

    impl MockBlockSource {
        fn new(blocks: Vec<L2BlockData>) -> Self {
            Self { blocks, index: Arc::new(Mutex::new(0)) }
        }
    }

    #[async_trait]
    impl BlockSource for MockBlockSource {
        async fn get_block(&mut self, _block_number: u64) -> Result<L2BlockData, BlockSourceError> {
            let mut idx = self.index.lock().unwrap();
            if *idx >= self.blocks.len() {
                return Err(BlockSourceError::BlockNotFound(_block_number));
            }
            let result = self.blocks[*idx].clone();
            *idx += 1;
            Ok(result)
        }

        async fn get_head(&self) -> Result<u64, BlockSourceError> {
            Ok(self.blocks.len() as u64)
        }
    }

    // Mock compressor for testing
    struct MockCompressor;

    impl Compressor for MockCompressor {
        fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
            // Simple mock: just return the same data
            Ok(data.to_vec())
        }

        fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
            Ok(data.to_vec())
        }

        fn estimated_ratio(&self) -> f64 {
            1.0
        }
    }

    // Mock batch sink for testing
    struct MockBatchSink {
        submissions: Arc<Mutex<Vec<CompressedBatch>>>,
    }

    impl MockBatchSink {
        fn new() -> Self {
            Self { submissions: Arc::new(Mutex::new(Vec::new())) }
        }
    }

    #[async_trait]
    impl BatchSink for MockBatchSink {
        async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
            let batch_number = batch.batch_number;
            self.submissions.lock().unwrap().push(batch);
            Ok(SubmissionReceipt {
                batch_number,
                tx_hash: [0xAB; 32],
                l1_block: 100,
                blob_hash: None,
            })
        }

        async fn capacity(&self) -> Result<usize, SinkError> {
            Ok(128 * 1024)
        }

        async fn health_check(&self) -> Result<(), SinkError> {
            Ok(())
        }
    }

    fn create_test_block(block_number: u64) -> L2BlockData {
        L2BlockData {
            block_number,
            timestamp: 1000 + block_number,
            transactions: vec![
                montana_pipeline::Bytes::from(vec![1, 2, 3, 4]),
                montana_pipeline::Bytes::from(vec![5, 6, 7, 8]),
            ],
        }
    }

    #[test]
    fn runner_debug() {
        let source = MockBlockSource::new(vec![]);
        let compressor = MockCompressor;
        let sink = MockBatchSink::new();
        let config = BatchSubmissionConfig::default();
        let runner = BatchSubmissionRunner::new(source, compressor, sink, config);

        let debug_str = format!("{:?}", runner);
        assert!(debug_str.contains("BatchSubmissionRunner"));
    }

    #[test]
    fn runner_pause_resume() {
        let source = MockBlockSource::new(vec![]);
        let compressor = MockCompressor;
        let sink = MockBatchSink::new();
        let config = BatchSubmissionConfig::default();
        let mut runner = BatchSubmissionRunner::new(source, compressor, sink, config);

        assert!(!runner.is_paused());

        runner.pause();
        assert!(runner.is_paused());

        runner.resume();
        assert!(!runner.is_paused());
    }

    #[test]
    fn runner_metrics_access() {
        let source = MockBlockSource::new(vec![]);
        let compressor = MockCompressor;
        let sink = MockBatchSink::new();
        let config = BatchSubmissionConfig::default();
        let mut runner = BatchSubmissionRunner::new(source, compressor, sink, config);

        // Test immutable access
        let metrics = runner.metrics();
        assert_eq!(metrics.batches_submitted, 0);

        // Test mutable access
        runner.metrics_mut().batches_submitted = 10;
        assert_eq!(runner.metrics().batches_submitted, 10);
    }

    #[test]
    fn encode_blocks() {
        let blocks = vec![create_test_block(1), create_test_block(2)];

        let encoded =
            BatchSubmissionRunner::<MockBlockSource, MockCompressor, MockBatchSink>::encode_blocks(
                &blocks,
            );

        // Each block has 2 transactions of 4 bytes each = 8 bytes per block
        // 2 blocks = 16 bytes
        assert_eq!(encoded.len(), 16);
        assert_eq!(&encoded[0..4], &[1, 2, 3, 4]);
        assert_eq!(&encoded[4..8], &[5, 6, 7, 8]);
    }
}
