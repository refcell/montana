//! Anvil batch sink implementation.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use alloy::{
    consensus::BlobTransactionSidecar,
    eips::eip4844::{
        MAX_BLOBS_PER_BLOCK_DENCUN,
        builder::{SidecarBuilder, SimpleCoder},
    },
    network::{TransactionBuilder, TransactionBuilder4844},
    primitives::{Address, Bytes},
};
use async_trait::async_trait;
use montana_pipeline::{BatchSink, CompressedBatch, SinkError, SubmissionReceipt};

use crate::manager::BoxedProvider;

/// Maximum gas limit for calldata transactions.
/// Anvil's default block gas limit is 30M, so we use a generous limit.
/// Gas cost: 21000 base + 16 per non-zero byte + 4 per zero byte
/// For ~1MB calldata (~90% non-zero): 21000 + 900000*16 + 100000*4 ≈ 15M gas
const MAX_CALLDATA_GAS_LIMIT: u64 = 25_000_000;

/// Maximum calldata size that can fit within the gas limit.
/// With ~16 gas per byte, 25M gas / 16 ≈ 1.5MB max calldata
/// We use a conservative limit to ensure transactions succeed.
const MAX_CALLDATA_SIZE: usize = 1_200_000; // 1.2MB

/// Sink that submits compressed batches as transactions to an Anvil chain.
///
/// This sink can submit batches as either:
/// - **Calldata transactions**: Data is included in the transaction's input field
/// - **Blob transactions (EIP-4844)**: Data is submitted as blob sidecars
///
/// The mode can be toggled dynamically via the shared `use_blobs` flag.
pub struct AnvilBatchSink {
    /// Provider for the Anvil instance.
    provider: Arc<BoxedProvider>,
    /// Sender address.
    sender: Address,
    /// Batch inbox address.
    batch_inbox: Address,
    /// Shared flag to control blob vs calldata mode.
    /// When true, uses EIP-4844 blob transactions; when false, uses calldata.
    use_blobs: Arc<AtomicBool>,
}

impl AnvilBatchSink {
    /// Create a new Anvil batch sink.
    pub(crate) fn new(
        provider: Arc<BoxedProvider>,
        sender: Address,
        batch_inbox: Address,
        use_blobs: Arc<AtomicBool>,
    ) -> Self {
        Self { provider, sender, batch_inbox, use_blobs }
    }

    /// Create a blob transaction sidecar from batch data.
    ///
    /// Uses alloy's SidecarBuilder to properly encode data and compute
    /// valid KZG commitments and proofs.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(sidecar))` if the data fits in blobs and was encoded successfully
    /// - `Ok(None)` if the data is too large for blobs (caller should fall back to calldata)
    /// - `Err` if there was an error encoding the data
    fn create_blob_sidecar(data: &[u8]) -> Result<Option<BlobTransactionSidecar>, SinkError> {
        if data.is_empty() {
            return Err(SinkError::TxFailed("Empty data for blob".to_string()));
        }

        // Calculate number of blobs needed (SimpleCoder uses ~31 bytes per 32-byte field element)
        // Each blob has 4096 field elements, so usable bytes per blob is approximately 4096 * 31
        let usable_bytes_per_blob = 4096 * 31;
        let num_blobs = data.len().div_ceil(usable_bytes_per_blob);
        if num_blobs > MAX_BLOBS_PER_BLOCK_DENCUN {
            // Data too large for blobs - return None to signal fallback to calldata
            tracing::debug!(
                data_size = data.len(),
                num_blobs_needed = num_blobs,
                max_blobs = MAX_BLOBS_PER_BLOCK_DENCUN,
                "Batch too large for blobs, falling back to calldata"
            );
            return Ok(None);
        }

        // Use SidecarBuilder to encode data and compute KZG commitments/proofs
        SidecarBuilder::<SimpleCoder>::from_slice(data).build().map(Some).map_err(|e| {
            SinkError::TxFailed(format!("Failed to build blob sidecar with KZG: {}", e))
        })
    }
}

