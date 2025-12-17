//! Error types for the sequencer buffer.

/// Errors that can occur when working with the block buffer.
#[derive(Debug, thiserror::Error)]
pub enum BufferError {
    /// Buffer is at capacity and cannot accept more blocks.
    #[error("Buffer is full (capacity: {capacity})")]
    Full {
        /// Maximum buffer capacity.
        capacity: usize,
    },
    /// Failed to acquire buffer lock.
    #[error("Failed to acquire buffer lock")]
    LockFailed,
    /// Block conversion failed.
    #[error("Block conversion failed: {0}")]
    ConversionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_error_full_display() {
        let err = BufferError::Full { capacity: 256 };
        assert_eq!(err.to_string(), "Buffer is full (capacity: 256)");
    }

    #[test]
    fn buffer_error_lock_failed_display() {
        let err = BufferError::LockFailed;
        assert_eq!(err.to_string(), "Failed to acquire buffer lock");
    }

    #[test]
    fn buffer_error_conversion_failed_display() {
        let err = BufferError::ConversionFailed("invalid transaction".to_string());
        assert_eq!(err.to_string(), "Block conversion failed: invalid transaction");
    }

    #[test]
    fn buffer_error_debug() {
        let err = BufferError::Full { capacity: 100 };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Full"));
        assert!(debug.contains("100"));
    }
}
