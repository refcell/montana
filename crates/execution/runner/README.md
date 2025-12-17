# runner

Block execution runner for Optimism (Base) blocks.

This crate provides functionality for executing Optimism (Base) blocks using op-revm with an in-memory database that falls back to RPC for missing state.

## Overview

The runner manages block execution with the following capabilities:

- **Block Execution**: Execute blocks using op-revm with Base stack specifications
- **State Management**: In-memory database with RPC fallback for missing state
- **Block Streaming**: Consume blocks from various producers (RPC, channels, etc.)
- **Progress Tracking**: Report execution progress and metrics

## Architecture

The execution runner operates in the **sync + handoff** model:

1. **Sync Stage**: Fetches and executes historical blocks from RPC to catch up to the chain tip
2. **Active Stage**: Continuously executes new blocks as they arrive (from RPC or derivation pipeline)

## Usage

```rust,ignore
use runner::Execution;
use blocksource::RpcBlockProducer;

// Create block producer (fetches blocks from RPC)
let (tx, rx) = channel::unbounded();
let producer = RpcBlockProducer::new(rpc_url, start_block, tx).await?;

// Create execution runner
let execution = Execution::new(rpc_url, rx);

// Start execution
tokio::spawn(async move {
    producer.start().await
});

execution.start().await?;
```

## License

Licensed under the MIT license.
