# montana-adapters

Adapter types for the Montana binary.

## Overview

This crate provides adapter types that bridge different trait interfaces between Montana's consensus, node, and utility crates. These adapters enable interoperability between components that use different but similar trait definitions.

## Core Components

| Component | Description |
|-----------|-------------|
| `BlockProducerWrapper` | Boxed block producer with type erasure |
| `BatchSinkAdapter` | Bridges `montana_batch_context::BatchSink` to `montana_pipeline::BatchSink` |
| `BatchSourceAdapter` | Bridges `BatchContext` source to `montana_pipeline::L1BatchSource` |
| `TuiExecutionCallback` | Sends execution events to TUI |
| `TuiBatchCallback` | Sends batch submission events to TUI |
| `TuiDerivationCallback` | Sends derivation events to TUI |
| `HarnessProgressAdapter` | Bridges harness progress to TUI events |

## Usage

```rust,ignore
use montana_adapters::{BatchSinkAdapter, TuiExecutionCallback};

// Create a batch sink adapter
let sink_adapter = BatchSinkAdapter::new(batch_context.sink());

// Create TUI callbacks
let exec_callback = TuiExecutionCallback::new(tui_handle.clone());
let batch_callback = TuiBatchCallback::new(tui_handle.clone());
```

## Purpose

These adapters solve the "trait mismatch" problem where:
- Different crates define similar traits (e.g., `BatchSink`)
- Components need to work together across crate boundaries
- Type erasure is needed for dynamic dispatch

## License

Licensed under the MIT license.
