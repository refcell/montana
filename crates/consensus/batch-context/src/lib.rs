#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::sync::Arc;

use async_trait::async_trait;
use montana_anvil::{AnvilConfig, AnvilManager};
use montana_pipeline::{
    BatchSink as PipelineBatchSink, CompressedBatch, L1BatchSource as PipelineL1BatchSource,
    SinkError as PipelineSinkError, SourceError as PipelineSourceError, SubmissionReceipt,
};
use tokio::sync::Mutex;
// Re-export BatchSubmissionMode from montana_batcher
pub use montana_batcher::BatchSubmissionMode;
// Re-export Address for convenience
pub use montana_anvil::Address;

/// Error type for batch sink operations.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// Connection error.
    #[error("Connection error: {0}")]
    Connection(String),
    /// Transaction failed.
    #[error("Transaction failed: {0}")]
    TxFailed(String),
    /// Mode not supported.
    #[error("Mode not supported: {0}")]
    Unsupported(String),
}

impl From<PipelineSinkError> for SinkError {
    fn from(err: PipelineSinkError) -> Self {
        match err {
            PipelineSinkError::Connection(msg) => Self::Connection(msg),
            PipelineSinkError::TxFailed(msg) => Self::TxFailed(msg),
            PipelineSinkError::Timeout => Self::TxFailed("Timeout".to_string()),
            PipelineSinkError::InsufficientFunds => {
                Self::TxFailed("Insufficient funds".to_string())
            }
            PipelineSinkError::BlobGasTooExpensive { max, current } => {
                Self::TxFailed(format!("Blob gas too expensive: max={}, current={}", max, current))
            }
        }
    }
}

/// Error type for batch source operations.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// Connection error.
    #[error("Connection error: {0}")]
    Connection(String),
    /// No batches available.
    #[error("No batches available")]
    Empty,
}

impl From<PipelineSourceError> for SourceError {
    fn from(err: PipelineSourceError) -> Self {
        match err {
            PipelineSourceError::Connection(msg) => Self::Connection(msg),
            PipelineSourceError::Empty => Self::Empty,
        }
    }
}

// ============================================================================
// Batch Sink/Source Traits
// ============================================================================

/// Trait for submitting batches to a destination.
#[async_trait]
pub trait BatchSink: Send + Sync {
    /// Submit a batch.
    async fn submit(&self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError>;
}

/// Trait for retrieving batches from a source.
///
/// Uses `&self` to allow shared access through interior mutability.
#[async_trait]
pub trait BatchSource: Send + Sync {
    /// Get the next batch, if available.
    async fn next_batch(&self) -> Result<Option<CompressedBatch>, SourceError>;

    /// Current L1 head block number.
    async fn l1_head(&self) -> Result<u64, SourceError>;
}

// ============================================================================
// In-Memory Implementation
// ============================================================================

/// In-memory batch queue for direct batch passing.
///
/// This implementation provides the current behavior where batches are
/// passed directly from batch submission to derivation via an in-memory queue.
#[derive(Debug)]
pub struct InMemoryBatchQueue {
    /// Queue of pending batches.
    batches: Arc<Mutex<Vec<CompressedBatch>>>,
}

impl InMemoryBatchQueue {
    /// Create a new in-memory batch queue.
    pub fn new() -> Self {
        Self { batches: Arc::new(Mutex::new(Vec::new())) }
    }

    /// Get a source that reads from this queue.
    pub fn source(&self) -> InMemoryBatchSource {
        InMemoryBatchSource { batches: Arc::clone(&self.batches) }
    }
}

impl Default for InMemoryBatchQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BatchSink for InMemoryBatchQueue {
    async fn submit(&self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        let batch_number = batch.batch_number;
        let mut guard = self.batches.lock().await;
        guard.push(batch);
        Ok(SubmissionReceipt { batch_number, tx_hash: [0u8; 32], l1_block: 0, blob_hash: None })
    }
}

/// Source that reads from an in-memory batch queue.
#[derive(Debug)]
pub struct InMemoryBatchSource {
    /// Shared queue with the sink.
    batches: Arc<Mutex<Vec<CompressedBatch>>>,
}

#[async_trait]
impl BatchSource for InMemoryBatchSource {
    async fn next_batch(&self) -> Result<Option<CompressedBatch>, SourceError> {
        let mut guard = self.batches.lock().await;
        if guard.is_empty() { Ok(None) } else { Ok(Some(guard.remove(0))) }
    }

    async fn l1_head(&self) -> Result<u64, SourceError> {
        Ok(0)
    }
}

// ============================================================================
// Anvil Implementation (using montana-anvil crate)
// ============================================================================

/// Wrapper around AnvilBatchSink from the crate.
pub struct AnvilBatchSinkWrapper {
    inner: Mutex<montana_anvil::AnvilBatchSink>,
}

impl AnvilBatchSinkWrapper {
    fn new(sink: montana_anvil::AnvilBatchSink) -> Self {
        Self { inner: Mutex::new(sink) }
    }
}

impl std::fmt::Debug for AnvilBatchSinkWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnvilBatchSinkWrapper").finish()
    }
}

