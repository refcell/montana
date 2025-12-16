//! Batch submission metrics types.

use std::time::Instant;

/// Batch submission metrics for observability.
#[derive(Clone, Debug, Default)]
pub struct BatchSubmissionMetrics {
    /// Total batches submitted.
    pub batches_submitted: u64,
    /// Total blocks processed.
    pub blocks_processed: u64,
    /// Total uncompressed bytes.
    pub bytes_original: u64,
    /// Total compressed bytes.
    pub bytes_compressed: u64,
    /// Latest block number processed.
    pub latest_block: u64,
    /// Submission latencies in milliseconds (for calculating avg).
    latencies_ms: Vec<u64>,
    /// Batch submission timestamps for latency tracking.
    submission_times: std::collections::HashMap<u64, Instant>,
}

impl BatchSubmissionMetrics {
    /// Create new metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a submission latency sample.
    pub fn record_latency(&mut self, latency_ms: u64) {
        // Keep at most the last 100 samples
        if self.latencies_ms.len() >= 100 {
            self.latencies_ms.remove(0);
        }
        self.latencies_ms.push(latency_ms);
    }

    /// Get the average submission latency in milliseconds.
    pub fn avg_latency_ms(&self) -> f64 {
        if self.latencies_ms.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.latencies_ms.iter().sum();
        sum as f64 / self.latencies_ms.len() as f64
    }

    /// Get the compression ratio.
    pub fn compression_ratio(&self) -> f64 {
        if self.bytes_original == 0 {
            return 1.0;
        }
        self.bytes_compressed as f64 / self.bytes_original as f64
    }

    /// Record a batch submission time.
    pub fn record_submission_time(&mut self, batch_number: u64, time: Instant) {
        self.submission_times.insert(batch_number, time);
    }

    /// Get and remove a batch submission time.
    pub fn take_submission_time(&mut self, batch_number: u64) -> Option<Instant> {
        self.submission_times.remove(&batch_number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_default_metrics() {
        let metrics = BatchSubmissionMetrics::new();
        assert_eq!(metrics.batches_submitted, 0);
        assert_eq!(metrics.blocks_processed, 0);
        assert_eq!(metrics.bytes_original, 0);
        assert_eq!(metrics.bytes_compressed, 0);
        assert_eq!(metrics.latest_block, 0);
    }

    #[test]
    fn record_latency() {
        let mut metrics = BatchSubmissionMetrics::new();
        metrics.record_latency(100);
        metrics.record_latency(200);
        metrics.record_latency(150);

        assert_eq!(metrics.avg_latency_ms(), 150.0);
    }

    #[test]
    fn avg_latency_ms_empty() {
        let metrics = BatchSubmissionMetrics::new();
        assert_eq!(metrics.avg_latency_ms(), 0.0);
    }

    #[test]
    fn record_latency_limit() {
        let mut metrics = BatchSubmissionMetrics::new();
        for i in 0..150 {
            metrics.record_latency(i);
        }
        // Should only keep last 100
        assert_eq!(metrics.latencies_ms.len(), 100);
    }

    #[test]
    fn compression_ratio() {
        let mut metrics = BatchSubmissionMetrics::new();
        metrics.bytes_original = 1000;
        metrics.bytes_compressed = 500;

        assert!((metrics.compression_ratio() - 0.5).abs() < 0.01);
    }

    #[test]
    fn compression_ratio_zero_original() {
        let metrics = BatchSubmissionMetrics::new();
        assert_eq!(metrics.compression_ratio(), 1.0);
    }

    #[test]
    fn metrics_clone() {
        let mut metrics = BatchSubmissionMetrics::new();
        metrics.batches_submitted = 10;
        metrics.blocks_processed = 100;
        metrics.bytes_original = 1000;
        metrics.bytes_compressed = 500;
        metrics.latest_block = 123;

        let cloned = metrics.clone();
        assert_eq!(metrics.batches_submitted, cloned.batches_submitted);
        assert_eq!(metrics.blocks_processed, cloned.blocks_processed);
        assert_eq!(metrics.bytes_original, cloned.bytes_original);
        assert_eq!(metrics.bytes_compressed, cloned.bytes_compressed);
        assert_eq!(metrics.latest_block, cloned.latest_block);
    }

    #[test]
    fn metrics_debug() {
        let metrics = BatchSubmissionMetrics::new();
        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("BatchSubmissionMetrics"));
    }

    #[test]
    fn submission_times() {
        let mut metrics = BatchSubmissionMetrics::new();
        let time = Instant::now();
        metrics.record_submission_time(1, time);

        let retrieved = metrics.take_submission_time(1);
        assert!(retrieved.is_some());

        // Should be removed after take
        let again = metrics.take_submission_time(1);
        assert!(again.is_none());
    }
}
