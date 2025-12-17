//! Transaction manager configuration.

use std::time::Duration;

use alloy::primitives::U256;

/// Transaction manager configuration.
#[derive(Clone, Debug)]
pub struct TxManagerConfig {
    // Confirmation
    /// Number of confirmations required (default: 10).
    pub num_confirmations: u64,
    /// Receipt polling interval (default: 12s).
    pub receipt_query_interval: Duration,
    /// Network timeout for RPC calls (default: 10s).
    pub network_timeout: Duration,

    // Resubmission
    /// Interval before bumping gas price (default: 48s).
    pub resubmission_timeout: Duration,
    /// Maximum time to wait for mempool inclusion (default: 2m).
    pub not_in_mempool_timeout: Duration,
    /// "Nonce too low" errors before abort (default: 3).
    pub safe_abort_nonce_too_low_count: u32,

    // Fee limits
    /// Multiplier for fee cap limit (default: 5x).
    pub fee_limit_multiplier: u64,
    /// Minimum base fee in gwei (default: 1.0).
    pub min_base_fee_gwei: f64,
    /// Minimum tip cap in gwei (default: 1.0).
    pub min_tip_cap_gwei: f64,
    /// Maximum blob fee per gas (optional limit).
    pub max_blob_fee_per_gas: Option<U256>,

    // Price bumping
    /// Minimum percentage bump for regular txs (default: 10%).
    pub price_bump_percent: u64,
    /// Minimum percentage bump for blob txs (default: 100%).
    pub blob_price_bump_percent: u64,
}

impl Default for TxManagerConfig {
    fn default() -> Self {
        Self {
            num_confirmations: 10,
            receipt_query_interval: Duration::from_secs(12),
            network_timeout: Duration::from_secs(10),
            resubmission_timeout: Duration::from_secs(48),
            not_in_mempool_timeout: Duration::from_secs(120), // 2 minutes
            safe_abort_nonce_too_low_count: 3,
            fee_limit_multiplier: 5,
            min_base_fee_gwei: 1.0,
            min_tip_cap_gwei: 1.0,
            max_blob_fee_per_gas: None,
            price_bump_percent: 10,
            blob_price_bump_percent: 100,
        }
    }
}

impl TxManagerConfig {
    /// Creates a new builder for configuring a transaction manager.
    pub fn builder() -> TxManagerConfigBuilder {
        TxManagerConfigBuilder::default()
    }
}

/// Builder for [`TxManagerConfig`].
#[derive(Clone, Debug)]
pub struct TxManagerConfigBuilder {
    num_confirmations: u64,
    receipt_query_interval: Duration,
    network_timeout: Duration,
    resubmission_timeout: Duration,
    not_in_mempool_timeout: Duration,
    safe_abort_nonce_too_low_count: u32,
    fee_limit_multiplier: u64,
    min_base_fee_gwei: f64,
    min_tip_cap_gwei: f64,
    max_blob_fee_per_gas: Option<U256>,
    price_bump_percent: u64,
    blob_price_bump_percent: u64,
}

impl Default for TxManagerConfigBuilder {
    fn default() -> Self {
        let defaults = TxManagerConfig::default();
        Self {
            num_confirmations: defaults.num_confirmations,
            receipt_query_interval: defaults.receipt_query_interval,
            network_timeout: defaults.network_timeout,
            resubmission_timeout: defaults.resubmission_timeout,
            not_in_mempool_timeout: defaults.not_in_mempool_timeout,
            safe_abort_nonce_too_low_count: defaults.safe_abort_nonce_too_low_count,
            fee_limit_multiplier: defaults.fee_limit_multiplier,
            min_base_fee_gwei: defaults.min_base_fee_gwei,
            min_tip_cap_gwei: defaults.min_tip_cap_gwei,
            max_blob_fee_per_gas: defaults.max_blob_fee_per_gas,
            price_bump_percent: defaults.price_bump_percent,
            blob_price_bump_percent: defaults.blob_price_bump_percent,
        }
    }
}

impl TxManagerConfigBuilder {
    /// Sets the number of confirmations required.
    pub const fn num_confirmations(mut self, num_confirmations: u64) -> Self {
        self.num_confirmations = num_confirmations;
        self
    }

