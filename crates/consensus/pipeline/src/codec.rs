//! Batch encoding/decoding traits and types.

use crate::L2BlockData;

/// Batch header: fixed 19 bytes.
/// All integers little-endian.
#[derive(Clone, Debug)]
pub struct BatchHeader {
    /// Wire format version (0x00).
    pub version: u8,
    /// Monotonic sequence number.
    pub batch_number: u64,
    /// First block timestamp.
    pub timestamp: u64,
    /// Number of L2 blocks.
    pub block_count: u16,
}

impl BatchHeader {
    /// Size of the batch header in bytes.
    pub const SIZE: usize = 1 + 8 + 8 + 2; // 19 bytes
}

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

    #[test]
    fn batch_header_size_is_19_bytes() {
        assert_eq!(BatchHeader::SIZE, 19);
    }

    #[rstest]
    #[case(1, 8, 8, 2, 19)]
    fn batch_header_size_components(
        #[case] version: usize,
        #[case] batch_number: usize,
        #[case] timestamp: usize,
        #[case] block_count: usize,
        #[case] expected_total: usize,
    ) {
        let total = version + batch_number + timestamp + block_count;
        assert_eq!(total, expected_total);
        assert_eq!(total, BatchHeader::SIZE);
    }

    #[test]
    fn batch_header_default_values() {
        let header = BatchHeader { version: 0x00, batch_number: 0, timestamp: 0, block_count: 0 };
        assert_eq!(header.version, 0x00);
        assert_eq!(header.batch_number, 0);
        assert_eq!(header.block_count, 0);
    }

    #[rstest]
    #[case(0x00, true)]
    #[case(0x01, false)]
    #[case(0xFF, false)]
    fn batch_header_version_validity(#[case] version: u8, #[case] is_valid: bool) {
        // Currently version 0x00 is the only valid version
        let valid = version == 0x00;
        assert_eq!(valid, is_valid);
    }

    #[rstest]
    #[case(0, "first batch")]
    #[case(1, "second batch")]
    #[case(u64::MAX, "max batch number")]
    fn batch_header_batch_numbers(#[case] batch_number: u64, #[case] _description: &str) {
        let header = BatchHeader { version: 0x00, batch_number, timestamp: 1000, block_count: 10 };
        assert_eq!(header.batch_number, batch_number);
    }

    #[rstest]
    #[case(0, "no blocks")]
    #[case(1, "single block")]
    #[case(100, "many blocks")]
    #[case(u16::MAX, "max blocks")]
    fn batch_header_block_counts(#[case] block_count: u16, #[case] _description: &str) {
        let header = BatchHeader { version: 0x00, batch_number: 1, timestamp: 1000, block_count };
        assert_eq!(header.block_count, block_count);
    }

    #[test]
    fn batch_header_clone() {
        let header =
            BatchHeader { version: 0x00, batch_number: 42, timestamp: 1234567890, block_count: 50 };
        let cloned = header.clone();
        assert_eq!(cloned.version, header.version);
        assert_eq!(cloned.batch_number, header.batch_number);
        assert_eq!(cloned.timestamp, header.timestamp);
        assert_eq!(cloned.block_count, header.block_count);
    }

    #[test]
    fn batch_header_debug() {
        let header =
            BatchHeader { version: 0x00, batch_number: 1, timestamp: 1000, block_count: 5 };
        let debug_str = format!("{:?}", header);
        assert!(debug_str.contains("BatchHeader"));
        assert!(debug_str.contains("version"));
        assert!(debug_str.contains("batch_number"));
    }

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
