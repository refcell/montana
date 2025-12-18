//! `TrieDB` wrapper for Ethereum state trie database operations
//!
//! Provides a thin wrapper around the triedb crate for managing
//! Ethereum account and storage state, implementing the revm `Database` traits.

use std::{io::ErrorKind, path::Path, sync::Arc};

use alloy::{
    consensus::constants::KECCAK_EMPTY,
    genesis::Genesis,
    primitives::{Address, B256, U256},
};
use revm::{
    bytecode::Bytecode,
    database_interface::{Database as RevmDatabase, DatabaseCommit, DatabaseRef},
    state::{AccountInfo, EvmState},
};
pub use triedb::{
    account::Account as TrieAccount,
    database::{Database as TrieDb, OpenError},
    path::{AddressPath, StoragePath},
};

use crate::{
    errors::DbError,
    kvdb::{Header, KeyValueDatabase},
    traits::Database,
};

/// A wrapper around `triedb::Database` that implements revm's `Database` traits.
///
/// This allows using a TrieDB database with revm for EVM execution.
/// Since TrieDB only stores code hashes (not actual bytecode), this wrapper
/// uses a [`KeyValueDatabase`] for persistent code storage and block hash lookups.
#[derive(Clone, Debug)]
pub struct TrieDatabase<KV: KeyValueDatabase> {
    /// The underlying TrieDB database
    inner: Arc<TrieDb>,
    /// Key-value database for code and block hashes
    kvdb: KV,
}

impl<KV: KeyValueDatabase> TrieDatabase<KV> {
    /// Open an existing TrieDB database or create a new one with genesis state.
    ///
    /// If the database exists at `path`, it will be opened. If it does not exist,
    /// a new database will be created and initialized with the provided `genesis` state.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened or created.
    pub fn open_or_create(
        path: impl AsRef<Path>,
        genesis: &Genesis,
        kvdb: KV,
    ) -> Result<Self, OpenError> {
        match TrieDb::open(&path) {
            Ok(db) => Ok(Self { inner: Arc::new(db), kvdb }),
            Err(OpenError::IO(e)) if e.kind() == ErrorKind::NotFound => {
                let db = TrieDb::create_new(&path)?;
                let trie_db = Self { inner: Arc::new(db), kvdb };
                trie_db.apply_genesis(genesis)?;
                Ok(trie_db)
            }
            Err(e) => Err(e),
        }
    }

    /// Apply genesis allocations to the database.
    fn apply_genesis(&self, genesis: &Genesis) -> Result<(), OpenError> {
        let mut tx = self.inner.begin_rw().map_err(|e| {
            OpenError::IO(std::io::Error::other(format!("Failed to begin write transaction: {e}")))
        })?;

        for (address, account) in &genesis.alloc {
            let address_path = AddressPath::for_address(*address);

            // Compute code hash if code is present
            let code_hash = if let Some(ref code) = account.code {
                let bytecode = Bytecode::new_raw(code.clone());
                let hash = bytecode.hash_slow();
                // Store code in the key-value database
                self.kvdb.set_code(&hash, code).map_err(|e| {
                    OpenError::IO(std::io::Error::other(format!(
                        "Failed to store genesis code: {e}"
                    )))
                })?;
                hash
            } else {
                KECCAK_EMPTY
            };

            let trie_account = TrieAccount::new(
                account.nonce.unwrap_or(0),
                account.balance,
                B256::ZERO, // storage_root computed by TrieDB
                code_hash,
            );

            tx.set_account(address_path.clone(), Some(trie_account)).map_err(|e| {
                OpenError::IO(std::io::Error::other(format!(
                    "Failed to set genesis account {address}: {e}"
                )))
            })?;

            // Apply storage if present
            if let Some(ref storage) = account.storage {
                for (slot, value) in storage {
                    let storage_path = StoragePath::for_address_and_slot(*address, *slot);

                    let storage_value = if *value == B256::ZERO {
                        None
                    } else {
                        Some(U256::from_be_bytes(value.0))
                    };

                    tx.set_storage_slot(storage_path, storage_value).map_err(|e| {
                        OpenError::IO(std::io::Error::other(format!(
                            "Failed to set genesis storage {address}:{slot}: {e}"
                        )))
                    })?;
                }
            }
        }

        tx.commit().map_err(|e| {
            OpenError::IO(std::io::Error::other(format!("Failed to commit genesis state: {e}")))
        })?;

        // Store genesis header (block 0) with the computed state root
        let header = Header::new(self.inner.state_root());
        self.kvdb.set_header(0, &header).map_err(|e| {
            OpenError::IO(std::io::Error::other(format!("Failed to store genesis header: {e}")))
        })?;

        Ok(())
    }

