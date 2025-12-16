# Montana: Minimal Batch Submission Pipeline

> A robust, trait-abstracted compression pipeline for L2 batch submission.

---

## Overview

Montana is a unidirectional data pipeline:

```
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  DataSource   │ ──▶  │  Compressor   │ ──▶  │  DataSink     │
│  (L2 Blocks)  │      │  (Brotli 11)  │      │  (L1 Blobs)   │
└───────────────┘      └───────────────┘      └───────────────┘
```

The inverse pipeline for derivation:

```
┌───────────────┐      ┌───────────────┐      ┌───────────────┐
│  DataSource   │ ──▶  │ Decompressor  │ ──▶  │  DataSink     │
│  (L1 Blobs)   │      │  (Brotli)     │      │  (L2 Blocks)  │
└───────────────┘      └───────────────┘      └───────────────┘
```

---

## Core Traits

### Data Input: `BatchSource`

Abstracts the source of L2 transaction data.

```rust
/// Raw transaction bytes (RLP-encoded, opaque to the pipeline).
#[derive(Clone, Debug)]
pub struct RawTransaction(pub Vec<u8>);

/// A block's worth of transactions with metadata.
#[derive(Clone, Debug)]
pub struct L2BlockData {
    pub timestamp: u64,
    pub transactions: Vec<RawTransaction>,
}

/// Source of L2 blocks to be batched.
///
/// Implementations:
/// - `RpcBlockSource`: Polls L2 execution client via JSON-RPC
/// - `EngineApiSource`: Receives blocks via Engine API
/// - `MockSource`: For testing
#[async_trait]
pub trait BatchSource: Send + Sync {
    /// Returns pending L2 blocks since last call.
    /// Returns empty vec if no new blocks.
    async fn pending_blocks(&mut self) -> Result<Vec<L2BlockData>, SourceError>;

    /// Current L1 origin block number for epoch reference.
    async fn l1_origin(&self) -> Result<u64, SourceError>;

    /// L1 origin block hash (first 20 bytes).
    async fn l1_origin_hash(&self) -> Result<[u8; 20], SourceError>;

    /// Parent L2 block hash (first 20 bytes).
    async fn parent_hash(&self) -> Result<[u8; 20], SourceError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("RPC connection failed: {0}")]
    Connection(String),
    #[error("No blocks available")]
    Empty,
    #[error("L1 origin unavailable")]
    NoOrigin,
}
```

### Data Output: `BatchSink`

Abstracts the destination for compressed batch data.

```rust
/// Compressed batch ready for submission.
#[derive(Clone, Debug)]
pub struct CompressedBatch {
    pub batch_number: u64,
    pub data: Vec<u8>,
}

/// Result of a successful submission.
#[derive(Clone, Debug)]
pub struct SubmissionReceipt {
    pub batch_number: u64,
    pub tx_hash: [u8; 32],
    pub l1_block: u64,
    pub blob_hash: Option<[u8; 32]>,
}

/// Sink for compressed batch data.
///
/// Implementations:
/// - `BlobSink`: Submits via EIP-4844 blob transactions
/// - `CalldataSink`: Submits via calldata (fallback)
/// - `FileSink`: Writes to disk (testing/debugging)
/// - `MultiSink`: Tries blob, falls back to calldata
#[async_trait]
pub trait BatchSink: Send + Sync {
    /// Submit a compressed batch. Blocks until confirmed.
    async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError>;

    /// Current submission capacity (e.g., blob gas available).
    async fn capacity(&self) -> Result<usize, SinkError>;

    /// Check if sink is healthy/connected.
    async fn health_check(&self) -> Result<(), SinkError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    #[error("L1 connection failed: {0}")]
    Connection(String),
    #[error("Transaction failed: {0}")]
    TxFailed(String),
    #[error("Insufficient funds")]
    InsufficientFunds,
    #[error("Blob gas too expensive: {current} > {max}")]
    BlobGasTooExpensive { current: u64, max: u64 },
    #[error("Timeout waiting for confirmation")]
    Timeout,
}
```

