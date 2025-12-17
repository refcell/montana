# VM

Block execution for Base stack chains.

## Overview

This crate provides a `BlockExecutor` that takes a database and chain specification, then executes blocks and their transactions using op-revm. It handles all Base stack-specific execution logic including deposit transactions and fee handling.

## Core Components

| Component | Description |
|-----------|-------------|
| `BlockExecutor` | Main executor for processing Base stack blocks |
| `BlockResult` | Result of executing an entire block |
| `TxResult` | Result of executing a single transaction |

## Usage

```rust,ignore
use execution::{BlockExecutor, BlockResult};
use chainspec::BASE_MAINNET;
use database::CachedDatabase;

// Create executor with database and chain spec
let executor = BlockExecutor::new(database, &BASE_MAINNET);

// Execute a block
let result: BlockResult = executor.execute_block(&block)?;

// Access transaction results
for tx_result in result.tx_results {
    println!("Gas used: {}", tx_result.gas_used);
}
```

## License

Licensed under the MIT license.
