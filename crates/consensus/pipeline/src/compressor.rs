//! Compression traits and types.

/// Compressor configuration.
#[derive(Clone, Debug)]
pub struct CompressionConfig {
    /// Compression level (1-11 for Brotli).
    pub level: u32,
    /// Log2 of window size.
    pub window_size: u32,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            level: 11,       // Maximum compression
            window_size: 22, // 4MB window
        }
    }
}

/// Compression errors.
#[derive(Debug, thiserror::Error)]
pub enum CompressionError {
    /// Compression failed.
    #[error("Compression failed: {0}")]
    Failed(String),
    /// Decompression failed due to corrupted data.
    #[error("Decompression failed: corrupted data")]
    Corrupted,
    /// Input too large.
    #[error("Input too large: {size} > {max}")]
    TooLarge {
        /// Actual size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },
}

/// Compresses raw batch data.
///
/// Implementations:
/// - `BrotliCompressor`: Brotli algorithm (default)
/// - `ZstdCompressor`: Alternative for testing
/// - `NoopCompressor`: Passthrough for debugging
pub trait Compressor: Send + Sync {
    /// Compress data. Must be deterministic.
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Decompress data. Must roundtrip with compress.
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Estimated compression ratio for capacity planning.
    fn estimated_ratio(&self) -> f64;
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn compression_config_default() {
        let config = CompressionConfig::default();
        assert_eq!(config.level, 11);
        assert_eq!(config.window_size, 22);
    }

    #[rstest]
    #[case(1, 20, "minimum compression")]
    #[case(6, 22, "medium compression")]
    #[case(11, 22, "maximum compression")]
    #[case(11, 24, "large window")]
    fn compression_config_custom(
        #[case] level: u32,
        #[case] window_size: u32,
        #[case] _description: &str,
    ) {
        let config = CompressionConfig { level, window_size };
        assert_eq!(config.level, level);
        assert_eq!(config.window_size, window_size);
    }

    #[test]
    fn compression_config_clone() {
        let config = CompressionConfig { level: 9, window_size: 20 };
        let cloned = config.clone();
        assert_eq!(cloned.level, config.level);
        assert_eq!(cloned.window_size, config.window_size);
    }

    #[test]
    fn compression_config_debug() {
        let config = CompressionConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("CompressionConfig"));
        assert!(debug_str.contains("level"));
        assert!(debug_str.contains("window_size"));
    }

    #[rstest]
    #[case("io error", "Compression failed: io error")]
    #[case("buffer overflow", "Compression failed: buffer overflow")]
    #[case("", "Compression failed: ")]
    fn compression_error_failed_display(#[case] msg: &str, #[case] expected: &str) {
        let err = CompressionError::Failed(msg.to_string());
        assert_eq!(err.to_string(), expected);
    }

    #[test]
    fn compression_error_corrupted_display() {
        let err = CompressionError::Corrupted;
        assert_eq!(err.to_string(), "Decompression failed: corrupted data");
    }

    #[rstest]
    #[case(1000, 512, "Input too large: 1000 > 512")]
    #[case(128 * 1024 + 1, 128 * 1024, "Input too large: 131073 > 131072")]
    #[case(usize::MAX, 0, format!("Input too large: {} > 0", usize::MAX))]
    fn compression_error_too_large_display(
        #[case] size: usize,
        #[case] max: usize,
        #[case] expected: String,
    ) {
        let err = CompressionError::TooLarge { size, max };
        assert_eq!(err.to_string(), expected);
    }

    #[test]
    fn compression_error_debug() {
        let err = CompressionError::Corrupted;
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Corrupted"));
    }

    #[rstest]
    #[case(CompressionError::Failed("test".into()))]
    #[case(CompressionError::Corrupted)]
    #[case(CompressionError::TooLarge { size: 100, max: 50 })]
    fn compression_error_variants_are_debug(#[case] err: CompressionError) {
        // All variants should implement Debug
        let _ = format!("{:?}", err);
    }
}
