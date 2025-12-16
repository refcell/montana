//! Historical range block producer that fetches a range of past blocks.

use alloy::{
    eips::BlockNumberOrTag,
    providers::{Provider, RootProvider},
};
use async_trait::async_trait;
use eyre::{Result, eyre};
use op_alloy::network::Optimism;
use tokio::sync::mpsc::Sender;
use tracing::{debug, info, warn};

use super::BlockProducer;
use crate::OpBlock;

/// A block producer that fetches a historical range of blocks.
///
/// This producer fetches blocks from `start_block` to `end_block` (inclusive)
/// and then completes. It sends blocks in sequential order.
#[derive(Debug)]
pub struct HistoricalRangeProducer {
    provider: RootProvider<Optimism>,
    start_block: u64,
    end_block: u64,
}

impl HistoricalRangeProducer {
    /// Create a new historical range producer.
    ///
    /// # Arguments
    /// * `provider` - The RPC provider
    /// * `start_block` - The first block to fetch (inclusive)
    /// * `end_block` - The last block to fetch (inclusive)
    #[must_use]
    pub const fn new(provider: RootProvider<Optimism>, start_block: u64, end_block: u64) -> Self {
        Self { provider, start_block, end_block }
    }
}

#[async_trait]
impl BlockProducer for HistoricalRangeProducer {
    async fn produce(&self, tx: Sender<OpBlock>) -> Result<()> {
        let total_blocks = self.end_block.saturating_sub(self.start_block) + 1;
        info!(
            start = self.start_block,
            end = self.end_block,
            total = total_blocks,
            "Starting historical block producer"
        );

        for block_num in self.start_block..=self.end_block {
            debug!(block = block_num, "Fetching block");

            let block = self
                .provider
                .get_block_by_number(BlockNumberOrTag::Number(block_num))
                .full()
                .await?
                .ok_or_else(|| eyre!("Block {} not found", block_num))?;

            // Send to consumer (backpressure applied automatically)
            if tx.send(block).await.is_err() {
                warn!("Consumer channel closed, stopping producer");
                return Ok(());
            }
        }

        info!(
            start = self.start_block,
            end = self.end_block,
            "Historical block producer completed"
        );
        Ok(())
    }
}
