//! Compressed batch type.

/// Compressed batch ready for submission.
#[derive(Clone, Debug)]
pub struct CompressedBatch {
    /// Batch number.
    pub batch_number: u64,
    /// Compressed data.
    pub data: Vec<u8>,
    /// Number of L2 blocks in this batch.
    pub block_count: u64,
    /// First L2 block number in this batch.
    pub first_block: u64,
    /// Last L2 block number in this batch.
    pub last_block: u64,
}
