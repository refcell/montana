# montana-batcher

Batcher service for L2 batch submission orchestration.

## Overview

`montana-batcher` provides the core batcher service that orchestrates the L2 batch submission pipeline. It:

- Collects L2 blocks from a configurable source
- Batches blocks according to configurable strategy (size, time, block count)
- Compresses batches using pluggable compression algorithms
- Submits to L1 via the transaction manager

## Core Components

| Component | Description |
|-----------|-------------|
| `BatcherConfig` | Configuration with builder pattern and sensible defaults |
| `BatcherService<S,C,K>` | Core service generic over pipeline traits (BatchSource, Compressor, BatchSink) |
| `BatchDriver` | Batching strategy and decision logic - accumulates blocks and decides when to submit |
| `PendingBatch` | A batch ready for submission with blocks and size metadata |
| `BatcherState` | Current service state (last batch number, L1 block, health, totals) |
| `BatcherMetrics` | Observability metrics (stub implementation) |
| `BatcherError` | Comprehensive error types with retry classification |

## Usage

```rust,ignore
use montana_batcher::{BatcherConfig, BatcherService};
use std::time::Duration;

// Configure the batcher
let config = BatcherConfig::builder()
    .max_batch_size(128 * 1024)  // 128 KB
    .batch_interval(Duration::from_secs(12))
    .use_blobs(true)
    .build();

// Create service with your implementations of the pipeline traits
let mut service = BatcherService::new(source, compressor, sink, config);

// Run the service loop
loop {
    match service.tick().await {
        Ok(Some(receipt)) => {
            println!("Submitted batch {}", receipt.batch_number);
        }
        Ok(None) => {
            // No batch submitted this tick
        }
        Err(e) => {
            eprintln!("Error: {:?}", e);
            // Handle error based on whether it's retryable
        }
    }

    tokio::time::sleep(Duration::from_millis(100)).await;
}
```

## Configuration

Default configuration values:

| Setting | Default | Description |
|---------|---------|-------------|
| `max_batch_size` | 128 KB | Maximum batch size (before compression) |
| `max_blocks_per_batch` | 100 | Maximum blocks per batch |
| `batch_interval` | 12s | Target interval between submissions |
| `min_batch_size` | 1 KB | Minimum batch size before submission |
| `use_blobs` | true | Use EIP-4844 blob transactions |
| `max_blob_fee_per_gas` | 10 gwei | Maximum blob gas price |
| `max_blobs_per_tx` | 6 | Maximum blobs per transaction |
| `num_confirmations` | 10 | Required L1 confirmations |
| `max_l1_drift` | 30 blocks | Maximum L1 blocks behind |

## Batching Strategy

The `BatchDriver` determines when to submit a batch based on:

1. **Size threshold**: Accumulated block data exceeds `min_batch_size`
2. **Time threshold**: Time since last submission exceeds `batch_interval`
3. **Block count threshold**: Pending blocks exceed `max_blocks_per_batch`

## Error Handling

Errors are classified for retry logic:

- **Retryable**: `SourceUnavailable`, `SubmissionFailed`, `BlobGasTooExpensive`, `ConfirmationTimeout`
- **Fatal**: `SequencingWindowExpired`, `L1DriftExceeded`

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│                      BatcherService                              │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │BatcherConfig│  │ BatchDriver │  │    BatcherMetrics       │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                  Pipeline Traits                          │   │
│  │  ┌───────────┐ ┌───────────┐ ┌───────────┐               │   │
│  │  │BatchSource│ │Compressor │ │ BatchSink │               │   │
│  │  └───────────┘ └───────────┘ └───────────┘               │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## License

Licensed under the MIT license.