### Compression: `Compressor`

Abstracts the compression algorithm.

```rust
/// Compressor configuration.
#[derive(Clone, Debug)]
pub struct CompressionConfig {
    pub level: u32,        // 1-11 for Brotli
    pub window_size: u32,  // Log2 of window size
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            level: 11,       // Maximum compression
            window_size: 22, // 4MB window
        }
    }
}

/// Compresses raw batch data.
///
/// Implementations:
/// - `BrotliCompressor`: Brotli algorithm (default)
/// - `ZstdCompressor`: Alternative for testing
/// - `NoopCompressor`: Passthrough for debugging
pub trait Compressor: Send + Sync {
    /// Compress data. Must be deterministic.
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Decompress data. Must roundtrip with compress.
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Estimated compression ratio for capacity planning.
    fn estimated_ratio(&self) -> f64;
}

#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    #[error("Compression failed: {0}")]
    Failed(String),
    #[error("Decompression failed: corrupted data")]
    Corrupted,
    #[error("Input too large: {size} > {max}")]
    TooLarge { size: usize, max: usize },
}
```

---

## Wire Format

### Batch Encoding

```rust
/// Batch header: fixed 67 bytes.
/// All integers little-endian.
#[derive(Clone, Debug)]
pub struct BatchHeader {
    pub version: u8,           // 0x00
    pub batch_number: u64,     // Monotonic sequence
    pub l1_origin: u64,        // L1 block number (epoch)
    pub l1_origin_check: [u8; 20], // L1 block hash prefix
    pub parent_check: [u8; 20],    // Parent L2 block hash prefix
    pub timestamp: u64,        // First block timestamp
    pub block_count: u16,      // Number of L2 blocks
}

impl BatchHeader {
    pub const SIZE: usize = 1 + 8 + 8 + 20 + 20 + 8 + 2; // 67 bytes
}
```

### Encoding Trait

```rust
/// Encodes/decodes batches to wire format.
pub trait BatchCodec: Send + Sync {
    /// Encode blocks into uncompressed batch bytes.
    fn encode(
        &self,
        header: &BatchHeader,
        blocks: &[L2BlockData],
    ) -> Result<Vec<u8>, CodecError>;

    /// Decode uncompressed batch bytes into header + blocks.
    fn decode(&self, data: &[u8]) -> Result<(BatchHeader, Vec<L2BlockData>), CodecError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("Invalid version: {0}")]
    InvalidVersion(u8),
    #[error("Truncated data at offset {0}")]
    Truncated(usize),
    #[error("Invalid block count")]
    InvalidBlockCount,
}
```

### Binary Layout

```
Uncompressed Batch:
┌─────────────────────────────────────────────────────────────┐
│ Header (67 bytes)                                           │
├─────────────────────────────────────────────────────────────┤
│ Block 0: [timestamp_delta: u16] [tx_count: u16] [txs...]    │
│ Block 1: [timestamp_delta: u16] [tx_count: u16] [txs...]    │
│ ...                                                         │
└─────────────────────────────────────────────────────────────┘

Transaction encoding within block:
┌─────────────────────────────────────────────────────────────┐
│ [tx_len: u24] [tx_data: tx_len bytes]                       │
│ [tx_len: u24] [tx_data: tx_len bytes]                       │
│ ...                                                         │
└─────────────────────────────────────────────────────────────┘

Compressed Batch (wire format):
┌─────────────────────────────────────────────────────────────┐
│ [version: u8 = 0x00] [brotli_compressed_payload]            │
└─────────────────────────────────────────────────────────────┘
```

---

## Pipeline Implementation

### Batcher Pipeline

