//! Data sink traits and types.

use async_trait::async_trait;

/// Compressed batch ready for submission.
#[derive(Clone, Debug)]
pub struct CompressedBatch {
    /// Batch number.
    pub batch_number: u64,
    /// Compressed data.
    pub data: Vec<u8>,
}

/// Result of a successful submission.
#[derive(Clone, Debug)]
pub struct SubmissionReceipt {
    /// Batch number.
    pub batch_number: u64,
    /// Transaction hash.
    pub tx_hash: [u8; 32],
    /// L1 block number.
    pub l1_block: u64,
    /// Blob hash (if blob submission).
    pub blob_hash: Option<[u8; 32]>,
}

/// Sink errors.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// L1 connection failed.
    #[error("L1 connection failed: {0}")]
    Connection(String),
    /// Transaction failed.
    #[error("Transaction failed: {0}")]
    TxFailed(String),
    /// Insufficient funds.
    #[error("Insufficient funds")]
    InsufficientFunds,
    /// Blob gas too expensive.
    #[error("Blob gas too expensive: {current} > {max}")]
    BlobGasTooExpensive {
        /// Current blob gas price.
        current: u64,
        /// Maximum allowed blob gas price.
        max: u64,
    },
    /// Timeout waiting for confirmation.
    #[error("Timeout waiting for confirmation")]
    Timeout,
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

/// Sink for derived L2 blocks.
#[async_trait]
pub trait L2BlockSink: Send + Sync {
    /// Ingest a derived L2 block.
    async fn ingest(&mut self, block: crate::L2BlockData) -> Result<(), SinkError>;
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn compressed_batch_new() {
        let batch = CompressedBatch { batch_number: 42, data: vec![1, 2, 3, 4, 5] };
        assert_eq!(batch.batch_number, 42);
        assert_eq!(batch.data, vec![1, 2, 3, 4, 5]);
    }

    #[rstest]
    #[case(0, vec![], "empty batch")]
    #[case(1, vec![0u8; 128 * 1024], "max size batch")]
    #[case(u64::MAX, vec![0xFF], "max batch number")]
    fn compressed_batch_various(
        #[case] batch_number: u64,
        #[case] data: Vec<u8>,
        #[case] _description: &str,
    ) {
        let batch = CompressedBatch { batch_number, data: data.clone() };
        assert_eq!(batch.batch_number, batch_number);
        assert_eq!(batch.data.len(), data.len());
    }

    #[test]
    fn compressed_batch_clone() {
        let batch = CompressedBatch { batch_number: 1, data: vec![1, 2, 3] };
        let cloned = batch.clone();
        assert_eq!(cloned.batch_number, batch.batch_number);
        assert_eq!(cloned.data, batch.data);
    }

    #[test]
    fn compressed_batch_debug() {
        let batch = CompressedBatch { batch_number: 1, data: vec![1, 2, 3] };
        let debug_str = format!("{:?}", batch);
        assert!(debug_str.contains("CompressedBatch"));
        assert!(debug_str.contains("batch_number"));
        assert!(debug_str.contains("data"));
    }

    #[test]
    fn submission_receipt_new() {
        let receipt = SubmissionReceipt {
            batch_number: 1,
            tx_hash: [0xABu8; 32],
            l1_block: 100,
            blob_hash: Some([0xCDu8; 32]),
        };
        assert_eq!(receipt.batch_number, 1);
        assert_eq!(receipt.tx_hash, [0xABu8; 32]);
        assert_eq!(receipt.l1_block, 100);
        assert_eq!(receipt.blob_hash, Some([0xCDu8; 32]));
    }

    #[rstest]
    #[case(0, 0, None, "genesis calldata")]
    #[case(1, 100, Some([0u8; 32]), "blob submission")]
    #[case(u64::MAX, u64::MAX, None, "max values")]
    fn submission_receipt_various(
        #[case] batch_number: u64,
        #[case] l1_block: u64,
        #[case] blob_hash: Option<[u8; 32]>,
        #[case] _description: &str,
    ) {
        let receipt = SubmissionReceipt { batch_number, tx_hash: [0u8; 32], l1_block, blob_hash };
        assert_eq!(receipt.batch_number, batch_number);
        assert_eq!(receipt.l1_block, l1_block);
        assert_eq!(receipt.blob_hash, blob_hash);
    }

    #[test]
    fn submission_receipt_clone() {
        let receipt = SubmissionReceipt {
            batch_number: 1,
            tx_hash: [0xABu8; 32],
            l1_block: 100,
            blob_hash: Some([0xCDu8; 32]),
        };
        let cloned = receipt.clone();
        assert_eq!(cloned.batch_number, receipt.batch_number);
        assert_eq!(cloned.tx_hash, receipt.tx_hash);
        assert_eq!(cloned.l1_block, receipt.l1_block);
        assert_eq!(cloned.blob_hash, receipt.blob_hash);
    }

