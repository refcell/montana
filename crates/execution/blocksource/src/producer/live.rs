//! Live RPC block producer that polls for new blocks.

use std::time::Duration;

use alloy::{eips::BlockNumberOrTag, providers::Provider};
use async_trait::async_trait;
use eyre::{Result, eyre};
use op_alloy::network::Optimism;
use tokio::sync::mpsc::Sender;
use tracing::{debug, info, warn};

use super::BlockProducer;
use crate::OpBlock;

/// A block producer that polls an RPC endpoint for new blocks.
///
/// This producer continuously polls `eth_blockNumber` and fetches any new blocks
/// since the last seen block, sending them to the consumer channel.
#[derive(Debug)]
pub struct LiveRpcProducer<P> {
    provider: P,
    poll_interval: Duration,
    start_block: Option<u64>,
}

impl<P> LiveRpcProducer<P> {
    /// Create a new live RPC producer.
    ///
    /// # Arguments
    /// * `provider` - The RPC provider
    /// * `poll_interval` - How often to poll for new blocks
    /// * `start_block` - Optional starting block number (None = start from latest)
    #[must_use]
    pub const fn new(provider: P, poll_interval: Duration, start_block: Option<u64>) -> Self {
        Self { provider, poll_interval, start_block }
    }
}

#[async_trait]
impl<P: Provider<Optimism> + Sync> BlockProducer for LiveRpcProducer<P> {
    async fn produce(&self, tx: Sender<OpBlock>) -> Result<()> {
        // Determine starting block
        let mut last_block = match self.start_block {
            Some(n) => n.saturating_sub(1), // Will fetch starting from n
            None => self.provider.get_block_number().await?,
        };

        info!(start_block = last_block + 1, "Starting live block producer");

        loop {
            // Poll for latest block number
            let latest = self.provider.get_block_number().await?;

            // Fetch any new blocks
            while last_block < latest {
                let next_block = last_block + 1;
                debug!(block = next_block, "Fetching block");

                let block = self
                    .provider
                    .get_block_by_number(BlockNumberOrTag::Number(next_block))
                    .full()
                    .await?
                    .ok_or_else(|| eyre!("Block {} not found", next_block))?;

                // Send to consumer (backpressure applied automatically)
                if tx.send(block).await.is_err() {
                    warn!("Consumer channel closed, stopping producer");
                    return Ok(());
                }

                last_block = next_block;
            }

            // Wait before next poll
            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn get_chain_tip(&self) -> Result<u64> {
        self.provider.get_block_number().await.map_err(Into::into)
    }

    async fn get_block(&self, number: u64) -> Result<Option<OpBlock>> {
        self.provider
            .get_block_by_number(BlockNumberOrTag::Number(number))
            .full()
            .await
            .map_err(Into::into)
    }
}
