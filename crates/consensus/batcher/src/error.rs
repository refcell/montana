//! Batcher error types.

use thiserror::Error;

/// Batcher errors.
#[derive(Debug, Clone, Error)]
pub enum BatcherError {
    /// L2 source unavailable.
    #[error("L2 source unavailable: {0}")]
    SourceUnavailable(String),

    /// No pending blocks available.
    #[error("No pending blocks available")]
    NoPendingBlocks,

    /// Block sequence gap.
    #[error("Block sequence gap: expected {expected}, got {got}")]
    SequenceGap {
        /// Expected sequence number.
        expected: u64,
        /// Received sequence number.
        got: u64,
    },

    /// Compression failed.
    #[error("Compression failed: {0}")]
    CompressionFailed(String),

    /// Batch too large after compression.
    #[error("Batch too large after compression: {size} > {max}")]
    BatchTooLarge {
        /// Actual batch size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },

    /// L1 submission failed.
    #[error("L1 submission failed: {0}")]
    SubmissionFailed(String),

    /// Blob gas too expensive.
    #[error("Blob gas too expensive: {current} > {max}")]
    BlobGasTooExpensive {
        /// Current blob gas price.
        current: u128,
        /// Maximum blob gas price.
        max: u128,
    },

    /// Transaction confirmation timeout.
    #[error("Transaction confirmation timeout")]
    ConfirmationTimeout,

    /// L1 drift exceeded.
    #[error("L1 drift exceeded: {drift} > {max}")]
    L1DriftExceeded {
        /// Current drift value.
        drift: u64,
        /// Maximum allowed drift.
        max: u64,
    },

    /// Sequencing window expired.
    #[error("Sequencing window expired")]
    SequencingWindowExpired,

    /// Pipeline error.
    #[error("Pipeline error: {0}")]
    Pipeline(String),

    /// Transaction manager error.
    #[error("Transaction manager error: {0}")]
    TxManager(String),
}

impl BatcherError {
    /// Classifies whether an error is retryable.
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::SourceUnavailable(_)
                | Self::SubmissionFailed(_)
                | Self::BlobGasTooExpensive { .. }
                | Self::ConfirmationTimeout
        )
    }

    /// Classifies whether an error is fatal.
    pub const fn is_fatal(&self) -> bool {
        matches!(self, Self::SequencingWindowExpired | Self::L1DriftExceeded { .. })
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(BatcherError::SourceUnavailable("test error".to_string()), true)]
    #[case(BatcherError::SubmissionFailed("test error".to_string()), true)]
    #[case(BatcherError::BlobGasTooExpensive { current: 100, max: 50 }, true)]
    #[case(BatcherError::ConfirmationTimeout, true)]
    #[case(BatcherError::NoPendingBlocks, false)]
    #[case(BatcherError::SequenceGap { expected: 10, got: 12 }, false)]
    #[case(BatcherError::CompressionFailed("test".to_string()), false)]
    #[case(BatcherError::BatchTooLarge { size: 1000, max: 500 }, false)]
    #[case(BatcherError::L1DriftExceeded { drift: 100, max: 50 }, false)]
    #[case(BatcherError::SequencingWindowExpired, false)]
    #[case(BatcherError::Pipeline("test".to_string()), false)]
    #[case(BatcherError::TxManager("test".to_string()), false)]
    fn test_is_retryable(#[case] error: BatcherError, #[case] expected: bool) {
        assert_eq!(error.is_retryable(), expected);
    }

    #[rstest]
    #[case(BatcherError::SequencingWindowExpired, true)]
    #[case(BatcherError::L1DriftExceeded { drift: 100, max: 50 }, true)]
    #[case(BatcherError::SourceUnavailable("test".to_string()), false)]
    #[case(BatcherError::NoPendingBlocks, false)]
    #[case(BatcherError::SequenceGap { expected: 10, got: 12 }, false)]
    #[case(BatcherError::CompressionFailed("test".to_string()), false)]
    #[case(BatcherError::BatchTooLarge { size: 1000, max: 500 }, false)]
    #[case(BatcherError::SubmissionFailed("test".to_string()), false)]
    #[case(BatcherError::BlobGasTooExpensive { current: 100, max: 50 }, false)]
    #[case(BatcherError::ConfirmationTimeout, false)]
    #[case(BatcherError::Pipeline("test".to_string()), false)]
    #[case(BatcherError::TxManager("test".to_string()), false)]
    fn test_is_fatal(#[case] error: BatcherError, #[case] expected: bool) {
        assert_eq!(error.is_fatal(), expected);
    }

    #[test]
    fn test_source_unavailable_display() {
        let err = BatcherError::SourceUnavailable("connection failed".to_string());
        assert_eq!(err.to_string(), "L2 source unavailable: connection failed");
    }

    #[test]
    fn test_no_pending_blocks_display() {
        let err = BatcherError::NoPendingBlocks;
        assert_eq!(err.to_string(), "No pending blocks available");
    }

    #[test]
    fn test_sequence_gap_display() {
        let err = BatcherError::SequenceGap { expected: 10, got: 12 };
        assert_eq!(err.to_string(), "Block sequence gap: expected 10, got 12");
    }

    #[test]
    fn test_compression_failed_display() {
        let err = BatcherError::CompressionFailed("invalid data".to_string());
        assert_eq!(err.to_string(), "Compression failed: invalid data");
    }

    #[test]
    fn test_batch_too_large_display() {
        let err = BatcherError::BatchTooLarge { size: 1000, max: 500 };
        assert_eq!(err.to_string(), "Batch too large after compression: 1000 > 500");
    }

    #[test]
    fn test_submission_failed_display() {
        let err = BatcherError::SubmissionFailed("nonce too low".to_string());
        assert_eq!(err.to_string(), "L1 submission failed: nonce too low");
    }

    #[test]
    fn test_blob_gas_too_expensive_display() {
        let err = BatcherError::BlobGasTooExpensive { current: 100, max: 50 };
        assert_eq!(err.to_string(), "Blob gas too expensive: 100 > 50");
    }

    #[test]
    fn test_confirmation_timeout_display() {
        let err = BatcherError::ConfirmationTimeout;
        assert_eq!(err.to_string(), "Transaction confirmation timeout");
    }

    #[test]
    fn test_l1_drift_exceeded_display() {
        let err = BatcherError::L1DriftExceeded { drift: 100, max: 50 };
        assert_eq!(err.to_string(), "L1 drift exceeded: 100 > 50");
    }

    #[test]
    fn test_sequencing_window_expired_display() {
        let err = BatcherError::SequencingWindowExpired;
        assert_eq!(err.to_string(), "Sequencing window expired");
    }

    #[test]
    fn test_pipeline_error_display() {
        let err = BatcherError::Pipeline("pipeline failed".to_string());
        assert_eq!(err.to_string(), "Pipeline error: pipeline failed");
    }

    #[test]
    fn test_tx_manager_error_display() {
        let err = BatcherError::TxManager("tx failed".to_string());
        assert_eq!(err.to_string(), "Transaction manager error: tx failed");
    }

    #[rstest]
    #[case(BatcherError::SourceUnavailable("test".to_string()))]
    #[case(BatcherError::NoPendingBlocks)]
    #[case(BatcherError::SequenceGap { expected: 10, got: 12 })]
    #[case(BatcherError::CompressionFailed("test".to_string()))]
    #[case(BatcherError::BatchTooLarge { size: 1000, max: 500 })]
    #[case(BatcherError::SubmissionFailed("test".to_string()))]
    #[case(BatcherError::BlobGasTooExpensive { current: 100, max: 50 })]
    #[case(BatcherError::ConfirmationTimeout)]
    #[case(BatcherError::L1DriftExceeded { drift: 100, max: 50 })]
    #[case(BatcherError::SequencingWindowExpired)]
    #[case(BatcherError::Pipeline("test".to_string()))]
    #[case(BatcherError::TxManager("test".to_string()))]
    fn test_error_variants_are_debug(#[case] err: BatcherError) {
        let _ = format!("{:?}", err);
    }

    #[test]
    fn test_error_is_clone() {
        let err = BatcherError::SourceUnavailable("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_sequence_gap_clone() {
        let err = BatcherError::SequenceGap { expected: 10, got: 12 };
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_batch_too_large_clone() {
        let err = BatcherError::BatchTooLarge { size: 1000, max: 500 };
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_blob_gas_too_expensive_clone() {
        let err = BatcherError::BlobGasTooExpensive { current: 200, max: 100 };
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_l1_drift_exceeded_clone() {
        let err = BatcherError::L1DriftExceeded { drift: 150, max: 100 };
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_source_unavailable_fields() {
        let err = BatcherError::SourceUnavailable("db connection failed".to_string());

        if let BatcherError::SourceUnavailable(msg) = err {
            assert_eq!(msg, "db connection failed");
        } else {
            panic!("Expected SourceUnavailable variant");
        }
    }

    #[test]
    fn test_sequence_gap_fields() {
        let err = BatcherError::SequenceGap { expected: 100, got: 102 };

        if let BatcherError::SequenceGap { expected, got } = err {
            assert_eq!(expected, 100);
            assert_eq!(got, 102);
        } else {
            panic!("Expected SequenceGap variant");
        }
    }

    #[test]
    fn test_batch_too_large_fields() {
        let err = BatcherError::BatchTooLarge { size: 2000, max: 1000 };

        if let BatcherError::BatchTooLarge { size, max } = err {
            assert_eq!(size, 2000);
            assert_eq!(max, 1000);
        } else {
            panic!("Expected BatchTooLarge variant");
        }
    }

    #[test]
    fn test_blob_gas_too_expensive_fields() {
        let err = BatcherError::BlobGasTooExpensive { current: 300, max: 200 };

        if let BatcherError::BlobGasTooExpensive { current, max } = err {
            assert_eq!(current, 300);
            assert_eq!(max, 200);
        } else {
            panic!("Expected BlobGasTooExpensive variant");
        }
    }

    #[test]
    fn test_l1_drift_exceeded_fields() {
        let err = BatcherError::L1DriftExceeded { drift: 250, max: 200 };

        if let BatcherError::L1DriftExceeded { drift, max } = err {
            assert_eq!(drift, 250);
            assert_eq!(max, 200);
        } else {
            panic!("Expected L1DriftExceeded variant");
        }
    }

    #[test]
    fn test_retryable_source_unavailable() {
        let err = BatcherError::SourceUnavailable("network error".to_string());
        assert!(err.is_retryable());
    }

    #[test]
    fn test_retryable_submission_failed() {
        let err = BatcherError::SubmissionFailed("temporary failure".to_string());
        assert!(err.is_retryable());
    }

    #[test]
    fn test_retryable_blob_gas_too_expensive() {
        let err = BatcherError::BlobGasTooExpensive { current: 150, max: 100 };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_retryable_confirmation_timeout() {
        let err = BatcherError::ConfirmationTimeout;
        assert!(err.is_retryable());
    }

    #[test]
    fn test_fatal_sequencing_window_expired() {
        let err = BatcherError::SequencingWindowExpired;
        assert!(err.is_fatal());
    }

    #[test]
    fn test_fatal_l1_drift_exceeded() {
        let err = BatcherError::L1DriftExceeded { drift: 500, max: 100 };
        assert!(err.is_fatal());
    }

    #[test]
    fn test_non_retryable_non_fatal_errors() {
        let errors = vec![
            BatcherError::NoPendingBlocks,
            BatcherError::SequenceGap { expected: 10, got: 12 },
            BatcherError::CompressionFailed("error".to_string()),
            BatcherError::BatchTooLarge { size: 1000, max: 500 },
            BatcherError::Pipeline("error".to_string()),
            BatcherError::TxManager("error".to_string()),
        ];

        for err in errors {
            assert!(!err.is_retryable(), "Expected {} to not be retryable", err);
            assert!(!err.is_fatal(), "Expected {} to not be fatal", err);
        }
    }
}
