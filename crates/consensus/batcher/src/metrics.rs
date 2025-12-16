//! Batcher metrics types.
//!
//! These are stub implementations until the metrics crate is added to the workspace.

/// Batcher metrics for observability.
///
/// This is a stub implementation that will be replaced with actual metrics
/// (using the `metrics` crate) in a future iteration.
#[derive(Clone, Debug, Default)]
pub struct BatcherMetrics {
    // Submission metrics
    /// Total batches submitted.
    pub batches_submitted: u64,
    /// Total batches failed.
    pub batches_failed: u64,

    // Block metrics
    /// Total L2 blocks processed.
    pub blocks_processed: u64,
    /// Current pending blocks in buffer.
    pub pending_blocks: u64,
    /// Current L2 safe block number.
    pub l2_safe_block: u64,

    // Size metrics
    /// Total uncompressed bytes submitted.
    pub total_uncompressed_bytes: u64,
    /// Total compressed bytes submitted.
    pub total_compressed_bytes: u64,

    // Transaction metrics
    /// Total L1 gas used.
    pub l1_gas_used: u64,
    /// Total blob gas used.
    pub blob_gas_used: u64,
    /// Total transaction resubmissions.
    pub tx_resubmissions: u64,

    // Health metrics
    /// Current L1 block drift.
    pub l1_drift: u64,
}

impl BatcherMetrics {
    /// Create new metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a successful batch submission.
    pub fn record_submission(&mut self, uncompressed: u64, compressed: u64, gas_used: u64) {
        self.batches_submitted += 1;
        self.total_uncompressed_bytes += uncompressed;
        self.total_compressed_bytes += compressed;
        self.l1_gas_used += gas_used;
    }

    /// Record a failed batch submission.
    pub fn record_failure(&mut self) {
        self.batches_failed += 1;
    }

    /// Record blocks processed.
    pub fn record_blocks(&mut self, count: u64) {
        self.blocks_processed += count;
    }

    /// Update pending blocks count.
    pub fn set_pending_blocks(&mut self, count: u64) {
        self.pending_blocks = count;
    }

    /// Calculate compression ratio.
    pub fn compression_ratio(&self) -> f64 {
        if self.total_uncompressed_bytes == 0 {
            return 1.0;
        }
        self.total_uncompressed_bytes as f64 / self.total_compressed_bytes.max(1) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_default_metrics() {
        let metrics = BatcherMetrics::new();
        assert_eq!(metrics.batches_submitted, 0);
        assert_eq!(metrics.batches_failed, 0);
        assert_eq!(metrics.blocks_processed, 0);
        assert_eq!(metrics.pending_blocks, 0);
        assert_eq!(metrics.l2_safe_block, 0);
        assert_eq!(metrics.total_uncompressed_bytes, 0);
        assert_eq!(metrics.total_compressed_bytes, 0);
        assert_eq!(metrics.l1_gas_used, 0);
        assert_eq!(metrics.blob_gas_used, 0);
        assert_eq!(metrics.tx_resubmissions, 0);
        assert_eq!(metrics.l1_drift, 0);
    }

    #[test]
    fn test_record_submission_increments_counters() {
        let mut metrics = BatcherMetrics::new();
        metrics.record_submission(1000, 500, 100000);

        assert_eq!(metrics.batches_submitted, 1);
        assert_eq!(metrics.total_uncompressed_bytes, 1000);
        assert_eq!(metrics.total_compressed_bytes, 500);
        assert_eq!(metrics.l1_gas_used, 100000);
    }

    #[test]
    fn test_record_submission_multiple_calls() {
        let mut metrics = BatcherMetrics::new();
        metrics.record_submission(1000, 500, 100000);
        metrics.record_submission(2000, 900, 150000);

        assert_eq!(metrics.batches_submitted, 2);
        assert_eq!(metrics.total_uncompressed_bytes, 3000);
        assert_eq!(metrics.total_compressed_bytes, 1400);
        assert_eq!(metrics.l1_gas_used, 250000);
    }

    #[test]
    fn test_record_failure_increments_failure_counter() {
        let mut metrics = BatcherMetrics::new();
        metrics.record_failure();
        assert_eq!(metrics.batches_failed, 1);

        metrics.record_failure();
        assert_eq!(metrics.batches_failed, 2);
    }

    #[test]
    fn test_record_blocks_increments_blocks_processed() {
        let mut metrics = BatcherMetrics::new();
        metrics.record_blocks(5);
        assert_eq!(metrics.blocks_processed, 5);

        metrics.record_blocks(3);
        assert_eq!(metrics.blocks_processed, 8);
    }

    #[test]
    fn test_set_pending_blocks_updates_count() {
        let mut metrics = BatcherMetrics::new();
        metrics.set_pending_blocks(10);
        assert_eq!(metrics.pending_blocks, 10);

        metrics.set_pending_blocks(20);
        assert_eq!(metrics.pending_blocks, 20);

        metrics.set_pending_blocks(0);
        assert_eq!(metrics.pending_blocks, 0);
    }

    #[test]
    fn test_compression_ratio_calculates_correctly() {
        let mut metrics = BatcherMetrics::new();
        metrics.record_submission(1000, 500, 100000);

        let ratio = metrics.compression_ratio();
        assert!((ratio - 2.0).abs() < 0.01); // 1000 / 500 = 2.0
    }

    #[test]
    fn test_compression_ratio_with_multiple_submissions() {
        let mut metrics = BatcherMetrics::new();
        metrics.record_submission(1000, 500, 100000);
        metrics.record_submission(1000, 500, 100000);

        let ratio = metrics.compression_ratio();
        assert!((ratio - 2.0).abs() < 0.01); // 2000 / 1000 = 2.0
    }

    #[test]
    fn test_compression_ratio_returns_one_when_no_bytes() {
        let metrics = BatcherMetrics::new();
        assert_eq!(metrics.compression_ratio(), 1.0);
    }

    #[test]
    fn test_compression_ratio_with_zero_compressed_bytes() {
        let mut metrics = BatcherMetrics::new();
        metrics.total_uncompressed_bytes = 1000;
        metrics.total_compressed_bytes = 0;

        // Should use max(1) to avoid division by zero
        let ratio = metrics.compression_ratio();
        assert_eq!(ratio, 1000.0);
    }

    #[test]
    fn test_metrics_clone() {
        let mut metrics = BatcherMetrics::new();
        metrics.record_submission(1000, 500, 100000);
        metrics.record_failure();
        metrics.record_blocks(5);

        let cloned = metrics.clone();
        assert_eq!(metrics.batches_submitted, cloned.batches_submitted);
        assert_eq!(metrics.batches_failed, cloned.batches_failed);
        assert_eq!(metrics.blocks_processed, cloned.blocks_processed);
        assert_eq!(metrics.total_uncompressed_bytes, cloned.total_uncompressed_bytes);
        assert_eq!(metrics.total_compressed_bytes, cloned.total_compressed_bytes);
    }
}
