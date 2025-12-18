# montana-batch-context

Batch sink and source implementations for different submission modes.

## Overview

This crate provides the abstraction layer for submitting batches and retrieving them in different modes:

- **In-Memory**: Direct batch passing between tasks (fast, no chain simulation)
- **Anvil**: Spawns a local Anvil chain and submits batches as transactions
- **Remote**: Submit to a remote L1 chain (currently unsupported)

## Core Components

| Component | Description |
|-----------|-------------|
| `BatchContext` | Factory for creating sinks and sources for the current mode |
| `BatchSink` | Trait for submitting batches to a destination |
| `BatchSource` | Trait for retrieving batches from a source |
| `InMemoryBatchQueue` | In-memory batch queue for direct batch passing |

## Usage

```rust,ignore
use montana_batch_context::{BatchContext, BatchSubmissionMode, Address};

// Create batch context for Anvil mode
let batch_inbox = Address::repeat_byte(0x42);
let ctx = BatchContext::new(BatchSubmissionMode::Anvil, batch_inbox).await?;

// Get sink and source
let sink = ctx.sink();
let source = ctx.source();

// Submit a batch
sink.submit(batch).await?;

// Retrieve a batch
let batch = source.next_batch().await?;
```

## Submission Modes

| Mode | Description |
|------|-------------|
| `InMemory` | Passes batches directly between tasks via in-memory queue |
| `Anvil` | Spawns local Anvil instance, submits as transactions |
| `Remote` | Submit to remote L1 (currently unsupported) |

## License

Licensed under the MIT license.
