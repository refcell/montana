//! Database abstractions for EVM execution
//!
//! This crate provides database implementations for revm, including:
//! - `CachedDatabase`: In-memory caching wrapper for any `DatabaseRef`
//! - `RPCDatabase`: Remote RPC-backed database with file-based caching
//! - `triedb`: `TrieDB` wrapper for Ethereum state trie operations
//! - Re-exports of database traits from revm

pub mod cached;
pub mod errors;
pub mod remote;
pub mod traits;
pub mod triedb;

pub use cached::CachedDatabase;
pub use errors::DbError;
pub use remote::RPCDatabase;
pub use traits::{DBErrorMarker, Database, DatabaseCommit, DatabaseRef};
