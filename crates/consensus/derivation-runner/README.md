# montana-derivation-runner

Derivation pipeline runner for batch decompression and derivation.

## Overview

`montana-derivation-runner` provides the core derivation runner that orchestrates the L2 batch derivation pipeline. It:

- Polls for compressed batches from a configurable source
- Decompresses batches using pluggable decompression algorithms
- Derives L2 blocks from batch data
- Tracks latency between batch submission and derivation
- Reports metrics and statistics to the caller

## Core Components

| Component | Description |
|-----------|-------------|
| `DerivationConfig` | Configuration with builder pattern and sensible defaults |
| `DerivationRunner` | Core runner that polls, decompresses, and derives batches |
| `DerivationMetrics` | Observability metrics for tracking derivation performance |
| `DerivationError` | Comprehensive error types for derivation failures |

## Usage

```rust,no_run
use montana_derivation_runner::{DerivationConfig, DerivationRunner};
use std::time::Duration;

# async fn example() {
// Configure the derivation runner
let config = DerivationConfig::builder()
    .poll_interval_ms(100)
    .build();

// Create runner with your implementations of the pipeline traits
// let source = ...; // Your L1BatchSource implementation
// let compressor = ...; // Your Compressor implementation
// let mut runner = DerivationRunner::new(source, compressor, config);

// Run the derivation loop
// loop {
//     match runner.tick().await {
//         Ok(Some(metrics)) => {
//             println!("Derived batch: {} blocks, {} bytes",
//                 metrics.blocks_derived,
//                 metrics.bytes_decompressed);
//         }
//         Ok(None) => {
//             // No batch available this tick
//         }
//         Err(e) => {
//             eprintln!("Derivation error: {:?}", e);
//             // Handle error
//         }
//     }
// }
# }
```

## Configuration

Default configuration values:

| Setting | Default | Description |
|---------|---------|-------------|
| `poll_interval_ms` | 100 | Poll interval for checking new batches (ms) |

## Metrics

The `DerivationMetrics` struct tracks:

- **batches_derived**: Total number of batches successfully derived
- **blocks_derived**: Total number of L2 blocks derived
- **bytes_decompressed**: Total bytes decompressed
- **avg_latency_ms**: Average latency between submission and derivation

## Error Handling

Errors are classified for retry logic:

- **SourceError**: Failed to fetch batch from source
- **DecompressionFailed**: Failed to decompress batch data
- **EmptyBatch**: No batch available from source

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│                   DerivationRunner                               │
│  ┌─────────────────┐  ┌───────────────────────────────────────┐ │
│  │DerivationConfig │  │     DerivationMetrics                 │ │
│  └─────────────────┘  └───────────────────────────────────────┘ │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                  Pipeline Traits                          │   │
│  │  ┌────────────────┐ ┌────────────────────┐               │   │
│  │  │ L1BatchSource  │ │    Compressor      │               │   │
│  │  └────────────────┘ └────────────────────┘               │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## License

Licensed under the MIT license.
