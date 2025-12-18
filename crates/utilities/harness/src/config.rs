//! Configuration for the test harness.

/// Configuration for the test harness.
///
/// Controls anvil block time, transaction generation rate, and initial
/// block backlog for sync testing.
#[derive(Debug, Clone)]
pub struct HarnessConfig {
    /// Anvil block time in milliseconds.
    ///
    /// Lower values create faster block production.
    /// Default: 50ms
    pub block_time_ms: u64,

    /// Number of transactions to generate per block.
    ///
    /// Higher values create blocks with more data, improving batch compression ratios.
    /// Default: 1000
    pub tx_per_block: u64,

    /// Number of blocks to generate before returning from spawn.
    ///
    /// This creates a "historical" backlog that Montana must sync through,
    /// enabling testing of the sync stage.
    /// Default: 10 blocks
    pub initial_delay_blocks: u64,

    /// Number of test accounts to use for transaction generation.
    ///
    /// Anvil provides 10 pre-funded accounts by default.
    /// Default: 10
    pub accounts: u8,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self { block_time_ms: 50, tx_per_block: 1000, initial_delay_blocks: 10, accounts: 10 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default() {
        let config = HarnessConfig::default();
        assert_eq!(config.block_time_ms, 50);
        assert_eq!(config.tx_per_block, 1000);
        assert_eq!(config.initial_delay_blocks, 10);
        assert_eq!(config.accounts, 10);
    }

    #[test]
    fn config_custom() {
        let config = HarnessConfig {
            block_time_ms: 500,
            tx_per_block: 150,
            initial_delay_blocks: 100,
            accounts: 5,
        };
        assert_eq!(config.block_time_ms, 500);
        assert_eq!(config.tx_per_block, 150);
        assert_eq!(config.initial_delay_blocks, 100);
        assert_eq!(config.accounts, 5);
    }
}
