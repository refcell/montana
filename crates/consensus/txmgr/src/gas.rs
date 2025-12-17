//! Gas price oracle and fee estimation.

use std::sync::Arc;

use alloy::{primitives::U256, providers::Provider, rpc::types::BlockNumberOrTag};

use crate::{config::TxManagerConfig, error::TxError};

/// Gas price caps for transaction submission.
#[derive(Clone, Debug, Default)]
pub struct GasCaps {
    /// Maximum priority fee per gas (tip).
    pub gas_tip_cap: U256,
    /// Maximum fee per gas.
    pub gas_fee_cap: U256,
    /// Maximum fee per blob gas (optional for blob transactions).
    pub blob_fee_cap: Option<U256>,
}

impl GasCaps {
    /// Creates new gas caps from u128 values.
    ///
    /// # Arguments
    ///
    /// * `max_fee_per_gas` - Maximum fee per gas in wei
    /// * `max_priority_fee_per_gas` - Maximum priority fee per gas in wei
    /// * `blob_fee_cap` - Optional maximum fee per blob gas in wei
    #[must_use]
    pub const fn new(
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
        blob_fee_cap: Option<u128>,
    ) -> Self {
        Self {
            gas_tip_cap: U256::from_limbs([max_priority_fee_per_gas as u64, 0, 0, 0]),
            gas_fee_cap: U256::from_limbs([max_fee_per_gas as u64, 0, 0, 0]),
            blob_fee_cap: match blob_fee_cap {
                Some(fee) => Some(U256::from_limbs([fee as u64, 0, 0, 0])),
                None => None,
            },
        }
    }

    /// Returns max_fee_per_gas as u128.
    #[must_use]
    pub fn max_fee_per_gas(&self) -> u128 {
        self.gas_fee_cap.to::<u128>()
    }

    /// Returns max_priority_fee_per_gas as u128.
    #[must_use]
    pub fn max_priority_fee_per_gas(&self) -> u128 {
        self.gas_tip_cap.to::<u128>()
    }
}

/// Oracle for gas price estimation.
///
/// Fetches current gas prices from the chain and provides methods to bump fees
/// according to configured percentages.
pub struct GasOracle<P>
where
    P: Provider + Clone + Send + Sync,
{
    provider: Arc<P>,
    config: TxManagerConfig,
}

impl<P> std::fmt::Debug for GasOracle<P>
where
    P: Provider + Clone + Send + Sync,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GasOracle").field("config", &self.config).finish_non_exhaustive()
    }
}

