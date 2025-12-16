//! Noop compressor implementation for testing and benchmarking.

use montana_pipeline::{CompressionError, Compressor};

/// A no-operation compressor that passes data through unchanged.
///
/// Useful for benchmarking the pipeline without compression overhead,
/// or for debugging to see the raw batch data.
#[derive(Debug, Clone, Default)]
pub struct NoopCompressor;

impl NoopCompressor {
    /// Create a new noop compressor.
    pub const fn new() -> Self {
        Self
    }
}

impl Compressor for NoopCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        Ok(data.to_vec())
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        Ok(data.to_vec())
    }

    fn estimated_ratio(&self) -> f64 {
        1.0 // No compression
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn noop_compressor_new() {
        let compressor = NoopCompressor::new();
        assert_eq!(compressor.estimated_ratio(), 1.0);
    }

    #[test]
    fn noop_compressor_default() {
        let compressor = NoopCompressor::default();
        assert_eq!(compressor.estimated_ratio(), 1.0);
    }

    #[rstest]
    #[case(&[], "empty data")]
    #[case(&[1, 2, 3], "small data")]
    #[case(&[0u8; 1000], "large data")]
    #[case(&[0xde, 0xad, 0xbe, 0xef], "hex data")]
    fn noop_compressor_compress(#[case] data: &[u8], #[case] _description: &str) {
        let compressor = NoopCompressor::new();
        let compressed = compressor.compress(data).unwrap();
        assert_eq!(compressed, data);
    }

    #[rstest]
    #[case(&[], "empty data")]
    #[case(&[1, 2, 3], "small data")]
    #[case(&[0u8; 1000], "large data")]
    fn noop_compressor_decompress(#[case] data: &[u8], #[case] _description: &str) {
        let compressor = NoopCompressor::new();
        let decompressed = compressor.decompress(data).unwrap();
        assert_eq!(decompressed, data);
    }

    #[rstest]
    #[case(&[1, 2, 3, 4, 5])]
    #[case(&[0u8; 100])]
    #[case(&[0xff; 50])]
    fn noop_compressor_roundtrip(#[case] data: &[u8]) {
        let compressor = NoopCompressor::new();
        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn noop_compressor_estimated_ratio() {
        let compressor = NoopCompressor::new();
        assert_eq!(compressor.estimated_ratio(), 1.0);
    }

    #[test]
    fn noop_compressor_debug() {
        let compressor = NoopCompressor::new();
        let debug_str = format!("{:?}", compressor);
        assert!(debug_str.contains("NoopCompressor"));
    }

    #[test]
    fn noop_compressor_clone() {
        let compressor = NoopCompressor::new();
        let cloned = compressor.clone();
        assert_eq!(cloned.estimated_ratio(), compressor.estimated_ratio());
    }
}
