//! Block source trait for the batch submission runner.

use async_trait::async_trait;
use primitives::OpBlock;
use thiserror::Error;

/// Block source errors.
#[derive(Debug, Clone, Error)]
pub enum BlockSourceError {
    /// Block not found.
    #[error("Block not found: {0}")]
    BlockNotFound(u64),
    /// Connection error.
    #[error("Connection error: {0}")]
    ConnectionError(String),
    /// RPC error.
    #[error("RPC error: {0}")]
    RpcError(String),
}

/// Trait for fetching L2 blocks by number.
///
/// This trait is used by the [`BatchSubmissionRunner`](crate::BatchSubmissionRunner)
/// to fetch blocks from various sources (RPC, local files, etc.).
#[async_trait]
pub trait BlockSource: Send + Sync {
    /// Fetch a block by its number.
    ///
    /// Returns the full `OpBlock` which includes all block metadata, header,
    /// and full transaction data. This is used for batch serialization.
    ///
    /// Returns `BlockSourceError::BlockNotFound` if the block doesn't exist yet.
    async fn get_block(&mut self, block_number: u64) -> Result<OpBlock, BlockSourceError>;

    /// Get the current chain head block number.
    async fn get_head(&self) -> Result<u64, BlockSourceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_not_found_display() {
        let err = BlockSourceError::BlockNotFound(123);
        assert_eq!(err.to_string(), "Block not found: 123");
    }

    #[test]
    fn connection_error_display() {
        let err = BlockSourceError::ConnectionError("timeout".to_string());
        assert_eq!(err.to_string(), "Connection error: timeout");
    }

    #[test]
    fn rpc_error_display() {
        let err = BlockSourceError::RpcError("invalid response".to_string());
        assert_eq!(err.to_string(), "RPC error: invalid response");
    }

    #[test]
    fn errors_are_clone() {
        let err = BlockSourceError::BlockNotFound(123);
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn errors_are_debug() {
        let err = BlockSourceError::BlockNotFound(123);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("BlockNotFound"));
    }
}
