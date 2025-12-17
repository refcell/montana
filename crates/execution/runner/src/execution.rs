//! Block execution client

use std::fmt;

use blocksource::{BlockProducer, OpBlock};
use chainspec::BASE_MAINNET;
use database::Database;
use eyre::Result;
use tokio::sync::mpsc;
use tracing::{info, trace, warn};
use vm::{BlockExecutor, BlockResult};

/// An executed block with its execution result
#[derive(Debug, Clone)]
pub struct ExecutedBlock {
    /// The original block
    pub block: OpBlock,
    /// The execution result
    pub result: BlockResult,
}

/// Block execution client
pub struct Execution<DB, P> {
    db: DB,
    producer: P,
    /// Channel to send executed blocks downstream.
    output_tx: mpsc::Sender<ExecutedBlock>,
}

impl<DB: fmt::Debug, P: fmt::Debug> fmt::Debug for Execution<DB, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Execution")
            .field("db", &self.db)
            .field("producer", &self.producer)
            .field("output_tx", &"<Sender>")
            .finish()
    }
}

impl<DB, P> Execution<DB, P>
where
    DB: Database,
    P: BlockProducer + 'static,
{
    /// Create a new execution client
    pub const fn new(db: DB, producer: P, output_tx: mpsc::Sender<ExecutedBlock>) -> Self {
        Self { db, producer, output_tx }
    }

    /// Start executing blocks from the producer
    ///
    /// This spawns the producer in a background task and consumes blocks,
    /// executing each one.
    pub async fn start(self) -> Result<()> {
        // Destructure self to separate producer from the rest
        let Self { mut db, producer, output_tx } = self;

        // Create channel for block production
        let (tx, mut rx) = mpsc::channel(16);

        // Spawn the producer
        let producer_handle = tokio::spawn(async move { producer.produce(tx).await });

        // Consume and execute blocks
        while let Some(block) = rx.recv().await {
            let block_num = block.header.number;
            info!(block = block_num, "Executing block");

            // Execute the block
            let mut executor = BlockExecutor::new(db.clone(), BASE_MAINNET);
            let result = executor.execute_block(block.clone())?;

            // Commit the block state after successful execution
            db.commit_block();

            // Forward executed block to downstream consumer
            let executed = ExecutedBlock { block, result };
            if output_tx.send(executed).await.is_err() {
                warn!("Downstream consumer dropped, stopping output forwarding");
            } else {
                trace!("Forwarded executed block to downstream consumer");
            }
        }

        // Wait for producer to finish
        producer_handle.abort();

        info!("Execution complete");
        Ok(())
    }
}
