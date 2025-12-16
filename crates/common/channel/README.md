# channels

Channels abstractions for batch source data flow.

## Overview

This crate provides the core abstractions for sourcing L2 block data that will be batched and submitted to L1. It defines the traits and types used throughout the Montana pipeline.

## Core Types

| Type | Description |
|------|-------------|
| `BatchSource` | Async trait for sourcing L2 blocks for batching |
| `L2BlockData` | Block data containing timestamp and transactions |
| `RawTransaction` | Raw transaction bytes wrapper |
| `SourceError` | Error types for source operations |

## Usage

```rust,ignore
use channels::{BatchSource, L2BlockData, SourceError};
use async_trait::async_trait;

struct MyBlockSource;

#[async_trait]
impl BatchSource for MyBlockSource {
    async fn next_block(&mut self) -> Result<Option<L2BlockData>, SourceError> {
        // Fetch next block from your data source
        Ok(None)
    }
}
```

## License

Licensed under the MIT license.
