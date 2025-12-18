//! Batch context for managing sink/source pairs.
//!
//! This module provides the abstraction layer for submitting batches
//! in different modes (in-memory, anvil, remote).

use std::sync::Arc;

use async_trait::async_trait;
use montana_anvil::{Address, AnvilConfig, AnvilManager};
use montana_cli::BatchSubmissionMode;
use montana_pipeline::{
    BatchSink as PipelineBatchSink, CompressedBatch, SinkError as PipelineSinkError,
    SubmissionReceipt,
};
use tokio::sync::Mutex;
use tracing::info;

/// Error type for batch context operations.
#[derive(Debug, thiserror::Error)]
pub enum BatchContextError {
    /// Connection error.
    #[error("Connection error: {0}")]
    Connection(String),
    /// Mode not supported.
    #[error("Mode not supported: {0}")]
    Unsupported(String),
}

// ============================================================================
// Batch Sink Trait (local wrapper for Arc compatibility)
// ============================================================================

/// Trait for submitting batches to a destination.
///
/// This is a wrapper around the pipeline's BatchSink trait that uses `&self`
/// instead of `&mut self` for Arc compatibility.
#[async_trait]
pub trait BatchSink: Send + Sync {
    /// Submit a batch.
    async fn submit(&self, batch: CompressedBatch) -> Result<SubmissionReceipt, PipelineSinkError>;
}

// ============================================================================
// In-Memory Implementation (NoOp)
// ============================================================================

/// In-memory batch sink that logs submissions but doesn't persist them.
///
/// This implementation provides a lightweight option for testing execution
/// without the overhead of actual batch submission.
#[derive(Debug, Default)]
pub struct InMemoryBatchSink {
    /// Counter for submitted batches.
    submitted: Mutex<u64>,
}

impl InMemoryBatchSink {
    /// Create a new in-memory batch sink.
    pub fn new() -> Self {
        Self { submitted: Mutex::new(0) }
    }
}

#[async_trait]
impl BatchSink for InMemoryBatchSink {
    async fn submit(&self, batch: CompressedBatch) -> Result<SubmissionReceipt, PipelineSinkError> {
        let batch_number = batch.batch_number;
        let mut count = self.submitted.lock().await;
        *count += 1;
        info!(
            batch_number,
            size = batch.data.len(),
            total_submitted = *count,
            "InMemoryBatchSink: batch submitted (not persisted)"
        );
        Ok(SubmissionReceipt { batch_number, tx_hash: [0u8; 32], l1_block: 0, blob_hash: None })
    }
}

// ============================================================================
// Anvil Implementation
// ============================================================================

/// Wrapper around AnvilBatchSink from the crate.
struct AnvilBatchSinkWrapper {
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
    async fn submit(&self, batch: CompressedBatch) -> Result<SubmissionReceipt, PipelineSinkError> {
        let mut guard = self.inner.lock().await;
        guard.submit(batch).await
    }
}

// ============================================================================
// Batch Context Factory
// ============================================================================

/// Context holding the batch sink for the current mode.
///
/// This provides a unified interface for batch submission across different
/// modes (in-memory, anvil, remote).
pub struct BatchContext {
    /// The batch sink for submission.
    sink: Arc<dyn BatchSink>,
    /// Anvil manager (kept alive for the duration of the session).
    anvil: Option<AnvilManager>,
    /// The submission mode.
    mode: BatchSubmissionMode,
}

impl BatchContext {
    /// Create a new batch context for the given mode.
    ///
    /// The `batch_inbox` parameter specifies the address where batches are sent
    /// (only used in Anvil mode).
    ///
    /// # Errors
    ///
    /// Returns an error if the mode is not supported or if Anvil fails to spawn.
    pub async fn new(
        mode: BatchSubmissionMode,
        batch_inbox: Address,
    ) -> Result<Self, BatchContextError> {
        match mode {
            BatchSubmissionMode::InMemory => {
                info!("Using in-memory batch sink (batches will not be persisted)");
                Ok(Self { sink: Arc::new(InMemoryBatchSink::new()), anvil: None, mode })
            }
            BatchSubmissionMode::Anvil => {
                info!("Spawning local Anvil instance for batch submission");
                let config = AnvilConfig { batch_inbox, ..Default::default() };
                let anvil = AnvilManager::spawn(config)
                    .await
                    .map_err(|e| BatchContextError::Connection(e.to_string()))?;
                info!(
                    endpoint = %anvil.endpoint(),
                    sender = ?anvil.sender(),
                    batch_inbox = ?anvil.batch_inbox(),
                    "Anvil instance spawned"
                );
                let sink = AnvilBatchSinkWrapper::new(anvil.sink());
                Ok(Self { sink: Arc::new(sink), anvil: Some(anvil), mode })
            }
            BatchSubmissionMode::Remote => Err(BatchContextError::Unsupported(
                "Remote batch submission mode is currently unsupported. \
                 Please use 'anvil' (default) or 'in-memory' mode."
                    .to_string(),
            )),
        }
    }

    /// Get an Arc clone of the sink (for sharing across tasks).
    pub fn sink(&self) -> Arc<dyn BatchSink> {
        Arc::clone(&self.sink)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_sink_submits_batch() {
        let sink = InMemoryBatchSink::new();
        let batch = CompressedBatch {
            batch_number: 1,
            data: vec![1, 2, 3],
            block_count: 1,
            first_block: 1,
            last_block: 1,
        };

        let receipt = sink.submit(batch).await.unwrap();
        assert_eq!(receipt.batch_number, 1);
    }

    #[tokio::test]
    async fn in_memory_sink_tracks_submissions() {
        let sink = InMemoryBatchSink::new();

        for i in 0..5 {
            let batch = CompressedBatch {
                batch_number: i,
                data: vec![i as u8],
                block_count: 1,
                first_block: i,
                last_block: i,
            };
            sink.submit(batch).await.unwrap();
        }

        let count = *sink.submitted.lock().await;
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn batch_context_in_memory_mode() {
        let ctx = BatchContext::new(BatchSubmissionMode::InMemory, Address::ZERO).await.unwrap();

        assert_eq!(ctx.mode(), BatchSubmissionMode::InMemory);
        assert!(ctx.anvil_endpoint().is_none());
    }

    #[tokio::test]
    async fn batch_context_remote_mode_unsupported() {
        let result = BatchContext::new(BatchSubmissionMode::Remote, Address::ZERO).await;
        assert!(result.is_err());
    }
}