```rust
/// The complete batcher pipeline.
pub struct BatcherPipeline<S, C, K>
where
    S: BatchSource,
    C: Compressor,
    K: BatchSink,
{
    source: S,
    compressor: C,
    sink: K,
    codec: StandardCodec,
    config: PipelineConfig,
    state: PipelineState,
}

#[derive(Clone, Debug)]
pub struct PipelineConfig {
    pub batch_interval: Duration,
    pub max_batch_bytes: usize,      // Compressed size limit (128KB)
    pub min_batch_bytes: usize,      // Skip if smaller (1KB)
    pub max_blocks_per_batch: u16,   // Hard limit
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            batch_interval: Duration::from_secs(12),
            max_batch_bytes: 128 * 1024,
            min_batch_bytes: 1024,
            max_blocks_per_batch: 1000,
        }
    }
}

#[derive(Default)]
struct PipelineState {
    next_batch_number: u64,
    pending_blocks: Vec<L2BlockData>,
    last_submission: Option<Instant>,
}

impl<S, C, K> BatcherPipeline<S, C, K>
where
    S: BatchSource,
    C: Compressor,
    K: BatchSink,
{
    pub fn new(source: S, compressor: C, sink: K, config: PipelineConfig) -> Self {
        Self {
            source,
            compressor,
            sink,
            codec: StandardCodec,
            config,
            state: PipelineState::default(),
        }
    }

    /// Run one iteration of the pipeline.
    /// Call this in a loop with appropriate interval.
    pub async fn tick(&mut self) -> Result<Option<SubmissionReceipt>, PipelineError> {
        // 1. Collect pending blocks
        let new_blocks = self.source.pending_blocks().await?;
        self.state.pending_blocks.extend(new_blocks);

        // 2. Check if we should submit
        if self.state.pending_blocks.is_empty() {
            return Ok(None);
        }

        // 3. Build header
        let header = BatchHeader {
            version: 0x00,
            batch_number: self.state.next_batch_number,
            l1_origin: self.source.l1_origin().await?,
            l1_origin_check: self.source.l1_origin_hash().await?,
            parent_check: self.source.parent_hash().await?,
            timestamp: self.state.pending_blocks[0].timestamp,
            block_count: self.state.pending_blocks.len() as u16,
        };

        // 4. Encode
        let raw = self.codec.encode(&header, &self.state.pending_blocks)?;

        // 5. Compress
        let compressed = self.compressor.compress(&raw)?;

        // 6. Check size constraints
        if compressed.len() < self.config.min_batch_bytes {
            return Ok(None); // Accumulate more
        }

        if compressed.len() > self.config.max_batch_bytes {
            // Split and submit multiple batches
            return self.submit_split(raw).await;
        }

        // 7. Submit
        let batch = CompressedBatch {
            batch_number: self.state.next_batch_number,
            data: compressed,
        };

        let receipt = self.sink.submit(batch).await?;

        // 8. Update state
        self.state.next_batch_number += 1;
        self.state.pending_blocks.clear();
        self.state.last_submission = Some(Instant::now());

        Ok(Some(receipt))
    }
}
```

### Derivation Pipeline

