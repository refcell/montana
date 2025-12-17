//! Data source traits and types.

use async_trait::async_trait;
use primitives::Bytes;

/// A block's worth of transactions with metadata.
#[derive(Clone, Debug)]
pub struct L2BlockData {
    /// Block number.
    pub block_number: u64,
    /// Block timestamp.
    pub timestamp: u64,
    /// Block transactions.
    pub transactions: Vec<Bytes>,
}

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

    #[test]
    fn l2_block_data_new() {
        let block = L2BlockData { block_number: 42, timestamp: 1234567890, transactions: vec![] };
        assert_eq!(block.block_number, 42);
        assert_eq!(block.timestamp, 1234567890);
        assert!(block.transactions.is_empty());
    }

    #[rstest]
    #[case(0, 0, 0, "genesis block")]
    #[case(100, 1234567890, 1, "single tx block")]
    #[case(u64::MAX, u64::MAX, 100, "far future with many txs")]
    fn l2_block_data_various(
        #[case] block_number: u64,
        #[case] timestamp: u64,
        #[case] tx_count: usize,
        #[case] _description: &str,
    ) {
        let transactions: Vec<Bytes> = (0..tx_count).map(|i| Bytes::from(vec![i as u8])).collect();
        let block = L2BlockData { block_number, timestamp, transactions };
        assert_eq!(block.block_number, block_number);
        assert_eq!(block.timestamp, timestamp);
        assert_eq!(block.transactions.len(), tx_count);
    }

    #[test]
    fn l2_block_data_clone() {
        let block = L2BlockData {
            block_number: 42,
            timestamp: 1000,
            transactions: vec![Bytes::from(vec![1, 2, 3])],
        };
        let cloned = block.clone();
        assert_eq!(cloned.block_number, block.block_number);
        assert_eq!(cloned.timestamp, block.timestamp);
        assert_eq!(cloned.transactions.len(), block.transactions.len());
        assert_eq!(cloned.transactions[0], block.transactions[0]);
    }

    #[test]
    fn l2_block_data_debug() {
        let block = L2BlockData { block_number: 42, timestamp: 1000, transactions: vec![] };
        let debug_str = format!("{:?}", block);
        assert!(debug_str.contains("L2BlockData"));
        assert!(debug_str.contains("block_number"));
        assert!(debug_str.contains("timestamp"));
        assert!(debug_str.contains("transactions"));
    }

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
                block_number: 42,
                timestamp: 1000,
                transactions: vec![Bytes::from(vec![1, 2, 3])],
            }],
        };

        let blocks = source.pending_blocks().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_number, 42);
        assert_eq!(blocks[0].timestamp, 1000);

        // Second call should return empty (blocks taken)
        let blocks = source.pending_blocks().await.unwrap();
        assert!(blocks.is_empty());
    }
}
