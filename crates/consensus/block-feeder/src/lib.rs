#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::time::Duration;

use alloy::providers::Provider;
use eyre::Result;
use montana_tui::{TuiEvent, TuiHandle};
use op_alloy::network::Optimism;
use primitives::OpBlock;
use tokio::sync::mpsc;

/// Fetches blocks and sends them to the sequencer via channel.
/// This is used when sync is skipped.
///
/// Uses an unbounded channel to decouple block fetching from execution - the feeder
/// continuously pulls blocks from the RPC while the sequencer processes them at its
/// own pace. The TUI shows both blocks fetched (backlog) and blocks executed.
///
/// Sends full `OpBlock` types to preserve all block information for serialization.
pub async fn run_block_feeder<P: Provider<Optimism> + Clone>(
    provider: P,
    start: u64,
    poll_interval_ms: u64,
    tx: mpsc::UnboundedSender<OpBlock>,
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

            // Calculate approximate block size from serialized block
            let size_bytes = serde_json::to_vec(&block).map(|v| v.len()).unwrap_or(0);

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

            // Send full OpBlock to sequencer (unbounded - never blocks)
            if tx.send(block).is_err() {
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
