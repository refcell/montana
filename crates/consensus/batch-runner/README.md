# montana-batch-runner

Batch submission runner for L2 block streaming and batch orchestration.

## Overview

`montana-batch-runner` provides a reusable batch submission runner that orchestrates
the L2 batch submission pipeline. It encapsulates the logic for:

- Streaming blocks from a block source
- Accumulating blocks into batches based on configurable thresholds
- Encoding and compressing batch data
- Submitting batches through a pluggable sink
- Tracking metrics and statistics

This crate extracts and generalizes the batch submission logic from the shadow
binary, making it reusable across different contexts.

## Core Components

| Component | Description |
|-----------|-------------|
| `BatchSubmissionRunner` | Main orchestrator that manages the batch submission loop |
| `BatchSubmissionConfig` | Configuration with builder pattern and sensible defaults |
| `BatchSubmissionMetrics` | Real-time metrics tracking for submissions |
| `BatchSubmissionError` | Comprehensive error types for batch submission failures |

## Usage

```rust,ignore
use montana_batch_runner::{BatchSubmissionRunner, BatchSubmissionConfig};

// Configure the runner
let config = BatchSubmissionConfig::builder()
    .max_blocks_per_batch(10)
    .target_batch_size(128 * 1024)
    .poll_interval_ms(100)
    .build();

// Create runner with your implementations
let mut runner = BatchSubmissionRunner::new(
    block_source,
    compressor,
    batch_sink,
    config,
);

// Run the batch submission loop
runner.run(start_block).await?;
```

## Configuration

Default configuration values:

| Setting | Default | Description |
|---------|---------|-------------|
| `max_blocks_per_batch` | 10 | Maximum blocks to accumulate before submission |
| `target_batch_size` | 128 KB | Target batch size in bytes (uncompressed) |
| `poll_interval_ms` | 100 | Interval between polls in milliseconds |

## Metrics

The runner tracks the following metrics:

- `batches_submitted`: Total number of batches submitted
- `blocks_processed`: Total number of L2 blocks processed
- `bytes_original`: Total uncompressed batch data
- `bytes_compressed`: Total compressed batch data
- `latest_block`: Latest block number processed
- `avg_latency_ms`: Average submission latency

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                BatchSubmissionRunner                         │
│  ┌────────────────┐  ┌────────────┐  ┌──────────────────┐   │
│  │BatchSubmission │  │Compressor  │  │   BatchSink      │   │
│  │    Config      │  │            │  │                  │   │
│  └────────────────┘  └────────────┘  └──────────────────┘   │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │         Batch Accumulation & Submission Loop           │  │
│  │  1. Fetch block from source                            │  │
│  │  2. Accumulate until threshold reached                 │  │
│  │  3. Encode blocks into raw batch                       │  │
│  │  4. Compress batch data                                │  │
│  │  5. Submit through sink                                │  │
│  │  6. Update metrics                                     │  │
│  └────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Error Handling

Errors are classified for appropriate handling:

- **Source Errors**: Connection failures, timeouts, block not found
- **Encoding Errors**: Invalid block data
- **Compression Errors**: Compression algorithm failures
- **Sink Errors**: Submission failures, transaction errors

## License

Licensed under the MIT license.
