//! Batch sink and source implementations for different submission modes.
//!
//! This module provides the abstraction layer for submitting batches and
//! retrieving them in different modes (in-memory, anvil). Simplified version
//! adapted from shadow binary for use in validator/dual mode.

use std::sync::Arc;

use async_trait::async_trait;
use montana_anvil::{Address, AnvilConfig, AnvilManager};
use montana_pipeline::{
    BatchSink as PipelineBatchSink, CompressedBatch, L1BatchSource as PipelineL1BatchSource,
    SinkError as PipelineSinkError, SourceError as PipelineSourceError,
};
use tokio::sync::Mutex;
// Re-export BatchSubmissionMode from montana_batcher
pub(crate) use montana_batcher::BatchSubmissionMode;

/// Error type for batch sink operations.
#[derive(Debug, thiserror::Error)]
pub(crate) enum SinkError {
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
pub(crate) enum SourceError {
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
// Batch Sink/Source Traits (local wrappers)
// ============================================================================

/// Trait for submitting batches to a destination.
#[async_trait]
pub(crate) trait BatchSink: Send + Sync {
    /// Submit a batch.
    async fn submit(
        &self,
        batch: CompressedBatch,
    ) -> Result<montana_pipeline::SubmissionReceipt, SinkError>;
}

/// Trait for retrieving batches from a source.
#[async_trait]
pub(crate) trait BatchSource: Send + Sync {
    /// Get the next batch, if available.
    async fn next_batch(&mut self) -> Result<Option<CompressedBatch>, SourceError>;

    /// Current L1 head block number.
    async fn l1_head(&self) -> Result<u64, SourceError>;
}

// ============================================================================
// In-Memory Implementation
// ============================================================================

/// In-memory batch queue for direct batch passing.
#[derive(Debug)]
pub(crate) struct InMemoryBatchQueue {
    batches: Arc<Mutex<Vec<CompressedBatch>>>,
}

impl InMemoryBatchQueue {
    /// Create a new in-memory batch queue.
    pub(crate) fn new() -> Self {
        Self { batches: Arc::new(Mutex::new(Vec::new())) }
    }

    /// Get a source that reads from this queue.
    pub(crate) fn source(&self) -> InMemoryBatchSource {
        InMemoryBatchSource { batches: Arc::clone(&self.batches) }
    }
}

#[async_trait]
impl BatchSink for InMemoryBatchQueue {
    async fn submit(
        &self,
        batch: CompressedBatch,
    ) -> Result<montana_pipeline::SubmissionReceipt, SinkError> {
        let batch_number = batch.batch_number;
        let mut guard = self.batches.lock().await;
        guard.push(batch);
        Ok(montana_pipeline::SubmissionReceipt {
            batch_number,
            tx_hash: [0u8; 32],
            l1_block: 0,
            blob_hash: None,
        })
    }
}

/// Source that reads from an in-memory batch queue.
#[derive(Debug)]
pub(crate) struct InMemoryBatchSource {
    batches: Arc<Mutex<Vec<CompressedBatch>>>,
}

#[async_trait]
impl BatchSource for InMemoryBatchSource {
    async fn next_batch(&mut self) -> Result<Option<CompressedBatch>, SourceError> {
        let mut guard = self.batches.lock().await;
        if guard.is_empty() { Ok(None) } else { Ok(Some(guard.remove(0))) }
    }

    async fn l1_head(&self) -> Result<u64, SourceError> {
        Ok(0)
    }
}

// ============================================================================
// Anvil Implementation
// ============================================================================

/// Wrapper around AnvilBatchSink from the crate.
pub(crate) struct AnvilBatchSinkWrapper {
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
    async fn submit(
        &self,
        batch: CompressedBatch,
    ) -> Result<montana_pipeline::SubmissionReceipt, SinkError> {
        let mut guard = self.inner.lock().await;
        guard.submit(batch).await.map_err(SinkError::from)
    }
}

/// Wrapper around AnvilBatchSource from the crate.
pub(crate) struct AnvilBatchSourceWrapper {
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
    async fn next_batch(&mut self) -> Result<Option<CompressedBatch>, SourceError> {
        let mut guard = self.inner.lock().await;
        guard.next_batch().await.map_err(SourceError::from)
    }

    async fn l1_head(&self) -> Result<u64, SourceError> {
        let guard = self.inner.lock().await;
        guard.l1_head().await.map_err(SourceError::from)
    }
}

// ============================================================================
// Batch Context Factory
// ============================================================================

/// Context holding the batch sink and source for the current mode.
pub(crate) struct BatchContext {
    /// The batch sink for submission.
    sink: Arc<dyn BatchSink>,
    /// The batch source for derivation.
    source: Mutex<Box<dyn BatchSource>>,
    /// Anvil manager (kept alive for the duration).
    #[allow(dead_code)]
    anvil: Option<AnvilManager>,
    /// The submission mode.
    mode: BatchSubmissionMode,
}

impl BatchContext {
    /// Create a new batch context for the given mode.
    pub(crate) async fn new(
        mode: BatchSubmissionMode,
        batch_inbox: Address,
    ) -> Result<Self, SinkError> {
        match mode {
            BatchSubmissionMode::InMemory => {
                let queue = InMemoryBatchQueue::new();
                let source = queue.source();
                Ok(Self {
                    sink: Arc::new(queue),
                    source: Mutex::new(Box::new(source)),
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
                    sink: Arc::new(sink),
                    source: Mutex::new(Box::new(source)),
                    anvil: Some(anvil),
                    mode,
                })
            }
            BatchSubmissionMode::Remote => Err(SinkError::Unsupported(
                "Remote batch submission mode is currently unsupported.".to_string(),
            )),
        }
    }

    /// Get an Arc clone of the sink.
    pub(crate) fn sink(&self) -> Arc<dyn BatchSink> {
        Arc::clone(&self.sink)
    }

    /// Get the Anvil endpoint URL (if in Anvil mode).
    pub(crate) fn anvil_endpoint(&self) -> Option<String> {
        self.anvil.as_ref().map(|a| a.endpoint().to_string())
    }

    /// Get the submission mode.
    #[allow(dead_code)]
    pub(crate) const fn mode(&self) -> BatchSubmissionMode {
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
pub(crate) struct L1BatchSourceAdapter<'a> {
    context: &'a BatchContext,
}

impl<'a> L1BatchSourceAdapter<'a> {
    pub(crate) const fn new(context: &'a BatchContext) -> Self {
        Self { context }
    }
}

#[async_trait]
impl montana_pipeline::L1BatchSource for L1BatchSourceAdapter<'_> {
    async fn next_batch(&mut self) -> Result<Option<CompressedBatch>, PipelineSourceError> {
        let mut source = self.context.source.lock().await;
        source.next_batch().await.map_err(|e| PipelineSourceError::Connection(e.to_string()))
    }

    async fn l1_head(&self) -> Result<u64, PipelineSourceError> {
        let source = self.context.source.lock().await;
        source.l1_head().await.map_err(|e| PipelineSourceError::Connection(e.to_string()))
    }
}
