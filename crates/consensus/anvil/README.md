# montana-anvil

Anvil integration for Montana - Local chain management for testing and development.

This crate provides integration with [Anvil](https://github.com/foundry-rs/foundry/tree/master/crates/anvil),
a local Ethereum node for testing and development.

## Overview

The main components are:

- `AnvilManager`: Manages the lifecycle of an Anvil instance
- `AnvilBatchSink`: Submits compressed batches as transactions to Anvil
- `AnvilBatchSource`: Reads batches from Anvil blocks

## Usage

```rust,ignore
use montana_anvil::{AnvilConfig, AnvilManager};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Spawn Anvil with default config
    let anvil = AnvilManager::spawn(AnvilConfig::default()).await?;

    // Get sink and source for batch operations
    let mut sink = anvil.sink();
    let mut source = anvil.source();

    println!("Anvil running at: {}", anvil.endpoint());
    Ok(())
}
```

## License

Licensed under the MIT License.