    /// Insert bytecode into the key-value database.
    ///
    /// This should be called when deploying contracts or loading code from
    /// an external source, as TrieDB only stores code hashes.
    pub fn insert_code(&self, code_hash: B256, bytecode: Bytecode) -> Result<(), DbError> {
        self.kvdb.set_code(&code_hash, bytecode.original_bytes().as_ref())
    }

    /// Get the current state root.
    #[must_use]
    pub fn state_root(&self) -> B256 {
        self.inner.state_root()
    }
}

impl<KV: KeyValueDatabase> DatabaseRef for TrieDatabase<KV> {
    type Error = DbError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let address_path = AddressPath::for_address(address);

        let mut tx = self
            .inner
            .begin_ro()
            .map_err(|e| DbError::new(format!("Failed to begin read transaction: {e}")))?;

        let account = tx
            .get_account(&address_path)
            .map_err(|e| DbError::new(format!("Failed to get account {address}: {e}")))?;

        tx.commit().map_err(|e| DbError::new(format!("Failed to commit read transaction: {e}")))?;

        match account {
            Some(acc) => {
                // Get code from the key-value database
                let code = if acc.code_hash != KECCAK_EMPTY {
                    self.kvdb.get_code(&acc.code_hash)?.map(|bytes| Bytecode::new_raw(bytes.into()))
                } else {
                    None
                };

                Ok(Some(AccountInfo {
                    nonce: acc.nonce,
                    balance: acc.balance,
                    code_hash: acc.code_hash,
                    code,
                }))
            }
            None => Ok(None),
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // Check key-value database for code
        if let Some(bytes) = self.kvdb.get_code(&code_hash)? {
            return Ok(Bytecode::new_raw(bytes.into()));
        }

        // If not found, return empty bytecode
        Ok(Bytecode::default())
    }

    fn storage_ref(&self, address: Address, slot: U256) -> Result<U256, Self::Error> {
        let storage_path = StoragePath::for_address_and_slot(address, slot.into());

        let mut tx = self
            .inner
            .begin_ro()
            .map_err(|e| DbError::new(format!("Failed to begin read transaction: {e}")))?;

        let value = tx
            .get_storage_slot(&storage_path)
            .map_err(|e| DbError::new(format!("Failed to get storage {address}:{slot}: {e}")))?;

        tx.commit().map_err(|e| DbError::new(format!("Failed to commit read transaction: {e}")))?;

        Ok(value.unwrap_or(U256::ZERO))
    }

    fn block_hash_ref(&self, _block_number: u64) -> Result<B256, Self::Error> {
        // Block hashes are not currently stored in the key-value database.
        // The BLOCKHASH opcode will return zero for now.
        Ok(B256::ZERO)
    }
}

