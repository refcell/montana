//! Derivation error types.

use thiserror::Error;

/// Derivation errors.
#[derive(Debug, Clone, Error)]
pub enum DerivationError {
    /// Batch source error.
    #[error("Batch source error: {0}")]
    SourceError(String),

    /// Decompression failed.
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),

    /// No batch available from source.
    #[error("No batch available")]
    EmptyBatch,

    /// Pipeline error.
    #[error("Pipeline error: {0}")]
    Pipeline(String),

    /// Checkpoint or other internal error.
    #[error("Internal error: {0}")]
    InternalError(String),

    /// Block execution failed.
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),
}

impl DerivationError {
    /// Classifies whether an error is retryable.
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::SourceError(_) | Self::EmptyBatch)
    }

    /// Classifies whether an error is fatal.
    pub const fn is_fatal(&self) -> bool {
        matches!(self, Self::DecompressionFailed(_) | Self::ExecutionFailed(_))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(DerivationError::SourceError("test".to_string()), true)]
    #[case(DerivationError::EmptyBatch, true)]
    #[case(DerivationError::DecompressionFailed("test".to_string()), false)]
    #[case(DerivationError::Pipeline("test".to_string()), false)]
    #[case(DerivationError::InternalError("test".to_string()), false)]
    #[case(DerivationError::ExecutionFailed("test".to_string()), false)]
    fn test_is_retryable(#[case] error: DerivationError, #[case] expected: bool) {
        assert_eq!(error.is_retryable(), expected);
    }

    #[rstest]
    #[case(DerivationError::DecompressionFailed("test".to_string()), true)]
    #[case(DerivationError::ExecutionFailed("test".to_string()), true)]
    #[case(DerivationError::SourceError("test".to_string()), false)]
    #[case(DerivationError::EmptyBatch, false)]
    #[case(DerivationError::Pipeline("test".to_string()), false)]
    #[case(DerivationError::InternalError("test".to_string()), false)]
    fn test_is_fatal(#[case] error: DerivationError, #[case] expected: bool) {
        assert_eq!(error.is_fatal(), expected);
    }

    #[test]
    fn test_source_error_display() {
        let err = DerivationError::SourceError("connection failed".to_string());
        assert_eq!(err.to_string(), "Batch source error: connection failed");
    }

    #[test]
    fn test_decompression_failed_display() {
        let err = DerivationError::DecompressionFailed("invalid data".to_string());
        assert_eq!(err.to_string(), "Decompression failed: invalid data");
    }

    #[test]
    fn test_empty_batch_display() {
        let err = DerivationError::EmptyBatch;
        assert_eq!(err.to_string(), "No batch available");
    }

    #[test]
    fn test_pipeline_error_display() {
        let err = DerivationError::Pipeline("pipeline failed".to_string());
        assert_eq!(err.to_string(), "Pipeline error: pipeline failed");
    }

    #[rstest]
    #[case(DerivationError::SourceError("test".to_string()))]
    #[case(DerivationError::DecompressionFailed("test".to_string()))]
    #[case(DerivationError::EmptyBatch)]
    #[case(DerivationError::Pipeline("test".to_string()))]
    #[case(DerivationError::InternalError("test".to_string()))]
    fn test_error_variants_are_debug(#[case] err: DerivationError) {
        let _ = format!("{:?}", err);
    }

    #[test]
    fn test_error_is_clone() {
        let err = DerivationError::SourceError("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_decompression_failed_clone() {
        let err = DerivationError::DecompressionFailed("corrupt data".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_source_error_fields() {
        let err = DerivationError::SourceError("network timeout".to_string());

        if let DerivationError::SourceError(msg) = err {
            assert_eq!(msg, "network timeout");
        } else {
            panic!("Expected SourceError variant");
        }
    }

    #[test]
    fn test_decompression_failed_fields() {
        let err = DerivationError::DecompressionFailed("checksum mismatch".to_string());

        if let DerivationError::DecompressionFailed(msg) = err {
            assert_eq!(msg, "checksum mismatch");
        } else {
            panic!("Expected DecompressionFailed variant");
        }
    }

    #[test]
    fn test_retryable_source_error() {
        let err = DerivationError::SourceError("temporary failure".to_string());
        assert!(err.is_retryable());
    }

    #[test]
    fn test_retryable_empty_batch() {
        let err = DerivationError::EmptyBatch;
        assert!(err.is_retryable());
    }

    #[test]
    fn test_fatal_decompression_failed() {
        let err = DerivationError::DecompressionFailed("corrupt data".to_string());
        assert!(err.is_fatal());
    }

    #[test]
    fn test_non_retryable_non_fatal_errors() {
        let err = DerivationError::Pipeline("error".to_string());
        assert!(!err.is_retryable(), "Expected {} to not be retryable", err);
        assert!(!err.is_fatal(), "Expected {} to not be fatal", err);
    }
}