```rust
/// Source of compressed batches from L1.
#[async_trait]
pub trait L1BatchSource: Send + Sync {
    /// Fetch next batch from L1. Returns None if caught up.
    async fn next_batch(&mut self) -> Result<Option<CompressedBatch>, SourceError>;

    /// Current L1 head block number.
    async fn l1_head(&self) -> Result<u64, SourceError>;
}

/// Sink for derived L2 blocks.
#[async_trait]
pub trait L2BlockSink: Send + Sync {
    /// Ingest a derived L2 block.
    async fn ingest(&mut self, block: L2BlockData) -> Result<(), SinkError>;
}

/// The derivation pipeline (inverse of batcher).
pub struct DerivationPipeline<S, C, K>
where
    S: L1BatchSource,
    C: Compressor,
    K: L2BlockSink,
{
    source: S,
    compressor: C,
    sink: K,
    codec: StandardCodec,
    last_batch: u64,
}

impl<S, C, K> DerivationPipeline<S, C, K>
where
    S: L1BatchSource,
    C: Compressor,
    K: L2BlockSink,
{
    /// Process next available batch.
    pub async fn step(&mut self) -> Result<bool, PipelineError> {
        let Some(compressed) = self.source.next_batch().await? else {
            return Ok(false); // No new batches
        };

        // Validate sequence
        if compressed.batch_number != self.last_batch + 1 {
            return Err(PipelineError::SequenceGap {
                expected: self.last_batch + 1,
                got: compressed.batch_number,
            });
        }

        // Decompress
        let raw = self.compressor.decompress(&compressed.data)?;

        // Decode
        let (header, blocks) = self.codec.decode(&raw)?;

        // Validate header
        self.validate_header(&header)?;

        // Emit blocks
        for block in blocks {
            self.sink.ingest(block).await?;
        }

        self.last_batch = compressed.batch_number;
        Ok(true)
    }

    fn validate_header(&self, header: &BatchHeader) -> Result<(), PipelineError> {
        if header.version != 0x00 {
            return Err(PipelineError::UnsupportedVersion(header.version));
        }
        // Additional validation: L1 origin within sequencing window, etc.
        Ok(())
    }
}
```

---

## Standard Implementations

### BrotliCompressor

```rust
pub struct BrotliCompressor {
    config: CompressionConfig,
}

impl BrotliCompressor {
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    pub fn max_compression() -> Self {
        Self::new(CompressionConfig::default())
    }
}

impl Compressor for BrotliCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut output = Vec::new();
        let params = brotli::enc::BrotliEncoderParams {
            quality: self.config.level as i32,
            lgwin: self.config.window_size as i32,
            ..Default::default()
        };

        brotli::enc::BrotliCompress(
            &mut std::io::Cursor::new(data),
            &mut output,
            &params,
        ).map_err(|e| CompressionError::Failed(e.to_string()))?;

        Ok(output)
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut output = Vec::new();
        brotli::BrotliDecompress(&mut std::io::Cursor::new(data), &mut output)
            .map_err(|_| CompressionError::Corrupted)?;
        Ok(output)
    }

    fn estimated_ratio(&self) -> f64 {
        6.0 // Conservative estimate for level 11
    }
}
```

### BlobSink

```rust
pub struct BlobSink {
    provider: L1Provider,
    signer: PrivateKey,
    inbox: Address,
    config: BlobSinkConfig,
}

#[derive(Clone, Debug)]
pub struct BlobSinkConfig {
    pub max_blob_fee_per_gas: u64,
    pub confirmation_blocks: u64,
    pub retry_attempts: u32,
    pub retry_delay: Duration,
}

#[async_trait]
impl BatchSink for BlobSink {
    async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        let mut attempts = 0;

        loop {
            match self.try_submit(&batch).await {
                Ok(receipt) => return Ok(receipt),
                Err(e) if attempts < self.config.retry_attempts => {
                    attempts += 1;
                    tracing::warn!(
                        attempt = attempts,
                        error = %e,
                        "Submission failed, retrying"
                    );
                    tokio::time::sleep(self.config.retry_delay).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn capacity(&self) -> Result<usize, SinkError> {
        // 128KB per blob, up to 6 blobs per block
        Ok(128 * 1024)
    }

    async fn health_check(&self) -> Result<(), SinkError> {
        self.provider.block_number().await
            .map(|_| ())
            .map_err(|e| SinkError::Connection(e.to_string()))
    }
}

impl BlobSink {
    async fn try_submit(&self, batch: &CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        // Pad to blob size
        let blob = self.pad_to_blob(&batch.data);
        let blob_hash = kzg_to_versioned_hash(&blob);

        // Build transaction
        let tx = TransactionRequest::new()
            .to(self.inbox)
            .data(batch.batch_number.to_le_bytes())
            .blob(blob)
            .max_fee_per_blob_gas(self.config.max_blob_fee_per_gas);

        // Sign and send
        let pending = self.provider.send_transaction(tx, &self.signer).await
            .map_err(|e| SinkError::TxFailed(e.to_string()))?;

        // Wait for confirmation
        let receipt = pending
            .confirmations(self.config.confirmation_blocks)
            .await
            .map_err(|_| SinkError::Timeout)?;

        Ok(SubmissionReceipt {
            batch_number: batch.batch_number,
            tx_hash: receipt.transaction_hash.into(),
            l1_block: receipt.block_number,
            blob_hash: Some(blob_hash),
        })
    }

    fn pad_to_blob(&self, data: &[u8]) -> [u8; 131072] {
        let mut blob = [0u8; 131072];
        blob[..data.len()].copy_from_slice(data);
        blob
    }
}
```

