//! Batch submission runner configuration.

/// Batch submission runner configuration.
#[derive(Clone, Debug)]
pub struct BatchSubmissionConfig {
    /// Maximum blocks per batch before submission (default: 10).
    pub max_blocks_per_batch: usize,
    /// Target batch size in bytes before submission (default: 131072 = 128 KB).
    pub target_batch_size: usize,
    /// Poll interval in milliseconds for fetching new blocks (default: 100).
    pub poll_interval_ms: u64,
}

impl Default for BatchSubmissionConfig {
    fn default() -> Self {
        Self {
            max_blocks_per_batch: 10,
            target_batch_size: 131_072, // 128 KB
            poll_interval_ms: 100,
        }
    }
}

impl BatchSubmissionConfig {
    /// Creates a new builder for configuring a batch submission runner.
    pub fn builder() -> BatchSubmissionConfigBuilder {
        BatchSubmissionConfigBuilder::default()
    }
}

/// Builder for [`BatchSubmissionConfig`].
#[derive(Clone, Debug)]
pub struct BatchSubmissionConfigBuilder {
    max_blocks_per_batch: usize,
    target_batch_size: usize,
    poll_interval_ms: u64,
}

impl Default for BatchSubmissionConfigBuilder {
    fn default() -> Self {
        let defaults = BatchSubmissionConfig::default();
        Self {
            max_blocks_per_batch: defaults.max_blocks_per_batch,
            target_batch_size: defaults.target_batch_size,
            poll_interval_ms: defaults.poll_interval_ms,
        }
    }
}

impl BatchSubmissionConfigBuilder {
    /// Sets the maximum blocks per batch.
    pub const fn max_blocks_per_batch(mut self, max_blocks_per_batch: usize) -> Self {
        self.max_blocks_per_batch = max_blocks_per_batch;
        self
    }

    /// Sets the target batch size in bytes.
    pub const fn target_batch_size(mut self, target_batch_size: usize) -> Self {
        self.target_batch_size = target_batch_size;
        self
    }

    /// Sets the poll interval in milliseconds.
    pub const fn poll_interval_ms(mut self, poll_interval_ms: u64) -> Self {
        self.poll_interval_ms = poll_interval_ms;
        self
    }

    /// Builds the [`BatchSubmissionConfig`].
    pub const fn build(self) -> BatchSubmissionConfig {
        BatchSubmissionConfig {
            max_blocks_per_batch: self.max_blocks_per_batch,
            target_batch_size: self.target_batch_size,
            poll_interval_ms: self.poll_interval_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn config_default() {
        let config = BatchSubmissionConfig::default();
        assert_eq!(config.max_blocks_per_batch, 10);
        assert_eq!(config.target_batch_size, 131_072);
        assert_eq!(config.poll_interval_ms, 100);
    }

    #[test]
    fn builder_default() {
        let config = BatchSubmissionConfig::builder().build();
        let default_config = BatchSubmissionConfig::default();
        assert_eq!(config.max_blocks_per_batch, default_config.max_blocks_per_batch);
        assert_eq!(config.target_batch_size, default_config.target_batch_size);
        assert_eq!(config.poll_interval_ms, default_config.poll_interval_ms);
    }

    #[test]
    fn builder_max_blocks_per_batch() {
        let config = BatchSubmissionConfig::builder().max_blocks_per_batch(20).build();
        assert_eq!(config.max_blocks_per_batch, 20);
    }

    #[test]
    fn builder_target_batch_size() {
        let config = BatchSubmissionConfig::builder().target_batch_size(256 * 1024).build();
        assert_eq!(config.target_batch_size, 256 * 1024);
    }

    #[test]
    fn builder_poll_interval_ms() {
        let config = BatchSubmissionConfig::builder().poll_interval_ms(50).build();
        assert_eq!(config.poll_interval_ms, 50);
    }

    #[test]
    fn builder_chaining() {
        let config = BatchSubmissionConfig::builder()
            .max_blocks_per_batch(20)
            .target_batch_size(256 * 1024)
            .poll_interval_ms(50)
            .build();

        assert_eq!(config.max_blocks_per_batch, 20);
        assert_eq!(config.target_batch_size, 256 * 1024);
        assert_eq!(config.poll_interval_ms, 50);
    }

    #[test]
    fn config_clone() {
        let config = BatchSubmissionConfig {
            max_blocks_per_batch: 20,
            target_batch_size: 256 * 1024,
            poll_interval_ms: 50,
        };
        let cloned = config.clone();
        assert_eq!(cloned.max_blocks_per_batch, config.max_blocks_per_batch);
        assert_eq!(cloned.target_batch_size, config.target_batch_size);
        assert_eq!(cloned.poll_interval_ms, config.poll_interval_ms);
    }

    #[test]
    fn config_debug() {
        let config = BatchSubmissionConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("BatchSubmissionConfig"));
        assert!(debug_str.contains("max_blocks_per_batch"));
        assert!(debug_str.contains("target_batch_size"));
        assert!(debug_str.contains("poll_interval_ms"));
    }

    #[rstest]
    #[case(5, 64 * 1024, 200, "conservative batching")]
    #[case(10, 128 * 1024, 100, "default batching")]
    #[case(20, 256 * 1024, 50, "aggressive batching")]
    fn builder_multiple_configurations(
        #[case] blocks: usize,
        #[case] batch_size: usize,
        #[case] interval: u64,
        #[case] _description: &str,
    ) {
        let config = BatchSubmissionConfig::builder()
            .max_blocks_per_batch(blocks)
            .target_batch_size(batch_size)
            .poll_interval_ms(interval)
            .build();

        assert_eq!(config.max_blocks_per_batch, blocks);
        assert_eq!(config.target_batch_size, batch_size);
        assert_eq!(config.poll_interval_ms, interval);
    }
}
