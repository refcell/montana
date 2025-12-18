//! Batch submission runner error types.

use thiserror::Error;

/// Batch submission runner errors.
#[derive(Debug, Clone, Error)]
pub enum BatchSubmissionError {
    /// Block source error.
    #[error("Block source error: {0}")]
    SourceError(String),

    /// Block not found.
    #[error("Block not found: {0}")]
    BlockNotFound(u64),

    /// Encoding failed.
    #[error("Encoding failed: {0}")]
    EncodingFailed(String),

    /// Compression failed.
    #[error("Compression failed: {0}")]
    CompressionFailed(String),

    /// Batch sink error.
    #[error("Batch sink error: {0}")]
    SinkError(String),
}

impl BatchSubmissionError {
    /// Classifies whether an error is retryable.
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::SourceError(_) | Self::BlockNotFound(_))
    }

    /// Classifies whether an error is fatal.
    pub const fn is_fatal(&self) -> bool {
        matches!(self, Self::EncodingFailed(_))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn source_error_display() {
        let err = BatchSubmissionError::SourceError("connection failed".to_string());
        assert_eq!(err.to_string(), "Block source error: connection failed");
    }

    #[test]
    fn block_not_found_display() {
        let err = BatchSubmissionError::BlockNotFound(123);
        assert_eq!(err.to_string(), "Block not found: 123");
    }

    #[test]
    fn encoding_failed_display() {
        let err = BatchSubmissionError::EncodingFailed("invalid data".to_string());
        assert_eq!(err.to_string(), "Encoding failed: invalid data");
    }

    #[test]
    fn compression_failed_display() {
        let err = BatchSubmissionError::CompressionFailed("algorithm error".to_string());
        assert_eq!(err.to_string(), "Compression failed: algorithm error");
    }

    #[test]
    fn sink_error_display() {
        let err = BatchSubmissionError::SinkError("submission failed".to_string());
        assert_eq!(err.to_string(), "Batch sink error: submission failed");
    }

    #[test]
    fn error_is_clone() {
        let err = BatchSubmissionError::SourceError("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn error_is_debug() {
        let err = BatchSubmissionError::SourceError("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("SourceError"));
    }

    #[rstest]
    #[case(BatchSubmissionError::SourceError("test".to_string()), true)]
    #[case(BatchSubmissionError::BlockNotFound(123), true)]
    #[case(BatchSubmissionError::EncodingFailed("test".to_string()), false)]
    #[case(BatchSubmissionError::CompressionFailed("test".to_string()), false)]
    #[case(BatchSubmissionError::SinkError("test".to_string()), false)]
    fn is_retryable(#[case] error: BatchSubmissionError, #[case] expected: bool) {
        assert_eq!(error.is_retryable(), expected);
    }

    #[rstest]
    #[case(BatchSubmissionError::EncodingFailed("test".to_string()), true)]
    #[case(BatchSubmissionError::SourceError("test".to_string()), false)]
    #[case(BatchSubmissionError::BlockNotFound(123), false)]
    #[case(BatchSubmissionError::CompressionFailed("test".to_string()), false)]
    #[case(BatchSubmissionError::SinkError("test".to_string()), false)]
    fn is_fatal(#[case] error: BatchSubmissionError, #[case] expected: bool) {
        assert_eq!(error.is_fatal(), expected);
    }
}
