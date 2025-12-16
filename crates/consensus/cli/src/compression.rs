//! Compression algorithm selection for CLI.

use clap::ValueEnum;

/// Available compression algorithms.
///
/// This enum provides selection between different compression algorithms for batch
/// submission and derivation. The `All` variant enables comparison mode where all
/// algorithms are run and their results are compared.
///
/// # Examples
///
/// ```
/// use montana_cli::CompressionAlgorithm;
///
/// // Get the default algorithm
/// let default = CompressionAlgorithm::default();
/// assert_eq!(default, CompressionAlgorithm::Brotli);
///
/// // Iterate over all single compression algorithms
/// let algorithms: Vec<_> = CompressionAlgorithm::all_algorithms().collect();
/// assert_eq!(algorithms.len(), 3);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum CompressionAlgorithm {
    /// Brotli compression (default).
    ///
    /// Provides high compression ratios, commonly used in web contexts.
    #[default]
    Brotli,
    /// Zlib (DEFLATE) compression.
    ///
    /// Widely supported compression format with good balance of speed and ratio.
    Zlib,
    /// Zstandard compression.
    ///
    /// Modern compression algorithm with tunable compression levels and fast decompression.
    Zstd,
    /// Run all compression algorithms and compare results.
    ///
    /// Executes all three compression algorithms and provides comparative metrics.
    All,
}

impl CompressionAlgorithm {
    /// Returns an iterator over all single compression algorithms (excludes All).
    ///
    /// This is useful when you need to iterate over the concrete compression algorithms
    /// without including the `All` comparison mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use montana_cli::CompressionAlgorithm;
    ///
    /// for algo in CompressionAlgorithm::all_algorithms() {
    ///     println!("Algorithm: {}", algo);
    /// }
    /// ```
    pub fn all_algorithms() -> impl Iterator<Item = Self> {
        [Self::Brotli, Self::Zlib, Self::Zstd].into_iter()
    }
}

impl std::fmt::Display for CompressionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Brotli => write!(f, "brotli"),
            Self::Zlib => write!(f, "zlib"),
            Self::Zstd => write!(f, "zstd"),
            Self::All => write!(f, "all"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compression_algorithm_default() {
        assert_eq!(CompressionAlgorithm::default(), CompressionAlgorithm::Brotli);
    }

    #[test]
    fn compression_algorithm_display() {
        assert_eq!(CompressionAlgorithm::Brotli.to_string(), "brotli");
        assert_eq!(CompressionAlgorithm::Zlib.to_string(), "zlib");
        assert_eq!(CompressionAlgorithm::Zstd.to_string(), "zstd");
        assert_eq!(CompressionAlgorithm::All.to_string(), "all");
    }

    #[test]
    fn compression_algorithm_clone() {
        let algo = CompressionAlgorithm::Brotli;
        let cloned = algo;
        assert_eq!(algo, cloned);
    }

    #[test]
    fn compression_algorithm_debug() {
        let debug = format!("{:?}", CompressionAlgorithm::Brotli);
        assert!(debug.contains("Brotli"));
    }

    #[test]
    fn compression_algorithm_all_algorithms() {
        let algos: Vec<_> = CompressionAlgorithm::all_algorithms().collect();
        assert_eq!(algos.len(), 3);
        assert!(algos.contains(&CompressionAlgorithm::Brotli));
        assert!(algos.contains(&CompressionAlgorithm::Zlib));
        assert!(algos.contains(&CompressionAlgorithm::Zstd));
    }
}
