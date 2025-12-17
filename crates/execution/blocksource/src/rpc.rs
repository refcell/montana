//! RPC-based block source implementation.

use alloy::{eips::BlockNumberOrTag, providers::Provider};
use async_trait::async_trait;
use eyre::{Result, eyre};
use op_alloy::network::Optimism;

use crate::types::OpBlock;

/// A source of blocks.
#[async_trait]
pub trait BlockSource: Send + Sync {
    /// Fetch a block by number.
    async fn get_block(&self, number: u64) -> Result<OpBlock>;
}

/// A block source that fetches blocks from an RPC endpoint.
#[derive(Debug, Clone)]
pub struct RpcBlockSource<P> {
    provider: P,
}

impl<P> RpcBlockSource<P> {
    /// Create a new RPC block source from a provider.
    #[must_use]
    pub const fn new(provider: P) -> Self {
        Self { provider }
    }
}

#[async_trait]
impl<P: Provider<Optimism> + Clone + Sync> BlockSource for RpcBlockSource<P> {
    async fn get_block(&self, number: u64) -> Result<OpBlock> {
        self.provider
            .get_block_by_number(BlockNumberOrTag::Number(number))
            .full()
            .await?
            .ok_or_else(|| eyre!("Block {} not found", number))
    }
}
