//! Pipeline error types.

use crate::{CodecError, CompressionError, SinkError, SourceError};

/// Pipeline errors.
#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    /// Source error.
    #[error("Source error: {0}")]
    Source(#[from] SourceError),
    /// Sink error.
    #[error("Sink error: {0}")]
    Sink(#[from] SinkError),
    /// Compression error.
    #[error("Compression error: {0}")]
    Compression(#[from] CompressionError),
    /// Codec error.
    #[error("Codec error: {0}")]
    Codec(#[from] CodecError),
    /// Unsupported version.
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),
    /// Sequence gap.
    #[error("Sequence gap: expected {expected}, got {got}")]
    SequenceGap {
        /// Expected batch number.
        expected: u64,
        /// Actual batch number.
        got: u64,
    },
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn pipeline_error_from_source_error() {
        let source_err = SourceError::Empty;
        let pipeline_err: PipelineError = source_err.into();
        assert!(matches!(pipeline_err, PipelineError::Source(_)));
        assert_eq!(pipeline_err.to_string(), "Source error: No blocks available");
    }

    #[test]
    fn pipeline_error_from_sink_error() {
        let sink_err = SinkError::Timeout;
        let pipeline_err: PipelineError = sink_err.into();
        assert!(matches!(pipeline_err, PipelineError::Sink(_)));
        assert_eq!(pipeline_err.to_string(), "Sink error: Timeout waiting for confirmation");
    }

    #[test]
    fn pipeline_error_from_compression_error() {
        let compression_err = CompressionError::Corrupted;
        let pipeline_err: PipelineError = compression_err.into();
        assert!(matches!(pipeline_err, PipelineError::Compression(_)));
        assert_eq!(
            pipeline_err.to_string(),
            "Compression error: Decompression failed: corrupted data"
        );
    }

    #[test]
    fn pipeline_error_from_codec_error() {
        let codec_err = CodecError::InvalidVersion(0x01);
        let pipeline_err: PipelineError = codec_err.into();
        assert!(matches!(pipeline_err, PipelineError::Codec(_)));
        assert_eq!(pipeline_err.to_string(), "Codec error: Invalid version: 1");
    }

    #[rstest]
    #[case(0x01, "Unsupported version: 1")]
    #[case(0x02, "Unsupported version: 2")]
    #[case(0xFF, "Unsupported version: 255")]
    fn pipeline_error_unsupported_version_display(#[case] version: u8, #[case] expected: &str) {
        let err = PipelineError::UnsupportedVersion(version);
        assert_eq!(err.to_string(), expected);
    }

    #[rstest]
    #[case(1, 2, "Sequence gap: expected 1, got 2")]
    #[case(100, 105, "Sequence gap: expected 100, got 105")]
    #[case(0, 1, "Sequence gap: expected 0, got 1")]
    fn pipeline_error_sequence_gap_display(
        #[case] expected: u64,
        #[case] got: u64,
        #[case] expected_msg: &str,
    ) {
        let err = PipelineError::SequenceGap { expected, got };
        assert_eq!(err.to_string(), expected_msg);
    }

    #[test]
    fn pipeline_error_debug() {
        let err = PipelineError::UnsupportedVersion(0x01);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("UnsupportedVersion"));
    }

    #[rstest]
    #[case(PipelineError::Source(SourceError::Empty))]
    #[case(PipelineError::Sink(SinkError::Timeout))]
    #[case(PipelineError::Compression(CompressionError::Corrupted))]
    #[case(PipelineError::Codec(CodecError::InvalidBlockCount))]
    #[case(PipelineError::UnsupportedVersion(0x01))]
    #[case(PipelineError::SequenceGap { expected: 1, got: 2 })]
    fn pipeline_error_variants_are_debug(#[case] err: PipelineError) {
        let _ = format!("{:?}", err);
    }

    #[test]
    fn pipeline_error_source_connection() {
        let source_err = SourceError::Connection("timeout".to_string());
        let pipeline_err: PipelineError = source_err.into();
        assert_eq!(pipeline_err.to_string(), "Source error: RPC connection failed: timeout");
    }

    #[test]
    fn pipeline_error_sink_blob_gas() {
        let sink_err = SinkError::BlobGasTooExpensive { current: 100, max: 50 };
        let pipeline_err: PipelineError = sink_err.into();
        assert_eq!(pipeline_err.to_string(), "Sink error: Blob gas too expensive: 100 > 50");
    }

    #[test]
    fn pipeline_error_compression_too_large() {
        let compression_err = CompressionError::TooLarge { size: 1000, max: 500 };
        let pipeline_err: PipelineError = compression_err.into();
        assert_eq!(pipeline_err.to_string(), "Compression error: Input too large: 1000 > 500");
    }

    #[test]
    fn pipeline_error_codec_truncated() {
        let codec_err = CodecError::Truncated(42);
        let pipeline_err: PipelineError = codec_err.into();
        assert_eq!(pipeline_err.to_string(), "Codec error: Truncated data at offset 42");
    }
}
