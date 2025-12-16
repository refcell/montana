//! Zstandard compressor implementation.

use montana_pipeline::{CompressionConfig, CompressionError, Compressor};

/// A compressor using the Zstandard algorithm.
///
/// Zstandard provides excellent compression ratios with very fast decompression,
/// making it ideal for L2 batch compression where derivation speed matters.
#[derive(Debug, Clone)]
pub struct ZstdCompressor {
    config: CompressionConfig,
}

impl ZstdCompressor {
    /// Create a new Zstandard compressor with the given configuration.
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Create a new Zstandard compressor with best compression settings.
    ///
    /// Uses compression level 19 for maximum compression.
    pub fn best() -> Self {
        Self::new(CompressionConfig { level: 19, window_size: 0 })
    }

    /// Create a new Zstandard compressor optimized for speed.
    ///
    /// Uses compression level 1 for faster compression at the cost of ratio.
    pub fn fast() -> Self {
        Self::new(CompressionConfig { level: 1, window_size: 0 })
    }

    /// Create a new Zstandard compressor with balanced settings.
    ///
    /// Uses compression level 3 (zstd default) for a balance between speed and ratio.
    pub fn balanced() -> Self {
        Self::new(CompressionConfig { level: 3, window_size: 0 })
    }

    /// Get the compression configuration.
    pub const fn config(&self) -> &CompressionConfig {
        &self.config
    }
}

impl Default for ZstdCompressor {
    fn default() -> Self {
        Self::balanced()
    }
}

impl Compressor for ZstdCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        zstd::bulk::compress(data, self.config.level as i32)
            .map_err(|e| CompressionError::Failed(e.to_string()))
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        zstd::bulk::decompress(data, 10 * 1024 * 1024) // 10MB max decompressed size
            .map_err(|_| CompressionError::Corrupted)
    }

    fn estimated_ratio(&self) -> f64 {
        // Estimated compression ratios based on compression level
        // Zstd typically achieves ratios between Brotli and Zlib
        match self.config.level {
            0..=2 => 2.0,
            3..=6 => 3.0,
            7..=12 => 4.0,
            _ => 5.0, // Level 13+
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn zstd_compressor_default() {
        let compressor = ZstdCompressor::default();
        assert_eq!(compressor.config().level, 3);
    }

    #[test]
    fn zstd_compressor_best() {
        let compressor = ZstdCompressor::best();
        assert_eq!(compressor.config().level, 19);
    }

    #[test]
    fn zstd_compressor_fast() {
        let compressor = ZstdCompressor::fast();
        assert_eq!(compressor.config().level, 1);
    }

    #[test]
    fn zstd_compressor_balanced() {
        let compressor = ZstdCompressor::balanced();
        assert_eq!(compressor.config().level, 3);
    }

    #[rstest]
    #[case(1, 0)]
    #[case(3, 0)]
    #[case(19, 0)]
    fn zstd_compressor_custom_config(#[case] level: u32, #[case] window_size: u32) {
        let config = CompressionConfig { level, window_size };
        let compressor = ZstdCompressor::new(config);
        assert_eq!(compressor.config().level, level);
        assert_eq!(compressor.config().window_size, window_size);
    }

    #[test]
    fn zstd_compressor_compress_empty() {
        let compressor = ZstdCompressor::default();
        let compressed = compressor.compress(&[]).unwrap();
        // Zstd produces a small header even for empty input
        assert!(!compressed.is_empty());
    }

    #[rstest]
    #[case(&[1, 2, 3, 4, 5], "small data")]
    #[case(&[0u8; 100], "zeros")]
    #[case(&[0xde, 0xad, 0xbe, 0xef], "hex data")]
    #[case(b"hello world hello world hello world", "repetitive text")]
    fn zstd_compressor_roundtrip(#[case] data: &[u8], #[case] _description: &str) {
        let compressor = ZstdCompressor::default();
        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn zstd_compressor_roundtrip_large() {
        let compressor = ZstdCompressor::default();
        // Create 10KB of pseudo-random data
        let data: Vec<u8> = (0..10240).map(|i| (i * 17 + 31) as u8).collect();
        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn zstd_compressor_compression_reduces_size() {
        let compressor = ZstdCompressor::default();
        // Highly compressible data (repetitive)
        let data = vec![0u8; 10000];
        let compressed = compressor.compress(&data).unwrap();
        // Should be significantly smaller
        assert!(compressed.len() < data.len() / 10);
    }

    #[test]
    fn zstd_compressor_decompress_invalid_data() {
        let compressor = ZstdCompressor::default();
        let invalid_data = vec![0xFF, 0xFE, 0xFD, 0xFC];
        let result = compressor.decompress(&invalid_data);
        assert!(matches!(result, Err(CompressionError::Corrupted)));
    }

    #[rstest]
    #[case(1, 2.0)]
    #[case(2, 2.0)]
    #[case(3, 3.0)]
    #[case(6, 3.0)]
    #[case(7, 4.0)]
    #[case(12, 4.0)]
    #[case(13, 5.0)]
    #[case(19, 5.0)]
    fn zstd_compressor_estimated_ratio(#[case] level: u32, #[case] expected_ratio: f64) {
        let compressor = ZstdCompressor::new(CompressionConfig { level, window_size: 0 });
        assert_eq!(compressor.estimated_ratio(), expected_ratio);
    }

    #[test]
    fn zstd_compressor_debug() {
        let compressor = ZstdCompressor::default();
        let debug_str = format!("{:?}", compressor);
        assert!(debug_str.contains("ZstdCompressor"));
        assert!(debug_str.contains("config"));
    }

    #[test]
    fn zstd_compressor_clone() {
        let compressor = ZstdCompressor::new(CompressionConfig { level: 5, window_size: 0 });
        let cloned = compressor.clone();
        assert_eq!(cloned.config().level, compressor.config().level);
        assert_eq!(cloned.config().window_size, compressor.config().window_size);
    }

    #[test]
    fn zstd_compressor_deterministic() {
        let compressor = ZstdCompressor::default();
        let data = b"test data for deterministic compression";

        let compressed1 = compressor.compress(data).unwrap();
        let compressed2 = compressor.compress(data).unwrap();

        assert_eq!(compressed1, compressed2);
    }

    #[rstest]
    #[case(ZstdCompressor::fast())]
    #[case(ZstdCompressor::balanced())]
    #[case(ZstdCompressor::best())]
    fn zstd_compressor_all_presets_work(#[case] compressor: ZstdCompressor) {
        let data = b"test data for all presets";
        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed.as_slice(), data);
    }

    #[test]
    fn zstd_higher_level_better_compression() {
        let data = b"hello world ".repeat(1000);

        let fast = ZstdCompressor::fast();
        let best = ZstdCompressor::best();

        let fast_compressed = fast.compress(&data).unwrap();
        let best_compressed = best.compress(&data).unwrap();

        // Higher compression level should produce smaller or equal output
        assert!(best_compressed.len() <= fast_compressed.len());
    }
}
