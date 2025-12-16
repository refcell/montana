//! Anvil batch sink implementation.

use std::sync::Arc;

use alloy::{
    network::TransactionBuilder,
    primitives::{Address, Bytes},
};
use async_trait::async_trait;
use montana_pipeline::{BatchSink, CompressedBatch, SinkError, SubmissionReceipt};

use crate::manager::BoxedProvider;

/// Sink that submits compressed batches as transactions to an Anvil chain.
///
/// This sink submits batches as calldata transactions to a configured batch inbox
/// address. The batch data is included in the transaction's input field.
pub struct AnvilBatchSink {
    /// Provider for the Anvil instance.
    provider: Arc<BoxedProvider>,
    /// Sender address.
    sender: Address,
    /// Batch inbox address.
    batch_inbox: Address,
}

impl AnvilBatchSink {
    /// Create a new Anvil batch sink.
    pub(crate) fn new(provider: Arc<BoxedProvider>, sender: Address, batch_inbox: Address) -> Self {
        Self { provider, sender, batch_inbox }
    }
}

impl std::fmt::Debug for AnvilBatchSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnvilBatchSink")
            .field("sender", &self.sender)
            .field("batch_inbox", &self.batch_inbox)
            .finish()
    }
}

#[async_trait]
impl BatchSink for AnvilBatchSink {
    async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        let batch_number = batch.batch_number;

        // Build transaction request
        let tx_request = alloy::rpc::types::TransactionRequest::default()
            .to(self.batch_inbox)
            .with_input(Bytes::from(batch.data))
            .from(self.sender);

        // Send the transaction
        let pending_tx = self
            .provider
            .send_transaction(tx_request)
            .await
            .map_err(|e| SinkError::TxFailed(format!("Failed to send transaction: {}", e)))?;

        let tx_hash = *pending_tx.tx_hash();

        // Wait for confirmation
        let receipt = pending_tx
            .get_receipt()
            .await
            .map_err(|e| SinkError::TxFailed(format!("Failed to get receipt: {}", e)))?;

        let l1_block = receipt.block_number.unwrap_or(0);

        Ok(SubmissionReceipt { batch_number, tx_hash: tx_hash.0, l1_block, blob_hash: None })
    }

    async fn capacity(&self) -> Result<usize, SinkError> {
        // Anvil has no practical limit for testing purposes
        // Return a large value representing max calldata size
        Ok(128 * 1024) // 128 KB
    }

    async fn health_check(&self) -> Result<(), SinkError> {
        let _block_num: u64 = self
            .provider
            .get_block_number()
            .await
            .map_err(|e| SinkError::Connection(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests would require spawning Anvil
    // Unit tests focus on struct creation and basic properties

    #[test]
    fn anvil_batch_sink_debug() {
        // We can't create a real sink without a provider, but we can test the Debug impl
        // This is a compile-time check that Debug is implemented
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<AnvilBatchSink>();
    }
}
