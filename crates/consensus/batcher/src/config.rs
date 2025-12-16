//! Batcher configuration.

use std::time::Duration;

/// Batcher configuration.
#[derive(Clone, Debug)]
pub struct BatcherConfig {
    // Batching Strategy
    /// Maximum batch size in bytes (default: 128 KB).
    pub max_batch_size: usize,
    /// Maximum blocks per batch (default: 100).
    pub max_blocks_per_batch: u16,
    /// Batch submission interval (default: 12s).
    pub batch_interval: Duration,
    /// Minimum batch size in bytes (default: 1 KB).
    pub min_batch_size: usize,

    // L1 Submission
    /// Use EIP-4844 blobs for submission (default: true).
    pub use_blobs: bool,
    /// Maximum blob fee per gas (default: 10 gwei).
    pub max_blob_fee_per_gas: u128,
    /// Maximum blobs per transaction (default: 6).
    pub max_blobs_per_tx: usize,

    // Transaction Manager
    /// Number of confirmations required (default: 10).
    pub num_confirmations: u64,
    /// Interval before resubmitting transactions (default: 48s).
    pub resubmission_timeout: Duration,
    /// Network timeout for RPC calls (default: 10s).
    pub network_timeout: Duration,

    // Safety
    /// Maximum L1 block drift (default: 30).
    pub max_l1_drift: u64,
    /// Sequencing window in blocks (default: 3600).
    pub sequencing_window: u64,
    /// Safe confirmations threshold (default: 12).
    pub safe_confirmations: u64,
}

impl Default for BatcherConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 128 * 1024, // 128 KB
            max_blocks_per_batch: 100,
            batch_interval: Duration::from_secs(12),
            min_batch_size: 1024, // 1 KB
            use_blobs: true,
            max_blob_fee_per_gas: 10_000_000_000, // 10 gwei
            max_blobs_per_tx: 6,
            num_confirmations: 10,
            resubmission_timeout: Duration::from_secs(48),
            network_timeout: Duration::from_secs(10),
            max_l1_drift: 30,
            sequencing_window: 3600,
            safe_confirmations: 12,
        }
    }
}

impl BatcherConfig {
    /// Creates a new builder for configuring a batcher.
    pub fn builder() -> BatcherConfigBuilder {
        BatcherConfigBuilder::default()
    }
}

/// Builder for [`BatcherConfig`].
#[derive(Clone, Debug)]
pub struct BatcherConfigBuilder {
    max_batch_size: usize,
    max_blocks_per_batch: u16,
    batch_interval: Duration,
    min_batch_size: usize,
    use_blobs: bool,
    max_blob_fee_per_gas: u128,
    max_blobs_per_tx: usize,
    num_confirmations: u64,
    resubmission_timeout: Duration,
    network_timeout: Duration,
    max_l1_drift: u64,
    sequencing_window: u64,
    safe_confirmations: u64,
}

impl Default for BatcherConfigBuilder {
    fn default() -> Self {
        let defaults = BatcherConfig::default();
        Self {
            max_batch_size: defaults.max_batch_size,
            max_blocks_per_batch: defaults.max_blocks_per_batch,
            batch_interval: defaults.batch_interval,
            min_batch_size: defaults.min_batch_size,
            use_blobs: defaults.use_blobs,
            max_blob_fee_per_gas: defaults.max_blob_fee_per_gas,
            max_blobs_per_tx: defaults.max_blobs_per_tx,
            num_confirmations: defaults.num_confirmations,
            resubmission_timeout: defaults.resubmission_timeout,
            network_timeout: defaults.network_timeout,
            max_l1_drift: defaults.max_l1_drift,
            sequencing_window: defaults.sequencing_window,
            safe_confirmations: defaults.safe_confirmations,
        }
    }
}

impl BatcherConfigBuilder {
    /// Sets the maximum batch size in bytes.
    pub fn max_batch_size(mut self, max_batch_size: usize) -> Self {
        self.max_batch_size = max_batch_size;
        self
    }

