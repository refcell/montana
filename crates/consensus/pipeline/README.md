# montana-pipeline

Core pipeline traits and types for Montana.

A robust, trait-abstracted compression pipeline for L2 batch submission and derivation.

## Architecture

Montana implements a **three-stage, trait-abstracted duplex pipeline**:

### Batch Submission (L2 → L1)

```
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  BatchSource  │ ──▶  │  Compressor   │ ──▶  │  BatchSink    │
│  (L2 Blocks)  │      │  (Brotli 11)  │      │  (L1 Blobs)   │
└───────────────┘      └───────────────┘      └───────────────┘
```

### Derivation (L1 → L2)

```
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│ L1BatchSource │ ──▶  │ Decompressor  │ ──▶  │ L2BlockSink   │
│  (L1 Blobs)   │      │  (Brotli)     │      │  (L2 Blocks)  │
└───────────────┘      └───────────────┘      └───────────────┘
```

## Core Traits

| Trait | Purpose | Key Methods |
|-------|---------|-------------|
| `BatchSource` | Provides L2 blocks to batch | `pending_blocks()`, `l1_origin()`, `l1_origin_hash()`, `parent_hash()` |
| `Compressor` | Compress/decompress data | `compress()`, `decompress()`, `estimated_ratio()` |
| `BatchSink` | Submits compressed batches | `submit()`, `capacity()`, `health_check()` |
| `BatchCodec` | Encode/decode wire format | `encode()`, `decode()` |
| `L1BatchSource` | Fetches batches from L1 | `next_batch()`, `l1_head()` |
| `L2BlockSink` | Ingests derived L2 blocks | `ingest()` |

## Pipeline Configuration

The pipeline configuration is not exported directly from this crate. Configuration is typically done through implementations that use these traits. For example, the `montana-batcher` crate provides `BatcherConfig` for configuring batch submission pipelines.

Compression is configured separately:

```rust
pub struct CompressionConfig {
    pub level: u32,        // 1-11 for Brotli (default: 11, maximum)
    pub window_size: u32,  // Log2 of window size (default: 22 = 4MB)
}
```

## Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `MAX_BATCH_SIZE` | 128 KB | Maximum compressed batch size (fits in 1 blob) |
| `MIN_BATCH_SIZE` | 1 KB | Minimum batch size to avoid dust submissions |
| `COMPRESSION_LEVEL` | 11 | Brotli maximum compression level |
| `COMPRESSION_WINDOW` | 22 | 4MB sliding window for Brotli |
| `BATCH_INTERVAL` | 12 seconds | Aligned with L1 block time |
| `SEQUENCING_WINDOW` | 3600 L1 blocks | ~12 hours of L1 blocks |
| `SAFE_CONFIRMATIONS` | 12 L1 blocks | Required confirmations for safe head |

## Batching Model

### Trait-Based Architecture

The pipeline provides core traits for implementing batching strategies:

- `BatchSource` provides pending L2 blocks
- `Compressor` handles compression/decompression
- `BatchCodec` encodes blocks into wire format
- `BatchSink` submits compressed batches to L1

Implementations using these traits (like `montana-batcher`) typically follow a polling accumulation model where batches are submitted based on:
- **Time-based**: A configured batch interval (e.g., 12 seconds)
- **Size-based**: When accumulated data reaches minimum thresholds

The trait design allows for different batching strategies and submission patterns.

### Streaming Considerations

The current architecture supports continuous data feeding but uses **batch processing** semantics. For true streaming with target size control, the following enhancements would be needed:

#### What's Missing for Streaming

1. **Target Size Configuration**: The current design has `min` and `max` but no `target_batch_bytes` to aim for a specific output size.

2. **Predictive Batching**: Logic to estimate when to "cut" a batch before it exceeds limits:
   ```rust
   let estimated_compressed = uncompressed_size as f64 / compressor.estimated_ratio();
   if estimated_compressed >= target_batch_bytes {
       // Cut batch here
   }
   ```

3. **Incremental Compression** (optional): Brotli's streaming API could provide intermediate size estimates for better prediction.

#### Proposed Streaming Configuration

```rust
pub struct StreamingPipelineConfig {
    pub target_batch_bytes: usize,    // Target compressed size (e.g., 100KB)
    pub max_batch_bytes: usize,       // Hard limit (128KB)
    pub min_batch_bytes: usize,       // Minimum to submit (1KB)
    pub max_blocks_per_batch: u16,
    pub max_wait_time: Duration,      // Force submit after this time
}
```

#### What Works Today

The trait abstractions are well-suited for streaming:
- Implement a streaming `BatchSource` that yields blocks as they arrive
- The `Compressor::estimated_ratio()` method exists for size prediction
- The pipeline's accumulation loop can be adapted for target-based cutting

## Wire Format

### Batch Header (67 bytes)

```
┌─────────────────────────────────────────────────────────────┐
│ version (1B) | batch_number (8B) | l1_origin (8B)           │
│ l1_origin_check (20B) | parent_check (20B)                  │
│ timestamp (8B) | block_count (2B)                           │
└─────────────────────────────────────────────────────────────┘
```

### Batch Body

```
┌─────────────────────────────────────────────────────────────┐
│ Block 0: [timestamp_delta: u16] [tx_count: u16] [txs...]    │
│ Block 1: [timestamp_delta: u16] [tx_count: u16] [txs...]    │
│ ...                                                         │
└─────────────────────────────────────────────────────────────┘
```

Each transaction: `[tx_len: u24] [tx_data: tx_len bytes]`

### Compressed Wire Format

```
┌─────────────────────────────────────────────────────────────┐
│ [version: u8 = 0x00] [brotli_compressed_payload]            │
└─────────────────────────────────────────────────────────────┘
```

## Error Handling

The pipeline uses comprehensive error types for each component:

- `SourceError`: Connection failures, empty results, missing L1 origin
- `SinkError`: L1 connection failures, transaction failures, insufficient funds, blob gas pricing, timeouts
- `CompressionError`: Compression failures, corrupted data, size limits
- `CodecError`: Invalid versions, truncated data, invalid block counts
- `PipelineError`: Wrapper combining all error types

Implementations can determine retry strategies based on error types. For example, connection failures and timeout errors are typically retryable, while insufficient funds or sequencing window expiration are not.

## Usage

This crate provides the core traits for building pipelines. Here's an example of implementing a custom source:

```rust
use montana_pipeline::{BatchSource, L2BlockData, SourceError};
use async_trait::async_trait;

struct MyBlockSource {
    // Your implementation details
}

#[async_trait]
impl BatchSource for MyBlockSource {
    async fn pending_blocks(&mut self) -> Result<Vec<L2BlockData>, SourceError> {
        // Fetch blocks from your L2 execution client
        todo!()
    }

    async fn l1_origin(&self) -> Result<u64, SourceError> {
        // Return current L1 origin block number
        todo!()
    }

    async fn l1_origin_hash(&self) -> Result<[u8; 20], SourceError> {
        // Return L1 origin block hash prefix
        todo!()
    }

    async fn parent_hash(&self) -> Result<[u8; 20], SourceError> {
        // Return parent L2 block hash prefix
        todo!()
    }
}
```

For a complete batching system, see the `montana-batcher` crate which provides a service that orchestrates these traits.

## License

MIT
