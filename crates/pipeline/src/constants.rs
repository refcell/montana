//! Pipeline constants.

use std::time::Duration;

/// Maximum compressed batch size (1 blob).
pub const MAX_BATCH_SIZE: usize = 128 * 1024;

/// Minimum batch size to avoid dust submissions.
pub const MIN_BATCH_SIZE: usize = 1024;

/// Brotli compression level.
pub const COMPRESSION_LEVEL: u32 = 11;

/// Brotli window size (log2).
pub const COMPRESSION_WINDOW: u32 = 22;

/// Batch submission interval.
pub const BATCH_INTERVAL: Duration = Duration::from_secs(12);

/// Sequencing window in L1 blocks.
pub const SEQUENCING_WINDOW: u64 = 3600;

/// L1 confirmations for safe head.
pub const SAFE_CONFIRMATIONS: u64 = 12;

/// Wire format version.
pub const BATCH_VERSION: u8 = 0x00;

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn max_batch_size_is_128kb() {
        assert_eq!(MAX_BATCH_SIZE, 128 * 1024);
        assert_eq!(MAX_BATCH_SIZE, 131072);
    }

    #[test]
    fn min_batch_size_is_1kb() {
        assert_eq!(MIN_BATCH_SIZE, 1024);
    }

    #[test]
    fn min_batch_size_less_than_max() {
        assert!(MIN_BATCH_SIZE < MAX_BATCH_SIZE);
    }

    #[test]
    fn compression_level_is_max_brotli() {
        assert_eq!(COMPRESSION_LEVEL, 11);
        // Brotli max is 11
        assert!(COMPRESSION_LEVEL <= 11);
    }

    #[test]
    fn compression_window_is_4mb() {
        assert_eq!(COMPRESSION_WINDOW, 22);
        // 2^22 = 4MB window
        assert_eq!(1 << COMPRESSION_WINDOW, 4 * 1024 * 1024);
    }

    #[test]
    fn batch_interval_is_12_seconds() {
        assert_eq!(BATCH_INTERVAL, Duration::from_secs(12));
        assert_eq!(BATCH_INTERVAL.as_secs(), 12);
        assert_eq!(BATCH_INTERVAL.as_millis(), 12000);
    }

    #[test]
    fn sequencing_window_is_one_hour() {
        assert_eq!(SEQUENCING_WINDOW, 3600);
        // At 12 second blocks, this is roughly 12 hours of L1 blocks
        // (3600 blocks * 12 seconds = 43200 seconds = 12 hours)
    }

    #[test]
    fn safe_confirmations_is_12() {
        assert_eq!(SAFE_CONFIRMATIONS, 12);
    }

    #[test]
    fn batch_version_is_zero() {
        assert_eq!(BATCH_VERSION, 0x00);
    }

    #[rstest]
    #[case(MAX_BATCH_SIZE, 128 * 1024, "max batch")]
    #[case(MIN_BATCH_SIZE, 1024, "min batch")]
    fn batch_size_constants(
        #[case] constant: usize,
        #[case] expected: usize,
        #[case] _description: &str,
    ) {
        assert_eq!(constant, expected);
    }

    #[rstest]
    #[case(COMPRESSION_LEVEL, 11, "compression level")]
    #[case(COMPRESSION_WINDOW, 22, "compression window")]
    fn compression_constants(
        #[case] constant: u32,
        #[case] expected: u32,
        #[case] _description: &str,
    ) {
        assert_eq!(constant, expected);
    }

    #[rstest]
    #[case(SEQUENCING_WINDOW, 3600, "sequencing window")]
    #[case(SAFE_CONFIRMATIONS, 12, "safe confirmations")]
    fn l1_constants(#[case] constant: u64, #[case] expected: u64, #[case] _description: &str) {
        assert_eq!(constant, expected);
    }

    #[test]
    fn constants_are_consistent_with_compression_config() {
        use crate::CompressionConfig;

        let default_config = CompressionConfig::default();
        assert_eq!(default_config.level, COMPRESSION_LEVEL);
        assert_eq!(default_config.window_size, COMPRESSION_WINDOW);
    }

    #[test]
    fn constants_are_consistent_with_batch_header() {
        use crate::BatchHeader;

        // BatchHeader::SIZE should be less than MIN_BATCH_SIZE
        // (a batch with just a header is too small to submit)
        assert!(BatchHeader::SIZE < MIN_BATCH_SIZE);
    }
}
