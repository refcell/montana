//! Compressed batch type.

/// Compressed batch ready for submission.
#[derive(Clone, Debug)]
pub struct CompressedBatch {
    /// Batch number.
    pub batch_number: u64,
    /// Compressed data.
    pub data: Vec<u8>,
}
