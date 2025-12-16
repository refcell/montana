//! Zlib compressor implementation.

use std::io::{Read, Write};

use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use montana_pipeline::{CompressionConfig, CompressionError, Compressor};

/// A compressor using the Zlib (DEFLATE) algorithm.
///
/// Zlib provides good compression ratios with fast decompression speeds.
/// It's widely supported and well-suited for general-purpose compression.
///
/// # Compression Levels
///
/// Zlib supports compression levels 0-9:
/// - **0**: No compression (store only)
/// - **1**: Fastest compression (use [`ZlibCompressor::fast()`])
/// - **6**: Balanced compression (use [`ZlibCompressor::balanced()`] or default)
/// - **9**: Maximum compression (use [`ZlibCompressor::best()`])
///
/// # Examples
///
/// ```
/// use montana_zlib::ZlibCompressor;
/// use montana_pipeline::Compressor;
///
/// // Use default balanced compression
/// let compressor = ZlibCompressor::default();
/// let data = b"Hello, World!";
/// let compressed = compressor.compress(data).unwrap();
/// let decompressed = compressor.decompress(&compressed).unwrap();
/// assert_eq!(decompressed, data);
///
/// // Use best compression
/// let best = ZlibCompressor::best();
/// let compressed = best.compress(data).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct ZlibCompressor {
    config: CompressionConfig,
}

impl ZlibCompressor {
    /// Create a new Zlib compressor with the given configuration.
    pub const fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Create a new Zlib compressor with best compression settings.
    ///
    /// Uses compression level 9 for maximum compression.
    pub const fn best() -> Self {
        Self::new(CompressionConfig { level: 9, window_size: 15 })
    }

    /// Create a new Zlib compressor optimized for speed.
    ///
    /// Uses compression level 1 for faster compression at the cost of ratio.
    pub const fn fast() -> Self {
        Self::new(CompressionConfig { level: 1, window_size: 15 })
    }

    /// Create a new Zlib compressor with balanced settings.
    ///
    /// Uses compression level 6 for a balance between speed and ratio.
    pub const fn balanced() -> Self {
        Self::new(CompressionConfig { level: 6, window_size: 15 })
    }

    /// Get the compression configuration.
    pub const fn config(&self) -> &CompressionConfig {
        &self.config
    }
}

impl Default for ZlibCompressor {
    fn default() -> Self {
        Self::balanced()
    }
}

