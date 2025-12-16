//! Brotli compressor implementation.

use std::io::Cursor;

use montana_pipeline::{CompressionConfig, CompressionError, Compressor};

/// A compressor using the Brotli algorithm.
///
/// Brotli provides excellent compression ratios, especially for text-like data
/// such as RLP-encoded transactions. The default configuration uses maximum
/// compression (level 11) with a 4MB window for optimal batch sizes.
#[derive(Debug, Clone)]
pub struct BrotliCompressor {
    config: CompressionConfig,
}

impl BrotliCompressor {
    /// Create a new Brotli compressor with the given configuration.
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Create a new Brotli compressor with maximum compression settings.
    ///
    /// Uses compression level 11 and a 4MB window (log2 = 22).
    pub fn max_compression() -> Self {
        Self::new(CompressionConfig::default())
    }

    /// Create a new Brotli compressor optimized for speed.
    ///
    /// Uses compression level 1 for faster compression at the cost of ratio.
    pub fn fast() -> Self {
        Self::new(CompressionConfig { level: 1, window_size: 22 })
    }

    /// Create a new Brotli compressor with balanced settings.
    ///
    /// Uses compression level 6 for a balance between speed and ratio.
    pub fn balanced() -> Self {
        Self::new(CompressionConfig { level: 6, window_size: 22 })
    }

    /// Get the compression configuration.
    pub const fn config(&self) -> &CompressionConfig {
        &self.config
    }
}

impl Default for BrotliCompressor {
    fn default() -> Self {
        Self::max_compression()
    }
}

impl Compressor for BrotliCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut output = Vec::new();
        let params = brotli::enc::BrotliEncoderParams {
            quality: self.config.level as i32,
            lgwin: self.config.window_size as i32,
            ..Default::default()
        };

        brotli::BrotliCompress(&mut Cursor::new(data), &mut output, &params)
            .map_err(|e| CompressionError::Failed(e.to_string()))?;

        Ok(output)
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut output = Vec::new();
        brotli::BrotliDecompress(&mut Cursor::new(data), &mut output)
            .map_err(|_| CompressionError::Corrupted)?;
        Ok(output)
    }

    fn estimated_ratio(&self) -> f64 {
        // Estimated compression ratios based on compression level
        // These are conservative estimates for RLP-encoded transaction data
        match self.config.level {
            0..=3 => 2.0,
            4..=6 => 3.5,
            7..=9 => 5.0,
            _ => 6.0, // Level 10-11
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn brotli_compressor_default() {
        let compressor = BrotliCompressor::default();
        assert_eq!(compressor.config().level, 11);
        assert_eq!(compressor.config().window_size, 22);
    }

    #[test]
    fn brotli_compressor_max_compression() {
        let compressor = BrotliCompressor::max_compression();
        assert_eq!(compressor.config().level, 11);
        assert_eq!(compressor.config().window_size, 22);
    }

    #[test]
    fn brotli_compressor_fast() {
        let compressor = BrotliCompressor::fast();
        assert_eq!(compressor.config().level, 1);
    }

    #[test]
    fn brotli_compressor_balanced() {
        let compressor = BrotliCompressor::balanced();
        assert_eq!(compressor.config().level, 6);
    }

    #[rstest]
    #[case(1, 20)]
    #[case(6, 22)]
    #[case(11, 22)]
    #[case(11, 24)]
    fn brotli_compressor_custom_config(#[case] level: u32, #[case] window_size: u32) {
        let config = CompressionConfig { level, window_size };
        let compressor = BrotliCompressor::new(config);
        assert_eq!(compressor.config().level, level);
        assert_eq!(compressor.config().window_size, window_size);
    }

    #[test]
    fn brotli_compressor_compress_empty() {
        let compressor = BrotliCompressor::default();
        let compressed = compressor.compress(&[]).unwrap();
        // Brotli produces a small header even for empty input
        assert!(!compressed.is_empty());
    }

    #[rstest]
    #[case(&[1, 2, 3, 4, 5], "small data")]
    #[case(&[0u8; 100], "zeros")]
    #[case(&[0xde, 0xad, 0xbe, 0xef], "hex data")]
    #[case(b"hello world hello world hello world", "repetitive text")]
    fn brotli_compressor_roundtrip(#[case] data: &[u8], #[case] _description: &str) {
        let compressor = BrotliCompressor::default();
        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn brotli_compressor_roundtrip_large() {
        let compressor = BrotliCompressor::default();
        // Create 10KB of pseudo-random data
        let data: Vec<u8> = (0..10240).map(|i| (i * 17 + 31) as u8).collect();
        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn brotli_compressor_compression_reduces_size() {
        let compressor = BrotliCompressor::default();
        // Highly compressible data (repetitive)
        let data = vec![0u8; 10000];
        let compressed = compressor.compress(&data).unwrap();
        // Should be significantly smaller
        assert!(compressed.len() < data.len() / 10);
    }

    #[test]
    fn brotli_compressor_decompress_invalid_data() {
        let compressor = BrotliCompressor::default();
        let invalid_data = vec![0xFF, 0xFE, 0xFD, 0xFC];
        let result = compressor.decompress(&invalid_data);
        assert!(matches!(result, Err(CompressionError::Corrupted)));
    }

    #[rstest]
    #[case(1, 2.0)]
    #[case(3, 2.0)]
    #[case(4, 3.5)]
    #[case(6, 3.5)]
    #[case(7, 5.0)]
    #[case(9, 5.0)]
    #[case(10, 6.0)]
    #[case(11, 6.0)]
    fn brotli_compressor_estimated_ratio(#[case] level: u32, #[case] expected_ratio: f64) {
        let compressor = BrotliCompressor::new(CompressionConfig { level, window_size: 22 });
        assert_eq!(compressor.estimated_ratio(), expected_ratio);
    }

    #[test]
    fn brotli_compressor_debug() {
        let compressor = BrotliCompressor::default();
        let debug_str = format!("{:?}", compressor);
        assert!(debug_str.contains("BrotliCompressor"));
        assert!(debug_str.contains("config"));
    }

    #[test]
    fn brotli_compressor_clone() {
        let compressor = BrotliCompressor::new(CompressionConfig { level: 5, window_size: 20 });
        let cloned = compressor.clone();
        assert_eq!(cloned.config().level, compressor.config().level);
        assert_eq!(cloned.config().window_size, compressor.config().window_size);
    }

    #[test]
    fn brotli_compressor_deterministic() {
        let compressor = BrotliCompressor::default();
        let data = b"test data for deterministic compression";

        let compressed1 = compressor.compress(data).unwrap();
        let compressed2 = compressor.compress(data).unwrap();

        assert_eq!(compressed1, compressed2);
    }

    #[rstest]
    #[case(BrotliCompressor::fast())]
    #[case(BrotliCompressor::balanced())]
    #[case(BrotliCompressor::max_compression())]
    fn brotli_compressor_all_presets_work(#[case] compressor: BrotliCompressor) {
        let data = b"test data for all presets";
        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed.as_slice(), data);
    }

    #[test]
    fn brotli_higher_level_better_compression() {
        let data = b"hello world ".repeat(1000);

        let fast = BrotliCompressor::fast();
        let max = BrotliCompressor::max_compression();

        let fast_compressed = fast.compress(&data).unwrap();
        let max_compressed = max.compress(&data).unwrap();

        // Higher compression level should produce smaller output
        assert!(max_compressed.len() <= fast_compressed.len());
    }
}