#[async_trait]
impl BatchSink for AnvilBatchSinkWrapper {
    async fn submit(&self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        let mut guard = self.inner.lock().await;
        PipelineBatchSink::submit(&mut *guard, batch).await.map_err(SinkError::from)
    }
}

/// Wrapper around AnvilBatchSource from the crate.
pub struct AnvilBatchSourceWrapper {
    inner: Mutex<montana_anvil::AnvilBatchSource>,
}

impl AnvilBatchSourceWrapper {
    fn new(source: montana_anvil::AnvilBatchSource) -> Self {
        Self { inner: Mutex::new(source) }
    }
}

impl std::fmt::Debug for AnvilBatchSourceWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnvilBatchSourceWrapper").finish()
    }
}

#[async_trait]
impl BatchSource for AnvilBatchSourceWrapper {
    async fn next_batch(&self) -> Result<Option<CompressedBatch>, SourceError> {
        let mut guard = self.inner.lock().await;
        PipelineL1BatchSource::next_batch(&mut *guard).await.map_err(SourceError::from)
    }

    async fn l1_head(&self) -> Result<u64, SourceError> {
        let guard = self.inner.lock().await;
        PipelineL1BatchSource::l1_head(&*guard).await.map_err(SourceError::from)
    }
}

// ============================================================================
// Batch Context Factory
// ============================================================================

/// Context holding the batch sink and source for the current mode.
pub struct BatchContext {
    /// The batch sink for submission.
    sink: Arc<Box<dyn BatchSink>>,
    /// The batch source for derivation.
    source: Arc<Box<dyn BatchSource>>,
    /// Anvil manager (kept alive for the duration of the simulation).
    #[allow(dead_code)]
    anvil: Option<AnvilManager>,
    /// The submission mode.
    mode: BatchSubmissionMode,
}

impl BatchContext {
    /// Create a new batch context for the given mode.
    ///
    /// The `batch_inbox` parameter specifies the address where batches are sent.
    /// Both the sink and source use this address to ensure consistency.
    pub async fn new(mode: BatchSubmissionMode, batch_inbox: Address) -> Result<Self, SinkError> {
        match mode {
            BatchSubmissionMode::InMemory => {
                let queue = InMemoryBatchQueue::new();
                let source = queue.source();
                Ok(Self {
                    sink: Arc::new(Box::new(queue)),
                    source: Arc::new(Box::new(source)),
                    anvil: None,
                    mode,
                })
            }
            BatchSubmissionMode::Anvil => {
                let config = AnvilConfig { batch_inbox, ..Default::default() };
                let anvil = AnvilManager::spawn(config)
                    .await
                    .map_err(|e| SinkError::Connection(e.to_string()))?;
                let sink = AnvilBatchSinkWrapper::new(anvil.sink());
                let source = AnvilBatchSourceWrapper::new(anvil.source());
                Ok(Self {
                    sink: Arc::new(Box::new(sink)),
                    source: Arc::new(Box::new(source)),
                    anvil: Some(anvil),
                    mode,
                })
            }
            BatchSubmissionMode::Remote => Err(SinkError::Unsupported(
                "Remote batch submission mode is currently unsupported.".to_string(),
            )),
        }
    }

    /// Get an Arc clone of the sink (for sharing).
    pub fn sink_arc(&self) -> Arc<Box<dyn BatchSink>> {
        Arc::clone(&self.sink)
    }

    /// Get an Arc clone of the sink.
    ///
    /// Alias for `sink_arc()` for convenience.
    pub fn sink(&self) -> Arc<Box<dyn BatchSink>> {
        self.sink_arc()
    }

    /// Get a reference to the source.
    pub fn source(&self) -> &dyn BatchSource {
        self.source.as_ref().as_ref()
    }

    /// Get the Anvil endpoint URL (if in Anvil mode).
    pub fn anvil_endpoint(&self) -> Option<String> {
        self.anvil.as_ref().map(|a| a.endpoint().to_string())
    }

    /// Get the submission mode.
    pub const fn mode(&self) -> BatchSubmissionMode {
        self.mode
    }
}

impl std::fmt::Debug for BatchContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchContext")
            .field("mode", &self.mode)
            .field("anvil_endpoint", &self.anvil_endpoint())
            .finish()
    }
}

// ============================================================================
// L1BatchSource Adapter
// ============================================================================

/// Adapter that wraps BatchContext source to implement L1BatchSource.
#[derive(Debug)]
pub struct L1BatchSourceAdapter<'a> {
    context: &'a BatchContext,
}

impl<'a> L1BatchSourceAdapter<'a> {
    /// Create a new adapter from a BatchContext.
    pub const fn new(context: &'a BatchContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl montana_pipeline::L1BatchSource for L1BatchSourceAdapter<'_> {
    async fn next_batch(&mut self) -> Result<Option<CompressedBatch>, PipelineSourceError> {
        self.context
            .source()
            .next_batch()
            .await
            .map_err(|e| PipelineSourceError::Connection(e.to_string()))
    }

    async fn l1_head(&self) -> Result<u64, PipelineSourceError> {
        self.context
            .source()
            .l1_head()
            .await
            .map_err(|e| PipelineSourceError::Connection(e.to_string()))
    }
}
