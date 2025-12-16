//! `TrieDB` wrapper for Ethereum state trie database operations
//!
//! Provides a thin wrapper around the triedb crate for managing
//! Ethereum account and storage state.

use std::path::Path;

pub use triedb::{
    account::Account,
    database::{Database, OpenError},
    path::{AddressPath, StoragePath},
};

/// Opens an existing `TrieDB` database at the given path.
///
/// # Errors
/// Returns an error if the database cannot be opened.
pub fn open(path: impl AsRef<Path>) -> Result<Database, OpenError> {
    Database::open(path)
}

/// Creates a new `TrieDB` database at the given path.
///
/// # Errors
/// Returns an error if the database cannot be created.
pub fn create(path: impl AsRef<Path>) -> Result<Database, OpenError> {
    Database::create_new(path)
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, B256, U256};
    use alloy_trie::{EMPTY_ROOT_HASH, KECCAK_EMPTY};

    use super::*;

    #[test]
    fn test_insert_get_delete_account() {
        // Create a temporary directory for the database
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");

        // Create a new database
        let db = create(&db_path).expect("failed to create database");

        // Create a test address and account
        let address = Address::repeat_byte(0x42);
        let address_path = AddressPath::for_address(address);
        let account = Account::new(1, U256::from(1000), EMPTY_ROOT_HASH, KECCAK_EMPTY);

        // INSERT: Begin a read-write transaction and insert the account
        {
            let mut tx = db.begin_rw().expect("failed to begin rw transaction");
            tx.set_account(address_path.clone(), Some(account.clone()))
                .expect("failed to set account");
            tx.commit().expect("failed to commit");
        }

        // GET: Verify the account was inserted
        {
            let mut tx = db.begin_ro().expect("failed to begin ro transaction");
            let retrieved = tx.get_account(&address_path).expect("failed to get account");
            assert!(retrieved.is_some());
            let retrieved = retrieved.expect("account should exist");
            assert_eq!(retrieved.nonce, account.nonce);
            assert_eq!(retrieved.balance, account.balance);
            assert_eq!(retrieved.code_hash, account.code_hash);
            tx.commit().expect("failed to commit ro transaction");
        }

        // DELETE: Remove the account by setting it to None
        {
            let mut tx = db.begin_rw().expect("failed to begin rw transaction");
            tx.set_account(address_path.clone(), None).expect("failed to delete account");
            tx.commit().expect("failed to commit");
        }

        // Verify the account was deleted
        {
            let mut tx = db.begin_ro().expect("failed to begin ro transaction");
            let retrieved = tx.get_account(&address_path).expect("failed to get account");
            assert_eq!(retrieved, None);
            tx.commit().expect("failed to commit ro transaction");
        }

        // Clean up
        db.close().expect("failed to close database");
    }

    #[test]
    fn test_insert_get_delete_storage() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let db_path = temp_dir.path().join("test_storage_db");

        let db = create(&db_path).expect("failed to create database");

        let address = Address::repeat_byte(0x42);
        let storage_key = B256::repeat_byte(0x01);
        let storage_value = U256::from(12345);
        let storage_path = StoragePath::for_address_and_slot(address, storage_key);

        // First insert an account (required for storage)
        let account = Account::new(0, U256::ZERO, EMPTY_ROOT_HASH, KECCAK_EMPTY);
        {
            let mut tx = db.begin_rw().expect("failed to begin rw transaction");
            tx.set_account(AddressPath::for_address(address), Some(account))
                .expect("failed to set account");
            tx.commit().expect("failed to commit");
        }

        // INSERT storage slot
        {
            let mut tx = db.begin_rw().expect("failed to begin rw transaction");
            tx.set_storage_slot(storage_path.clone(), Some(storage_value))
                .expect("failed to set storage");
            tx.commit().expect("failed to commit");
        }

        // GET storage slot
        {
            let mut tx = db.begin_ro().expect("failed to begin ro transaction");
            let retrieved = tx.get_storage_slot(&storage_path).expect("failed to get storage");
            assert_eq!(retrieved, Some(storage_value));
            tx.commit().expect("failed to commit");
        }

        // DELETE storage slot
        {
            let mut tx = db.begin_rw().expect("failed to begin rw transaction");
            tx.set_storage_slot(storage_path.clone(), None).expect("failed to delete storage");
            tx.commit().expect("failed to commit");
        }

        // Verify deletion
        {
            let mut tx = db.begin_ro().expect("failed to begin ro transaction");
            let retrieved = tx.get_storage_slot(&storage_path).expect("failed to get storage");
            assert_eq!(retrieved, None);
            tx.commit().expect("failed to commit");
        }

        db.close().expect("failed to close database");
    }
}
