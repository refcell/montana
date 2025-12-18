//! Anvil state file parsing and conversion to Genesis format.
//!
//! This module provides types for deserializing Anvil's `--dump-state` / `--load-state`
//! JSON format and converting it to [`alloy::genesis::Genesis`] for initializing
//! TrieDB/KeyValueDB databases.

use std::{collections::BTreeMap, path::Path};

use alloy::{
    genesis::{Genesis, GenesisAccount},
    primitives::{Address, Bytes, B256, U256},
};
use serde::Deserialize;

/// Anvil's block environment from state dump.
///
/// Contains block-level parameters at the time of state dump.
#[derive(Debug, Clone, Deserialize)]
pub struct AnvilBlockEnv {
    /// Block number (hex encoded)
    #[serde(default)]
    pub number: U256,
    /// Block beneficiary/coinbase address
    #[serde(default)]
    pub beneficiary: Address,
    /// Block timestamp (hex encoded)
    #[serde(default)]
    pub timestamp: U256,
    /// Block gas limit
    #[serde(default)]
    pub gas_limit: u64,
    /// Base fee per gas (EIP-1559)
    #[serde(default)]
    pub basefee: u128,
    /// Block difficulty (pre-merge)
    #[serde(default)]
    pub difficulty: U256,
    /// Prevrandao/mixHash (post-merge)
    #[serde(default)]
    pub prevrandao: B256,
}

/// Anvil's serializable account record.
///
/// Represents the state of a single account in the Anvil state dump.
#[derive(Debug, Clone, Deserialize)]
pub struct AnvilAccountRecord {
    /// Account nonce
    pub nonce: u64,
    /// Account balance (hex encoded U256)
    pub balance: U256,
    /// Contract bytecode (empty for EOAs)
    #[serde(default)]
    pub code: Bytes,
    /// Storage slots (slot -> value mapping)
    #[serde(default)]
    pub storage: BTreeMap<B256, B256>,
}

/// Anvil's serializable state format.
///
/// This matches the JSON structure produced by `anvil --dump-state` and
/// consumed by `anvil --load-state`.
///
/// # Example
///
/// ```ignore
/// let state = AnvilState::from_file("anvil_state.json")?;
/// let genesis = state.into_genesis();
/// let trie_db = TrieDatabase::open_or_create(path, &genesis, kvdb)?;
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct AnvilState {
    /// Block environment at time of dump
    #[serde(default)]
    pub block: Option<AnvilBlockEnv>,
    /// Account states keyed by address
    #[serde(default)]
    pub accounts: BTreeMap<Address, AnvilAccountRecord>,
    /// Best block number at time of dump
    #[serde(default)]
    pub best_block_number: Option<u64>,
    // Note: blocks, transactions, historical_states are ignored for genesis conversion
}

/// Error type for anvil state operations.
#[derive(Debug)]
pub enum AnvilStateError {
    /// IO error reading the file
    Io(std::io::Error),
    /// JSON parsing error
    Json(serde_json::Error),
}

impl std::fmt::Display for AnvilStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "failed to read anvil state file: {e}"),
            Self::Json(e) => write!(f, "failed to parse anvil state JSON: {e}"),
        }
    }
}

impl std::error::Error for AnvilStateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for AnvilStateError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for AnvilStateError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl AnvilState {
    /// Load anvil state from a JSON file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, AnvilStateError> {
        let contents = std::fs::read_to_string(path)?;
        Self::from_json(&contents)
    }

    /// Parse anvil state from a JSON string.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON cannot be parsed.
    pub fn from_json(json: &str) -> Result<Self, AnvilStateError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Convert this anvil state into an alloy Genesis.
    ///
    /// This maps:
    /// - `accounts` → `Genesis.alloc` (account allocations)
    /// - `block.gas_limit` → `Genesis.gas_limit`
    /// - `block.timestamp` → `Genesis.timestamp`
    /// - `block.basefee` → `Genesis.base_fee_per_gas`
    /// - `block.difficulty` → `Genesis.difficulty`
    #[must_use]
    pub fn into_genesis(self) -> Genesis {
        let alloc: BTreeMap<Address, GenesisAccount> = self
            .accounts
            .into_iter()
            .map(|(addr, record)| {
                let account = GenesisAccount {
                    nonce: Some(record.nonce),
                    balance: record.balance,
                    code: if record.code.is_empty() { None } else { Some(record.code) },
                    storage: if record.storage.is_empty() { None } else { Some(record.storage) },
                    private_key: None,
                };
                (addr, account)
            })
            .collect();

        let mut genesis = Genesis { alloc, ..Default::default() };

        if let Some(block) = self.block {
            genesis.gas_limit = block.gas_limit;
            genesis.timestamp = block.timestamp.try_into().unwrap_or(0);
            genesis.base_fee_per_gas = Some(block.basefee);
            genesis.difficulty = block.difficulty;
            genesis.coinbase = block.beneficiary;
        }

        genesis
    }

    /// Returns the number of accounts in this state.
    #[must_use]
    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_genesis_state() {
        let json = include_str!("../genesis/anvil_state.json");
        let state = AnvilState::from_json(json).expect("should parse default genesis");

        // Should have 11 accounts (10 test accounts + 1 CREATE2 deployer)
        assert_eq!(state.account_count(), 11);

        // Check block info
        let block = state.block.as_ref().expect("should have block");
        assert_eq!(block.gas_limit, 30_000_000);
        assert_eq!(block.basefee, 1_000_000_000); // 1 gwei

        // Check a known test account
        let test_addr: Address = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".parse().unwrap();
        let account = state.accounts.get(&test_addr).expect("should have test account");
        assert_eq!(account.nonce, 0);
        assert!(account.balance > U256::ZERO);
        assert!(account.code.is_empty()); // EOA, no code
    }

    #[test]
    fn convert_to_genesis() {
        let json = include_str!("../genesis/anvil_state.json");
        let state = AnvilState::from_json(json).expect("should parse");
        let genesis = state.into_genesis();

        // Check allocations were converted
        assert_eq!(genesis.alloc.len(), 11);

        // Check block params
        assert_eq!(genesis.gas_limit, 30_000_000);
        assert_eq!(genesis.base_fee_per_gas, Some(1_000_000_000));

        // Verify a test account allocation
        let test_addr: Address = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".parse().unwrap();
        let account = genesis.alloc.get(&test_addr).expect("should have account");
        assert_eq!(account.nonce, Some(0));
        assert!(account.balance > U256::ZERO);
        assert!(account.code.is_none()); // EOA
    }

    #[test]
    fn handles_account_with_code() {
        let json = include_str!("../genesis/anvil_state.json");
        let state = AnvilState::from_json(json).expect("should parse");

        // CREATE2 deployer has code
        let deployer: Address = "0x4e59b44847b379578588920ca78fbf26c0b4956c".parse().unwrap();
        let account = state.accounts.get(&deployer).expect("should have deployer");
        assert!(!account.code.is_empty());

        let genesis = state.into_genesis();
        let alloc = genesis.alloc.get(&deployer).expect("should have deployer alloc");
        assert!(alloc.code.is_some());
    }
}
