//! Storage value deserialization for PlainStorageState.
//!
//! Decodes storage entries from Reth's DupSort table format.

use alloy_primitives::{B256, U256};

/// Decoded storage entry from PlainStorageState.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageEntry {
    /// Storage slot key (the SubKey in DupSort).
    pub slot: B256,
    /// Storage value.
    pub value: U256,
}

/// Decode a PlainStorageState value (DupSort format).
///
/// Format: `[SubKey: B256 (32 bytes)][Value: compact U256 (variable)]`
///
/// The U256 value is stored in compact big-endian form with leading zeros stripped.
///
/// # Errors
/// Returns an error if the data is malformed.
pub fn decode_storage_entry(bytes: &[u8]) -> Result<StorageEntry, StorageDecodeError> {
    if bytes.len() < 32 {
        return Err(StorageDecodeError::InsufficientData { expected: 32, got: bytes.len() });
    }

    let slot = B256::from_slice(&bytes[..32]);
    let value_bytes = &bytes[32..];

    // U256 is stored in compact big-endian (leading zeros stripped)
    let value = if value_bytes.is_empty() {
        U256::ZERO
    } else if value_bytes.len() <= 32 {
        let mut padded = [0u8; 32];
        padded[32 - value_bytes.len()..].copy_from_slice(value_bytes);
        U256::from_be_bytes(padded)
    } else {
        return Err(StorageDecodeError::InvalidValueLength { got: value_bytes.len() });
    };

    Ok(StorageEntry { slot, value })
}

/// Error type for storage decoding failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageDecodeError {
    /// Not enough bytes for the slot.
    InsufficientData {
        /// Expected minimum bytes.
        expected: usize,
        /// Actual bytes available.
        got: usize,
    },
    /// Value portion is too long for U256.
    InvalidValueLength {
        /// Actual length.
        got: usize,
    },
}

impl std::fmt::Display for StorageDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientData { expected, got } => {
                write!(f, "insufficient data: expected {} bytes, got {}", expected, got)
            }
            Self::InvalidValueLength { got } => {
                write!(f, "value too long for U256: {} bytes", got)
            }
        }
    }
}

impl std::error::Error for StorageDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_storage_zero_value() {
        let mut bytes = vec![0u8; 32]; // slot = 0x00...00
        bytes[31] = 0x42; // slot = 0x00...42
        // No value bytes = U256::ZERO

        let entry = decode_storage_entry(&bytes).unwrap();
        assert_eq!(entry.slot, B256::from_slice(&bytes));
        assert_eq!(entry.value, U256::ZERO);
    }

    #[test]
    fn test_decode_storage_with_value() {
        let mut bytes = vec![0u8; 32]; // slot
        bytes[31] = 0x01;
        bytes.push(0x42); // value = 0x42

        let entry = decode_storage_entry(&bytes).unwrap();
        assert_eq!(entry.value, U256::from(0x42));
    }

    #[test]
    fn test_insufficient_data() {
        let bytes = vec![0u8; 20]; // Only 20 bytes, need at least 32
        let result = decode_storage_entry(&bytes);
        assert!(result.is_err());
    }
}
