//! Montana block feeder.
//!
//! This crate provides functionality for fetching blocks from an RPC provider
//! and sending them to the sequencer via a channel.

use std::time::Duration;

use alloy::{eips::eip2718::Encodable2718, providers::Provider};
use eyre::Result;
use montana_pipeline::{Bytes, L2BlockData};
use montana_tui::{TuiEvent, TuiHandle};
use op_alloy::network::Optimism;
use tokio::sync::mpsc;

/// Fetches blocks and sends them to the sequencer via channel.
/// This is used when sync is skipped.
///
/// Uses an unbounded channel to decouple block fetching from execution - the feeder
/// continuously pulls blocks from the RPC while the sequencer processes them at its
/// own pace. The TUI shows both blocks fetched (backlog) and blocks executed.
pub async fn run_block_feeder<P: Provider<Optimism> + Clone>(
    provider: P,
    start: u64,
    poll_interval_ms: u64,
    tx: mpsc::UnboundedSender<L2BlockData>,
    tui_handle: Option<TuiHandle>,
) -> Result<()> {
    tracing::info!(start, has_tui_handle = tui_handle.is_some(), "Block feeder started");

    let mut current = start;
    let mut blocks_fetched: u64 = 0;

    loop {
        // Get chain tip
        let chain_tip = provider.get_block_number().await?;

        // Fetch blocks up to current chain tip
        while current <= chain_tip {
            // Fetch block with full transactions
            let block = provider
                .get_block_by_number(current.into())
                .full()
                .await?
                .ok_or_else(|| eyre::eyre!("Block {} not found", current))?;

            let tx_count = block.transactions.len();
            let gas_used = block.header.gas_used;

            // Convert to L2BlockData
            let block_data = L2BlockData {
                block_number: current,
                timestamp: block.header.timestamp,
                transactions: block
                    .transactions
                    .txns()
                    .map(|rpc_tx| {
                        // Get the inner OpTxEnvelope and encode it
                        let envelope = rpc_tx.inner.inner.inner();
                        Bytes::from(envelope.encoded_2718())
                    })
                    .collect(),
            };

            // Calculate approximate block size from transaction data
            let size_bytes: usize = block_data.transactions.iter().map(|tx| tx.len()).sum();

            // Emit TUI events for the block fetched
            if let Some(ref handle) = tui_handle {
                handle.send(TuiEvent::BlockBuilt {
                    number: current,
                    tx_count,
                    size_bytes,
                    gas_used,
                });
                handle.send(TuiEvent::UnsafeHeadUpdated(current));
            }

            // Send to sequencer (unbounded - never blocks)
            if tx.send(block_data).is_err() {
                tracing::info!("Block receiver closed, stopping feeder");
                return Ok(());
            }

            blocks_fetched += 1;

            // Emit backlog update event for TUI
            if let Some(ref handle) = tui_handle {
                handle
                    .send(TuiEvent::BacklogUpdated { blocks_fetched, last_fetched_block: current });
            }

            tracing::debug!(block = current, tx_count, blocks_fetched, "Fed block to sequencer");
            current += 1;

            // Yield to allow other tasks to run (e.g., TUI event processing)
            tokio::task::yield_now().await;
        }

        // Wait before polling for new blocks
        tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
    }
}
