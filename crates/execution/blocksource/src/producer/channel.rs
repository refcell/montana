//! Channel-based block producer for validator mode.

use std::sync::Arc;

use async_trait::async_trait;
use eyre::Result;
use tokio::sync::{
    Mutex,
    mpsc::{Receiver, Sender},
};

use super::BlockProducer;
use crate::OpBlock;

/// A BlockProducer that receives blocks via a channel.
///
/// This is used in validator mode where the derivation pipeline
/// pushes derived blocks through a channel to be executed.
pub struct ChannelBlockProducer {
    /// Receiver for incoming blocks.
    rx: Arc<Mutex<Receiver<OpBlock>>>,
}

impl ChannelBlockProducer {
    /// Create a new channel-based block producer.
    pub fn new(rx: Receiver<OpBlock>) -> Self {
        Self { rx: Arc::new(Mutex::new(rx)) }
    }
}

impl std::fmt::Debug for ChannelBlockProducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelBlockProducer").finish()
    }
}

#[async_trait]
impl BlockProducer for ChannelBlockProducer {
    async fn produce(&self, tx: Sender<OpBlock>) -> Result<()> {
        let mut rx = self.rx.lock().await;
        while let Some(block) = rx.recv().await {
            if tx.send(block).await.is_err() {
                // Receiver dropped, stop producing
                break;
            }
        }
        Ok(())
    }

    async fn get_chain_tip(&self) -> Result<u64> {
        Err(eyre::eyre!("Channel producer does not support get_chain_tip"))
    }

    async fn get_block(&self, _number: u64) -> Result<Option<OpBlock>> {
        Err(eyre::eyre!("Channel producer does not support get_block"))
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use super::*;

    #[tokio::test]
    async fn test_channel_producer_debug() {
        let (_tx, rx) = mpsc::channel::<OpBlock>(16);
        let producer = ChannelBlockProducer::new(rx);
        assert!(format!("{:?}", producer).contains("ChannelBlockProducer"));
    }
}