impl<P> GasOracle<P>
where
    P: Provider + Clone + Send + Sync,
{
    /// Creates a new gas oracle with the given provider and configuration.
    ///
    /// # Arguments
    ///
    /// * `provider` - The RPC provider to fetch gas prices from
    /// * `config` - Transaction manager configuration containing fee limits and bump percentages
    pub const fn new(provider: Arc<P>, config: TxManagerConfig) -> Self {
        Self { provider, config }
    }

    /// Suggests gas price caps based on current chain conditions.
    ///
    /// Fetches the latest block to get current base fee and calculates appropriate
    /// gas tip cap and gas fee cap. For blob transactions, also calculates blob fee cap.
    ///
    /// # Returns
    ///
    /// Returns `GasCaps` with suggested fee values, or `TxError` if fetching fails.
    ///
    /// # Errors
    ///
    /// Returns `TxError::GasEstimation` if:
    /// - Unable to fetch the latest block
    /// - Block header is missing required fee information
    pub async fn suggest_gas_caps(&self) -> Result<GasCaps, TxError> {
        // Fetch the latest block to get current gas prices
        let block = self
            .provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .await
            .map_err(|e| TxError::GasEstimation(format!("Failed to fetch latest block: {}", e)))?
            .ok_or_else(|| TxError::GasEstimation("Latest block not found".to_string()))?;

        let header = block.header;

        // Get base fee from the block header
        let base_fee = header
            .base_fee_per_gas
            .ok_or_else(|| TxError::GasEstimation("Base fee not available".to_string()))?;

        // Calculate minimum tip cap (convert gwei to wei)
        let min_tip_cap_wei = U256::from((self.config.min_tip_cap_gwei * 1e9) as u128);
        let gas_tip_cap = min_tip_cap_wei;

        // Gas fee cap = base fee + tip cap
        let gas_fee_cap = U256::from(base_fee) + gas_tip_cap;

        // Calculate blob fee cap if excess blob gas is present
        let blob_fee_cap = header.excess_blob_gas.map(|excess_blob_gas| {
            let blob_fee = self.calc_blob_fee(excess_blob_gas);
            U256::from(blob_fee)
        });

        Ok(GasCaps { gas_tip_cap, gas_fee_cap, blob_fee_cap })
    }

    /// Bumps gas price caps by the configured percentage.
    ///
    /// Increases the provided gas caps according to the bump percentages configured
    /// in `TxManagerConfig`. Uses different bump rates for regular transactions vs
    /// blob transactions.
    ///
    /// # Arguments
    ///
    /// * `caps` - The current gas price caps to bump
    /// * `is_blob` - Whether this is a blob transaction (uses higher bump percentage)
    ///
    /// # Returns
    ///
    /// New `GasCaps` with bumped values
    pub fn bump_gas_caps(&self, caps: &GasCaps, is_blob: bool) -> GasCaps {
        let bump_percent = if is_blob {
            self.config.blob_price_bump_percent
        } else {
            self.config.price_bump_percent
        };

        // Calculate bumped values: value * (100 + percent) / 100
        let multiplier = U256::from(100 + bump_percent);
        let divisor = U256::from(100);

        let gas_tip_cap = (caps.gas_tip_cap * multiplier) / divisor;
        let gas_fee_cap = (caps.gas_fee_cap * multiplier) / divisor;
        let blob_fee_cap = caps.blob_fee_cap.map(|fee| (fee * multiplier) / divisor);

        GasCaps { gas_tip_cap, gas_fee_cap, blob_fee_cap }
    }

    /// Calculates blob fee using EIP-4844 formula.
    ///
    /// Implements the exponential pricing function:
    /// `fee = MIN_BLOB_GASPRICE * e^(excess_blob_gas / BLOB_GASPRICE_UPDATE_FRACTION)`
    ///
    /// Where:
    /// - `MIN_BLOB_GASPRICE = 1 wei`
    /// - `BLOB_GASPRICE_UPDATE_FRACTION = 3338477`
    ///
    /// # Arguments
    ///
    /// * `excess_blob_gas` - The excess blob gas from the block header
    ///
    /// # Returns
    ///
    /// The calculated blob fee in wei
    const fn calc_blob_fee(&self, excess_blob_gas: u64) -> u128 {
        const MIN_BLOB_GASPRICE: u128 = 1;
        const BLOB_GASPRICE_UPDATE_FRACTION: u64 = 3338477;

        // Use fake exponential approximation: fee ≈ MIN * (1 + excess/fraction)
        // This is a simplified calculation that avoids floating point math.
        // For a proper implementation, we'd use the exp approximation from geth:
        // https://github.com/ethereum/go-ethereum/blob/master/consensus/misc/eip4844.go
        fake_exponential(MIN_BLOB_GASPRICE, excess_blob_gas, BLOB_GASPRICE_UPDATE_FRACTION)
    }
}

