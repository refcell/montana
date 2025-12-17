# blocksource

Block source abstractions for aggregating blocks from multiple data sources.

## Overview

This crate provides a unified interface for receiving blocks from various sources. It handles block conversion and provides producers for fetching and streaming blocks to the execution layer.

## Core Components

| Component | Description |
|-----------|-------------|
| `BlockProducer` | Trait for producing blocks from any source |
| `RpcBlockProducer` | Fetches blocks from L2 RPC and streams them via channel |
| `ChannelBlockProducer` | Receives blocks from a channel (used in derivation pipeline) |
| `RpcBlockSource` | RPC-backed block source implementation |
| `OpBlock` | Base stack block type with all necessary fields |

## Block Producers

### RpcBlockProducer

Fetches blocks from an L2 RPC endpoint and sends them to a channel. Used during the sync stage and by the sequencer role.

```rust,ignore
use blocksource::RpcBlockProducer;

// Create producer that fetches blocks from RPC
let producer = RpcBlockProducer::new(rpc_url, start_block, tx).await?;

// Start producing blocks
producer.start().await?;
```

### ChannelBlockProducer

Receives blocks from a channel. Used by the validator role to consume blocks derived from L1.

```rust,ignore
use blocksource::ChannelBlockProducer;

// Create producer that receives blocks from a channel
let producer = ChannelBlockProducer::new(rx);

// Start receiving blocks
producer.start().await?;
```

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
