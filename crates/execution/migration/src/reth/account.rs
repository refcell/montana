//! Reth Account deserialization.
//!
//! Decodes accounts from Reth's Compact encoding format used in PlainAccountState.

use alloy_primitives::{B256, U256};
use triedb::account::Account as TrieDbAccount;

use super::compact::{read_compact_u64, read_compact_u256};

/// Empty code hash (keccak256 of empty bytes).
const KECCAK_EMPTY: B256 = B256::new(hex_literal::hex!(
    "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
));

/// Empty trie root hash.
const EMPTY_ROOT_HASH: B256 = B256::new(hex_literal::hex!(
    "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
));

/// Represents a decoded Reth Account.
///
/// This mirrors Reth's Account struct for deserialization purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RethAccount {
    /// Account nonce (transaction count).
    pub nonce: u64,
    /// Account balance in wei.
    pub balance: U256,
    /// Hash of the account's bytecode, if any.
    pub bytecode_hash: Option<B256>,
}

impl RethAccount {
    /// Decode a `RethAccount` from Reth's Compact encoding.
    ///
    /// The encoding format is:
    /// - Bytes 0-1: Flags (little-endian bitfield)
    ///   - Bits 0-3: nonce length (4 bits, max 8 bytes)
    ///   - Bits 4-9: balance length (6 bits, max 32 bytes)
    ///   - Bit 10: bytecode_hash present (1 bit)
    /// - Remaining bytes: nonce (variable), balance (variable), bytecode_hash (32 if present)
    ///
    /// # Errors
    /// Returns an error if the buffer is too short or contains invalid data.
    pub fn from_compact(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.len() < 2 {
            return Err(DecodeError::InsufficientData { expected: 2, got: bytes.len() });
        }

        // Read flags (2 bytes, little-endian bitfield)
        let flags = u16::from_le_bytes([bytes[0], bytes[1]]);
        let buf = &bytes[2..];

        // Extract lengths from flags
        let nonce_len = (flags & 0x0F) as usize; // bits 0-3
        let balance_len = ((flags >> 4) & 0x3F) as usize; // bits 4-9
        let has_bytecode_hash = ((flags >> 10) & 0x01) != 0; // bit 10

        // Validate we have enough data
        let required_len = nonce_len + balance_len + if has_bytecode_hash { 32 } else { 0 };
        if buf.len() < required_len {
            return Err(DecodeError::InsufficientData {
                expected: required_len + 2,
                got: bytes.len(),
            });
        }

        // Decode fields
        let (nonce, buf) = read_compact_u64(buf, nonce_len);
        let (balance, buf) = read_compact_u256(buf, balance_len);

        let bytecode_hash =
            if has_bytecode_hash { Some(B256::from_slice(&buf[..32])) } else { None };

        Ok(Self { nonce, balance, bytecode_hash })
    }

    /// Convert this Reth account to a TrieDB account.
    ///
    /// Note: The storage root is set to `EMPTY_ROOT_HASH` because Reth stores
    /// storage separately in PlainStorageState. The actual storage root will be
    /// computed by TrieDB when storage slots are inserted.
    #[must_use]
    pub fn to_triedb_account(&self) -> TrieDbAccount {
        let code_hash = self.bytecode_hash.unwrap_or(KECCAK_EMPTY);
        TrieDbAccount::new(self.nonce, self.balance, EMPTY_ROOT_HASH, code_hash)
    }
}

/// Error type for decoding failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Not enough bytes in the buffer.
    InsufficientData {
        /// Expected minimum bytes.
        expected: usize,
        /// Actual bytes available.
        got: usize,
    },
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientData { expected, got } => {
                write!(f, "insufficient data: expected {} bytes, got {}", expected, got)
            }
        }
    }
}

impl std::error::Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_empty_account() {
        // Empty account: nonce=0, balance=0, no bytecode_hash
        // Flags: 0x0000 (all zeros)
        let bytes = [0x00, 0x00];
        let account = RethAccount::from_compact(&bytes).unwrap();
        assert_eq!(account.nonce, 0);
        assert_eq!(account.balance, U256::ZERO);
        assert_eq!(account.bytecode_hash, None);
    }

    #[test]
    fn test_decode_account_with_nonce() {
        // nonce=5 (1 byte), balance=0, no bytecode_hash
        // Flags: nonce_len=1 -> 0x0001
        let bytes = [0x01, 0x00, 0x05];
        let account = RethAccount::from_compact(&bytes).unwrap();
        assert_eq!(account.nonce, 5);
        assert_eq!(account.balance, U256::ZERO);
        assert_eq!(account.bytecode_hash, None);
    }

    #[test]
    fn test_insufficient_data() {
        let bytes = [0x00]; // Only 1 byte, need at least 2
        let result = RethAccount::from_compact(&bytes);
        assert!(result.is_err());
    }
}