impl<KV: KeyValueDatabase> RevmDatabase for TrieDatabase<KV> {
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

impl<KV: KeyValueDatabase> DatabaseCommit for TrieDatabase<KV> {
    fn commit(&mut self, _changes: EvmState) {
        // handled by cachedb
    }
}

impl<KV: KeyValueDatabase> Database for TrieDatabase<KV> {
    fn commit_block(
        &mut self,
        block_number: u64,
        transaction_changes: Vec<EvmState>,
    ) -> Result<B256, DbError> {
        // Begin a write transaction and commit all changes
        let mut tx = self
            .inner
            .begin_rw()
            .map_err(|e| DbError::new(format!("Failed to begin write transaction: {e}")))?;

        for changes in transaction_changes {
            for (address, account) in changes {
                let address_path = AddressPath::for_address(address);

                // Handle account updates
                if account.is_selfdestructed() {
                    // Delete the account
                    tx.set_account(address_path.clone(), None).map_err(|e| {
                        DbError::new(format!("Failed to delete account {address}: {e}"))
                    })?;
                } else {
                    // Update or create the account
                    let trie_account = TrieAccount::new(
                        account.info.nonce,
                        account.info.balance,
                        // Note: storage_root is computed by TrieDB, we pass a placeholder
                        // The actual storage root will be updated when we commit storage changes
                        B256::ZERO,
                        account.info.code_hash,
                    );

                    tx.set_account(address_path.clone(), Some(trie_account)).map_err(|e| {
                        DbError::new(format!("Failed to set account {address}: {e}"))
                    })?;

                    // Store the code in the key-value database if present
                    if let Some(ref code) = account.info.code
                        && !code.is_empty()
                        && account.info.code_hash != KECCAK_EMPTY
                    {
                        self.kvdb
                            .set_code(&account.info.code_hash, code.original_bytes().as_ref())?;
                    }
                }

                // Handle storage updates
                for (slot, value) in account.storage {
                    let storage_path = StoragePath::for_address_and_slot(address, slot.into());

                    let storage_value = if value.present_value.is_zero() {
                        None // Delete zero storage values
                    } else {
                        Some(value.present_value)
                    };

                    tx.set_storage_slot(storage_path, storage_value).map_err(|e| {
                        DbError::new(format!("Failed to set storage {address}:{slot}: {e}"))
                    })?;
                }
            }
        }

        // Commit the transaction
        tx.commit().map_err(|e| DbError::new(format!("Failed to commit transaction: {e}")))?;

        let state_root = self.inner.state_root();

        // Store the header for this block number
        let header = Header::new(state_root);
        self.kvdb.set_header(block_number, &header)?;

        Ok(state_root)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use alloy::{
        genesis::{Genesis, GenesisAccount},
        primitives::{Address, Bytes, U256},
    };
    use alloy_trie::{EMPTY_ROOT_HASH, KECCAK_EMPTY};
    use revm::{
        bytecode::Bytecode,
        state::{Account, EvmStorageSlot},
    };

    use super::*;
    use crate::kvdb::RocksDbKvDatabase;

    fn empty_genesis() -> Genesis {
        Genesis::default()
    }

    fn create_kvdb(path: &std::path::Path, genesis: &Genesis) -> RocksDbKvDatabase {
        RocksDbKvDatabase::open_or_create(path.join("kvdb"), genesis)
            .expect("failed to create kvdb")
    }

    #[test]
    fn test_open_or_create_new_database() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");

        // Create a genesis with some accounts
        let alice = Address::repeat_byte(0x01);
        let bob = Address::repeat_byte(0x02);

        let mut alloc = BTreeMap::new();
        alloc.insert(
            alice,
            GenesisAccount {
                balance: U256::from(1_000_000),
                nonce: Some(1),
                code: Some(Bytes::from(vec![0x60, 0x42, 0x00])),
                storage: None,
                private_key: None,
            },
        );
        alloc.insert(
            bob,
            GenesisAccount {
                balance: U256::from(500_000),
                nonce: Some(5),
                code: None,
                storage: Some(BTreeMap::from([(B256::ZERO, B256::from(U256::from(1)))])),
                private_key: None,
            },
        );

        let genesis = Genesis { alloc, ..Default::default() };
        let kvdb = create_kvdb(temp_dir.path(), &genesis);

        // 1. Create database with genesis
        let db = TrieDatabase::open_or_create(&db_path, &genesis, kvdb.clone())
            .expect("failed to create database");

        // 2. Verify state root is not empty
        let state_root = db.state_root();
        assert_ne!(state_root, EMPTY_ROOT_HASH, "state root should not be empty");

        // 3. Verify genesis accounts were applied
        let alice_info = db.basic_ref(alice).expect("failed to get alice").expect("alice exists");
        assert_eq!(alice_info.nonce, 1);
        assert_eq!(alice_info.balance, U256::from(1_000_000));
        assert!(alice_info.code.is_some());

        let bob_info = db.basic_ref(bob).expect("failed to get bob").expect("bob exists");
        assert_eq!(bob_info.nonce, 5);
        assert_eq!(bob_info.balance, U256::from(500_000));
        assert_eq!(bob_info.code_hash, KECCAK_EMPTY);

        // 4. Verify genesis storage was applied
        assert_eq!(db.storage_ref(bob, U256::from(0)).unwrap(), U256::from(1));

        // 5. Verify genesis header was stored
        let header = kvdb.get_header(0).unwrap().expect("genesis header should exist");
        assert_eq!(header.state_root, state_root);
    }

    #[test]
    fn test_open_or_create_existing_database() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");

        let alice = Address::repeat_byte(0x01);

        let mut alloc = BTreeMap::new();
        alloc
            .insert(alice, GenesisAccount { balance: U256::from(1_000_000), ..Default::default() });
        let genesis = Genesis { alloc, ..Default::default() };
        let kvdb = create_kvdb(temp_dir.path(), &genesis);

        // Create the database first
        let db1 = TrieDatabase::open_or_create(&db_path, &genesis, kvdb.clone())
            .expect("failed to create database");
        let state_root1 = db1.state_root();
        drop(db1);

