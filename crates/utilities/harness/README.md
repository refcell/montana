# harness

Test harness for Montana - spawns a local anvil chain with synthetic transaction activity.

## Overview

This crate provides a self-contained test environment for Montana that:

- Spawns a local anvil instance as a simulated L2 chain
- Generates semi-random transactions using anvil's pre-funded test accounts
- Runs transaction generation in a background thread
- Supports configurable block time and initial block backlog for sync testing

## Usage

```rust,ignore
use montana_harness::{Harness, HarnessConfig};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Create harness with default config
    let harness = Harness::spawn(HarnessConfig::default()).await?;

    // Get the RPC URL to connect to
    let rpc_url = harness.rpc_url();

    // Use rpc_url with Montana binary...

    Ok(())
}
```

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `block_time_ms` | 1000 | Anvil block time in milliseconds |
| `tx_interval_ms` | 500 | Interval between transaction generation |
| `initial_delay_blocks` | 50 | Blocks to generate before returning (creates sync backlog) |
| `accounts` | 10 | Number of test accounts to use |

## Sync Testing

The `initial_delay_blocks` option creates a backlog of blocks before Montana starts.
This simulates a chain that's already progressed, forcing Montana to sync from block 0
to the current tip - perfect for testing the sync stage.

## License

Licensed under the MIT license.
