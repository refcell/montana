//! Sequencer role implementation.
//!
//! The sequencer role executes blocks and submits batches to L1.

use std::path::PathBuf;

use async_trait::async_trait;
use tokio::sync::mpsc;
use montana_batcher::BatchDriver;
use montana_checkpoint::Checkpoint;
use montana_pipeline::{BatchSink, CompressedBatch, Compressor, L2BlockData};

use crate::{Role, RoleCheckpoint, TickResult};

/// Events emitted by the sequencer for observer pattern.
///
/// These events allow external observers to track the sequencer's progress
/// without tight coupling.
#[derive(Debug, Clone)]
pub enum SequencerEvent {
    /// A block was successfully executed.
    BlockExecuted {
        /// The block number that was executed.
        block_number: u64,
        /// The block's hash.
        block_hash: [u8; 32],
    },
    /// A batch was built and is ready for submission.
    BatchBuilt {
        /// The batch number.
        batch_number: u64,
        /// Number of blocks in the batch.
        block_count: usize,
    },
    /// A batch was successfully submitted to L1.
    BatchSubmitted {
        /// The batch number.
        batch_number: u64,
        /// The L1 transaction hash.
        tx_hash: [u8; 32],
    },
}

/// The sequencer role: receives blocks, batches them, and submits to L1.
///
/// The sequencer is generic over:
/// - `S`: The batch sink implementation (e.g., blob sink, calldata sink)
/// - `C`: The compressor implementation (e.g., Brotli, Zstd)
///
/// The sequencer receives L2 blocks via a channel, batches them according
/// to configured criteria, compresses them, and submits them to L1.
#[derive(Debug)]
pub struct Sequencer<S, C> {
    /// Batch sink for L1 submission.
    batch_sink: S,
    /// Compressor for batch data.
    compressor: C,
    /// Batch driver managing batch building logic.
    batch_driver: BatchDriver,
    /// Checkpoint for resumption.
    checkpoint: Checkpoint,
    /// Path to save checkpoints.
    checkpoint_path: PathBuf,
    /// Channel for receiving blocks.
    block_rx: mpsc::Receiver<L2BlockData>,
    /// Event sender (for observers).
    event_tx: Option<mpsc::UnboundedSender<SequencerEvent>>,
}

