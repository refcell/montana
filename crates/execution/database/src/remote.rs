//! Remote database implementation backed by RPC
//!
//! This module provides an RPC-backed database with file-based caching.
//!
//! Cache hierarchy (checked in order):
//! 1. In-memory cache (via `CachedDatabase` wrapper)
//! 2. JSON file cache at `cache/remote-db/<blockNum>.json`
//! 3. RPC fallback (slowest, persists to file cache)

use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use alloy::{
    consensus::constants::KECCAK_EMPTY,
    eips::BlockNumberOrTag,
    primitives::{Address, B256, Bytes, U256},
    providers::Provider,
};
use op_alloy::network::Optimism;
use revm::{bytecode::Bytecode, database_interface::DatabaseRef, state::AccountInfo};
use serde::{Deserialize, Serialize};
use tokio::runtime::Handle;
use tracing::{debug, warn};

use crate::errors::DbError;

/// Serializable storage key (address, slot) as a string for JSON map keys
fn storage_key(address: Address, slot: U256) -> String {
    format!("{address}:{slot}")
}

/// Serializable block state cache for JSON persistence
#[derive(Serialize, Deserialize, Default, Debug)]
struct BlockCache {
    accounts: HashMap<Address, AccountInfo>,
    /// Storage keyed by "address:slot" string
    storage: HashMap<String, U256>,
    block_hashes: HashMap<u64, B256>,
}

/// RPC-backed database with file-based caching
///
/// This database fetches state from an RPC provider and persists results
/// to a JSON file cache. For in-memory caching, wrap this in a `CachedDatabase`.
#[derive(Clone, Debug)]
pub struct RPCDatabase<P> {
    /// File-based cache for persistence across runs
    file_cache: Arc<RwLock<BlockCache>>,
    /// Path to the cache file
    cache_path: PathBuf,
    /// RPC provider for fallback lookups
    provider: P,
    /// Block number to query state at (the block before the one we're executing)
    state_block: u64,
    /// Tokio runtime handle for blocking RPC calls
    runtime_handle: Handle,
}

impl<P: Provider<Optimism> + Clone> RPCDatabase<P> {
    /// Create a new RPC-backed database with file caching
    ///
    /// # Arguments
    /// * `provider` - The RPC provider for fallback lookups
    /// * `state_block` - The block number to query state at
    /// * `runtime_handle` - Tokio runtime handle for blocking RPC calls
    ///
    /// File cache is stored at `cache/remote-db/<state_block>.json`
    #[must_use]
    pub fn new(provider: P, state_block: u64, runtime_handle: Handle) -> Self {
        let cache_path = PathBuf::from(format!("static/remote-db/{state_block}.json"));

        // Load existing file cache if present
        let file_cache = Self::load_file_cache(&cache_path);

        if !file_cache.accounts.is_empty() || !file_cache.storage.is_empty() {
            debug!(
                "Loaded file cache for block {}: {} accounts, {} storage slots, {} block hashes",
                state_block,
                file_cache.accounts.len(),
                file_cache.storage.len(),
                file_cache.block_hashes.len()
            );
        }

        Self {
            file_cache: Arc::new(RwLock::new(file_cache)),
            cache_path,
            provider,
            state_block,
            runtime_handle,
        }
    }

    /// Load file cache from disk, returns empty cache if not found or invalid
    fn load_file_cache(path: &PathBuf) -> BlockCache {
        fs::read_to_string(path).map_or_else(
            |_| BlockCache::default(),
            |contents| serde_json::from_str(&contents).unwrap_or_default(),
        )
    }

    /// Save current file cache to disk
    fn save_file_cache(&self) {
        let Ok(cache) = self.file_cache.read() else {
            warn!("Failed to acquire file cache lock for saving");
            return;
        };

        // Create cache directory if needed
        if let Some(parent) = self.cache_path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            warn!("Failed to create cache directory: {e}");
            return;
        }

