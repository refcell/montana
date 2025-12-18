//! RPC block source implementation.
//!
//! This module provides an RPC-based implementation of the BlockSource trait
//! for fetching L2 blocks from an Ethereum-compatible RPC endpoint.

use alloy::{eips::BlockNumberOrTag, providers::Provider};
use async_trait::async_trait;
use op_alloy::network::Optimism;
use primitives::OpBlock;

use crate::{BlockSource, BlockSourceError};

/// RPC block source for fetching L2 blocks from an RPC endpoint.
///
/// This implementation fetches full `OpBlock` data using the alloy provider,
/// which includes all block metadata, header, and full transaction data.
pub struct RpcBlockSource<P> {
    /// The alloy provider for RPC calls.
    provider: P,
    /// The RPC URL (for debugging).
    url: String,
}

impl<P> RpcBlockSource<P> {
    /// Create a new RPC block source with the given provider.
    pub fn new(provider: P, url: impl Into<String>) -> Self {
        Self { provider, url: url.into() }
    }
}

impl<P> std::fmt::Debug for RpcBlockSource<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcBlockSource").field("url", &self.url).finish()
    }
}

#[async_trait]
impl<P> BlockSource for RpcBlockSource<P>
where
    P: Provider<Optimism> + Clone + Send + Sync,
{
    async fn get_block(&mut self, block_number: u64) -> Result<OpBlock, BlockSourceError> {
        self.provider
            .get_block_by_number(BlockNumberOrTag::Number(block_number))
            .full()
            .await
            .map_err(|e| BlockSourceError::RpcError(format!("Failed to fetch block: {}", e)))?
            .ok_or(BlockSourceError::BlockNotFound(block_number))
    }

    async fn get_head(&self) -> Result<u64, BlockSourceError> {
        self.provider
            .get_block_number()
            .await
            .map_err(|e| BlockSourceError::RpcError(format!("Failed to get block number: {}", e)))
    }
}