impl Compressor for ZlibCompressor {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(self.config.level));
        encoder.write_all(data).map_err(|e| CompressionError::Failed(e.to_string()))?;
        encoder.finish().map_err(|e| CompressionError::Failed(e.to_string()))
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut decoder = ZlibDecoder::new(data);
        let mut output = Vec::new();
        decoder.read_to_end(&mut output).map_err(|_| CompressionError::Corrupted)?;
        Ok(output)
    }

    fn estimated_ratio(&self) -> f64 {
        // Estimated compression ratios based on compression level
        // Zlib typically achieves lower ratios than Brotli
        match self.config.level {
            0 => 1.0,
            1..=3 => 1.5,
            4..=6 => 2.0,
            _ => 2.5, // Level 7-9
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn zlib_compressor_default() {
        let compressor = ZlibCompressor::default();
        assert_eq!(compressor.config().level, 6);
        assert_eq!(compressor.config().window_size, 15);
    }

    #[test]
    fn zlib_compressor_best() {
        let compressor = ZlibCompressor::best();
        assert_eq!(compressor.config().level, 9);
    }

    #[test]
    fn zlib_compressor_fast() {
        let compressor = ZlibCompressor::fast();
        assert_eq!(compressor.config().level, 1);
    }

    #[test]
    fn zlib_compressor_balanced() {
        let compressor = ZlibCompressor::balanced();
        assert_eq!(compressor.config().level, 6);
    }

    #[rstest]
    #[case(1, 15)]
    #[case(6, 15)]
    #[case(9, 15)]
    fn zlib_compressor_custom_config(#[case] level: u32, #[case] window_size: u32) {
        let config = CompressionConfig { level, window_size };
        let compressor = ZlibCompressor::new(config);
        assert_eq!(compressor.config().level, level);
        assert_eq!(compressor.config().window_size, window_size);
    }

    #[test]
    fn zlib_compressor_compress_empty() {
        let compressor = ZlibCompressor::default();
        let compressed = compressor.compress(&[]).unwrap();
        // Zlib produces a small header even for empty input
        assert!(!compressed.is_empty());
    }

    #[rstest]
    #[case(&[1, 2, 3, 4, 5], "small data")]
    #[case(&[0u8; 100], "zeros")]
    #[case(&[0xde, 0xad, 0xbe, 0xef], "hex data")]
    #[case(b"hello world hello world hello world", "repetitive text")]
    fn zlib_compressor_roundtrip(#[case] data: &[u8], #[case] _description: &str) {
        let compressor = ZlibCompressor::default();
        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn zlib_compressor_roundtrip_large() {
        let compressor = ZlibCompressor::default();
        // Create 10KB of pseudo-random data
        let data: Vec<u8> = (0..10240).map(|i| (i * 17 + 31) as u8).collect();
        let compressed = compressor.compress(&data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed, data);
    }

    #[test]
    fn zlib_compressor_compression_reduces_size() {
        let compressor = ZlibCompressor::default();
        // Highly compressible data (repetitive)
        let data = vec![0u8; 10000];
        let compressed = compressor.compress(&data).unwrap();
        // Should be significantly smaller
        assert!(compressed.len() < data.len() / 10);
    }

    #[test]
    fn zlib_compressor_decompress_invalid_data() {
        let compressor = ZlibCompressor::default();
        let invalid_data = vec![0xFF, 0xFE, 0xFD, 0xFC];
        let result = compressor.decompress(&invalid_data);
        assert!(matches!(result, Err(CompressionError::Corrupted)));
    }

    #[rstest]
    #[case(0, 1.0)]
    #[case(1, 1.5)]
    #[case(3, 1.5)]
    #[case(4, 2.0)]
    #[case(6, 2.0)]
    #[case(7, 2.5)]
    #[case(9, 2.5)]
    fn zlib_compressor_estimated_ratio(#[case] level: u32, #[case] expected_ratio: f64) {
        let compressor = ZlibCompressor::new(CompressionConfig { level, window_size: 15 });
        assert_eq!(compressor.estimated_ratio(), expected_ratio);
    }

    #[test]
    fn zlib_compressor_debug() {
        let compressor = ZlibCompressor::default();
        let debug_str = format!("{:?}", compressor);
        assert!(debug_str.contains("ZlibCompressor"));
        assert!(debug_str.contains("config"));
    }

    #[test]
    fn zlib_compressor_clone() {
        let compressor = ZlibCompressor::new(CompressionConfig { level: 5, window_size: 15 });
        let cloned = compressor.clone();
        assert_eq!(cloned.config().level, compressor.config().level);
        assert_eq!(cloned.config().window_size, compressor.config().window_size);
    }

    #[test]
    fn zlib_compressor_deterministic() {
        let compressor = ZlibCompressor::default();
        let data = b"test data for deterministic compression";

        let compressed1 = compressor.compress(data).unwrap();
        let compressed2 = compressor.compress(data).unwrap();

        assert_eq!(compressed1, compressed2);
    }

    #[rstest]
    #[case(ZlibCompressor::fast())]
    #[case(ZlibCompressor::balanced())]
    #[case(ZlibCompressor::best())]
    fn zlib_compressor_all_presets_work(#[case] compressor: ZlibCompressor) {
        let data = b"test data for all presets";
        let compressed = compressor.compress(data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(decompressed.as_slice(), data);
    }

    #[test]
    fn zlib_higher_level_better_compression() {
        let data = b"hello world ".repeat(1000);

        let fast = ZlibCompressor::fast();
        let best = ZlibCompressor::best();

        let fast_compressed = fast.compress(&data).unwrap();
        let best_compressed = best.compress(&data).unwrap();

        // Higher compression level should produce smaller or equal output
        assert!(best_compressed.len() <= fast_compressed.len());
    }
}
