//! Key-Value Database abstraction for storing code, headers, and chain state.
//!
//! This module provides a trait for key-value storage operations and a RocksDB-backed
//! implementation.

use std::{path::Path, sync::Arc};

use alloy::{genesis::Genesis, primitives::B256};
use rocksdb::{DB, Options};
use serde::{Deserialize, Serialize};

use crate::errors::DbError;

/// Column family names for RocksDB.
const CF_CODE: &str = "code";
const CF_HEADERS: &str = "headers";
const CF_CHAIN_STATE: &str = "chain_state";

/// Key for safe block number in chain state.
const KEY_SAFE: &[u8] = b"safe";
/// Key for unsafe block number in chain state.
const KEY_UNSAFE: &[u8] = b"unsafe";

/// Block header containing essential block state.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    /// The state root of the block.
    pub state_root: B256,
}

impl Header {
    /// Create a new header with the given state root.
    #[must_use]
    pub const fn new(state_root: B256) -> Self {
        Self { state_root }
    }

    /// Serialize the header to bytes.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        // Simple format: just the 32 bytes of the state root
        self.state_root.to_vec()
    }

    /// Deserialize the header from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DbError> {
        if bytes.len() != 32 {
            return Err(DbError::new(format!(
                "Invalid header bytes length: expected 32, got {}",
                bytes.len()
            )));
        }
        Ok(Self { state_root: B256::from_slice(bytes) })
    }
}

/// Trait for key-value database operations.
///
/// Supports storing and retrieving:
/// - Code: codehash => bytecode
/// - Headers: block_number => Header
/// - Safe block: "safe" => block_number
/// - Unsafe block: "unsafe" => block_number
pub trait KeyValueDatabase: Clone + Send + Sync {
    /// Get code by its hash.
    fn get_code(&self, code_hash: &B256) -> Result<Option<Vec<u8>>, DbError>;

    /// Set code by its hash.
    fn set_code(&self, code_hash: &B256, code: &[u8]) -> Result<(), DbError>;

    /// Set multiple code entries in a batch. More efficient for bulk inserts.
    fn set_code_batch(&self, entries: &[(B256, Vec<u8>)]) -> Result<(), DbError>;

    /// Get a header by block number.
    fn get_header(&self, block_number: u64) -> Result<Option<Header>, DbError>;

    /// Set a header for a block number.
    fn set_header(&self, block_number: u64, header: &Header) -> Result<(), DbError>;

    /// Get the safe block number.
    fn get_safe(&self) -> Result<Option<u64>, DbError>;

    /// Set the safe block number.
    fn set_safe(&self, block_number: u64) -> Result<(), DbError>;

    /// Get the unsafe block number.
    fn get_unsafe(&self) -> Result<Option<u64>, DbError>;

    /// Set the unsafe block number.
    fn set_unsafe(&self, block_number: u64) -> Result<(), DbError>;
}

/// RocksDB-backed implementation of [`KeyValueDatabase`].
#[derive(Clone, Debug)]
pub struct RocksDbKvDatabase {
    inner: Arc<DB>,
}

impl RocksDbKvDatabase {
    /// Open an existing database or create a new one.
    ///
    /// If the database exists at `path`, it will be opened. If it does not exist,
    /// a new database will be created and initialized with genesis state.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened or created.
    pub fn open_or_create(path: impl AsRef<Path>, genesis: &Genesis) -> Result<Self, DbError> {
        let path = path.as_ref();
        let exists = path.exists();

        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        // Limit open file handles to avoid "Too many open files" errors.
        // RocksDB will use an LRU cache to manage file descriptors.
        opts.set_max_open_files(512);

        let cfs = [CF_CODE, CF_HEADERS, CF_CHAIN_STATE];

        let db = DB::open_cf(&opts, path, cfs)
            .map_err(|e| DbError::new(format!("Failed to open RocksDB: {e}")))?;

        let kvdb = Self { inner: Arc::new(db) };

        if !exists {
            kvdb.apply_genesis(genesis)?;
        }

        Ok(kvdb)
    }

    /// Apply genesis state to the database.
    fn apply_genesis(&self, genesis: &Genesis) -> Result<(), DbError> {
        use revm::bytecode::Bytecode;

        // Store code from genesis accounts
        for account in genesis.alloc.values() {
            if let Some(ref code) = account.code {
                let bytecode = Bytecode::new_raw(code.clone());
                let code_hash = bytecode.hash_slow();
                self.set_code(&code_hash, code)?;
            }
        }

        // Initialize safe and unsafe to 0
        self.set_safe(0)?;
        self.set_unsafe(0)?;

        // Set genesis header (block 0) with empty state root
        // The actual state root will be set after TrieDB applies genesis
        let header = Header::new(B256::ZERO);
        self.set_header(0, &header)?;

        Ok(())
    }

    /// Get the column family handle for code storage.
    fn cf_code(&self) -> &rocksdb::ColumnFamily {
        self.inner.cf_handle(CF_CODE).expect("code column family should exist")
    }

    /// Get the column family handle for headers storage.
    fn cf_headers(&self) -> &rocksdb::ColumnFamily {
        self.inner.cf_handle(CF_HEADERS).expect("headers column family should exist")
    }

    /// Get the column family handle for chain state storage.
    fn cf_chain_state(&self) -> &rocksdb::ColumnFamily {
        self.inner.cf_handle(CF_CHAIN_STATE).expect("chain_state column family should exist")
    }
}

impl KeyValueDatabase for RocksDbKvDatabase {
    fn get_code(&self, code_hash: &B256) -> Result<Option<Vec<u8>>, DbError> {
        self.inner
            .get_cf(self.cf_code(), code_hash.as_slice())
            .map_err(|e| DbError::new(format!("Failed to get code: {e}")))
    }

