//! Data sink traits and types.

use async_trait::async_trait;
use primitives::{CompressedBatch, SubmissionReceipt};

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

        let batch = CompressedBatch {
            batch_number: 1,
            data: vec![1, 2, 3],
            block_count: 1,
            first_block: 1,
            last_block: 1,
        };

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
            let batch = CompressedBatch {
                batch_number: i,
                data: vec![i as u8],
                block_count: 1,
                first_block: i,
                last_block: i,
            };
            let receipt = sink.submit(batch).await.unwrap();
            assert_eq!(receipt.batch_number, i);
        }

        assert_eq!(sink.submissions.len(), 5);
    }
}
