//! Error types for the sequencer buffer.

/// Errors that can occur when working with the block buffer.
#[derive(Debug, derive_more::Display)]
pub enum BufferError {
    /// Buffer is at capacity and cannot accept more blocks.
    #[display("Buffer is full (capacity: {capacity})")]
    Full {
        /// Maximum buffer capacity.
        capacity: usize,
    },
    /// Failed to acquire buffer lock.
    #[display("Failed to acquire buffer lock")]
    LockFailed,
    /// Block conversion failed.
    #[display("Block conversion failed: {_0}")]
    ConversionFailed(String),
}

impl std::error::Error for BufferError {}

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