    /// Sets the maximum blocks per batch.
    pub fn max_blocks_per_batch(mut self, max_blocks_per_batch: u16) -> Self {
        self.max_blocks_per_batch = max_blocks_per_batch;
        self
    }

    /// Sets the batch submission interval.
    pub fn batch_interval(mut self, batch_interval: Duration) -> Self {
        self.batch_interval = batch_interval;
        self
    }

    /// Sets the minimum batch size in bytes.
    pub fn min_batch_size(mut self, min_batch_size: usize) -> Self {
        self.min_batch_size = min_batch_size;
        self
    }

    /// Sets whether to use EIP-4844 blobs for submission.
    pub fn use_blobs(mut self, use_blobs: bool) -> Self {
        self.use_blobs = use_blobs;
        self
    }

    /// Sets the maximum blob fee per gas.
    pub fn max_blob_fee_per_gas(mut self, max_blob_fee_per_gas: u128) -> Self {
        self.max_blob_fee_per_gas = max_blob_fee_per_gas;
        self
    }

    /// Sets the maximum blobs per transaction.
    pub fn max_blobs_per_tx(mut self, max_blobs_per_tx: usize) -> Self {
        self.max_blobs_per_tx = max_blobs_per_tx;
        self
    }

    /// Sets the number of confirmations required.
    pub fn num_confirmations(mut self, num_confirmations: u64) -> Self {
        self.num_confirmations = num_confirmations;
        self
    }

    /// Sets the interval before resubmitting transactions.
    pub fn resubmission_timeout(mut self, resubmission_timeout: Duration) -> Self {
        self.resubmission_timeout = resubmission_timeout;
        self
    }

    /// Sets the network timeout for RPC calls.
    pub fn network_timeout(mut self, network_timeout: Duration) -> Self {
        self.network_timeout = network_timeout;
        self
    }

    /// Sets the maximum L1 block drift.
    pub fn max_l1_drift(mut self, max_l1_drift: u64) -> Self {
        self.max_l1_drift = max_l1_drift;
        self
    }

    /// Sets the sequencing window in blocks.
    pub fn sequencing_window(mut self, sequencing_window: u64) -> Self {
        self.sequencing_window = sequencing_window;
        self
    }

    /// Sets the safe confirmations threshold.
    pub fn safe_confirmations(mut self, safe_confirmations: u64) -> Self {
        self.safe_confirmations = safe_confirmations;
        self
    }

