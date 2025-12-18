//! Sequencer role implementation.
//!
//! The sequencer role executes blocks and submits batches to L1.

use std::{path::PathBuf, sync::Arc, time::Instant};

use async_trait::async_trait;
use tokio::sync::mpsc;
use montana_batcher::BatchDriver;
use montana_checkpoint::Checkpoint;
use montana_pipeline::{BatchSink, CompressedBatch, Compressor, L2BlockData};

use crate::{Role, RoleCheckpoint, TickResult};

/// Callback for reporting block execution events.
///
/// This trait allows the sequencer to report execution metrics without
/// depending on the TUI crate directly, avoiding cyclic dependencies.
pub trait ExecutionCallback: Send + Sync {
    /// Called when a block has been executed.
    ///
    /// # Arguments
    ///
    /// * `block_number` - The block number that was executed
    /// * `execution_time_ms` - Time taken to execute the block in milliseconds
    fn on_block_executed(&self, block_number: u64, execution_time_ms: u64);
}

/// Callback for reporting batch submission events.
///
/// This trait allows the sequencer to report batch submissions without
/// depending on the TUI crate directly, avoiding cyclic dependencies.
pub trait BatchCallback: Send + Sync {
    /// Called when a batch has been submitted.
    ///
    /// # Arguments
    ///
    /// * `batch_number` - The batch number
    /// * `block_count` - Number of blocks in the batch
    /// * `first_block` - First block number in the batch
    /// * `last_block` - Last block number in the batch
    /// * `uncompressed_size` - Uncompressed batch size in bytes
    /// * `compressed_size` - Compressed batch size in bytes
    fn on_batch_submitted(
        &self,
        batch_number: u64,
        block_count: usize,
        first_block: u64,
        last_block: u64,
        uncompressed_size: usize,
        compressed_size: usize,
    );
}

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
/// The sequencer receives L2 blocks via an unbounded channel, batches them according
/// to configured criteria, compresses them, and submits them to L1. The unbounded
/// channel allows block fetching to be decoupled from execution - the feeder can
/// continuously fetch blocks while the sequencer processes them at its own pace.
pub struct Sequencer<S, C> {
    /// Batch sink for L1 submission.
    batch_sink: S,
    /// Compressor for batch data.
    compressor: C,
    /// Batch driver managing batch building logic.
    batch_driver: BatchDriver,
    /// Checkpoint for resumption.
    checkpoint: Checkpoint,
    /// Path to save checkpoints (None disables checkpoint persistence).
    checkpoint_path: Option<PathBuf>,
    /// Unbounded channel for receiving blocks (decouples fetching from execution).
    block_rx: mpsc::UnboundedReceiver<L2BlockData>,
    /// Event sender (for observers).
    event_tx: Option<mpsc::UnboundedSender<SequencerEvent>>,
    /// Optional callback for execution events.
    execution_callback: Option<Arc<dyn ExecutionCallback>>,
    /// Optional callback for batch submission events.
    batch_callback: Option<Arc<dyn BatchCallback>>,
    /// Counter for tracking blocks executed (for backlog calculation).
    blocks_executed: u64,
}

