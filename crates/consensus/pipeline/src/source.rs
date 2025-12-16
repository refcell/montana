//! Data source traits and types.

use async_trait::async_trait;
// Re-export types from channels crate
pub use channels::{BatchSource, L2BlockData, RawTransaction, SourceError};

/// Source of compressed batches from L1.
#[async_trait]
pub trait L1BatchSource: Send + Sync {
    /// Fetch next batch from L1. Returns None if caught up.
    async fn next_batch(&mut self) -> Result<Option<crate::CompressedBatch>, SourceError>;

    /// Current L1 head block number.
    async fn l1_head(&self) -> Result<u64, SourceError>;
}