---

## Robustness

### Design Principles

1. **Fail-fast, recover-fast**: Errors surface immediately; recovery is automatic.
2. **Idempotent operations**: Resubmitting the same batch is safe (deduped by batch_number).
3. **No silent failures**: All errors are logged and surfaced to metrics.
4. **Graceful degradation**: Blob submission falls back to calldata.

### Error Handling Strategy

```rust
/// Errors that can be retried.
pub trait Retryable {
    fn is_retryable(&self) -> bool;
}

impl Retryable for SinkError {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            SinkError::Connection(_) | SinkError::Timeout
        )
    }
}

impl Retryable for SourceError {
    fn is_retryable(&self) -> bool {
        matches!(self, SourceError::Connection(_))
    }
}

/// Wrapper that adds retry logic to any sink.
pub struct RetryingSink<S: BatchSink> {
    inner: S,
    max_attempts: u32,
    base_delay: Duration,
}

#[async_trait]
impl<S: BatchSink> BatchSink for RetryingSink<S> {
    async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        let mut attempts = 0;
        let mut delay = self.base_delay;

        loop {
            match self.inner.submit(batch.clone()).await {
                Ok(receipt) => return Ok(receipt),
                Err(e) if e.is_retryable() && attempts < self.max_attempts => {
                    attempts += 1;
                    tracing::warn!(
                        attempt = attempts,
                        delay_ms = delay.as_millis(),
                        error = %e,
                        "Retrying submission"
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2; // Exponential backoff
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn capacity(&self) -> Result<usize, SinkError> {
        self.inner.capacity().await
    }

    async fn health_check(&self) -> Result<(), SinkError> {
        self.inner.health_check().await
    }
}
```

### Fallback Sink

```rust
/// Tries primary sink, falls back to secondary on failure.
pub struct FallbackSink<P: BatchSink, S: BatchSink> {
    primary: P,
    secondary: S,
}

#[async_trait]
impl<P: BatchSink, S: BatchSink> BatchSink for FallbackSink<P, S> {
    async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        match self.primary.submit(batch.clone()).await {
            Ok(receipt) => Ok(receipt),
            Err(primary_err) => {
                tracing::warn!(
                    error = %primary_err,
                    "Primary sink failed, trying secondary"
                );
                self.secondary.submit(batch).await.map_err(|secondary_err| {
                    tracing::error!(
                        primary = %primary_err,
                        secondary = %secondary_err,
                        "Both sinks failed"
                    );
                    secondary_err
                })
            }
        }
    }

    async fn capacity(&self) -> Result<usize, SinkError> {
        // Use primary's capacity
        self.primary.capacity().await
    }

    async fn health_check(&self) -> Result<(), SinkError> {
        // Both must be healthy
        self.primary.health_check().await?;
        self.secondary.health_check().await
    }
}
```

### Invariants

The pipeline maintains these invariants:

| Invariant | Enforcement |
|-----------|-------------|
| `batch_number` is monotonically increasing | State machine in pipeline |
| Compressed data roundtrips correctly | `Compressor::decompress(compress(x)) == x` |
| No data loss on crash | Pending blocks persisted before submission |
| L1 origin within sequencing window | Validated in derivation pipeline |