    /// Builds the [`BatcherConfig`].
    pub fn build(self) -> BatcherConfig {
        BatcherConfig {
            max_batch_size: self.max_batch_size,
            max_blocks_per_batch: self.max_blocks_per_batch,
            batch_interval: self.batch_interval,
            min_batch_size: self.min_batch_size,
            use_blobs: self.use_blobs,
            max_blob_fee_per_gas: self.max_blob_fee_per_gas,
            max_blobs_per_tx: self.max_blobs_per_tx,
            num_confirmations: self.num_confirmations,
            resubmission_timeout: self.resubmission_timeout,
            network_timeout: self.network_timeout,
            max_l1_drift: self.max_l1_drift,
            sequencing_window: self.sequencing_window,
            safe_confirmations: self.safe_confirmations,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn batcher_config_default() {
        let config = BatcherConfig::default();
        assert_eq!(config.max_batch_size, 128 * 1024);
        assert_eq!(config.max_blocks_per_batch, 100);
        assert_eq!(config.batch_interval, Duration::from_secs(12));
        assert_eq!(config.min_batch_size, 1024);
        assert_eq!(config.use_blobs, true);
        assert_eq!(config.max_blob_fee_per_gas, 10_000_000_000);
        assert_eq!(config.max_blobs_per_tx, 6);
        assert_eq!(config.num_confirmations, 10);
        assert_eq!(config.resubmission_timeout, Duration::from_secs(48));
        assert_eq!(config.network_timeout, Duration::from_secs(10));
        assert_eq!(config.max_l1_drift, 30);
        assert_eq!(config.sequencing_window, 3600);
        assert_eq!(config.safe_confirmations, 12);
    }

    #[rstest]
    #[case(64 * 1024, "64 KB batch")]
    #[case(128 * 1024, "default 128 KB batch")]
    #[case(256 * 1024, "256 KB batch")]
    fn batcher_config_max_batch_size(#[case] size: usize, #[case] _description: &str) {
        let config = BatcherConfig { max_batch_size: size, ..Default::default() };
        assert_eq!(config.max_batch_size, size);
    }

    #[rstest]
    #[case(50, "50 blocks")]
    #[case(100, "default 100 blocks")]
    #[case(200, "200 blocks")]
    fn batcher_config_max_blocks_per_batch(#[case] blocks: u16, #[case] _description: &str) {
        let config = BatcherConfig { max_blocks_per_batch: blocks, ..Default::default() };
        assert_eq!(config.max_blocks_per_batch, blocks);
    }

    #[rstest]
    #[case(Duration::from_secs(6), "6 second interval")]
    #[case(Duration::from_secs(12), "default 12 second interval")]
    #[case(Duration::from_secs(30), "30 second interval")]
    fn batcher_config_batch_interval(#[case] interval: Duration, #[case] _description: &str) {
        let config = BatcherConfig { batch_interval: interval, ..Default::default() };
        assert_eq!(config.batch_interval, interval);
    }

    #[rstest]
    #[case(512, "512 bytes")]
    #[case(1024, "default 1 KB")]
    #[case(4096, "4 KB")]
    fn batcher_config_min_batch_size(#[case] size: usize, #[case] _description: &str) {
        let config = BatcherConfig { min_batch_size: size, ..Default::default() };
        assert_eq!(config.min_batch_size, size);
    }

    #[rstest]
    #[case(false, "no blobs")]
    #[case(true, "use blobs")]
    fn batcher_config_use_blobs(#[case] use_blobs: bool, #[case] _description: &str) {
        let config = BatcherConfig { use_blobs, ..Default::default() };
        assert_eq!(config.use_blobs, use_blobs);
    }

    #[rstest]
    #[case(5_000_000_000, "5 gwei")]
    #[case(10_000_000_000, "default 10 gwei")]
    #[case(50_000_000_000, "50 gwei")]
    fn batcher_config_max_blob_fee_per_gas(#[case] fee: u128, #[case] _description: &str) {
        let config = BatcherConfig { max_blob_fee_per_gas: fee, ..Default::default() };
        assert_eq!(config.max_blob_fee_per_gas, fee);
    }

    #[rstest]
    #[case(4, "4 blobs")]
    #[case(6, "default 6 blobs")]
    #[case(8, "8 blobs")]
    fn batcher_config_max_blobs_per_tx(#[case] blobs: usize, #[case] _description: &str) {
        let config = BatcherConfig { max_blobs_per_tx: blobs, ..Default::default() };
        assert_eq!(config.max_blobs_per_tx, blobs);
    }

    #[rstest]
    #[case(5, "5 confirmations")]
    #[case(10, "default 10 confirmations")]
    #[case(20, "20 confirmations")]
    fn batcher_config_num_confirmations(#[case] confirmations: u64, #[case] _description: &str) {
        let config = BatcherConfig { num_confirmations: confirmations, ..Default::default() };
        assert_eq!(config.num_confirmations, confirmations);
    }

    #[rstest]
    #[case(Duration::from_secs(24), "24 second resubmission")]
    #[case(Duration::from_secs(48), "default 48 second resubmission")]
    #[case(Duration::from_secs(120), "120 second resubmission")]
    fn batcher_config_resubmission_timeout(#[case] timeout: Duration, #[case] _description: &str) {
        let config = BatcherConfig { resubmission_timeout: timeout, ..Default::default() };
        assert_eq!(config.resubmission_timeout, timeout);
    }

    #[rstest]
    #[case(Duration::from_secs(5), "5 second timeout")]
    #[case(Duration::from_secs(10), "default 10 second timeout")]
    #[case(Duration::from_secs(30), "30 second timeout")]
    fn batcher_config_network_timeout(#[case] timeout: Duration, #[case] _description: &str) {
        let config = BatcherConfig { network_timeout: timeout, ..Default::default() };
        assert_eq!(config.network_timeout, timeout);
    }

    #[rstest]
    #[case(15, "15 block drift")]
    #[case(30, "default 30 block drift")]
    #[case(60, "60 block drift")]
    fn batcher_config_max_l1_drift(#[case] drift: u64, #[case] _description: &str) {
        let config = BatcherConfig { max_l1_drift: drift, ..Default::default() };
        assert_eq!(config.max_l1_drift, drift);
    }

    #[rstest]
    #[case(1800, "1800 block window")]
    #[case(3600, "default 3600 block window")]
    #[case(7200, "7200 block window")]
    fn batcher_config_sequencing_window(#[case] window: u64, #[case] _description: &str) {
        let config = BatcherConfig { sequencing_window: window, ..Default::default() };
        assert_eq!(config.sequencing_window, window);
    }

    #[rstest]
    #[case(6, "6 confirmations")]
    #[case(12, "default 12 confirmations")]
    #[case(24, "24 confirmations")]
    fn batcher_config_safe_confirmations(#[case] confirmations: u64, #[case] _description: &str) {
        let config = BatcherConfig { safe_confirmations: confirmations, ..Default::default() };
        assert_eq!(config.safe_confirmations, confirmations);
    }

    #[test]
    fn batcher_config_clone() {
        let config = BatcherConfig {
            max_batch_size: 256 * 1024,
            max_blocks_per_batch: 200,
            batch_interval: Duration::from_secs(20),
            min_batch_size: 2048,
            use_blobs: false,
            max_blob_fee_per_gas: 20_000_000_000,
            max_blobs_per_tx: 4,
            num_confirmations: 15,
            resubmission_timeout: Duration::from_secs(60),
            network_timeout: Duration::from_secs(15),
            max_l1_drift: 50,
            sequencing_window: 7200,
            safe_confirmations: 24,
        };
        let cloned = config.clone();
        assert_eq!(cloned.max_batch_size, config.max_batch_size);
        assert_eq!(cloned.max_blocks_per_batch, config.max_blocks_per_batch);
        assert_eq!(cloned.batch_interval, config.batch_interval);
        assert_eq!(cloned.min_batch_size, config.min_batch_size);
        assert_eq!(cloned.use_blobs, config.use_blobs);
        assert_eq!(cloned.max_blob_fee_per_gas, config.max_blob_fee_per_gas);
        assert_eq!(cloned.max_blobs_per_tx, config.max_blobs_per_tx);
        assert_eq!(cloned.num_confirmations, config.num_confirmations);
        assert_eq!(cloned.resubmission_timeout, config.resubmission_timeout);
        assert_eq!(cloned.network_timeout, config.network_timeout);
        assert_eq!(cloned.max_l1_drift, config.max_l1_drift);
        assert_eq!(cloned.sequencing_window, config.sequencing_window);
        assert_eq!(cloned.safe_confirmations, config.safe_confirmations);
    }

    #[test]
    fn batcher_config_debug() {
        let config = BatcherConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("BatcherConfig"));
        assert!(debug_str.contains("max_batch_size"));
        assert!(debug_str.contains("max_blocks_per_batch"));
        assert!(debug_str.contains("batch_interval"));
        assert!(debug_str.contains("min_batch_size"));
        assert!(debug_str.contains("use_blobs"));
        assert!(debug_str.contains("max_blob_fee_per_gas"));
        assert!(debug_str.contains("max_blobs_per_tx"));
        assert!(debug_str.contains("num_confirmations"));
        assert!(debug_str.contains("resubmission_timeout"));
        assert!(debug_str.contains("network_timeout"));
        assert!(debug_str.contains("max_l1_drift"));
        assert!(debug_str.contains("sequencing_window"));
        assert!(debug_str.contains("safe_confirmations"));
    }

    // Builder tests

    #[test]
    fn builder_default() {
        let config = BatcherConfig::builder().build();
        let default_config = BatcherConfig::default();
        assert_eq!(config.max_batch_size, default_config.max_batch_size);
        assert_eq!(config.max_blocks_per_batch, default_config.max_blocks_per_batch);
        assert_eq!(config.batch_interval, default_config.batch_interval);
        assert_eq!(config.min_batch_size, default_config.min_batch_size);
        assert_eq!(config.use_blobs, default_config.use_blobs);
        assert_eq!(config.max_blob_fee_per_gas, default_config.max_blob_fee_per_gas);
        assert_eq!(config.max_blobs_per_tx, default_config.max_blobs_per_tx);
        assert_eq!(config.num_confirmations, default_config.num_confirmations);
        assert_eq!(config.resubmission_timeout, default_config.resubmission_timeout);
        assert_eq!(config.network_timeout, default_config.network_timeout);
        assert_eq!(config.max_l1_drift, default_config.max_l1_drift);
        assert_eq!(config.sequencing_window, default_config.sequencing_window);
        assert_eq!(config.safe_confirmations, default_config.safe_confirmations);
    }

    #[test]
    fn builder_max_batch_size() {
        let config = BatcherConfig::builder().max_batch_size(256 * 1024).build();
        assert_eq!(config.max_batch_size, 256 * 1024);
    }

    #[test]
    fn builder_max_blocks_per_batch() {
        let config = BatcherConfig::builder().max_blocks_per_batch(200).build();
        assert_eq!(config.max_blocks_per_batch, 200);
    }

    #[test]
    fn builder_batch_interval() {
        let config = BatcherConfig::builder().batch_interval(Duration::from_secs(20)).build();
        assert_eq!(config.batch_interval, Duration::from_secs(20));
    }

    #[test]
    fn builder_min_batch_size() {
        let config = BatcherConfig::builder().min_batch_size(2048).build();
        assert_eq!(config.min_batch_size, 2048);
    }

    #[test]
    fn builder_use_blobs() {
        let config = BatcherConfig::builder().use_blobs(false).build();
        assert_eq!(config.use_blobs, false);
    }

    #[test]
    fn builder_max_blob_fee_per_gas() {
        let config = BatcherConfig::builder().max_blob_fee_per_gas(20_000_000_000).build();
        assert_eq!(config.max_blob_fee_per_gas, 20_000_000_000);
    }

    #[test]
    fn builder_max_blobs_per_tx() {
        let config = BatcherConfig::builder().max_blobs_per_tx(4).build();
        assert_eq!(config.max_blobs_per_tx, 4);
    }

    #[test]
    fn builder_num_confirmations() {
        let config = BatcherConfig::builder().num_confirmations(15).build();
        assert_eq!(config.num_confirmations, 15);
    }

    #[test]
    fn builder_resubmission_timeout() {
        let config = BatcherConfig::builder().resubmission_timeout(Duration::from_secs(60)).build();
        assert_eq!(config.resubmission_timeout, Duration::from_secs(60));
    }

    #[test]
    fn builder_network_timeout() {
        let config = BatcherConfig::builder().network_timeout(Duration::from_secs(15)).build();
        assert_eq!(config.network_timeout, Duration::from_secs(15));
    }

    #[test]
    fn builder_max_l1_drift() {
        let config = BatcherConfig::builder().max_l1_drift(50).build();
        assert_eq!(config.max_l1_drift, 50);
    }

    #[test]
    fn builder_sequencing_window() {
        let config = BatcherConfig::builder().sequencing_window(7200).build();
        assert_eq!(config.sequencing_window, 7200);
    }

    #[test]
    fn builder_safe_confirmations() {
        let config = BatcherConfig::builder().safe_confirmations(24).build();
        assert_eq!(config.safe_confirmations, 24);
    }

    #[test]
    fn builder_chaining() {
        let config = BatcherConfig::builder()
            .max_batch_size(256 * 1024)
            .max_blocks_per_batch(200)
            .batch_interval(Duration::from_secs(20))
            .min_batch_size(2048)
            .use_blobs(false)
            .max_blob_fee_per_gas(20_000_000_000)
            .max_blobs_per_tx(4)
            .num_confirmations(15)
            .resubmission_timeout(Duration::from_secs(60))
            .network_timeout(Duration::from_secs(15))
            .max_l1_drift(50)
            .sequencing_window(7200)
            .safe_confirmations(24)
            .build();

        assert_eq!(config.max_batch_size, 256 * 1024);
        assert_eq!(config.max_blocks_per_batch, 200);
        assert_eq!(config.batch_interval, Duration::from_secs(20));
        assert_eq!(config.min_batch_size, 2048);
        assert_eq!(config.use_blobs, false);
        assert_eq!(config.max_blob_fee_per_gas, 20_000_000_000);
        assert_eq!(config.max_blobs_per_tx, 4);
        assert_eq!(config.num_confirmations, 15);
        assert_eq!(config.resubmission_timeout, Duration::from_secs(60));
        assert_eq!(config.network_timeout, Duration::from_secs(15));
        assert_eq!(config.max_l1_drift, 50);
        assert_eq!(config.sequencing_window, 7200);
        assert_eq!(config.safe_confirmations, 24);
    }

    #[rstest]
    #[case(100, 64 * 1024, Duration::from_secs(6), "conservative batching")]
    #[case(100, 128 * 1024, Duration::from_secs(12), "default batching")]
    #[case(200, 256 * 1024, Duration::from_secs(20), "aggressive batching")]
    fn builder_multiple_configurations(
        #[case] blocks: u16,
        #[case] batch_size: usize,
        #[case] interval: Duration,
        #[case] _description: &str,
    ) {
        let config = BatcherConfig::builder()
            .max_blocks_per_batch(blocks)
            .max_batch_size(batch_size)
            .batch_interval(interval)
            .build();

        assert_eq!(config.max_blocks_per_batch, blocks);
        assert_eq!(config.max_batch_size, batch_size);
        assert_eq!(config.batch_interval, interval);
    }

    #[test]
    fn builder_clone() {
        let builder = BatcherConfig::builder().max_batch_size(256 * 1024).max_blocks_per_batch(200);
        let cloned = builder.clone();
        let config1 = builder.build();
        let config2 = cloned.build();
        assert_eq!(config1.max_batch_size, config2.max_batch_size);
        assert_eq!(config1.max_blocks_per_batch, config2.max_blocks_per_batch);
    }

    #[test]
    fn builder_debug() {
        let builder = BatcherConfig::builder();
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("BatcherConfigBuilder"));
    }

    #[test]
    fn builder_partial_configuration() {
        let config = BatcherConfig::builder()
            .max_batch_size(256 * 1024)
            .max_blob_fee_per_gas(15_000_000_000)
            .build();

        assert_eq!(config.max_batch_size, 256 * 1024);
        assert_eq!(config.max_blob_fee_per_gas, 15_000_000_000);
        // Other fields should have default values
        assert_eq!(config.max_blocks_per_batch, 100);
        assert_eq!(config.batch_interval, Duration::from_secs(12));
    }

    #[rstest]
    #[case(false, "no blobs")]
    #[case(true, "with blobs")]
    fn builder_blob_variants(#[case] use_blobs: bool, #[case] _description: &str) {
        let config = BatcherConfig::builder().use_blobs(use_blobs).build();
        assert_eq!(config.use_blobs, use_blobs);
    }

    #[test]
    fn builder_immutability() {
        let builder = BatcherConfig::builder();
        let builder2 = builder.clone().max_batch_size(256 * 1024);
        let config1 = builder.build();
        let config2 = builder2.build();

        assert_eq!(config1.max_batch_size, 128 * 1024); // Default
        assert_eq!(config2.max_batch_size, 256 * 1024); // Modified
    }
}
