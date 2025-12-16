# runner

Block execution runner for Optimism (Base) blocks.

This crate provides functionality for executing Optimism (Base) blocks using op-revm with an in-memory database that falls back to RPC for missing state.

## Usage

```rust,ignore
use runner::{Execution, ProducerMode};
use std::time::Duration;

// Live mode - poll for new blocks
let mode = ProducerMode::Live {
    poll_interval: Duration::from_secs(2),
    start_block: None,
};

let execution = Execution::new("https://mainnet.base.org".to_string(), mode);
execution.start().await?;

// Historical mode - fetch a range of blocks
let mode = ProducerMode::Historical {
    start: 1000000,
    end: 1000010,
};

let execution = Execution::new("https://mainnet.base.org".to_string(), mode);
execution.start().await?;
```
