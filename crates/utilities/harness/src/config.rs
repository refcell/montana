//! Configuration for the test harness.

use std::path::PathBuf;

/// Default genesis state for Anvil, embedded at compile time.
///
/// This contains the initial state with 10 pre-funded test accounts,
/// each with 10,000 ETH. Using this ensures consistent genesis state
/// across all harness runs.
pub const DEFAULT_GENESIS_STATE: &str = include_str!("../genesis/anvil_state.json");

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

    /// Whether to use the default embedded genesis state.
    ///
    /// When true (default), the harness will load Anvil with a consistent
    /// genesis state containing 10 pre-funded test accounts with 10,000 ETH each.
    /// This ensures deterministic behavior across runs.
    ///
    /// Set to false to start Anvil fresh without loading any state.
    ///
    /// Default: true
    pub use_default_genesis: bool,

    /// Optional path to a custom state file for Anvil persistence.
    ///
    /// If set, overrides `use_default_genesis` and Anvil will:
    /// - Load state from this file on startup (if it exists)
    /// - Dump state to this file on exit (if `dump_initial_state` is true)
    ///
    /// This is useful for maintaining consistent genesis state across runs,
    /// particularly when an executor database needs to be initialized with
    /// the same genesis state as Anvil.
    ///
    /// The state file uses Anvil's internal format (different from genesis.json).
    /// To create a clean state file, start Anvil fresh, then immediately dump
    /// the state before any transactions are submitted.
    pub state_path: Option<PathBuf>,

    /// If true and `state_path` is set, dump Anvil's initial state to the file
    /// on exit (when Anvil shuts down).
    ///
    /// This is useful for generating a clean genesis state file that can be
    /// used to initialize an execution database.
    ///
    /// Default: false
    pub dump_initial_state: bool,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            block_time_ms: 50,
            tx_per_block: 1000,
            initial_delay_blocks: 10,
            accounts: 10,
            use_default_genesis: true,
            state_path: None,
            dump_initial_state: false,
        }
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
        assert!(config.use_default_genesis);
        assert!(config.state_path.is_none());
        assert!(!config.dump_initial_state);
    }

    #[test]
    fn config_custom() {
        let config = HarnessConfig {
            block_time_ms: 500,
            tx_per_block: 150,
            initial_delay_blocks: 100,
            accounts: 5,
            use_default_genesis: false,
            state_path: Some(PathBuf::from("/tmp/state.json")),
            dump_initial_state: true,
        };
        assert_eq!(config.block_time_ms, 500);
        assert_eq!(config.tx_per_block, 150);
        assert_eq!(config.initial_delay_blocks, 100);
        assert_eq!(config.accounts, 5);
        assert!(!config.use_default_genesis);
        assert_eq!(config.state_path.unwrap().to_str().unwrap(), "/tmp/state.json");
    }

    #[test]
    fn default_genesis_state_is_valid_json() {
        // Verify the embedded genesis state is valid JSON
        let _: serde_json::Value = serde_json::from_str(DEFAULT_GENESIS_STATE)
            .expect("DEFAULT_GENESIS_STATE should be valid JSON");
    }
}
