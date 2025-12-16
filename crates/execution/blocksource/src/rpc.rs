//! RPC-based block source implementation.

use alloy::{
    eips::BlockNumberOrTag,
    providers::{Provider, ProviderBuilder, RootProvider},
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
    /// Create a new RPC block source from a URL.
    ///
    /// # Errors
    /// Returns an error if the connection to the RPC endpoint fails.
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let provider = ProviderBuilder::new()
            .disable_recommended_fillers()
            .network::<Optimism>()
            .connect(rpc_url)
            .await?;

        Ok(Self { provider })
    }

    /// Get the underlying provider for additional RPC operations.
    #[must_use]
    pub const fn provider(&self) -> &RootProvider<Optimism> {
        &self.provider
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
