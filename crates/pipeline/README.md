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

The pipeline is configured via `PipelineConfig`:

```rust
pub struct PipelineConfig {
    pub batch_interval: Duration,     // Default: 12 seconds (L1 block time)
    pub max_batch_bytes: usize,       // Default: 128 KB (compressed, fits 1 blob)
    pub min_batch_bytes: usize,       // Default: 1 KB (avoid dust submissions)
    pub max_blocks_per_batch: u16,    // Default: 1000
}
```

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

### Current: Polling Accumulation

The pipeline uses a **polling accumulation model** via the `tick()` method:

```rust
impl BatcherPipeline {
    pub async fn tick(&mut self) -> Result<Option<SubmissionReceipt>, PipelineError> {
        // 1. Collect pending blocks from source
        let new_blocks = self.source.pending_blocks().await?;
        self.state.pending_blocks.extend(new_blocks);

        // 2. Encode and compress
        let raw = self.codec.encode(&header, &self.state.pending_blocks)?;
        let compressed = self.compressor.compress(&raw)?;

        // 3. Check size constraints
        if compressed.len() < self.config.min_batch_bytes {
            return Ok(None); // Accumulate more
        }
        if compressed.len() > self.config.max_batch_bytes {
            return self.submit_split(raw).await; // Split into multiple batches
        }

        // 4. Submit batch
        self.sink.submit(batch).await
    }
}
```

Batches are submitted when:
- **Time-based**: The batch interval (12 seconds) has elapsed
- **Size-based**: Compressed data exceeds `min_batch_bytes` (1 KB)

If compressed data exceeds `max_batch_bytes` (128 KB), the batch is split.

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

The pipeline uses trait-based error handling with retry support:

```rust
pub trait Retryable {
    fn is_retryable(&self) -> bool;
}
```

Combinators for robustness:
- `RetryingSink<S>`: Adds exponential backoff retry logic
- `FallbackSink<P, S>`: Tries primary sink, falls back to secondary

## Usage

```rust
use montana_pipeline::{
    BatcherPipeline, PipelineConfig,
    BrotliCompressor, CompressionConfig,
};

// Create pipeline components
let source = MyBlockSource::new();
let compressor = BrotliCompressor::max_compression();
let sink = MyBlobSink::new();

// Configure and run
let config = PipelineConfig::default();
let mut pipeline = BatcherPipeline::new(source, compressor, sink, config);

// Run in a loop
loop {
    if let Some(receipt) = pipeline.tick().await? {
        println!("Submitted batch {}", receipt.batch_number);
    }
    tokio::time::sleep(config.batch_interval).await;
}
```

## License

MIT
