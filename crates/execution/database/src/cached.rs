//! Cached database wrapper
//!
//! This module provides an in-memory caching layer that wraps any `Database` implementation.
//! Cache misses are forwarded to the underlying database.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use alloy::primitives::{Address, B256, U256};
use revm::{
    bytecode::Bytecode,
    database_interface::{Database as RevmDatabase, DatabaseCommit, DatabaseRef},
    state::{AccountInfo, EvmState},
};

use crate::{errors::DbError, traits::Database};

/// In-memory caching database wrapper
///
/// Wraps any `DatabaseRef` implementation and caches all lookups in memory.
/// Cache hits are served directly; cache misses are forwarded to the inner database.
#[derive(Clone, Debug)]
pub struct CachedDatabase<DB> {
    /// Cached accounts: address -> `AccountInfo`
    accounts: Arc<RwLock<HashMap<Address, AccountInfo>>>,
    /// Cached storage: (address, slot) -> value
    storage: Arc<RwLock<HashMap<(Address, U256), U256>>>,
    /// Cached code: `code_hash` -> bytecode
    code: Arc<RwLock<HashMap<B256, Bytecode>>>,
    /// Cached block hashes: `block_number` -> hash
    block_hashes: Arc<RwLock<HashMap<u64, B256>>>,
    /// Inner database for cache misses
    inner: DB,
}

impl<DB: Database> CachedDatabase<DB> {
    /// Create a new cached database wrapping the given inner database
    #[must_use]
    pub fn new(inner: DB) -> Self {
        Self {
            accounts: Arc::new(RwLock::new(HashMap::new())),
            storage: Arc::new(RwLock::new(HashMap::new())),
            code: Arc::new(RwLock::new(HashMap::new())),
            block_hashes: Arc::new(RwLock::new(HashMap::new())),
            inner,
        }
    }

    /// Get a reference to the inner database
    #[must_use]
    pub const fn inner(&self) -> &DB {
        &self.inner
    }

    /// Get a mutable reference to the inner database
    pub const fn inner_mut(&mut self) -> &mut DB {
        &mut self.inner
    }

    /// Get the number of cached accounts
    #[must_use]
    pub fn cached_accounts_count(&self) -> usize {
        self.accounts.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Get the number of cached storage slots
    #[must_use]
    pub fn cached_storage_count(&self) -> usize {
        self.storage.read().map(|c| c.len()).unwrap_or(0)
    }

    /// Get the number of cached block hashes
    #[must_use]
    pub fn cached_block_hashes_count(&self) -> usize {
        self.block_hashes.read().map(|c| c.len()).unwrap_or(0)
    }
}

impl<DB> DatabaseRef for CachedDatabase<DB>
where
    DB: Database,
{
    type Error = DbError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // Check cache first
        {
            let accounts =
                self.accounts.read().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            if let Some(info) = accounts.get(&address) {
                return Ok(Some(info.clone()));
            }
        }

        // Forward to inner database
        let info = self.inner.basic_ref(address)?;

        // Cache the result
        if let Some(ref account_info) = info {
            // Cache the account info
            self.accounts
                .write()
                .map_err(|e| DbError(format!("Lock poisoned: {e}")))?
                .insert(address, account_info.clone());

            // Also cache the code if present (separate lock acquisition)
            if let Some(ref bytecode) = account_info.code
                && !bytecode.is_empty()
            {
                self.code
                    .write()
                    .map_err(|e| DbError(format!("Lock poisoned: {e}")))?
                    .insert(account_info.code_hash, bytecode.clone());
            }
        }

        Ok(info)
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // Check cache first
        {
            let code_cache =
                self.code.read().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            if let Some(code) = code_cache.get(&code_hash) {
                return Ok(code.clone());
            }
        }

        // Forward to inner database
        let code = self.inner.code_by_hash_ref(code_hash)?;

        // Cache the result
        if !code.is_empty() {
            let mut code_cache =
                self.code.write().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            code_cache.insert(code_hash, code.clone());
        }

        Ok(code)
    }

    fn storage_ref(&self, address: Address, slot: U256) -> Result<U256, Self::Error> {
        let key = (address, slot);

        // Check cache first
        {
            let storage_cache =
                self.storage.read().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            if let Some(value) = storage_cache.get(&key) {
                return Ok(*value);
            }
        }

        // Forward to inner database
        let value = self.inner.storage_ref(address, slot)?;

        // Cache the result
        {
            let mut storage_cache =
                self.storage.write().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            storage_cache.insert(key, value);
        }

        Ok(value)
    }

    fn block_hash_ref(&self, block_number: u64) -> Result<B256, Self::Error> {
        // Check cache first
        {
            let hashes =
                self.block_hashes.read().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            if let Some(hash) = hashes.get(&block_number) {
                return Ok(*hash);
            }
        }

        // Forward to inner database
        let hash = self.inner.block_hash_ref(block_number)?;

        // Cache the result
        {
            let mut hashes =
                self.block_hashes.write().map_err(|e| DbError(format!("Lock poisoned: {e}")))?;
            hashes.insert(block_number, hash);
        }

        Ok(hash)
    }
}

impl<DB> RevmDatabase for CachedDatabase<DB>
where
    DB: Database,
{
    type Error = DbError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.basic_ref(address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.code_by_hash_ref(code_hash)
    }

    fn storage(&mut self, address: Address, slot: U256) -> Result<U256, Self::Error> {
        self.storage_ref(address, slot)
    }

    fn block_hash(&mut self, block_number: u64) -> Result<B256, Self::Error> {
        self.block_hash_ref(block_number)
    }
}

impl<DB> DatabaseCommit for CachedDatabase<DB> {
    fn commit(&mut self, changes: EvmState) {
        // Update account cache with committed state
        if let Ok(mut accounts) = self.accounts.write() {
            for (address, account) in &changes {
                accounts.insert(*address, account.info.clone());
            }
        }

        // Update storage cache with committed state
        if let Ok(mut storage_cache) = self.storage.write() {
            for (address, account) in &changes {
                for (slot, value) in &account.storage {
                    storage_cache.insert((*address, *slot), value.present_value);
                }
            }
        }

        // Update code cache for any new contracts
        if let Ok(mut code_cache) = self.code.write() {
            for (_, account) in changes {
                if let Some(code) = account.info.code
                    && !code.is_empty()
                {
                    code_cache.insert(account.info.code_hash, code);
                }
            }
        }
    }
}

impl<DB> Database for CachedDatabase<DB>
where
    DB: Database,
{
    fn commit_block(
        &mut self,
        block_number: u64,
        transaction_changes: Vec<EvmState>,
    ) -> Result<alloy::primitives::B256, crate::errors::DbError> {
        self.inner.commit_block(block_number, transaction_changes)
    }
}