        match serde_json::to_string_pretty(&*cache) {
            Ok(json) => {
                if let Err(e) = fs::write(&self.cache_path, json) {
                    warn!("Failed to write cache file: {e}");
                }
            }
            Err(e) => warn!("Failed to serialize cache: {e}"),
        }
    }

    /// Fetch account info from RPC and persist to file cache
    fn fetch_account_info(&self, address: Address) -> Result<AccountInfo, DbError> {
        // Check file cache first
        {
            let cache =
                self.file_cache.read().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            if let Some(info) = cache.accounts.get(&address) {
                return Ok(info.clone());
            }
        }

        let provider = self.provider.clone();
        let block_num = self.state_block;

        let info = tokio::task::block_in_place(|| {
            self.runtime_handle.block_on(async move {
                let block_id = BlockNumberOrTag::Number(block_num);

                // Fetch nonce, balance, and code concurrently
                let (nonce, balance, code) = tokio::try_join!(
                    provider.get_transaction_count(address).block_id(block_id.into()),
                    provider.get_balance(address).block_id(block_id.into()),
                    provider.get_code_at(address).block_id(block_id.into()),
                )
                .map_err(|e| DbError(format!("RPC error fetching account {address}: {e}")))?;

                let code_bytes: Bytes = code;
                let bytecode = if code_bytes.is_empty() {
                    Bytecode::default()
                } else {
                    Bytecode::new_raw(code_bytes)
                };

                let code_hash =
                    if bytecode.is_empty() { KECCAK_EMPTY } else { bytecode.hash_slow() };

                Ok(AccountInfo { balance, nonce, code_hash, code: Some(bytecode) })
            })
        })?;

        // Persist to file cache
        if let Ok(mut cache) = self.file_cache.write() {
            cache.accounts.insert(address, info.clone());
        }
        self.save_file_cache();

        Ok(info)
    }

    /// Fetch storage slot from RPC and persist to file cache
    fn fetch_storage(&self, address: Address, slot: U256) -> Result<U256, DbError> {
        // Check file cache first
        {
            let cache =
                self.file_cache.read().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            if let Some(value) = cache.storage.get(&storage_key(address, slot)) {
                return Ok(*value);
            }
        }

        let provider = self.provider.clone();
        let block_num = self.state_block;

        let value = tokio::task::block_in_place(|| {
            self.runtime_handle.block_on(async move {
                let value = provider
                    .get_storage_at(address, slot)
                    .block_id(BlockNumberOrTag::Number(block_num).into())
                    .await
                    .map_err(|e| {
                        DbError(format!("RPC error fetching storage {address}:{slot}: {e}"))
                    })?;
                Ok(value)
            })
        })?;

        // Persist to file cache
        if let Ok(mut cache) = self.file_cache.write() {
            cache.storage.insert(storage_key(address, slot), value);
        }
        self.save_file_cache();

        Ok(value)
    }

    /// Fetch block hash from RPC and persist to file cache
    fn fetch_block_hash(&self, block_number: u64) -> Result<B256, DbError> {
        // Check file cache first
        {
            let cache =
                self.file_cache.read().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            if let Some(hash) = cache.block_hashes.get(&block_number) {
                return Ok(*hash);
            }
        }

        let provider = self.provider.clone();

        let hash = tokio::task::block_in_place(|| {
            self.runtime_handle.block_on(async move {
                let block = provider
                    .get_block_by_number(BlockNumberOrTag::Number(block_number))
                    .await
                    .map_err(|e| DbError(format!("RPC error fetching block {block_number}: {e}")))?
                    .ok_or_else(|| DbError(format!("Block {block_number} not found")))?;
                Ok(block.header.hash)
            })
        })?;

        // Persist to file cache
        if let Ok(mut cache) = self.file_cache.write() {
            cache.block_hashes.insert(block_number, hash);
        }
        self.save_file_cache();

        Ok(hash)
    }
}

impl<P: Provider<Optimism> + Clone> DatabaseRef for RPCDatabase<P> {
    type Error = DbError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(Some(self.fetch_account_info(address)?))
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // Code should have been loaded with account info
        warn!("Code lookup by hash {code_hash} - code should have been loaded with account");
        Ok(Bytecode::default())
    }

    fn storage_ref(&self, address: Address, slot: U256) -> Result<U256, Self::Error> {
        self.fetch_storage(address, slot)
    }

    fn block_hash_ref(&self, block_number: u64) -> Result<B256, Self::Error> {
        self.fetch_block_hash(block_number)
    }
}
