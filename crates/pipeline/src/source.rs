//! Data source traits and types.

use async_trait::async_trait;

/// Raw transaction bytes (RLP-encoded, opaque to the pipeline).
#[derive(Clone, Debug)]
pub struct RawTransaction(pub Vec<u8>);

/// A block's worth of transactions with metadata.
#[derive(Clone, Debug)]
pub struct L2BlockData {
    /// Block timestamp.
    pub timestamp: u64,
    /// Block transactions.
    pub transactions: Vec<RawTransaction>,
}

/// Source errors.
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    /// RPC connection failed.
    #[error("RPC connection failed: {0}")]
    Connection(String),
    /// No blocks available.
    #[error("No blocks available")]
    Empty,
    /// L1 origin unavailable.
    #[error("L1 origin unavailable")]
    NoOrigin,
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

/// Source of compressed batches from L1.
#[async_trait]
pub trait L1BatchSource: Send + Sync {
    /// Fetch next batch from L1. Returns None if caught up.
    async fn next_batch(&mut self) -> Result<Option<crate::CompressedBatch>, SourceError>;

    /// Current L1 head block number.
    async fn l1_head(&self) -> Result<u64, SourceError>;
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn raw_transaction_new() {
        let data = vec![1, 2, 3, 4, 5];
        let tx = RawTransaction(data.clone());
        assert_eq!(tx.0, data);
    }

    #[rstest]
    #[case(vec![], "empty transaction")]
    #[case(vec![0x00], "single byte")]
    #[case(vec![0xf8, 0x65, 0x80], "RLP prefix")]
    #[case(vec![0u8; 1000], "large transaction")]
    fn raw_transaction_various_sizes(#[case] data: Vec<u8>, #[case] _description: &str) {
        let tx = RawTransaction(data.clone());
        assert_eq!(tx.0.len(), data.len());
    }

    #[test]
    fn raw_transaction_clone() {
        let tx = RawTransaction(vec![1, 2, 3]);
        let cloned = tx.clone();
        assert_eq!(cloned.0, tx.0);
    }

    #[test]
    fn raw_transaction_debug() {
        let tx = RawTransaction(vec![1, 2, 3]);
        let debug_str = format!("{:?}", tx);
        assert!(debug_str.contains("RawTransaction"));
    }

    #[test]
    fn l2_block_data_new() {
        let block = L2BlockData { timestamp: 1234567890, transactions: vec![] };
        assert_eq!(block.timestamp, 1234567890);
        assert!(block.transactions.is_empty());
    }

    #[rstest]
    #[case(0, 0, "genesis block")]
    #[case(1234567890, 1, "single tx block")]
    #[case(u64::MAX, 100, "far future with many txs")]
    fn l2_block_data_various(
        #[case] timestamp: u64,
        #[case] tx_count: usize,
        #[case] _description: &str,
    ) {
        let transactions: Vec<RawTransaction> =
            (0..tx_count).map(|i| RawTransaction(vec![i as u8])).collect();
        let block = L2BlockData { timestamp, transactions };
        assert_eq!(block.timestamp, timestamp);
        assert_eq!(block.transactions.len(), tx_count);
    }

    #[test]
    fn l2_block_data_clone() {
        let block =
            L2BlockData { timestamp: 1000, transactions: vec![RawTransaction(vec![1, 2, 3])] };
        let cloned = block.clone();
        assert_eq!(cloned.timestamp, block.timestamp);
        assert_eq!(cloned.transactions.len(), block.transactions.len());
        assert_eq!(cloned.transactions[0].0, block.transactions[0].0);
    }

    #[test]
    fn l2_block_data_debug() {
        let block = L2BlockData { timestamp: 1000, transactions: vec![] };
        let debug_str = format!("{:?}", block);
        assert!(debug_str.contains("L2BlockData"));
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
    fn source_error_no_origin_display() {
        let err = SourceError::NoOrigin;
        assert_eq!(err.to_string(), "L1 origin unavailable");
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
    #[case(SourceError::NoOrigin)]
    fn source_error_variants_are_debug(#[case] err: SourceError) {
        let _ = format!("{:?}", err);
    }

    /// Mock implementation of BatchSource for testing
    struct MockBatchSource {
        blocks: Vec<L2BlockData>,
        l1_origin_val: u64,
    }

    #[async_trait]
    impl BatchSource for MockBatchSource {
        async fn pending_blocks(&mut self) -> Result<Vec<L2BlockData>, SourceError> {
            Ok(std::mem::take(&mut self.blocks))
        }

        async fn l1_origin(&self) -> Result<u64, SourceError> {
            Ok(self.l1_origin_val)
        }

        async fn l1_origin_hash(&self) -> Result<[u8; 20], SourceError> {
            Ok([0u8; 20])
        }

        async fn parent_hash(&self) -> Result<[u8; 20], SourceError> {
            Ok([1u8; 20])
        }
    }

    #[tokio::test]
    async fn mock_batch_source_pending_blocks() {
        let mut source = MockBatchSource {
            blocks: vec![L2BlockData {
                timestamp: 1000,
                transactions: vec![RawTransaction(vec![1, 2, 3])],
            }],
            l1_origin_val: 100,
        };

        let blocks = source.pending_blocks().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].timestamp, 1000);

        // Second call should return empty (blocks taken)
        let blocks = source.pending_blocks().await.unwrap();
        assert!(blocks.is_empty());
    }

    #[tokio::test]
    async fn mock_batch_source_l1_origin() {
        let source = MockBatchSource { blocks: vec![], l1_origin_val: 12345 };

        assert_eq!(source.l1_origin().await.unwrap(), 12345);
    }

    #[tokio::test]
    async fn mock_batch_source_hashes() {
        let source = MockBatchSource { blocks: vec![], l1_origin_val: 100 };

        assert_eq!(source.l1_origin_hash().await.unwrap(), [0u8; 20]);
        assert_eq!(source.parent_hash().await.unwrap(), [1u8; 20]);
    }
}