    #[test]
    fn submission_receipt_debug() {
        let receipt = SubmissionReceipt {
            batch_number: 1,
            tx_hash: [0u8; 32],
            l1_block: 100,
            blob_hash: None,
        };
        let debug_str = format!("{:?}", receipt);
        assert!(debug_str.contains("SubmissionReceipt"));
        assert!(debug_str.contains("tx_hash"));
    }

    #[rstest]
    #[case("connection refused", "L1 connection failed: connection refused")]
    #[case("timeout", "L1 connection failed: timeout")]
    fn sink_error_connection_display(#[case] msg: &str, #[case] expected: &str) {
        let err = SinkError::Connection(msg.to_string());
        assert_eq!(err.to_string(), expected);
    }

    #[rstest]
    #[case("nonce too low", "Transaction failed: nonce too low")]
    #[case("gas limit exceeded", "Transaction failed: gas limit exceeded")]
    fn sink_error_tx_failed_display(#[case] msg: &str, #[case] expected: &str) {
        let err = SinkError::TxFailed(msg.to_string());
        assert_eq!(err.to_string(), expected);
    }

    #[test]
    fn sink_error_insufficient_funds_display() {
        let err = SinkError::InsufficientFunds;
        assert_eq!(err.to_string(), "Insufficient funds");
    }

    #[rstest]
    #[case(100, 50, "Blob gas too expensive: 100 > 50")]
    #[case(1000000, 500000, "Blob gas too expensive: 1000000 > 500000")]
    fn sink_error_blob_gas_display(#[case] current: u64, #[case] max: u64, #[case] expected: &str) {
        let err = SinkError::BlobGasTooExpensive { current, max };
        assert_eq!(err.to_string(), expected);
    }

    #[test]
    fn sink_error_timeout_display() {
        let err = SinkError::Timeout;
        assert_eq!(err.to_string(), "Timeout waiting for confirmation");
    }

    #[test]
    fn sink_error_debug() {
        let err = SinkError::Timeout;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Timeout"));
    }

    #[rstest]
    #[case(SinkError::Connection("test".into()))]
    #[case(SinkError::TxFailed("test".into()))]
    #[case(SinkError::InsufficientFunds)]
    #[case(SinkError::BlobGasTooExpensive { current: 100, max: 50 })]
    #[case(SinkError::Timeout)]
    fn sink_error_variants_are_debug(#[case] err: SinkError) {
        let _ = format!("{:?}", err);
    }

    /// Mock implementation of BatchSink for testing
    struct MockBatchSink {
        submissions: Vec<CompressedBatch>,
        capacity_val: usize,
    }

    #[async_trait]
    impl BatchSink for MockBatchSink {
        async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
            let batch_number = batch.batch_number;
            self.submissions.push(batch);
            Ok(SubmissionReceipt {
                batch_number,
                tx_hash: [0u8; 32],
                l1_block: 100,
                blob_hash: Some([1u8; 32]),
            })
        }

        async fn capacity(&self) -> Result<usize, SinkError> {
            Ok(self.capacity_val)
        }

        async fn health_check(&self) -> Result<(), SinkError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn mock_batch_sink_submit() {
        let mut sink = MockBatchSink { submissions: vec![], capacity_val: 128 * 1024 };

        let batch = CompressedBatch { batch_number: 1, data: vec![1, 2, 3] };

        let receipt = sink.submit(batch).await.unwrap();
        assert_eq!(receipt.batch_number, 1);
        assert_eq!(receipt.l1_block, 100);
        assert!(receipt.blob_hash.is_some());
        assert_eq!(sink.submissions.len(), 1);
    }

    #[tokio::test]
    async fn mock_batch_sink_capacity() {
        let sink = MockBatchSink { submissions: vec![], capacity_val: 131072 };

        assert_eq!(sink.capacity().await.unwrap(), 131072);
    }

    #[tokio::test]
    async fn mock_batch_sink_health_check() {
        let sink = MockBatchSink { submissions: vec![], capacity_val: 128 * 1024 };

        assert!(sink.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn mock_batch_sink_multiple_submissions() {
        let mut sink = MockBatchSink { submissions: vec![], capacity_val: 128 * 1024 };

        for i in 0..5 {
            let batch = CompressedBatch { batch_number: i, data: vec![i as u8] };
            let receipt = sink.submit(batch).await.unwrap();
            assert_eq!(receipt.batch_number, i);
        }

        assert_eq!(sink.submissions.len(), 5);
    }
}