    /// Sets the receipt polling interval.
    pub const fn receipt_query_interval(mut self, receipt_query_interval: Duration) -> Self {
        self.receipt_query_interval = receipt_query_interval;
        self
    }

    /// Sets the network timeout for RPC calls.
    pub const fn network_timeout(mut self, network_timeout: Duration) -> Self {
        self.network_timeout = network_timeout;
        self
    }

    /// Sets the interval before bumping gas price.
    pub const fn resubmission_timeout(mut self, resubmission_timeout: Duration) -> Self {
        self.resubmission_timeout = resubmission_timeout;
        self
    }

    /// Sets the maximum time to wait for mempool inclusion.
    pub const fn not_in_mempool_timeout(mut self, not_in_mempool_timeout: Duration) -> Self {
        self.not_in_mempool_timeout = not_in_mempool_timeout;
        self
    }

    /// Sets the "nonce too low" error count before abort.
    pub const fn safe_abort_nonce_too_low_count(
        mut self,
        safe_abort_nonce_too_low_count: u32,
    ) -> Self {
        self.safe_abort_nonce_too_low_count = safe_abort_nonce_too_low_count;
        self
    }

    /// Sets the multiplier for fee cap limit.
    pub const fn fee_limit_multiplier(mut self, fee_limit_multiplier: u64) -> Self {
        self.fee_limit_multiplier = fee_limit_multiplier;
        self
    }

    /// Sets the minimum base fee in gwei.
    pub const fn min_base_fee_gwei(mut self, min_base_fee_gwei: f64) -> Self {
        self.min_base_fee_gwei = min_base_fee_gwei;
        self
    }

    /// Sets the minimum tip cap in gwei.
    pub const fn min_tip_cap_gwei(mut self, min_tip_cap_gwei: f64) -> Self {
        self.min_tip_cap_gwei = min_tip_cap_gwei;
        self
    }

    /// Sets the maximum blob fee per gas.
    pub const fn max_blob_fee_per_gas(mut self, max_blob_fee_per_gas: Option<U256>) -> Self {
        self.max_blob_fee_per_gas = max_blob_fee_per_gas;
        self
    }

    /// Sets the minimum percentage bump for regular transactions.
    pub const fn price_bump_percent(mut self, price_bump_percent: u64) -> Self {
        self.price_bump_percent = price_bump_percent;
        self
    }

    /// Sets the minimum percentage bump for blob transactions.
    pub const fn blob_price_bump_percent(mut self, blob_price_bump_percent: u64) -> Self {
        self.blob_price_bump_percent = blob_price_bump_percent;
        self
    }

