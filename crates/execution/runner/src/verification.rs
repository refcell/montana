//! Receipt verification for executed blocks

use alloy::{eips::BlockId, network::ReceiptResponse, providers::Provider};
use op_alloy::network::Optimism;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::ExecutedBlock;

/// Runs the verification loop, consuming executed blocks from the receiver
/// and verifying them against RPC receipts.
pub async fn run_verifier<P>(mut block_rx: mpsc::Receiver<ExecutedBlock>, provider: P)
where
    P: Provider<Optimism> + Clone + Send + Sync + 'static,
{
    while let Some(executed) = block_rx.recv().await {
        verify_block(&executed, &provider).await;
    }
}

/// Verify an executed block against RPC receipts.
pub async fn verify_block<P>(executed: &ExecutedBlock, provider: &P)
where
    P: Provider<Optimism> + Clone + Send + Sync,
{
    let block_number = executed.result.block_number;

    // Fetch receipts from RPC for verification
    let receipts = match provider.get_block_receipts(BlockId::number(block_number)).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            warn!(block = block_number, "Verification skipped: no receipts found for block");
            return;
        }
        Err(e) => {
            warn!(
                block = block_number,
                error = %e,
                "Verification skipped due to upstream RPC error"
            );
            return;
        }
    };

    // Verify execution results against receipts
    let mut verified = 0;
    let mut mismatched = 0;
    let total = executed.result.tx_results.len();

    for (idx, (tx_result, receipt)) in
        executed.result.tx_results.iter().zip(receipts.iter()).enumerate()
    {
        let receipt_gas = receipt.gas_used();
        let receipt_success = receipt.status();

        if tx_result.gas_used == receipt_gas && tx_result.success == receipt_success {
            verified += 1;
        } else {
            mismatched += 1;
            warn!(
                "Block {} tx {}: gas mismatch (exec={}, receipt={}) or status mismatch (exec={}, receipt={})",
                block_number,
                idx,
                tx_result.gas_used,
                receipt_gas,
                tx_result.success,
                receipt_success
            );
        }
    }

    if mismatched == 0 {
        info!(
            "Block {} verification PASSED: {}/{} transactions verified",
            block_number, verified, total
        );
    } else {
        warn!(
            "Block {} verification FAILED: {} mismatched, {} verified out of {} transactions",
            block_number, mismatched, verified, total
        );
    }
}