        // Re-open the database - genesis should NOT be re-applied
        let different_genesis = Genesis {
            alloc: BTreeMap::from([(
                alice,
                GenesisAccount { balance: U256::from(999_999_999), ..Default::default() },
            )]),
            ..Default::default()
        };
        let db2 = TrieDatabase::open_or_create(&db_path, &different_genesis, kvdb)
            .expect("failed to open database");
        let state_root2 = db2.state_root();

        // State roots should match (genesis wasn't re-applied)
        assert_eq!(state_root1, state_root2);

        // Balance should be original, not from the "different" genesis
        let alice_info = db2.basic_ref(alice).expect("query failed").expect("alice exists");
        assert_eq!(alice_info.balance, U256::from(1_000_000));
    }

    #[test]
    fn test_trie_database_commit() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");

        let genesis = empty_genesis();
        let kvdb = create_kvdb(temp_dir.path(), &genesis);

        // 1. Create database with empty genesis
        let mut db = TrieDatabase::open_or_create(&db_path, &genesis, kvdb.clone())
            .expect("failed to create database");

        let alice = Address::repeat_byte(0x01);
        let bob = Address::repeat_byte(0x02);

        let alice_code = Bytecode::new_raw(vec![0x60, 0x42, 0x00].into());
        let alice_code_hash = alice_code.hash_slow();

        // 2. Write accounts via commit
        let mut changes: EvmState = EvmState::default();

        let mut alice_account = Account::new_not_existing(0);
        alice_account.info.nonce = 1;
        alice_account.info.balance = U256::from(1_000_000);
        alice_account.info.code_hash = alice_code_hash;
        alice_account.info.code = Some(alice_code.clone());
        alice_account.mark_touch();

        // 3. Write storage for accounts
        alice_account.storage.insert(U256::from(0), EvmStorageSlot::new(U256::from(42), 0));
        alice_account.storage.insert(U256::from(1), EvmStorageSlot::new(U256::from(100), 0));
        alice_account.storage.insert(U256::from(100), EvmStorageSlot::new(U256::from(999), 0));
        changes.insert(alice, alice_account);

        let mut bob_account = Account::new_not_existing(0);
        bob_account.info.nonce = 5;
        bob_account.info.balance = U256::from(500_000);
        bob_account.info.code_hash = KECCAK_EMPTY;
        bob_account.mark_touch();
        bob_account.storage.insert(U256::from(0), EvmStorageSlot::new(U256::from(1), 0));
        changes.insert(bob, bob_account);

        // 4. Commit block
        let state_root = db.commit_block(1, vec![changes]).expect("commit_block failed");

        // 5. Check state root
        assert_ne!(state_root, EMPTY_ROOT_HASH, "state root should not be empty");

        // 6. Read state for accounts
        let alice_info = db.basic_ref(alice).expect("failed to get alice").expect("alice exists");
        assert_eq!(alice_info.nonce, 1);
        assert_eq!(alice_info.balance, U256::from(1_000_000));
        assert_eq!(alice_info.code_hash, alice_code_hash);
        assert!(alice_info.code.is_some());
        assert_eq!(alice_info.code.unwrap().original_bytes(), alice_code.original_bytes());

        let bob_info = db.basic_ref(bob).expect("failed to get bob").expect("bob exists");
        assert_eq!(bob_info.nonce, 5);
        assert_eq!(bob_info.balance, U256::from(500_000));
        assert_eq!(bob_info.code_hash, KECCAK_EMPTY);

        // 7. Read storage
        assert_eq!(db.storage_ref(alice, U256::from(0)).unwrap(), U256::from(42));
        assert_eq!(db.storage_ref(alice, U256::from(1)).unwrap(), U256::from(100));
        assert_eq!(db.storage_ref(alice, U256::from(100)).unwrap(), U256::from(999));
        assert_eq!(db.storage_ref(alice, U256::from(999)).unwrap(), U256::ZERO);

        assert_eq!(db.storage_ref(bob, U256::from(0)).unwrap(), U256::from(1));
        assert_eq!(db.storage_ref(bob, U256::from(1)).unwrap(), U256::ZERO);

        // Non-existent account
        let unknown = Address::repeat_byte(0xFF);
        assert!(db.basic_ref(unknown).expect("query failed").is_none());

        // 8. Verify header was stored for block 1
        let header = kvdb.get_header(1).unwrap().expect("block 1 header should exist");
        assert_eq!(header.state_root, state_root);
    }
}