    fn set_code(&self, code_hash: &B256, code: &[u8]) -> Result<(), DbError> {
        self.inner
            .put_cf(self.cf_code(), code_hash.as_slice(), code)
            .map_err(|e| DbError::new(format!("Failed to set code: {e}")))
    }

    fn set_code_batch(&self, entries: &[(B256, Vec<u8>)]) -> Result<(), DbError> {
        let mut batch = rocksdb::WriteBatch::default();
        let cf = self.cf_code();
        for (code_hash, code) in entries {
            batch.put_cf(cf, code_hash.as_slice(), code);
        }
        self.inner.write(batch).map_err(|e| DbError::new(format!("Failed to write batch: {e}")))
    }

    fn get_header(&self, block_number: u64) -> Result<Option<Header>, DbError> {
        let key = block_number.to_be_bytes();
        match self.inner.get_cf(self.cf_headers(), key) {
            Ok(Some(bytes)) => Ok(Some(Header::from_bytes(&bytes)?)),
            Ok(None) => Ok(None),
            Err(e) => Err(DbError::new(format!("Failed to get header: {e}"))),
        }
    }

    fn set_header(&self, block_number: u64, header: &Header) -> Result<(), DbError> {
        let key = block_number.to_be_bytes();
        self.inner
            .put_cf(self.cf_headers(), key, header.to_bytes())
            .map_err(|e| DbError::new(format!("Failed to set header: {e}")))
    }

    fn get_safe(&self) -> Result<Option<u64>, DbError> {
        match self.inner.get_cf(self.cf_chain_state(), KEY_SAFE) {
            Ok(Some(bytes)) => {
                if bytes.len() != 8 {
                    return Err(DbError::new(format!(
                        "Invalid safe block number bytes length: expected 8, got {}",
                        bytes.len()
                    )));
                }
                let arr: [u8; 8] = bytes.try_into().unwrap();
                Ok(Some(u64::from_be_bytes(arr)))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(DbError::new(format!("Failed to get safe block: {e}"))),
        }
    }

    fn set_safe(&self, block_number: u64) -> Result<(), DbError> {
        self.inner
            .put_cf(self.cf_chain_state(), KEY_SAFE, block_number.to_be_bytes())
            .map_err(|e| DbError::new(format!("Failed to set safe block: {e}")))
    }

    fn get_unsafe(&self) -> Result<Option<u64>, DbError> {
        match self.inner.get_cf(self.cf_chain_state(), KEY_UNSAFE) {
            Ok(Some(bytes)) => {
                if bytes.len() != 8 {
                    return Err(DbError::new(format!(
                        "Invalid unsafe block number bytes length: expected 8, got {}",
                        bytes.len()
                    )));
                }
                let arr: [u8; 8] = bytes.try_into().unwrap();
                Ok(Some(u64::from_be_bytes(arr)))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(DbError::new(format!("Failed to get unsafe block: {e}"))),
        }
    }

    fn set_unsafe(&self, block_number: u64) -> Result<(), DbError> {
        self.inner
            .put_cf(self.cf_chain_state(), KEY_UNSAFE, block_number.to_be_bytes())
            .map_err(|e| DbError::new(format!("Failed to set unsafe block: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use alloy::{
        genesis::{Genesis, GenesisAccount},
        primitives::Bytes,
    };

    use super::*;

    #[test]
    fn test_kvdb_operations() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("test_kvdb");

        // Create genesis with code
        let code = Bytes::from(vec![0x60, 0x42, 0x00]);
        let bytecode = revm::bytecode::Bytecode::new_raw(code.clone());
        let genesis_code_hash = bytecode.hash_slow();

        let mut alloc = BTreeMap::new();
        alloc.insert(
            alloy::primitives::Address::repeat_byte(0x01),
            GenesisAccount { code: Some(code.clone()), ..Default::default() },
        );
        let genesis = Genesis { alloc, ..Default::default() };

        // Open database with genesis
        let kvdb = RocksDbKvDatabase::open_or_create(&db_path, &genesis)
            .expect("failed to create database");

        // Verify genesis initialization
        assert_eq!(kvdb.get_safe().unwrap(), Some(0));
        assert_eq!(kvdb.get_unsafe().unwrap(), Some(0));
        let genesis_header = kvdb.get_header(0).unwrap().expect("genesis header should exist");
        assert_eq!(genesis_header.state_root, B256::ZERO);

        // Verify genesis code was stored
        let retrieved_code =
            kvdb.get_code(&genesis_code_hash).unwrap().expect("genesis code should exist");
        assert_eq!(retrieved_code.as_slice(), code.as_ref());

        // Test code storage
        let new_code_hash = B256::repeat_byte(0x99);
        let new_code = vec![0x01, 0x02, 0x03];
        assert!(kvdb.get_code(&new_code_hash).unwrap().is_none());
        kvdb.set_code(&new_code_hash, &new_code).unwrap();
        assert_eq!(kvdb.get_code(&new_code_hash).unwrap().unwrap(), new_code);

        // Test header storage and serialization
        let header = Header::new(B256::repeat_byte(0x42));
        let bytes = header.to_bytes();
        assert_eq!(Header::from_bytes(&bytes).unwrap(), header);

        assert!(kvdb.get_header(42).unwrap().is_none());
        kvdb.set_header(42, &header).unwrap();
        assert_eq!(kvdb.get_header(42).unwrap().unwrap(), header);

        // Test safe/unsafe storage
        kvdb.set_safe(100).unwrap();
        kvdb.set_unsafe(200).unwrap();
        assert_eq!(kvdb.get_safe().unwrap(), Some(100));
        assert_eq!(kvdb.get_unsafe().unwrap(), Some(200));
    }
}
