//! Submission receipt type.

/// Result of a successful submission.
#[derive(Clone, Debug)]
pub struct SubmissionReceipt {
    /// Batch number.
    pub batch_number: u64,
    /// Transaction hash.
    pub tx_hash: [u8; 32],
    /// L1 block number.
    pub l1_block: u64,
    /// Blob hash (if blob submission).
    pub blob_hash: Option<[u8; 32]>,
}