/// Calculates a fake exponential for EIP-4844 blob pricing.
///
/// This implements the approximation used in Ethereum clients for calculating
/// blob gas prices. The formula approximates `factor * e^(numerator / denominator)`
/// using Taylor series expansion.
///
/// # Arguments
///
/// * `factor` - The base multiplier (MIN_BLOB_GASPRICE)
/// * `numerator` - The numerator of the exponent (excess_blob_gas)
/// * `denominator` - The denominator of the exponent (BLOB_GASPRICE_UPDATE_FRACTION)
///
/// # Returns
///
/// The calculated value approximating the exponential function
const fn fake_exponential(factor: u128, numerator: u64, denominator: u64) -> u128 {
    let mut output = 0u128;
    let mut numerator_accum = factor.saturating_mul(denominator as u128);
    let mut i = 1u128;

    // Taylor series: e^x ≈ 1 + x + x²/2! + x³/3! + ...
    // We compute this iteratively to avoid overflow
    loop {
        output = output.saturating_add(numerator_accum);

        // Calculate next term: multiply by numerator, divide by (denominator * i)
        // Use saturating operations to avoid overflow
        let next_accum =
            numerator_accum.saturating_mul(numerator as u128).checked_div(denominator as u128 * i);

        match next_accum {
            Some(val) if val > 0 => numerator_accum = val,
            _ => break,
        }

        i += 1;

        // Safety check to prevent infinite loops
        if i > 100 {
            break;
        }
    }

    output / denominator as u128
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    // Mock provider for testing pure functions
    #[derive(Clone)]
    struct MockProvider;

    impl alloy::providers::Provider for MockProvider {
        fn root(&self) -> &alloy::providers::RootProvider {
            unimplemented!("MockProvider::root not implemented for testing")
        }
    }

    #[test]
    fn gas_caps_default() {
        let caps = GasCaps::default();
        assert_eq!(caps.gas_tip_cap, U256::ZERO);
        assert_eq!(caps.gas_fee_cap, U256::ZERO);
        assert_eq!(caps.blob_fee_cap, None);
    }

    #[test]
    fn gas_caps_clone() {
        let caps = GasCaps {
            gas_tip_cap: U256::from(1000000000u64),
            gas_fee_cap: U256::from(2000000000u64),
            blob_fee_cap: Some(U256::from(3000000000u64)),
        };
        let cloned = caps.clone();
        assert_eq!(cloned.gas_tip_cap, caps.gas_tip_cap);
        assert_eq!(cloned.gas_fee_cap, caps.gas_fee_cap);
        assert_eq!(cloned.blob_fee_cap, caps.blob_fee_cap);
    }

    #[test]
    fn gas_caps_debug() {
        let caps = GasCaps {
            gas_tip_cap: U256::from(1000000000u64),
            gas_fee_cap: U256::from(2000000000u64),
            blob_fee_cap: Some(U256::from(3000000000u64)),
        };
        let debug_str = format!("{:?}", caps);
        assert!(debug_str.contains("GasCaps"));
        assert!(debug_str.contains("gas_tip_cap"));
        assert!(debug_str.contains("gas_fee_cap"));
        assert!(debug_str.contains("blob_fee_cap"));
    }

    #[rstest]
    #[case(U256::from(1_000_000_000u64), U256::from(10_000_000_000u64), None)]
    #[case(
        U256::from(2_000_000_000u64),
        U256::from(20_000_000_000u64),
        Some(U256::from(5_000_000_000u64))
    )]
    #[case(U256::ZERO, U256::ZERO, None)]
    fn gas_caps_construction(#[case] tip: U256, #[case] fee: U256, #[case] blob: Option<U256>) {
        let caps = GasCaps { gas_tip_cap: tip, gas_fee_cap: fee, blob_fee_cap: blob };
        assert_eq!(caps.gas_tip_cap, tip);
        assert_eq!(caps.gas_fee_cap, fee);
        assert_eq!(caps.blob_fee_cap, blob);
    }

    // Test bump_gas_caps with various scenarios
    #[rstest]
    #[case(10, false, U256::from(100u64), U256::from(110u64))]
    #[case(10, false, U256::from(1000u64), U256::from(1100u64))]
    #[case(50, false, U256::from(100u64), U256::from(150u64))]
    #[case(100, true, U256::from(100u64), U256::from(200u64))]
    #[case(100, true, U256::from(1000u64), U256::from(2000u64))]
    fn bump_gas_caps_regular(
        #[case] bump_percent: u64,
        #[case] is_blob: bool,
        #[case] input: U256,
        #[case] expected: U256,
    ) {
        let config = if is_blob {
            TxManagerConfig { blob_price_bump_percent: bump_percent, ..Default::default() }
        } else {
            TxManagerConfig { price_bump_percent: bump_percent, ..Default::default() }
        };

        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let caps = GasCaps { gas_tip_cap: input, gas_fee_cap: input, blob_fee_cap: None };

        let bumped = oracle.bump_gas_caps(&caps, is_blob);
        assert_eq!(bumped.gas_tip_cap, expected);
        assert_eq!(bumped.gas_fee_cap, expected);
    }

    #[test]
    fn bump_gas_caps_with_blob_fee() {
        let config = TxManagerConfig { price_bump_percent: 10, ..Default::default() };

        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let caps = GasCaps {
            gas_tip_cap: U256::from(1_000_000_000u64),
            gas_fee_cap: U256::from(10_000_000_000u64),
            blob_fee_cap: Some(U256::from(5_000_000_000u64)),
        };

        let bumped = oracle.bump_gas_caps(&caps, false);
        assert_eq!(bumped.gas_tip_cap, U256::from(1_100_000_000u64));
        assert_eq!(bumped.gas_fee_cap, U256::from(11_000_000_000u64));
        assert_eq!(bumped.blob_fee_cap, Some(U256::from(5_500_000_000u64)));
    }

    #[test]
    fn bump_gas_caps_blob_transaction() {
        let config = TxManagerConfig {
            price_bump_percent: 10,
            blob_price_bump_percent: 100,
            ..Default::default()
        };

        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let caps = GasCaps {
            gas_tip_cap: U256::from(1_000_000_000u64),
            gas_fee_cap: U256::from(10_000_000_000u64),
            blob_fee_cap: Some(U256::from(5_000_000_000u64)),
        };

        let bumped = oracle.bump_gas_caps(&caps, true);
        // 100% bump = 2x the original value
        assert_eq!(bumped.gas_tip_cap, U256::from(2_000_000_000u64));
        assert_eq!(bumped.gas_fee_cap, U256::from(20_000_000_000u64));
        assert_eq!(bumped.blob_fee_cap, Some(U256::from(10_000_000_000u64)));
    }

    #[rstest]
    #[case(U256::from(100u64), 10, U256::from(110u64))]
    #[case(U256::from(1000u64), 50, U256::from(1500u64))]
    #[case(U256::from(12345u64), 10, U256::from(13579u64))]
    #[case(U256::ZERO, 10, U256::ZERO)]
    fn bump_gas_caps_various_inputs(
        #[case] input: U256,
        #[case] percent: u64,
        #[case] expected: U256,
    ) {
        let config = TxManagerConfig { price_bump_percent: percent, ..Default::default() };

        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let caps = GasCaps { gas_tip_cap: input, gas_fee_cap: input, blob_fee_cap: None };

        let bumped = oracle.bump_gas_caps(&caps, false);
        assert_eq!(bumped.gas_tip_cap, expected);
    }

    #[test]
    fn bump_gas_caps_preserves_none_blob_fee() {
        let config = TxManagerConfig::default();
        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let caps = GasCaps {
            gas_tip_cap: U256::from(1_000_000_000u64),
            gas_fee_cap: U256::from(10_000_000_000u64),
            blob_fee_cap: None,
        };

        let bumped = oracle.bump_gas_caps(&caps, false);
        assert_eq!(bumped.blob_fee_cap, None);
    }

    // Test calc_blob_fee
    #[rstest]
    #[case(0, 1)]
    #[case(1, 1)]
    #[case(100, 1)]
    fn calc_blob_fee_low_excess(#[case] excess: u64, #[case] _min_expected: u128) {
        let config = TxManagerConfig::default();
        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let fee = oracle.calc_blob_fee(excess);
        // With very low excess, fee should be close to MIN_BLOB_GASPRICE (1)
        assert!(fee >= 1, "Fee should be at least MIN_BLOB_GASPRICE");
        assert!(fee < 1000, "Fee should be small for low excess");
    }

    #[test]
    fn calc_blob_fee_zero_excess() {
        let config = TxManagerConfig::default();
        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let fee = oracle.calc_blob_fee(0);
        assert_eq!(fee, 1, "Fee should be MIN_BLOB_GASPRICE for zero excess");
    }

    #[rstest]
    #[case(3338477, 2)] // One unit should approximately double the price
    #[case(6676954, 7)] // Two units should increase more
    #[case(10015431, 20)] // Three units
    fn calc_blob_fee_increasing_with_excess(#[case] excess: u64, #[case] min_expected: u128) {
        let config = TxManagerConfig::default();
        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let fee = oracle.calc_blob_fee(excess);
        assert!(
            fee >= min_expected,
            "Fee {} should be at least {} for excess {}",
            fee,
            min_expected,
            excess
        );
    }

    #[test]
    fn calc_blob_fee_monotonic() {
        let config = TxManagerConfig::default();
        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        // Use larger values where the exponential growth is visible
        let fee1 = oracle.calc_blob_fee(1_000_000);
        let fee2 = oracle.calc_blob_fee(5_000_000);
        let fee3 = oracle.calc_blob_fee(10_000_000);

        assert!(fee1 < fee2, "Fee should increase with excess: {} >= {}", fee1, fee2);
        assert!(fee2 < fee3, "Fee should increase with excess: {} >= {}", fee2, fee3);
    }

    #[test]
    fn calc_blob_fee_large_excess() {
        let config = TxManagerConfig::default();
        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        // Test with large excess values
        let fee = oracle.calc_blob_fee(100_000_000);
        assert!(fee > 1, "Fee should be greater than MIN_BLOB_GASPRICE");
        // Should not overflow or panic
    }

    // Test fake_exponential directly
    #[test]
    fn fake_exponential_zero_numerator() {
        let result = fake_exponential(1, 0, 3338477);
        assert_eq!(result, 1, "e^0 should equal 1");
    }

    #[test]
    fn fake_exponential_increases() {
        // Use larger numerator values where exponential growth is visible
        let result1 = fake_exponential(1, 1_000_000, 3338477);
        let result2 = fake_exponential(1, 5_000_000, 3338477);
        let result3 = fake_exponential(1, 10_000_000, 3338477);

        assert!(result1 < result2, "Result should increase: {} >= {}", result1, result2);
        assert!(result2 < result3, "Result should increase: {} >= {}", result2, result3);
    }

    #[test]
    fn fake_exponential_with_factor() {
        let result1 = fake_exponential(1, 1000, 3338477);
        let result2 = fake_exponential(2, 1000, 3338477);

        // With double the factor, result should be approximately doubled
        assert!(result2 > result1);
        assert!(result2 >= result1 * 2 - 1); // Allow small rounding difference
    }

    #[rstest]
    #[case(1, 0, 1000, 1)]
    #[case(1, 1000, 1000, 2)] // e^1 ≈ 2.718, but our approximation gives ~2
    #[case(10, 0, 1000, 10)]
    #[case(100, 500, 1000, 164)] // e^0.5 ≈ 1.648
    fn fake_exponential_specific_cases(
        #[case] factor: u128,
        #[case] numerator: u64,
        #[case] denominator: u64,
        #[case] expected_min: u128,
    ) {
        let result = fake_exponential(factor, numerator, denominator);
        assert!(result >= expected_min, "Result {} should be at least {}", result, expected_min);
    }

    #[test]
    fn fake_exponential_no_overflow() {
        // Test with large values that could cause overflow if not handled properly
        let result = fake_exponential(u128::MAX / 1000000, 1000, 3338477);
        // Should not panic or overflow
        assert!(result > 0);
    }

    #[test]
    fn fake_exponential_termination() {
        // Ensure the function terminates even with unusual inputs
        let result = fake_exponential(1000, 100000, 1);
        // Should terminate and return a value
        assert!(result > 0);
    }

    #[test]
    fn gas_oracle_debug() {
        let config = TxManagerConfig::default();
        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let debug_str = format!("{:?}", oracle);
        assert!(debug_str.contains("GasOracle"));
        assert!(debug_str.contains("config"));
    }

    #[test]
    fn gas_oracle_new() {
        let config = TxManagerConfig::default();
        let provider = Arc::new(MockProvider);
        let oracle = GasOracle::new(provider, config);

        // Just verify it constructs without panicking
        let _ = format!("{:?}", oracle);
    }

    // Additional edge case tests
    #[test]
    fn bump_gas_caps_zero_bump_percent() {
        let config = TxManagerConfig { price_bump_percent: 0, ..Default::default() };

        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let caps = GasCaps {
            gas_tip_cap: U256::from(1_000_000_000u64),
            gas_fee_cap: U256::from(10_000_000_000u64),
            blob_fee_cap: Some(U256::from(5_000_000_000u64)),
        };

        let bumped = oracle.bump_gas_caps(&caps, false);
        // 0% bump should return the same values
        assert_eq!(bumped.gas_tip_cap, caps.gas_tip_cap);
        assert_eq!(bumped.gas_fee_cap, caps.gas_fee_cap);
        assert_eq!(bumped.blob_fee_cap, caps.blob_fee_cap);
    }

    #[test]
    fn bump_gas_caps_large_values() {
        let config = TxManagerConfig { price_bump_percent: 10, ..Default::default() };

        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let large_value = U256::from(1_000_000_000_000_000u128);
        let caps = GasCaps {
            gas_tip_cap: large_value,
            gas_fee_cap: large_value,
            blob_fee_cap: Some(large_value),
        };

        let bumped = oracle.bump_gas_caps(&caps, false);
        let expected = (large_value * U256::from(110u64)) / U256::from(100u64);
        assert_eq!(bumped.gas_tip_cap, expected);
        assert_eq!(bumped.gas_fee_cap, expected);
        assert_eq!(bumped.blob_fee_cap, Some(expected));
    }

    #[rstest]
    #[case(10)]
    #[case(25)]
    #[case(50)]
    #[case(100)]
    #[case(200)]
    fn bump_gas_caps_different_percentages(#[case] percent: u64) {
        let config = TxManagerConfig { price_bump_percent: percent, ..Default::default() };

        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        let input = U256::from(1_000_000_000u64);
        let caps = GasCaps { gas_tip_cap: input, gas_fee_cap: input, blob_fee_cap: None };

        let bumped = oracle.bump_gas_caps(&caps, false);
        let expected = (input * U256::from(100 + percent)) / U256::from(100u64);
        assert_eq!(bumped.gas_tip_cap, expected);
    }

    #[test]
    fn calc_blob_fee_consistency() {
        let config = TxManagerConfig::default();
        let oracle = GasOracle { provider: Arc::new(MockProvider), config };

        // Same input should always give same output
        let excess = 12345;
        let fee1 = oracle.calc_blob_fee(excess);
        let fee2 = oracle.calc_blob_fee(excess);
        assert_eq!(fee1, fee2, "calc_blob_fee should be deterministic");
    }
}
