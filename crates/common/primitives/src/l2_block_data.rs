//! L2 block data type.

use alloy_primitives::Bytes;

/// A block's worth of transactions with metadata.
///
/// Generic over the transaction type `T`, which defaults to `Bytes` for raw
/// RLP-encoded transaction bytes. This allows flexibility for typed transactions
/// (e.g., `L2BlockData<OpTxEnvelope>`) when needed.
#[derive(Clone, Debug)]
pub struct L2BlockData<T = Bytes> {
    /// Block timestamp.
    pub timestamp: u64,
    /// Block transactions.
    pub transactions: Vec<T>,
}
