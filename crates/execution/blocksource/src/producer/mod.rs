//! Block producer implementations for streaming blocks to consumers.
//!
//! This module provides a trait-based abstraction for block producers,
//! with implementations for both live RPC polling and historical range fetching.

mod channel;
pub use channel::ChannelBlockProducer;

mod historical;
pub use historical::HistoricalRangeProducer;

mod live;
pub use live::LiveRpcProducer;
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use crate::OpBlock;

/// A producer that streams blocks to a consumer via a channel.
///
/// Implementations send blocks to the provided channel with backpressure
/// automatically applied when the channel is full.
#[async_trait]
pub trait BlockProducer: Send + Sync {
    /// Start producing blocks, sending them to the provided channel.
    ///
    /// Returns when production is complete (for bounded producers like historical range)
    /// or when an error occurs. For live producers, this runs indefinitely.
    ///
    /// # Errors
    /// Returns an error if block fetching fails or the channel is closed.
    async fn produce(&self, tx: Sender<OpBlock>) -> eyre::Result<()>;
}
