//! Data source traits and types.

use async_trait::async_trait;
use primitives::L2BlockData;

/// Source errors.
#[derive(Debug, derive_more::Display, derive_more::Error)]
pub enum SourceError {
    /// RPC connection failed.
    #[display("RPC connection failed: {_0}")]
    Connection(#[error(not(source))] String),
    /// No blocks available.
    #[display("No blocks available")]
    Empty,
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
}

#[cfg(test)]
mod tests {
    use primitives::Bytes;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("connection refused", "RPC connection failed: connection refused")]
    #[case("timeout", "RPC connection failed: timeout")]
    #[case("", "RPC connection failed: ")]
    fn source_error_connection_display(#[case] msg: &str, #[case] expected: &str) {
        let err = SourceError::Connection(msg.to_string());
        assert_eq!(err.to_string(), expected);
    }

    #[test]
    fn source_error_empty_display() {
        let err = SourceError::Empty;
        assert_eq!(err.to_string(), "No blocks available");
    }

    #[test]
    fn source_error_debug() {
        let err = SourceError::Empty;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Empty"));
    }

    #[rstest]
    #[case(SourceError::Connection("test".into()))]
    #[case(SourceError::Empty)]
    fn source_error_variants_are_debug(#[case] err: SourceError) {
        let _ = format!("{:?}", err);
    }

    /// Mock implementation of BatchSource for testing
    struct MockBatchSource {
        blocks: Vec<L2BlockData>,
    }

    #[async_trait]
    impl BatchSource for MockBatchSource {
        async fn pending_blocks(&mut self) -> Result<Vec<L2BlockData>, SourceError> {
            Ok(std::mem::take(&mut self.blocks))
        }
    }

    #[tokio::test]
    async fn mock_batch_source_pending_blocks() {
        let mut source = MockBatchSource {
            blocks: vec![L2BlockData {
                timestamp: 1000,
                transactions: vec![Bytes::from(vec![1, 2, 3])],
            }],
        };

        let blocks = source.pending_blocks().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].timestamp, 1000);

        // Second call should return empty (blocks taken)
        let blocks = source.pending_blocks().await.unwrap();
        assert!(blocks.is_empty());
    }
}
