//! Anvil batch source implementation.

use std::sync::Arc;

use alloy::{consensus::Transaction, primitives::Address};
use async_trait::async_trait;
use montana_pipeline::{CompressedBatch, L1BatchSource, SourceError};

use crate::manager::BoxedProvider;

/// Source that reads batches from an Anvil chain.
///
/// This source scans blocks for transactions sent to the batch inbox address
/// and extracts their calldata as batch data.
pub struct AnvilBatchSource {
    /// Provider for the Anvil instance.
    provider: Arc<BoxedProvider>,
    /// Batch inbox address to filter transactions.
    batch_inbox: Address,
    /// Last scanned block number.
    last_scanned_block: u64,
    /// Next batch number to assign.
    next_batch_number: u64,
}

impl AnvilBatchSource {
    /// Create a new Anvil batch source.
    pub(crate) fn new(provider: Arc<BoxedProvider>, batch_inbox: Address) -> Self {
        // Start at batch 1 to match the BatchDriver which starts at 1 to avoid
        // checkpoint skip issues with batch 0.
        Self { provider, batch_inbox, last_scanned_block: 0, next_batch_number: 1 }
    }
}

impl std::fmt::Debug for AnvilBatchSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnvilBatchSource")
            .field("batch_inbox", &self.batch_inbox)
            .field("last_scanned_block", &self.last_scanned_block)
            .field("next_batch_number", &self.next_batch_number)
            .finish()
    }
}

#[async_trait]
impl L1BatchSource for AnvilBatchSource {
    async fn next_batch(&mut self) -> Result<Option<CompressedBatch>, SourceError> {
        let current_block = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| SourceError::Connection(e.to_string()))?;

        // Nothing new to scan
        if current_block <= self.last_scanned_block {
            return Ok(None);
        }

        // Scan blocks for batch transactions
        for block_num in (self.last_scanned_block + 1)..=current_block {
            // Request full transactions (not just hashes) so we can read calldata
            let block = self
                .provider
                .get_block_by_number(block_num.into())
                .full()
                .await
                .map_err(|e| SourceError::Connection(e.to_string()))?;

            let Some(block) = block else {
                continue;
            };

            // Check each transaction for batch inbox destination
            for tx in block.transactions.txns() {
                if tx.to() == Some(self.batch_inbox) {
                    let batch_number = self.next_batch_number;
                    self.next_batch_number += 1;
                    self.last_scanned_block = block_num;

                    // Parse the metadata header from the calldata:
                    // [8 bytes: block_count][8 bytes: first_block][8 bytes: last_block][remaining: compressed data]
                    let input = tx.input();
                    let (block_count, first_block, last_block, data) = if input.len() >= 24 {
                        let block_count = u64::from_le_bytes(input[0..8].try_into().unwrap());
                        let first_block = u64::from_le_bytes(input[8..16].try_into().unwrap());
                        let last_block = u64::from_le_bytes(input[16..24].try_into().unwrap());
                        let data = input[24..].to_vec();
                        (block_count, first_block, last_block, data)
                    } else {
                        // Fallback for legacy/malformed data - use defaults
                        (1, 0, 0, input.to_vec())
                    };

                    return Ok(Some(CompressedBatch {
                        batch_number,
                        data,
                        block_count,
                        first_block,
                        last_block,
                    }));
                }
            }

            self.last_scanned_block = block_num;
        }

        Ok(None)
    }

    async fn l1_head(&self) -> Result<u64, SourceError> {
        self.provider.get_block_number().await.map_err(|e| SourceError::Connection(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anvil_batch_source_debug() {
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<AnvilBatchSource>();
    }
}
