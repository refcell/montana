//! Batch encoding/decoding traits and types.

use primitives::BatchHeader;

use crate::L2BlockData;

/// Codec errors.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// Invalid version.
    #[error("Invalid version: {0}")]
    InvalidVersion(u8),
    /// Truncated data at offset.
    #[error("Truncated data at offset {0}")]
    Truncated(usize),
    /// Invalid block count.
    #[error("Invalid block count")]
    InvalidBlockCount,
}

/// Encodes/decodes batches to wire format.
pub trait BatchCodec: Send + Sync {
    /// Encode blocks into uncompressed batch bytes.
    fn encode(&self, header: &BatchHeader, blocks: &[L2BlockData]) -> Result<Vec<u8>, CodecError>;

    /// Decode uncompressed batch bytes into header + blocks.
    fn decode(&self, data: &[u8]) -> Result<(BatchHeader, Vec<L2BlockData>), CodecError>;
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(0x01, "Invalid version: 1")]
    #[case(0xFF, "Invalid version: 255")]
    fn codec_error_invalid_version_display(#[case] version: u8, #[case] expected: &str) {
        let err = CodecError::InvalidVersion(version);
        assert_eq!(err.to_string(), expected);
    }

    #[rstest]
    #[case(0, "Truncated data at offset 0")]
    #[case(67, "Truncated data at offset 67")]
    #[case(1000, "Truncated data at offset 1000")]
    fn codec_error_truncated_display(#[case] offset: usize, #[case] expected: &str) {
        let err = CodecError::Truncated(offset);
        assert_eq!(err.to_string(), expected);
    }

    #[test]
    fn codec_error_invalid_block_count_display() {
        let err = CodecError::InvalidBlockCount;
        assert_eq!(err.to_string(), "Invalid block count");
    }

    #[test]
    fn codec_error_debug() {
        let err = CodecError::InvalidVersion(0x01);
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InvalidVersion"));
    }
}
