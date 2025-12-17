//! Bytecode deserialization for Reth Bytecodes table.
//!
//! Decodes contract bytecode from Reth's database format into revm's Bytecode type.

use alloy_primitives::Bytes;
use revm::bytecode::Bytecode;

/// Decode bytecode from Reth's Bytecodes table.
///
/// Reth stores bytecode in raw form. This creates a `Bytecode::new_raw()` which
/// will be analyzed by revm when needed.
///
/// # Errors
/// Returns an error if the bytecode format is invalid.
pub fn decode_bytecode(bytes: &[u8]) -> Result<Bytecode, BytecodeDecodeError> {
    if bytes.is_empty() {
        return Ok(Bytecode::default());
    }

    // Create raw bytecode - revm will analyze it when needed
    Ok(Bytecode::new_raw(Bytes::copy_from_slice(bytes)))
}

/// Error type for bytecode decoding failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BytecodeDecodeError {
    /// Invalid bytecode format.
    InvalidFormat,
}

impl std::fmt::Display for BytecodeDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat => write!(f, "invalid bytecode format"),
        }
    }
}

impl std::error::Error for BytecodeDecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_bytecode() {
        let bytes = [];
        let bytecode = decode_bytecode(&bytes).unwrap();
        assert!(bytecode.is_empty());
    }

    #[test]
    fn test_decode_simple_bytecode() {
        // Simple EVM bytecode: PUSH1 0x42 STOP
        let bytes = [0x60, 0x42, 0x00];
        let bytecode = decode_bytecode(&bytes).unwrap();
        assert!(!bytecode.is_empty());
    }
}
