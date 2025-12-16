//! RPC-based block source implementation.

use alloy::{
    eips::BlockNumberOrTag,
    providers::{Provider, RootProvider},
};
use eyre::{Result, eyre};
use op_alloy::network::Optimism;

use crate::types::OpBlock;

/// A block source that fetches blocks from an RPC endpoint.
#[derive(Debug)]
pub struct RpcBlockSource {
    provider: RootProvider<Optimism>,
}

impl RpcBlockSource {
    /// Create a new RPC block source from a provider.
    #[must_use]
    pub const fn new(provider: RootProvider<Optimism>) -> Self {
        Self { provider }
    }

    /// Fetch a block by number.
    ///
    /// # Errors
    /// Returns an error if the RPC request fails or the block is not found.
    pub async fn get_block(&self, number: u64) -> Result<OpBlock> {
        self.provider
            .get_block_by_number(BlockNumberOrTag::Number(number))
            .full()
            .await?
            .ok_or_else(|| eyre!("Block {} not found", number))
    }
}