### Shutdown

```rust
impl<S, C, K> BatcherPipeline<S, C, K>
where
    S: BatchSource,
    C: Compressor,
    K: BatchSink,
{
    /// Graceful shutdown: submit any pending blocks.
    pub async fn shutdown(mut self) -> Result<(), PipelineError> {
        if !self.state.pending_blocks.is_empty() {
            tracing::info!(
                pending = self.state.pending_blocks.len(),
                "Submitting pending blocks before shutdown"
            );
            // Force submit regardless of min size
            self.force_submit().await?;
        }
        Ok(())
    }

    async fn force_submit(&mut self) -> Result<SubmissionReceipt, PipelineError> {
        // Same as tick() but ignores min_batch_bytes
        // ...
    }
}
```

---

## Constants

```rust
pub mod constants {
    use std::time::Duration;

    /// Maximum compressed batch size (1 blob).
    pub const MAX_BATCH_SIZE: usize = 128 * 1024;

    /// Minimum batch size to avoid dust submissions.
    pub const MIN_BATCH_SIZE: usize = 1024;

    /// Brotli compression level.
    pub const COMPRESSION_LEVEL: u32 = 11;

    /// Brotli window size (log2).
    pub const COMPRESSION_WINDOW: u32 = 22;

    /// Batch submission interval.
    pub const BATCH_INTERVAL: Duration = Duration::from_secs(12);

    /// Sequencing window in L1 blocks.
    pub const SEQUENCING_WINDOW: u64 = 3600;

    /// L1 confirmations for safe head.
    pub const SAFE_CONFIRMATIONS: u64 = 12;

    /// Wire format version.
    pub const BATCH_VERSION: u8 = 0x00;
}
```

---

## Comparison

| Aspect | OP Stack | Montana |
|--------|----------|---------|
| Abstractions | Channels, Frames, Spans | `BatchSource`, `BatchSink`, `Compressor` |
| State | Channel lifecycle, frame reassembly | Single `batch_number` counter |
| Compression | Configurable algo/level | Fixed Brotli 11 |
| Error handling | Complex retry/timeout logic | Trait-based `Retryable` + `FallbackSink` |
| Testability | Requires full stack | Mock traits in isolation |
| LoC | ~15,000 | ~2,000 |

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockSource {
        blocks: Vec<L2BlockData>,
    }

    #[async_trait]
    impl BatchSource for MockSource {
        async fn pending_blocks(&mut self) -> Result<Vec<L2BlockData>, SourceError> {
            Ok(std::mem::take(&mut self.blocks))
        }
        // ... other methods
    }

    struct MockSink {
        submissions: Vec<CompressedBatch>,
    }

    #[async_trait]
    impl BatchSink for MockSink {
        async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
            self.submissions.push(batch.clone());
            Ok(SubmissionReceipt {
                batch_number: batch.batch_number,
                tx_hash: [0u8; 32],
                l1_block: 1,
                blob_hash: None,
            })
        }
        // ... other methods
    }

    #[tokio::test]
    async fn test_roundtrip() {
        let blocks = vec![L2BlockData {
            timestamp: 1000,
            transactions: vec![RawTransaction(vec![1, 2, 3, 4])],
        }];

        let compressor = BrotliCompressor::max_compression();
        let codec = StandardCodec;

        // Encode
        let header = BatchHeader { /* ... */ };
        let raw = codec.encode(&header, &blocks).unwrap();

        // Compress
        let compressed = compressor.compress(&raw).unwrap();

        // Decompress
        let decompressed = compressor.decompress(&compressed).unwrap();

        // Decode
        let (decoded_header, decoded_blocks) = codec.decode(&decompressed).unwrap();

        assert_eq!(blocks.len(), decoded_blocks.len());
        assert_eq!(blocks[0].transactions[0].0, decoded_blocks[0].transactions[0].0);
    }
}
```

---

*Montana: Traits in, blobs out.*