impl<S, C> Sequencer<S, C>
where
    S: BatchSink,
    C: Compressor,
{
    /// Creates a new sequencer role.
    ///
    /// # Arguments
    ///
    /// * `batch_sink` - The sink for submitting batches to L1
    /// * `compressor` - The compressor for batch data
    /// * `batch_driver` - The batch driver managing batch building logic
    /// * `checkpoint_path` - Path to save/load checkpoints
    /// * `block_rx` - Channel for receiving L2 blocks
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint cannot be loaded or created.
    pub fn new(
        batch_sink: S,
        compressor: C,
        batch_driver: BatchDriver,
        checkpoint_path: PathBuf,
        block_rx: mpsc::Receiver<L2BlockData>,
    ) -> eyre::Result<Self> {
        let checkpoint = Checkpoint::load(&checkpoint_path)?.map_or_else(
            || {
                tracing::info!("No checkpoint found at {:?}, starting fresh", checkpoint_path);
                Checkpoint::default()
            },
            |cp| {
                tracing::info!(
                    "Loaded checkpoint from {:?}, last batch submitted: {}",
                    checkpoint_path,
                    cp.last_batch_submitted
                );
                cp
            },
        );

        Ok(Self {
            batch_sink,
            compressor,
            batch_driver,
            checkpoint,
            checkpoint_path,
            block_rx,
            event_tx: None,
        })
    }

    /// Sets the event sender for observer notifications.
    ///
    /// # Arguments
    ///
    /// * `tx` - The unbounded sender for sequencer events
    pub fn with_event_sender(mut self, tx: mpsc::UnboundedSender<SequencerEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Emits an event to observers if an event sender is configured.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to emit
    fn emit(&self, event: SequencerEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Processes a single incoming block.
    ///
    /// Adds the block to the batch driver and checks if a batch should be submitted.
    ///
    /// # Arguments
    ///
    /// * `block` - The L2 block data to process
    ///
    /// # Errors
    ///
    /// Returns an error if batch submission fails.
    async fn process_block(&mut self, block: L2BlockData) -> eyre::Result<()> {
        // Add block to batch driver
        self.batch_driver.add_blocks(vec![block.clone()]);

        // Emit block received event (using timestamp as a proxy for block number)
        self.emit(SequencerEvent::BlockExecuted {
            block_number: block.timestamp,
            block_hash: [0u8; 32], // TODO: compute actual block hash
        });

        // Check if batch is ready
        if self.batch_driver.should_submit() {
            self.submit_pending_batch().await?;
        }

        Ok(())
    }

    /// Submits a pending batch to L1.
    ///
    /// Builds the batch, compresses it, and submits it to the batch sink.
    /// Updates the checkpoint after successful submission.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Batch building fails
    /// - Compression fails
    /// - Submission to L1 fails
    /// - Checkpoint save fails
    async fn submit_pending_batch(&mut self) -> eyre::Result<()> {
        let Some(pending) = self.batch_driver.build_batch() else {
            return Ok(());
        };

        let batch_number = pending.batch_number;

        // Skip if already submitted
        if self.checkpoint.should_skip_batch(batch_number) {
            tracing::info!(batch_number, "Skipping already-submitted batch");
            return Ok(());
        }

        // Encode the batch data (concatenate all transaction data)
        let mut batch_data = Vec::new();
        for block in &pending.blocks {
            for tx in &block.transactions {
                batch_data.extend_from_slice(&tx.0);
            }
        }

        // Compress
        let compressed_data = self.compressor.compress(&batch_data)?;

        self.emit(SequencerEvent::BatchBuilt { batch_number, block_count: pending.blocks.len() });

        // Submit
        let compressed_batch = CompressedBatch { batch_number, data: compressed_data };

        let receipt = self.batch_sink.submit(compressed_batch).await?;

        // Update checkpoint
        self.checkpoint.record_batch_submitted(batch_number);
        self.checkpoint.touch();
        self.checkpoint.save(&self.checkpoint_path)?;

        // Record in batch driver as well
        self.batch_driver.record_batch_submitted(batch_number)?;

        self.emit(SequencerEvent::BatchSubmitted { batch_number, tx_hash: receipt.tx_hash });

        tracing::info!(
            batch_number,
            tx_hash = hex::encode(receipt.tx_hash),
            l1_block = receipt.l1_block,
            "Batch submitted successfully"
        );

        Ok(())
    }
}

#[async_trait]
impl<S, C> Role for Sequencer<S, C>
where
    S: BatchSink,
    C: Compressor,
{
    fn name(&self) -> &'static str {
        "sequencer"
    }

    async fn resume(&mut self, checkpoint: &Checkpoint) -> eyre::Result<()> {
        self.checkpoint = checkpoint.clone();

        tracing::info!(
            last_batch_submitted = checkpoint.last_batch_submitted,
            last_block_executed = checkpoint.last_block_executed,
            "Sequencer resuming from checkpoint"
        );

        Ok(())
    }

    async fn tick(&mut self) -> eyre::Result<TickResult> {
        // Try to receive a block (non-blocking)
        match self.block_rx.try_recv() {
            Ok(block) => {
                self.process_block(block).await?;
                Ok(TickResult::Progress)
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                // No blocks available, check if we should time-submit
                if self.batch_driver.should_submit() && self.batch_driver.pending_count() > 0 {
                    self.submit_pending_batch().await?;
                    Ok(TickResult::Progress)
                } else {
                    Ok(TickResult::Idle)
                }
            }
            Err(mpsc::error::TryRecvError::Disconnected) => {
                // Block producer finished, flush remaining
                if self.batch_driver.pending_count() > 0 {
                    self.submit_pending_batch().await?;
                }
                Ok(TickResult::Complete)
            }
        }
    }

    fn checkpoint(&self) -> RoleCheckpoint {
        RoleCheckpoint {
            last_batch_submitted: Some(self.checkpoint.last_batch_submitted),
            last_block_processed: Some(self.checkpoint.last_block_executed),
            last_batch_derived: None,
        }
    }
}