impl std::fmt::Debug for AnvilBatchSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnvilBatchSink")
            .field("sender", &self.sender)
            .field("batch_inbox", &self.batch_inbox)
            .field("use_blobs", &self.use_blobs.load(Ordering::SeqCst))
            .finish()
    }
}

#[async_trait]
impl BatchSink for AnvilBatchSink {
    async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        let batch_number = batch.batch_number;
        let use_blobs = self.use_blobs.load(Ordering::SeqCst);

        // Encode batch with metadata header:
        // [8 bytes: block_count][8 bytes: first_block][8 bytes: last_block][remaining: compressed data]
        let mut encoded_data = Vec::with_capacity(24 + batch.data.len());
        encoded_data.extend_from_slice(&batch.block_count.to_le_bytes());
        encoded_data.extend_from_slice(&batch.first_block.to_le_bytes());
        encoded_data.extend_from_slice(&batch.last_block.to_le_bytes());
        encoded_data.extend_from_slice(&batch.data);

        // Build transaction request based on mode.
        // Note: We always include data in calldata for derivation compatibility.
        // When using blobs, the blob sidecar is added for DA cost savings,
        // but calldata is still used by the source for reading batch data.
        //
        // If blob mode is enabled but the batch is too large for blobs,
        // we gracefully fall back to calldata to avoid errors.
        let tx_request = if use_blobs {
            // Try to create blob transaction (EIP-4844)
            // KZG computation is CPU-intensive, so we run it on a blocking thread
            // to avoid blocking the async runtime.
            let data_for_sidecar = encoded_data.clone();
            let sidecar_result =
                tokio::task::spawn_blocking(move || Self::create_blob_sidecar(&data_for_sidecar))
                    .await
                    .map_err(|e| SinkError::TxFailed(format!("KZG task failed: {}", e)))??;

            match sidecar_result {
                Some(sidecar) => {
                    tracing::debug!(
                        batch_number,
                        num_blobs = sidecar.blobs.len(),
                        data_size = encoded_data.len(),
                        "Submitting batch as blob transaction"
                    );

                    alloy::rpc::types::TransactionRequest::default()
                        .to(self.batch_inbox)
                        .from(self.sender)
                        .with_input(Bytes::from(encoded_data.clone()))
                        .with_blob_sidecar(sidecar)
                        .max_fee_per_blob_gas(1_000_000_000u128) // 1 gwei for testing
                }
                None => {
                    // Batch too large for blobs, fall back to calldata
                    // But first check if the calldata would exceed gas limits
                    if encoded_data.len() > MAX_CALLDATA_SIZE {
                        return Err(SinkError::TxFailed(format!(
                            "Batch too large for both blobs and calldata: {} bytes exceeds {} limit",
                            encoded_data.len(),
                            MAX_CALLDATA_SIZE
                        )));
                    }

                    tracing::warn!(
                        batch_number,
                        data_size = encoded_data.len(),
                        "Batch too large for blobs, falling back to calldata"
                    );

                    // Calculate gas based on calldata size
                    // Base: 21000 + 16 per non-zero byte (assume all non-zero for safety)
                    let estimated_gas = 21_000u64 + (encoded_data.len() as u64 * 16);
                    let gas_limit = estimated_gas.min(MAX_CALLDATA_GAS_LIMIT);

                    alloy::rpc::types::TransactionRequest::default()
                        .to(self.batch_inbox)
                        .with_input(Bytes::from(encoded_data))
                        .from(self.sender)
                        .gas_limit(gas_limit)
                }
            }
        } else {
            // Calldata transaction
            tracing::debug!(
                batch_number,
                data_size = encoded_data.len(),
                "Submitting batch as calldata transaction"
            );

            alloy::rpc::types::TransactionRequest::default()
                .to(self.batch_inbox)
                .with_input(Bytes::from(encoded_data))
                .from(self.sender)
        };

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

        // For blob transactions, we could extract the blob hash from the receipt
        // For now, we leave it as None since Anvil may not populate this
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
