//! OpBlock type alias and batch payload types.

use alloy::network::Network;
use op_alloy::network::Optimism;

/// Type alias for the Optimism block type.
///
/// This represents a full Optimism block response from the RPC,
/// including header, transactions, and all block metadata.
pub type OpBlock = <Optimism as Network>::BlockResponse;

/// A batch payload containing a list of OpBlocks.
///
/// This is the serializable format used for batch compression.
/// The blocks are serialized as JSON before compression.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OpBlockBatch {
    /// The blocks in this batch.
    pub blocks: Vec<OpBlock>,
}

impl OpBlockBatch {
    /// Create a new batch from blocks.
    pub const fn new(blocks: Vec<OpBlock>) -> Self {
        Self { blocks }
    }

    /// Serialize the batch to bytes using JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize a batch from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails.
    pub fn from_bytes(data: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(data)
    }

    /// Get the number of blocks in this batch.
    pub const fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Check if the batch is empty.
    pub const fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Get the first block number in the batch, if any.
    pub fn first_block_number(&self) -> Option<u64> {
        self.blocks.first().map(|b| b.header.number)
    }

    /// Get the last block number in the batch, if any.
    pub fn last_block_number(&self) -> Option<u64> {
        self.blocks.last().map(|b| b.header.number)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_batch() {
        let batch = OpBlockBatch::new(vec![]);
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
        assert!(batch.first_block_number().is_none());
        assert!(batch.last_block_number().is_none());
    }

    #[test]
    fn batch_roundtrip() {
        let batch = OpBlockBatch::new(vec![]);
        let bytes = batch.to_bytes().unwrap();
        let decoded = OpBlockBatch::from_bytes(&bytes).unwrap();
        assert_eq!(decoded.len(), 0);
    }
}