    /// Builds the [`TxManagerConfig`].
    pub const fn build(self) -> TxManagerConfig {
        TxManagerConfig {
            num_confirmations: self.num_confirmations,
            receipt_query_interval: self.receipt_query_interval,
            network_timeout: self.network_timeout,
            resubmission_timeout: self.resubmission_timeout,
            not_in_mempool_timeout: self.not_in_mempool_timeout,
            safe_abort_nonce_too_low_count: self.safe_abort_nonce_too_low_count,
            fee_limit_multiplier: self.fee_limit_multiplier,
            min_base_fee_gwei: self.min_base_fee_gwei,
            min_tip_cap_gwei: self.min_tip_cap_gwei,
            max_blob_fee_per_gas: self.max_blob_fee_per_gas,
            price_bump_percent: self.price_bump_percent,
            blob_price_bump_percent: self.blob_price_bump_percent,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn tx_manager_config_default() {
        let config = TxManagerConfig::default();
        assert_eq!(config.num_confirmations, 10);
        assert_eq!(config.receipt_query_interval, Duration::from_secs(12));
        assert_eq!(config.network_timeout, Duration::from_secs(10));
        assert_eq!(config.resubmission_timeout, Duration::from_secs(48));
        assert_eq!(config.not_in_mempool_timeout, Duration::from_secs(120));
        assert_eq!(config.safe_abort_nonce_too_low_count, 3);
        assert_eq!(config.fee_limit_multiplier, 5);
        assert_eq!(config.min_base_fee_gwei, 1.0);
        assert_eq!(config.min_tip_cap_gwei, 1.0);
        assert_eq!(config.max_blob_fee_per_gas, None);
        assert_eq!(config.price_bump_percent, 10);
        assert_eq!(config.blob_price_bump_percent, 100);
    }

    #[rstest]
    #[case(5, "fewer confirmations")]
    #[case(10, "default confirmations")]
    #[case(20, "more confirmations")]
    fn tx_manager_config_num_confirmations(#[case] confirmations: u64, #[case] _description: &str) {
        let config = TxManagerConfig { num_confirmations: confirmations, ..Default::default() };
        assert_eq!(config.num_confirmations, confirmations);
    }

    #[rstest]
    #[case(Duration::from_secs(6), "faster polling")]
    #[case(Duration::from_secs(12), "default polling")]
    #[case(Duration::from_secs(30), "slower polling")]
    fn tx_manager_config_receipt_query_interval(
        #[case] interval: Duration,
        #[case] _description: &str,
    ) {
        let config = TxManagerConfig { receipt_query_interval: interval, ..Default::default() };
        assert_eq!(config.receipt_query_interval, interval);
    }

    #[rstest]
    #[case(Duration::from_secs(5), "short timeout")]
    #[case(Duration::from_secs(10), "default timeout")]
    #[case(Duration::from_secs(30), "long timeout")]
    fn tx_manager_config_network_timeout(#[case] timeout: Duration, #[case] _description: &str) {
        let config = TxManagerConfig { network_timeout: timeout, ..Default::default() };
        assert_eq!(config.network_timeout, timeout);
    }

    #[rstest]
    #[case(Duration::from_secs(24), "fast resubmission")]
    #[case(Duration::from_secs(48), "default resubmission")]
    #[case(Duration::from_secs(120), "slow resubmission")]
    fn tx_manager_config_resubmission_timeout(
        #[case] timeout: Duration,
        #[case] _description: &str,
    ) {
        let config = TxManagerConfig { resubmission_timeout: timeout, ..Default::default() };
        assert_eq!(config.resubmission_timeout, timeout);
    }

    #[rstest]
    #[case(Duration::from_secs(60), "1 minute")]
    #[case(Duration::from_secs(120), "2 minutes")]
    #[case(Duration::from_secs(300), "5 minutes")]
    fn tx_manager_config_not_in_mempool_timeout(
        #[case] timeout: Duration,
        #[case] _description: &str,
    ) {
        let config = TxManagerConfig { not_in_mempool_timeout: timeout, ..Default::default() };
        assert_eq!(config.not_in_mempool_timeout, timeout);
    }

    #[rstest]
    #[case(1, "aggressive abort")]
    #[case(3, "default abort")]
    #[case(10, "lenient abort")]
    fn tx_manager_config_safe_abort_nonce_too_low_count(
        #[case] count: u32,
        #[case] _description: &str,
    ) {
        let config =
            TxManagerConfig { safe_abort_nonce_too_low_count: count, ..Default::default() };
        assert_eq!(config.safe_abort_nonce_too_low_count, count);
    }

    #[rstest]
    #[case(3, "conservative limit")]
    #[case(5, "default limit")]
    #[case(10, "generous limit")]
    fn tx_manager_config_fee_limit_multiplier(#[case] multiplier: u64, #[case] _description: &str) {
        let config = TxManagerConfig { fee_limit_multiplier: multiplier, ..Default::default() };
        assert_eq!(config.fee_limit_multiplier, multiplier);
    }

    #[rstest]
    #[case(0.1, "very low base fee")]
    #[case(1.0, "default base fee")]
    #[case(10.0, "high base fee")]
    fn tx_manager_config_min_base_fee_gwei(#[case] fee: f64, #[case] _description: &str) {
        let config = TxManagerConfig { min_base_fee_gwei: fee, ..Default::default() };
        assert_eq!(config.min_base_fee_gwei, fee);
    }

    #[rstest]
    #[case(0.1, "very low tip")]
    #[case(1.0, "default tip")]
    #[case(5.0, "high tip")]
    fn tx_manager_config_min_tip_cap_gwei(#[case] tip: f64, #[case] _description: &str) {
        let config = TxManagerConfig { min_tip_cap_gwei: tip, ..Default::default() };
        assert_eq!(config.min_tip_cap_gwei, tip);
    }

    #[rstest]
    #[case(None, "no limit")]
    #[case(Some(U256::from(1000000)), "low limit")]
    #[case(Some(U256::from(10000000000u64)), "high limit")]
    fn tx_manager_config_max_blob_fee_per_gas(
        #[case] max_fee: Option<U256>,
        #[case] _description: &str,
    ) {
        let config = TxManagerConfig { max_blob_fee_per_gas: max_fee, ..Default::default() };
        assert_eq!(config.max_blob_fee_per_gas, max_fee);
    }

    #[rstest]
    #[case(5, "conservative bump")]
    #[case(10, "default bump")]
    #[case(50, "aggressive bump")]
    fn tx_manager_config_price_bump_percent(#[case] percent: u64, #[case] _description: &str) {
        let config = TxManagerConfig { price_bump_percent: percent, ..Default::default() };
        assert_eq!(config.price_bump_percent, percent);
    }

    #[rstest]
    #[case(50, "conservative blob bump")]
    #[case(100, "default blob bump")]
    #[case(200, "aggressive blob bump")]
    fn tx_manager_config_blob_price_bump_percent(#[case] percent: u64, #[case] _description: &str) {
        let config = TxManagerConfig { blob_price_bump_percent: percent, ..Default::default() };
        assert_eq!(config.blob_price_bump_percent, percent);
    }

    #[test]
    fn tx_manager_config_clone() {
        let config = TxManagerConfig {
            num_confirmations: 15,
            receipt_query_interval: Duration::from_secs(20),
            network_timeout: Duration::from_secs(15),
            resubmission_timeout: Duration::from_secs(60),
            not_in_mempool_timeout: Duration::from_secs(180),
            safe_abort_nonce_too_low_count: 5,
            fee_limit_multiplier: 10,
            min_base_fee_gwei: 2.0,
            min_tip_cap_gwei: 1.5,
            max_blob_fee_per_gas: Some(U256::from(5000000)),
            price_bump_percent: 15,
            blob_price_bump_percent: 150,
        };
        let cloned = config.clone();
        assert_eq!(cloned.num_confirmations, config.num_confirmations);
        assert_eq!(cloned.receipt_query_interval, config.receipt_query_interval);
        assert_eq!(cloned.network_timeout, config.network_timeout);
        assert_eq!(cloned.resubmission_timeout, config.resubmission_timeout);
        assert_eq!(cloned.not_in_mempool_timeout, config.not_in_mempool_timeout);
        assert_eq!(cloned.safe_abort_nonce_too_low_count, config.safe_abort_nonce_too_low_count);
        assert_eq!(cloned.fee_limit_multiplier, config.fee_limit_multiplier);
        assert_eq!(cloned.min_base_fee_gwei, config.min_base_fee_gwei);
        assert_eq!(cloned.min_tip_cap_gwei, config.min_tip_cap_gwei);
        assert_eq!(cloned.max_blob_fee_per_gas, config.max_blob_fee_per_gas);
        assert_eq!(cloned.price_bump_percent, config.price_bump_percent);
        assert_eq!(cloned.blob_price_bump_percent, config.blob_price_bump_percent);
    }

    #[test]
    fn tx_manager_config_debug() {
        let config = TxManagerConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("TxManagerConfig"));
        assert!(debug_str.contains("num_confirmations"));
        assert!(debug_str.contains("receipt_query_interval"));
        assert!(debug_str.contains("network_timeout"));
        assert!(debug_str.contains("resubmission_timeout"));
        assert!(debug_str.contains("not_in_mempool_timeout"));
        assert!(debug_str.contains("safe_abort_nonce_too_low_count"));
        assert!(debug_str.contains("fee_limit_multiplier"));
        assert!(debug_str.contains("min_base_fee_gwei"));
        assert!(debug_str.contains("min_tip_cap_gwei"));
        assert!(debug_str.contains("max_blob_fee_per_gas"));
        assert!(debug_str.contains("price_bump_percent"));
        assert!(debug_str.contains("blob_price_bump_percent"));
    }

    // Builder tests

    #[test]
    fn builder_default() {
        let config = TxManagerConfig::builder().build();
        let default_config = TxManagerConfig::default();
        assert_eq!(config.num_confirmations, default_config.num_confirmations);
        assert_eq!(config.receipt_query_interval, default_config.receipt_query_interval);
        assert_eq!(config.network_timeout, default_config.network_timeout);
        assert_eq!(config.resubmission_timeout, default_config.resubmission_timeout);
        assert_eq!(config.not_in_mempool_timeout, default_config.not_in_mempool_timeout);
        assert_eq!(
            config.safe_abort_nonce_too_low_count,
            default_config.safe_abort_nonce_too_low_count
        );
        assert_eq!(config.fee_limit_multiplier, default_config.fee_limit_multiplier);
        assert_eq!(config.min_base_fee_gwei, default_config.min_base_fee_gwei);
        assert_eq!(config.min_tip_cap_gwei, default_config.min_tip_cap_gwei);
        assert_eq!(config.max_blob_fee_per_gas, default_config.max_blob_fee_per_gas);
        assert_eq!(config.price_bump_percent, default_config.price_bump_percent);
        assert_eq!(config.blob_price_bump_percent, default_config.blob_price_bump_percent);
    }

    #[test]
    fn builder_num_confirmations() {
        let config = TxManagerConfig::builder().num_confirmations(20).build();
        assert_eq!(config.num_confirmations, 20);
    }

    #[test]
    fn builder_receipt_query_interval() {
        let config =
            TxManagerConfig::builder().receipt_query_interval(Duration::from_secs(15)).build();
        assert_eq!(config.receipt_query_interval, Duration::from_secs(15));
    }

    #[test]
    fn builder_network_timeout() {
        let config = TxManagerConfig::builder().network_timeout(Duration::from_secs(20)).build();
        assert_eq!(config.network_timeout, Duration::from_secs(20));
    }

    #[test]
    fn builder_resubmission_timeout() {
        let config =
            TxManagerConfig::builder().resubmission_timeout(Duration::from_secs(60)).build();
        assert_eq!(config.resubmission_timeout, Duration::from_secs(60));
    }

    #[test]
    fn builder_not_in_mempool_timeout() {
        let config =
            TxManagerConfig::builder().not_in_mempool_timeout(Duration::from_secs(180)).build();
        assert_eq!(config.not_in_mempool_timeout, Duration::from_secs(180));
    }

    #[test]
    fn builder_safe_abort_nonce_too_low_count() {
        let config = TxManagerConfig::builder().safe_abort_nonce_too_low_count(5).build();
        assert_eq!(config.safe_abort_nonce_too_low_count, 5);
    }

    #[test]
    fn builder_fee_limit_multiplier() {
        let config = TxManagerConfig::builder().fee_limit_multiplier(10).build();
        assert_eq!(config.fee_limit_multiplier, 10);
    }

    #[test]
    fn builder_min_base_fee_gwei() {
        let config = TxManagerConfig::builder().min_base_fee_gwei(2.5).build();
        assert_eq!(config.min_base_fee_gwei, 2.5);
    }

    #[test]
    fn builder_min_tip_cap_gwei() {
        let config = TxManagerConfig::builder().min_tip_cap_gwei(1.5).build();
        assert_eq!(config.min_tip_cap_gwei, 1.5);
    }

    #[test]
    fn builder_max_blob_fee_per_gas() {
        let max_fee = Some(U256::from(10000000));
        let config = TxManagerConfig::builder().max_blob_fee_per_gas(max_fee).build();
        assert_eq!(config.max_blob_fee_per_gas, max_fee);
    }

    #[test]
    fn builder_price_bump_percent() {
        let config = TxManagerConfig::builder().price_bump_percent(15).build();
        assert_eq!(config.price_bump_percent, 15);
    }

    #[test]
    fn builder_blob_price_bump_percent() {
        let config = TxManagerConfig::builder().blob_price_bump_percent(150).build();
        assert_eq!(config.blob_price_bump_percent, 150);
    }

    #[test]
    fn builder_chaining() {
        let config = TxManagerConfig::builder()
            .num_confirmations(15)
            .receipt_query_interval(Duration::from_secs(20))
            .network_timeout(Duration::from_secs(15))
            .resubmission_timeout(Duration::from_secs(60))
            .not_in_mempool_timeout(Duration::from_secs(180))
            .safe_abort_nonce_too_low_count(5)
            .fee_limit_multiplier(10)
            .min_base_fee_gwei(2.0)
            .min_tip_cap_gwei(1.5)
            .max_blob_fee_per_gas(Some(U256::from(5000000)))
            .price_bump_percent(15)
            .blob_price_bump_percent(150)
            .build();

        assert_eq!(config.num_confirmations, 15);
        assert_eq!(config.receipt_query_interval, Duration::from_secs(20));
        assert_eq!(config.network_timeout, Duration::from_secs(15));
        assert_eq!(config.resubmission_timeout, Duration::from_secs(60));
        assert_eq!(config.not_in_mempool_timeout, Duration::from_secs(180));
        assert_eq!(config.safe_abort_nonce_too_low_count, 5);
        assert_eq!(config.fee_limit_multiplier, 10);
        assert_eq!(config.min_base_fee_gwei, 2.0);
        assert_eq!(config.min_tip_cap_gwei, 1.5);
        assert_eq!(config.max_blob_fee_per_gas, Some(U256::from(5000000)));
        assert_eq!(config.price_bump_percent, 15);
        assert_eq!(config.blob_price_bump_percent, 150);
    }

    #[rstest]
    #[case(5, Duration::from_secs(6), Duration::from_secs(5), "fast settings")]
    #[case(10, Duration::from_secs(12), Duration::from_secs(10), "default settings")]
    #[case(20, Duration::from_secs(30), Duration::from_secs(30), "slow settings")]
    fn builder_multiple_configurations(
        #[case] confirmations: u64,
        #[case] query_interval: Duration,
        #[case] network_timeout: Duration,
        #[case] _description: &str,
    ) {
        let config = TxManagerConfig::builder()
            .num_confirmations(confirmations)
            .receipt_query_interval(query_interval)
            .network_timeout(network_timeout)
            .build();

        assert_eq!(config.num_confirmations, confirmations);
        assert_eq!(config.receipt_query_interval, query_interval);
        assert_eq!(config.network_timeout, network_timeout);
    }

    #[test]
    fn builder_clone() {
        let builder = TxManagerConfig::builder().num_confirmations(15).fee_limit_multiplier(10);
        let cloned = builder.clone();
        let config1 = builder.build();
        let config2 = cloned.build();
        assert_eq!(config1.num_confirmations, config2.num_confirmations);
        assert_eq!(config1.fee_limit_multiplier, config2.fee_limit_multiplier);
    }

    #[test]
    fn builder_debug() {
        let builder = TxManagerConfig::builder();
        let debug_str = format!("{:?}", builder);
        assert!(debug_str.contains("TxManagerConfigBuilder"));
    }

    #[test]
    fn builder_partial_configuration() {
        let config =
            TxManagerConfig::builder().num_confirmations(25).min_base_fee_gwei(3.0).build();

        assert_eq!(config.num_confirmations, 25);
        assert_eq!(config.min_base_fee_gwei, 3.0);
        // Other fields should have default values
        assert_eq!(config.receipt_query_interval, Duration::from_secs(12));
        assert_eq!(config.price_bump_percent, 10);
    }

    #[rstest]
    #[case(None, "no blob fee limit")]
    #[case(Some(U256::from(1000)), "low blob fee limit")]
    #[case(Some(U256::from(u64::MAX)), "max blob fee limit")]
    fn builder_blob_fee_variants(#[case] max_fee: Option<U256>, #[case] _description: &str) {
        let config = TxManagerConfig::builder().max_blob_fee_per_gas(max_fee).build();
        assert_eq!(config.max_blob_fee_per_gas, max_fee);
    }

    #[test]
    fn builder_immutability() {
        let builder = TxManagerConfig::builder();
        let builder2 = builder.clone().num_confirmations(20);
        let config1 = builder.build();
        let config2 = builder2.build();

        assert_eq!(config1.num_confirmations, 10); // Default
        assert_eq!(config2.num_confirmations, 20); // Modified
    }
}