impl<S: std::fmt::Debug, C: std::fmt::Debug> std::fmt::Debug for Sequencer<S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sequencer")
            .field("batch_sink", &self.batch_sink)
            .field("compressor", &self.compressor)
            .field("batch_driver", &self.batch_driver)
            .field("checkpoint", &self.checkpoint)
            .field("checkpoint_path", &self.checkpoint_path)
            .field("blocks_executed", &self.blocks_executed)
            .finish_non_exhaustive()
    }
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
    /// * `checkpoint_path` - Optional path to save/load checkpoints. If `None`, checkpointing
    ///   is disabled (useful for harness/demo mode where fresh starts are expected).
    /// * `block_rx` - Unbounded channel for receiving L2 blocks (allows decoupled fetching)
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint cannot be loaded (when path is provided).
    pub fn new(
        batch_sink: S,
        compressor: C,
        batch_driver: BatchDriver,
        checkpoint_path: Option<PathBuf>,
        block_rx: mpsc::UnboundedReceiver<L2BlockData>,
    ) -> eyre::Result<Self> {
        let checkpoint = if let Some(ref path) = checkpoint_path {
            Checkpoint::load(path)?.map_or_else(
                || {
                    tracing::info!("No checkpoint found at {:?}, starting fresh", path);
                    Checkpoint::default()
                },
                |cp| {
                    tracing::info!(
                        "Loaded checkpoint from {:?}, last batch submitted: {}",
                        path,
                        cp.last_batch_submitted
                    );
                    cp
                },
            )
        } else {
            tracing::info!("Checkpoint disabled, starting fresh");
            Checkpoint::default()
        };

        Ok(Self {
            batch_sink,
            compressor,
            batch_driver,
            checkpoint,
            checkpoint_path,
            block_rx,
            event_tx: None,
            execution_callback: None,
            batch_callback: None,
            blocks_executed: 0,
        })
    }

    /// Sets the execution callback for reporting block execution events.
    ///
    /// This callback is invoked each time a block is executed, allowing
    /// external components (like the TUI) to track execution progress
    /// without the sequencer depending on them directly.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback to invoke on block execution
    pub fn with_execution_callback(mut self, callback: Arc<dyn ExecutionCallback>) -> Self {
        self.execution_callback = Some(callback);
        self
    }

    /// Sets the batch callback for reporting batch submission events.
    ///
    /// This callback is invoked each time a batch is submitted to L1, allowing
    /// external components (like the TUI) to track batch progress
    /// without the sequencer depending on them directly.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback to invoke on batch submission
    pub fn with_batch_callback(mut self, callback: Arc<dyn BatchCallback>) -> Self {
        self.batch_callback = Some(callback);
        self
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
    /// Measures execution time and emits events for TUI visibility.
    ///
    /// # Arguments
    ///
    /// * `block` - The L2 block data to process
    ///
    /// # Errors
    ///
    /// Returns an error if batch submission fails.
    async fn process_block(&mut self, block: L2BlockData) -> eyre::Result<()> {
        let start = Instant::now();
        let block_number = block.block_number;
        let tx_count = block.transactions.len();
        let tx_size: usize = block.transactions.iter().map(|tx| tx.len()).sum();

        tracing::debug!(block_number, tx_count, tx_size, "Processing block in sequencer");

        // Add block to batch driver
        self.batch_driver.add_blocks(vec![block.clone()]);

        // Calculate execution time
        let execution_time_ms = start.elapsed().as_millis() as u64;
        self.blocks_executed += 1;

        // Emit block executed event
        self.emit(SequencerEvent::BlockExecuted {
            block_number: block.block_number,
            block_hash: [0u8; 32], // TODO: compute actual block hash
        });

        // Invoke execution callback for TUI visibility
        if let Some(ref callback) = self.execution_callback {
            callback.on_block_executed(block.block_number, execution_time_ms);
        }

        // Check if batch is ready
        let should = self.batch_driver.should_submit();
        tracing::debug!(
            should_submit = should,
            pending_count = self.batch_driver.pending_count(),
            current_size = self.batch_driver.current_size(),
            "Checking if batch should be submitted"
        );
        if should {
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
        let uncompressed_size = batch_data.len();
        let compressed_data = self.compressor.compress(&batch_data)?;
        let compressed_size = compressed_data.len();

        let block_count = pending.blocks.len();
        let first_block = pending.blocks.first().map(|b| b.block_number).unwrap_or(0);
        let last_block = pending.blocks.last().map(|b| b.block_number).unwrap_or(0);
        self.emit(SequencerEvent::BatchBuilt { batch_number, block_count });

        // Submit
        let compressed_batch = CompressedBatch {
            batch_number,
            data: compressed_data,
            block_count: block_count as u64,
            first_block,
            last_block,
        };

        let receipt = self.batch_sink.submit(compressed_batch).await?;

        // Update checkpoint (only save if persistence is enabled)
        self.checkpoint.record_batch_submitted(batch_number);
        self.checkpoint.touch();
        if let Some(ref path) = self.checkpoint_path {
            self.checkpoint.save(path)?;
        }

        // Record in batch driver as well
        self.batch_driver.record_batch_submitted(batch_number)?;

        self.emit(SequencerEvent::BatchSubmitted { batch_number, tx_hash: receipt.tx_hash });

        // Invoke batch callback for TUI visibility
        if let Some(ref callback) = self.batch_callback {
            tracing::debug!(
                batch_number,
                block_count,
                first_block,
                last_block,
                uncompressed_size,
                compressed_size,
                "Invoking batch callback for TUI"
            );
            callback.on_batch_submitted(
                batch_number,
                block_count,
                first_block,
                last_block,
                uncompressed_size,
                compressed_size,
            );
        }

        tracing::info!(
            batch_number,
            block_count,
            first_block,
            last_block,
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
