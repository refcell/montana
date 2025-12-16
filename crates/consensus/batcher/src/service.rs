//! Service module for the batcher.
//!
//! This module contains the core batcher service that coordinates
//! batch collection, compression, and submission across the pipeline.

use montana_pipeline::{BatchSink, BatchSource, Compressor, SubmissionReceipt};

use crate::{BatchDriver, BatcherConfig, BatcherError, BatcherMetrics};

/// Current state of the batcher service.
#[derive(Clone, Debug, Default)]
pub struct BatcherState {
    /// Last submitted batch number.
    pub last_batch_number: u64,
    /// Last confirmed L1 block number.
    pub last_l1_block: u64,
    /// Total blocks processed.
    pub total_blocks_processed: u64,
    /// Total batches submitted.
    pub total_batches_submitted: u64,
    /// Whether the service is healthy.
    pub is_healthy: bool,
}

/// The core batcher service, generic over source, compressor, and sink.
pub struct BatcherService<S, C, K>
where
    S: BatchSource,
    C: Compressor,
    K: BatchSink,
{
    #[allow(dead_code)]
    source: S,
    #[allow(dead_code)]
    compressor: C,
    #[allow(dead_code)]
    sink: K,
    config: BatcherConfig,
    driver: BatchDriver,
    state: BatcherState,
    metrics: BatcherMetrics,
}

impl<S, C, K> BatcherService<S, C, K>
where
    S: BatchSource,
    C: Compressor,
    K: BatchSink,
{
    /// Creates a new batcher service with the provided components.
    ///
    /// Initializes the service with default state and metrics, and creates
    /// a new batch driver for managing pending blocks.
    pub fn new(source: S, compressor: C, sink: K, config: BatcherConfig) -> Self {
        let driver = BatchDriver::new(config.clone());
        Self {
            source,
            compressor,
            sink,
            config,
            driver,
            state: BatcherState::default(),
            metrics: BatcherMetrics::default(),
        }
    }

    /// Returns a reference to the current service state.
    pub fn state(&self) -> &BatcherState {
        &self.state
    }

    /// Returns a reference to the service metrics.
    pub fn metrics(&self) -> &BatcherMetrics {
        &self.metrics
    }

    /// Returns a reference to the service configuration.
    pub fn config(&self) -> &BatcherConfig {
        &self.config
    }

    /// Returns the number of pending blocks waiting to be batched.
    pub fn pending_blocks(&self) -> usize {
        self.driver.pending_count()
    }

    /// Process one tick of the batcher service.
    ///
    /// This method performs the following steps:
    /// - Collects pending blocks from the source
    /// - Adds blocks to the batch driver
    /// - Checks if a batch should be submitted based on driver state
    /// - If submission is needed:
    ///   - Builds a batch from pending blocks
    ///   - Compresses the batch using the configured compressor
    ///   - Submits the batch via the sink
    ///   - Updates service state and metrics
    ///   - Returns the submission receipt
    /// - Returns None if no batch was submitted
    pub async fn tick(&mut self) -> Result<Option<SubmissionReceipt>, BatcherError> {
        // TODO: Collect pending blocks from source
        // TODO: Add blocks to driver
        // TODO: Check if driver.should_submit()
        // TODO: If should submit:
        //   - Build batch from driver
        //   - Compress batch via compressor
        //   - Submit via sink
        //   - Update state and metrics
        //   - Return receipt
        // TODO: Return Ok(None) if no submission

        Ok(None)
    }
}

impl<S, C, K> std::fmt::Debug for BatcherService<S, C, K>
where
    S: BatchSource,
    C: Compressor,
    K: BatchSink,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatcherService")
            .field("config", &self.config)
            .field("state", &self.state)
            .field("metrics", &self.metrics)
            .field("pending_blocks", &self.pending_blocks())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use montana_pipeline::{
        CompressedBatch, CompressionError, L2BlockData, SinkError, SourceError,
    };

    use super::*;

    /// Mock implementation of BatchSource for testing.
    struct MockSource;

    #[async_trait]
    impl BatchSource for MockSource {
        async fn pending_blocks(&mut self) -> Result<Vec<L2BlockData>, SourceError> {
            Ok(vec![])
        }

        async fn l1_origin(&self) -> Result<u64, SourceError> {
            Ok(0)
        }

        async fn l1_origin_hash(&self) -> Result<[u8; 20], SourceError> {
            Ok([0u8; 20])
        }

        async fn parent_hash(&self) -> Result<[u8; 20], SourceError> {
            Ok([0u8; 20])
        }
    }

    /// Mock implementation of Compressor for testing.
    struct MockCompressor;

    impl Compressor for MockCompressor {
        fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
            Ok(data.to_vec())
        }

        fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
            Ok(data.to_vec())
        }

        fn estimated_ratio(&self) -> f64 {
            1.0
        }
    }

    /// Mock implementation of BatchSink for testing.
    struct MockSink;

    #[async_trait]
    impl BatchSink for MockSink {
        async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
            Ok(SubmissionReceipt {
                batch_number: batch.batch_number,
                tx_hash: [0u8; 32],
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

    #[test]
    fn test_new_creates_service_with_default_state() {
        let source = MockSource;
        let compressor = MockCompressor;
        let sink = MockSink;
        let config = BatcherConfig::default();

        let service = BatcherService::new(source, compressor, sink, config);

        assert_eq!(service.state().last_batch_number, 0);
        assert_eq!(service.state().last_l1_block, 0);
        assert_eq!(service.state().total_blocks_processed, 0);
        assert_eq!(service.state().total_batches_submitted, 0);
        assert!(!service.state().is_healthy);
    }

    #[test]
    fn test_state_returns_correct_state() {
        let service =
            BatcherService::new(MockSource, MockCompressor, MockSink, BatcherConfig::default());
        let state = service.state();

        assert_eq!(state.last_batch_number, 0);
    }

    #[test]
    fn test_pending_blocks_returns_zero_initially() {
        let service =
            BatcherService::new(MockSource, MockCompressor, MockSink, BatcherConfig::default());

        assert_eq!(service.pending_blocks(), 0);
    }
}
