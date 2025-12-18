# montana-block-feeder

Block feeder for Montana - fetches blocks from RPC and sends them to the sequencer.

## Overview

This crate provides functionality for fetching blocks from an RPC provider and sending them to the sequencer via a channel. It's used when sync is skipped (re-execution mode) to continuously stream blocks for processing.

## Usage

```rust,ignore
use montana_block_feeder::run_block_feeder;

// Create a channel for blocks
let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

// Start the block feeder
run_block_feeder(
    provider,           // Alloy provider for RPC
    start_block,        // Starting block number
    poll_interval_ms,   // Polling interval in milliseconds
    tx,                 // Channel sender for blocks
    tui_handle,         // Optional TUI handle for progress updates
).await?;
```

## How It Works

1. **Polling**: Continuously polls the RPC for new blocks
2. **Fetching**: Fetches full block data with transactions
3. **Converting**: Converts RPC blocks to Montana's `L2BlockData` format
4. **Sending**: Sends blocks through an unbounded channel to decouple fetching from execution
5. **TUI Updates**: Optionally sends progress events to the TUI

## License

Licensed under the MIT license.
