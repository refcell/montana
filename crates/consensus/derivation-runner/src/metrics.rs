//! Derivation metrics types.
//!
//! These are stub implementations until the metrics crate is added to the workspace.

/// Derivation metrics for observability.
///
/// This is a stub implementation that will be replaced with actual metrics
/// (using the `metrics` crate) in a future iteration.
#[derive(Clone, Debug, Default)]
pub struct DerivationMetrics {
    /// Total batches derived.
    pub batches_derived: u64,
    /// Total L2 blocks derived.
    pub blocks_derived: u64,
    /// Total bytes decompressed.
    pub bytes_decompressed: u64,
    /// The batch number of the most recently derived batch.
    pub current_batch_number: u64,
    /// Number of blocks in the most recently derived batch.
    pub blocks_in_current_batch: u64,
    /// First block number in the most recently derived batch.
    pub first_block_in_batch: u64,
    /// Last block number in the most recently derived batch (same as finalized head).
    pub last_block_in_batch: u64,
    /// Derivation latencies in milliseconds (for calculating avg).
    latencies_ms: Vec<u64>,
}

impl DerivationMetrics {
    /// Create new metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new derivation latency sample.
    pub fn record_latency(&mut self, latency_ms: u64) {
        // Keep at most the last 100 samples
        if self.latencies_ms.len() >= 100 {
            self.latencies_ms.remove(0);
        }
        self.latencies_ms.push(latency_ms);
    }

    /// Get the average derivation latency in milliseconds.
    pub fn avg_latency_ms(&self) -> f64 {
        if self.latencies_ms.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.latencies_ms.iter().sum();
        sum as f64 / self.latencies_ms.len() as f64
    }

    /// Get the number of latency samples.
    pub const fn latency_samples(&self) -> usize {
        self.latencies_ms.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_default_metrics() {
        let metrics = DerivationMetrics::new();
        assert_eq!(metrics.batches_derived, 0);
        assert_eq!(metrics.blocks_derived, 0);
        assert_eq!(metrics.bytes_decompressed, 0);
        assert_eq!(metrics.avg_latency_ms(), 0.0);
    }

    #[test]
    fn test_record_latency_single_sample() {
        let mut metrics = DerivationMetrics::new();
        metrics.record_latency(100);

        assert_eq!(metrics.latency_samples(), 1);
        assert_eq!(metrics.avg_latency_ms(), 100.0);
    }

    #[test]
    fn test_record_latency_multiple_samples() {
        let mut metrics = DerivationMetrics::new();
        metrics.record_latency(100);
        metrics.record_latency(200);
        metrics.record_latency(300);

        assert_eq!(metrics.latency_samples(), 3);
        assert_eq!(metrics.avg_latency_ms(), 200.0); // (100 + 200 + 300) / 3
    }

    #[test]
    fn test_record_latency_max_samples() {
        let mut metrics = DerivationMetrics::new();

        // Add 150 samples (more than the 100 limit)
        for i in 0..150 {
            metrics.record_latency(i);
        }

        // Should only keep the last 100
        assert_eq!(metrics.latency_samples(), 100);
    }

    #[test]
    fn test_avg_latency_no_samples() {
        let metrics = DerivationMetrics::new();
        assert_eq!(metrics.avg_latency_ms(), 0.0);
    }

    #[test]
    fn test_metrics_clone() {
        let mut metrics = DerivationMetrics::new();
        metrics.batches_derived = 5;
        metrics.blocks_derived = 10;
        metrics.bytes_decompressed = 1000;
        metrics.record_latency(100);

        let cloned = metrics.clone();
        assert_eq!(metrics.batches_derived, cloned.batches_derived);
        assert_eq!(metrics.blocks_derived, cloned.blocks_derived);
        assert_eq!(metrics.bytes_decompressed, cloned.bytes_decompressed);
        assert_eq!(metrics.avg_latency_ms(), cloned.avg_latency_ms());
    }

    #[test]
    fn test_metrics_debug() {
        let metrics = DerivationMetrics::new();
        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("DerivationMetrics"));
    }

    #[test]
    fn test_incrementing_metrics() {
        let mut metrics = DerivationMetrics::new();

        metrics.batches_derived += 1;
        metrics.blocks_derived += 5;
        metrics.bytes_decompressed += 1024;

        assert_eq!(metrics.batches_derived, 1);
        assert_eq!(metrics.blocks_derived, 5);
        assert_eq!(metrics.bytes_decompressed, 1024);
    }

    #[test]
    fn test_latency_with_varying_values() {
        let mut metrics = DerivationMetrics::new();

        metrics.record_latency(50);
        metrics.record_latency(100);
        metrics.record_latency(150);
        metrics.record_latency(200);

        // Average should be 125.0
        let avg = metrics.avg_latency_ms();
        assert!((avg - 125.0).abs() < 0.01);
    }
}
