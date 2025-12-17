//! Batch header type.

/// Batch header: fixed 19 bytes.
/// All integers little-endian.
#[derive(Clone, Debug)]
pub struct BatchHeader {
    /// Wire format version (0x00).
    pub version: u8,
    /// Monotonic sequence number.
    pub batch_number: u64,
    /// First block timestamp.
    pub timestamp: u64,
    /// Number of L2 blocks.
    pub block_count: u16,
}

impl BatchHeader {
    /// Size of the batch header in bytes.
    pub const SIZE: usize = 1 + 8 + 8 + 2; // 19 bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_header_size_is_19_bytes() {
        assert_eq!(BatchHeader::SIZE, 19);
    }
}
