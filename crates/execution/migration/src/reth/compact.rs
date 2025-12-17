//! Compact encoding utilities for Reth database values.
//!
//! Reth stores integers in big-endian with leading zeros stripped.
//! The length is stored in bitfield flags.

use alloy_primitives::U256;

/// Read a compact-encoded u64 with the given byte length.
///
/// Returns the decoded value and the remaining buffer.
pub(super) fn read_compact_u64(bytes: &[u8], len: usize) -> (u64, &[u8]) {
    if len == 0 {
        return (0, bytes);
    }

    let mut value_bytes = [0u8; 8];
    value_bytes[8 - len..].copy_from_slice(&bytes[..len]);
    (u64::from_be_bytes(value_bytes), &bytes[len..])
}

/// Read a compact-encoded U256 with the given byte length.
///
/// Returns the decoded value and the remaining buffer.
pub(super) fn read_compact_u256(bytes: &[u8], len: usize) -> (U256, &[u8]) {
    if len == 0 {
        return (U256::ZERO, bytes);
    }

    let mut value_bytes = [0u8; 32];
    value_bytes[32 - len..].copy_from_slice(&bytes[..len]);
    (U256::from_be_bytes(value_bytes), &bytes[len..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_compact_u64_zero() {
        let bytes = &[0x01, 0x02];
        let (val, rest) = read_compact_u64(bytes, 0);
        assert_eq!(val, 0);
        assert_eq!(rest, bytes);
    }

    #[test]
    fn test_read_compact_u64_single_byte() {
        let bytes = &[0x42, 0xFF];
        let (val, rest) = read_compact_u64(bytes, 1);
        assert_eq!(val, 0x42);
        assert_eq!(rest, &[0xFF]);
    }

    #[test]
    fn test_read_compact_u256_zero() {
        let bytes = &[0x01];
        let (val, rest) = read_compact_u256(bytes, 0);
        assert_eq!(val, U256::ZERO);
        assert_eq!(rest, bytes);
    }
}
