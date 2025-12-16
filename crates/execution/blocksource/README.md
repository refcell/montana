# blocksource

Block source abstractions for aggregating blocks from multiple data sources.

## Overview

This crate provides a unified interface for receiving blocks from various sources such as RPC polling, P2P gossip, and Engine API. It handles block conversion and provides producers for both historical and live block data.

## Core Components

| Component | Description |
|-----------|-------------|
| `BlockProducer` | Trait for producing blocks from any source |
| `HistoricalRangeProducer` | Producer for fetching historical block ranges |
| `LiveRpcProducer` | Producer for live blocks via RPC polling |
| `RpcBlockSource` | RPC-backed block source implementation |
| `OpBlock` | OP Stack block type with all necessary fields |

## Modules

| Module | Description |
|--------|-------------|
| `convert` | Block and transaction conversion utilities |
| `producer` | Block producer implementations |
| `rpc` | RPC block source |
| `types` | Core type definitions |

## Usage

```rust,ignore
use blocksource::{BlockProducer, RpcBlockSource, OpBlock};

// Create an RPC block source
let source = RpcBlockSource::new(rpc_url).await?;

// Fetch a block
let block = source.get_block(block_number).await?;

// Convert to execution environment
let env = blocksource::block_to_env(&block, spec_id);
```

## License

Licensed under the MIT license.
