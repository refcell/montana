# database

Database abstractions for EVM execution.

## Overview

This crate provides database implementations for revm, enabling EVM execution with various state backends. It includes caching layers, remote RPC-backed databases, and trie operations.

## Core Components

| Component | Description |
|-----------|-------------|
| `CachedDatabase` | In-memory caching wrapper for any `DatabaseRef` |
| `RPCDatabase` | Remote RPC-backed database with file-based caching |
| `TrieDB` | Wrapper for Ethereum state trie operations |
| `DbError` | Unified error type for database operations |

## Modules

| Module | Description |
|--------|-------------|
| `cached` | In-memory caching database wrapper |
| `errors` | Error types |
| `remote` | RPC-backed database implementation |
| `traits` | Re-exports of database traits from revm |
| `triedb` | State trie database operations |

## Usage

```rust,ignore
use database::{CachedDatabase, RPCDatabase, Database};

// Create an RPC-backed database
let rpc_db = RPCDatabase::new(rpc_url, block_number).await?;

// Wrap with caching for performance
let cached_db = CachedDatabase::new(rpc_db);

// Use with revm for EVM execution
```

## License

Licensed under the MIT license.
