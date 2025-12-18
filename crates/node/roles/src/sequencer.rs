//! Sequencer role implementation.
//!
//! The sequencer role executes blocks and submits batches to L1.

use std::{collections::BinaryHeap, path::PathBuf, sync::Arc, time::Instant};

use async_trait::async_trait;
use montana_batcher::BatcherConfig;
use montana_checkpoint::Checkpoint;
use montana_pipeline::{BatchSink, CompressedBatch, Compressor};
use primitives::{OpBlock, OpBlockBatch};
use tokio::sync::mpsc;

use crate::{Role, RoleCheckpoint, TickResult};

/// Maximum blocks per batch when heavily backlogged (200+ blocks behind).
const MAX_BLOCKS_HEAVY_BACKLOG: usize = 100;
/// Maximum blocks per batch when moderately backlogged (50-200 blocks behind).
const MAX_BLOCKS_MODERATE_BACKLOG: usize = 50;
/// Maximum blocks per batch when lightly backlogged (10-50 blocks behind).
const MAX_BLOCKS_LIGHT_BACKLOG: usize = 25;
/// Threshold for heavy backlog.
const HEAVY_BACKLOG_THRESHOLD: usize = 200;
/// Threshold for moderate backlog.
const MODERATE_BACKLOG_THRESHOLD: usize = 50;
/// Threshold for light backlog.
const LIGHT_BACKLOG_THRESHOLD: usize = 10;
/// Maximum concurrent batch preparations (compression workers).
const MAX_CONCURRENT_PREPARATIONS: usize = 4;

/// A batch that has been prepared (serialized and compressed) and is ready for submission.
///
/// This struct holds all the data needed to submit a batch to L1, allowing
/// the expensive compression work to be done in parallel while maintaining
/// proper ordering for submission.
#[derive(Debug)]
struct PreparedBatch {
    /// The batch number (used for ordering).
    batch_number: u64,
    /// The compressed batch data ready for submission.
    compressed_batch: CompressedBatch,
    /// Uncompressed size for metrics.
    uncompressed_size: usize,
    /// Compressed size for metrics.
    compressed_size: usize,
    /// Block count in this batch.
    block_count: usize,
    /// First block number in the batch.
    first_block: u64,
    /// Last block number in the batch.
    last_block: u64,
    /// Transaction counts per block (for TUI callback).
    block_tx_counts: Vec<usize>,
}

// Implement ordering for the priority queue (min-heap by batch_number)
impl PartialEq for PreparedBatch {
    fn eq(&self, other: &Self) -> bool {
        self.batch_number == other.batch_number
    }
}

impl Eq for PreparedBatch {}

impl PartialOrd for PreparedBatch {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PreparedBatch {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Reverse ordering for min-heap (lower batch_number = higher priority)
        other.batch_number.cmp(&self.batch_number)
    }
}

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
    /// * `gas_used` - Gas used by the block
    fn on_block_executed(&self, block_number: u64, execution_time_ms: u64, gas_used: u64);
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
    /// * `block_tx_counts` - Transaction counts for each block in the batch (in order)
    /// * `l1_block_number` - The L1 block number where the batch was included
    #[allow(clippy::too_many_arguments)]
    fn on_batch_submitted(
        &self,
        batch_number: u64,
        block_count: usize,
        first_block: u64,
        last_block: u64,
        uncompressed_size: usize,
        compressed_size: usize,
        block_tx_counts: &[usize],
        l1_block_number: u64,
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
/// The sequencer receives full `OpBlock` types via an unbounded channel, batches them
/// according to configured criteria, serializes them as `OpBlockBatch`, compresses the
/// serialized data, and submits to L1. Using full `OpBlock` preserves all block information
/// for the validator to deserialize. The unbounded channel allows block fetching to be
/// decoupled from execution - the feeder can continuously fetch blocks while the
/// sequencer processes them at its own pace.
///
/// ## Parallel Batch Preparation
///
/// When the sequencer falls behind, it spawns parallel compression workers to prepare
/// multiple batches concurrently. The expensive work (serialization + compression) happens
/// in parallel, while submission to L1 remains sequential to maintain proper ordering.
/// This architecture allows the sequencer to catch up quickly when backlogged.
pub struct Sequencer<S, C> {
    /// Batch sink for L1 submission.
    batch_sink: S,
    /// Compressor for batch data (wrapped in Arc for sharing with worker tasks).
    compressor: Arc<C>,
    /// Batcher configuration for batch submission criteria.
    batcher_config: BatcherConfig,
    /// Checkpoint for resumption.
    checkpoint: Checkpoint,
    /// Path to save checkpoints (None disables checkpoint persistence).
    checkpoint_path: Option<PathBuf>,
    /// Unbounded channel for receiving full OpBlocks (decouples fetching from execution).
    block_rx: mpsc::UnboundedReceiver<OpBlock>,
    /// Pending blocks waiting to be batched.
    pending_blocks: Vec<OpBlock>,
    /// Current accumulated batch size in bytes (serialized size).
    current_batch_size: usize,
    /// Last batch submission time.
    last_submission: Instant,
    /// Next batch number to assign to a preparation task.
    next_batch_number: u64,
    /// Next batch number expected for submission (for ordering).
    next_submission_number: u64,
    /// Event sender (for observers).
    event_tx: Option<mpsc::UnboundedSender<SequencerEvent>>,
    /// Optional callback for execution events.
    execution_callback: Option<Arc<dyn ExecutionCallback>>,
    /// Optional callback for batch submission events.
    batch_callback: Option<Arc<dyn BatchCallback>>,
    /// Counter for tracking blocks executed (for backlog calculation).
    blocks_executed: u64,
    /// Receiver for prepared batches from compression workers.
    prepared_rx: mpsc::UnboundedReceiver<Result<PreparedBatch, String>>,
    /// Sender for prepared batches (cloned to workers).
    prepared_tx: mpsc::UnboundedSender<Result<PreparedBatch, String>>,
    /// Number of batches currently being prepared by workers.
    in_flight_preparations: usize,
    /// Priority queue of prepared batches waiting for submission (ordered by batch_number).
    ready_batches: BinaryHeap<PreparedBatch>,
}

impl<S: std::fmt::Debug, C: std::fmt::Debug> std::fmt::Debug for Sequencer<S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sequencer")
            .field("batch_sink", &self.batch_sink)
            .field("compressor", &self.compressor)
            .field("batcher_config", &self.batcher_config)
            .field("checkpoint", &self.checkpoint)
            .field("checkpoint_path", &self.checkpoint_path)
            .field("blocks_executed", &self.blocks_executed)
            .field("pending_blocks", &self.pending_blocks.len())
            .field("current_batch_size", &self.current_batch_size)
            .field("next_batch_number", &self.next_batch_number)
            .field("next_submission_number", &self.next_submission_number)
            .field("in_flight_preparations", &self.in_flight_preparations)
            .field("ready_batches", &self.ready_batches.len())
            .finish_non_exhaustive()
    }
}

impl<S, C> Sequencer<S, C>
where
    S: BatchSink,
    C: Compressor + 'static,
{
    /// Creates a new sequencer role.
    ///
    /// # Arguments
    ///
    /// * `batch_sink` - The sink for submitting batches to L1
    /// * `compressor` - The compressor for batch data
    /// * `batcher_config` - Configuration for batch submission criteria
    /// * `checkpoint_path` - Optional path to save/load checkpoints. If `None`, checkpointing
    ///   is disabled (useful for harness/demo mode where fresh starts are expected).
    /// * `block_rx` - Unbounded channel for receiving full OpBlocks (allows decoupled fetching)
    ///
    /// # Errors
    ///
    /// Returns an error if the checkpoint cannot be loaded (when path is provided).
    pub fn new(
        batch_sink: S,
        compressor: C,
        batcher_config: BatcherConfig,
        checkpoint_path: Option<PathBuf>,
        block_rx: mpsc::UnboundedReceiver<OpBlock>,
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

        // Create channel for prepared batches from compression workers
        let (prepared_tx, prepared_rx) = mpsc::unbounded_channel();

        Ok(Self {
            batch_sink,
            compressor: Arc::new(compressor),
            batcher_config,
            checkpoint,
            checkpoint_path,
            block_rx,
            pending_blocks: Vec::new(),
            current_batch_size: 0,
            last_submission: Instant::now(),
            next_batch_number: 1, // Start at 1 to avoid checkpoint skip issue with batch 0
            next_submission_number: 1, // Must match next_batch_number initially
            event_tx: None,
            execution_callback: None,
            batch_callback: None,
            blocks_executed: 0,
            prepared_rx,
            prepared_tx,
            in_flight_preparations: 0,
            ready_batches: BinaryHeap::new(),
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

    /// Determines if a batch should be submitted based on configured criteria.
    fn should_submit(&self) -> bool {
        let size_ready = self.current_batch_size >= self.batcher_config.min_batch_size;
        let time_ready = self.last_submission.elapsed() >= self.batcher_config.batch_interval;
        let blocks_ready =
            self.pending_blocks.len() >= self.batcher_config.max_blocks_per_batch as usize;

        size_ready || time_ready || blocks_ready
    }

    /// Calculates the dynamic maximum blocks per batch based on backlog depth.
    ///
    /// When the sequencer falls behind, we dynamically increase the batch size
    /// to catch up faster. This reduces L1 transaction overhead and improves
    /// overall throughput.
    ///
    /// # Arguments
    ///
    /// * `backlog_depth` - Number of blocks waiting in the channel
    ///
    /// # Returns
    ///
    /// The maximum number of blocks to include in the next batch.
    const fn dynamic_max_blocks(&self, backlog_depth: usize) -> usize {
        if backlog_depth >= HEAVY_BACKLOG_THRESHOLD {
            MAX_BLOCKS_HEAVY_BACKLOG
        } else if backlog_depth >= MODERATE_BACKLOG_THRESHOLD {
            MAX_BLOCKS_MODERATE_BACKLOG
        } else if backlog_depth >= LIGHT_BACKLOG_THRESHOLD {
            MAX_BLOCKS_LIGHT_BACKLOG
        } else {
            // Use configured max when caught up or lightly behind
            self.batcher_config.max_blocks_per_batch as usize
        }
    }

    /// Estimates the current backlog depth by checking how many blocks are
    /// waiting in the channel plus any pending blocks not yet submitted.
    fn estimate_backlog(&self) -> usize {
        // Channel length gives us blocks waiting to be accumulated
        // Plus pending blocks that haven't been submitted yet
        self.block_rx.len() + self.pending_blocks.len()
    }

    /// Spawns a compression worker task to prepare a batch in the background.
    ///
    /// This method takes a set of blocks, assigns them a batch number, and spawns
    /// a tokio task to serialize and compress them. The result is sent back via
    /// the prepared_tx channel for ordered submission.
    ///
    /// # Arguments
    ///
    /// * `blocks` - The blocks to include in this batch
    /// * `batch_number` - The batch number to assign
    fn spawn_preparation_worker(&self, blocks: Vec<OpBlock>, batch_number: u64) {
        let compressor = Arc::clone(&self.compressor);
        let tx = self.prepared_tx.clone();

        // Collect metadata before moving blocks
        let block_count = blocks.len();
        let first_block = blocks.first().map(|b| b.header.number).unwrap_or(0);
        let last_block = blocks.last().map(|b| b.header.number).unwrap_or(0);
        let block_tx_counts: Vec<usize> = blocks.iter().map(|b| b.transactions.len()).collect();

        tracing::debug!(
            batch_number,
            block_count,
            first_block,
            last_block,
            "Spawning compression worker"
        );

        tokio::spawn(async move {
            // Do the expensive work in the background task
            let result = Self::prepare_batch_sync(
                compressor,
                blocks,
                batch_number,
                block_count,
                first_block,
                last_block,
                block_tx_counts,
            );

            // Send result back to main task
            let _ = tx.send(result);
        });
    }

    /// Synchronously prepares a batch (serialization + compression).
    ///
    /// This is the CPU-intensive work that runs in spawned tasks.
    fn prepare_batch_sync(
        compressor: Arc<C>,
        blocks: Vec<OpBlock>,
        batch_number: u64,
        block_count: usize,
        first_block: u64,
        last_block: u64,
        block_tx_counts: Vec<usize>,
    ) -> Result<PreparedBatch, String> {
        // Serialize the blocks as OpBlockBatch
        let batch_payload = OpBlockBatch::new(blocks);
        let batch_data = batch_payload
            .to_bytes()
            .map_err(|e| format!("Failed to serialize OpBlockBatch: {}", e))?;

        let uncompressed_size = batch_data.len();

        // Compress (this is the expensive part)
        let compressed_data =
            compressor.compress(&batch_data).map_err(|e| format!("Compression failed: {}", e))?;

        let compressed_size = compressed_data.len();

        tracing::debug!(
            batch_number,
            block_count,
            uncompressed_size,
            compressed_size,
            compression_ratio =
                format!("{:.2}x", uncompressed_size as f64 / compressed_size as f64),
            "Batch preparation complete"
        );

        Ok(PreparedBatch {
            batch_number,
            compressed_batch: CompressedBatch {
                batch_number,
                data: compressed_data,
                block_count: block_count as u64,
                first_block,
                last_block,
            },
            uncompressed_size,
            compressed_size,
            block_count,
            first_block,
            last_block,
            block_tx_counts,
        })
    }

    /// Collects completed preparations from workers and adds them to the ready queue.
    ///
    /// This is non-blocking - it only collects results that are already available.
    fn collect_prepared_batches(&mut self) {
        loop {
            match self.prepared_rx.try_recv() {
                Ok(Ok(prepared)) => {
                    tracing::debug!(
                        batch_number = prepared.batch_number,
                        "Collected prepared batch"
                    );
                    self.ready_batches.push(prepared);
                    self.in_flight_preparations = self.in_flight_preparations.saturating_sub(1);
                }
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "Batch preparation failed");
                    self.in_flight_preparations = self.in_flight_preparations.saturating_sub(1);
                    // TODO: Handle preparation failures (retry?)
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    tracing::warn!("Prepared batch channel disconnected");
                    break;
                }
            }
        }
    }

    /// Submits all ready batches that are in order.
    ///
    /// Only submits batches whose batch_number matches next_submission_number,
    /// to maintain proper ordering on L1.
    async fn submit_ready_batches(&mut self) -> eyre::Result<usize> {
        let mut submitted = 0;

        while let Some(prepared) = self.ready_batches.peek() {
            if prepared.batch_number != self.next_submission_number {
                // Next batch in queue isn't the one we need - wait for it
                tracing::debug!(
                    waiting_for = self.next_submission_number,
                    next_ready = prepared.batch_number,
                    "Waiting for in-order batch"
                );
                break;
            }

            // Pop and submit
            let prepared = self.ready_batches.pop().unwrap();
            self.submit_prepared_batch(prepared).await?;
            submitted += 1;
        }

        Ok(submitted)
    }

    /// Submits a prepared batch to L1.
    ///
    /// This handles the actual L1 submission and all the bookkeeping
    /// (checkpoint updates, callbacks, events, etc.)
    async fn submit_prepared_batch(&mut self, prepared: PreparedBatch) -> eyre::Result<()> {
        let batch_number = prepared.batch_number;

        // Skip if already submitted (shouldn't happen with proper ordering, but be safe)
        if self.checkpoint.should_skip_batch(batch_number) {
            tracing::info!(batch_number, "Skipping already-submitted batch");
            self.next_submission_number += 1;
            return Ok(());
        }

        self.emit(SequencerEvent::BatchBuilt { batch_number, block_count: prepared.block_count });

        // Submit to L1
        let receipt = self.batch_sink.submit(prepared.compressed_batch).await?;

        // Update checkpoint
        self.checkpoint.record_batch_submitted(batch_number);
        self.checkpoint.touch();
        if let Some(ref path) = self.checkpoint_path {
            self.checkpoint.save(path)?;
        }

        // Update internal state
        self.next_submission_number += 1;
        self.last_submission = Instant::now();

        self.emit(SequencerEvent::BatchSubmitted { batch_number, tx_hash: receipt.tx_hash });

        // Invoke batch callback for TUI visibility
        if let Some(ref callback) = self.batch_callback {
            tracing::debug!(
                batch_number,
                block_count = prepared.block_count,
                first_block = prepared.first_block,
                last_block = prepared.last_block,
                uncompressed_size = prepared.uncompressed_size,
                compressed_size = prepared.compressed_size,
                l1_block = receipt.l1_block,
                "Invoking batch callback for TUI"
            );
            callback.on_batch_submitted(
                batch_number,
                prepared.block_count,
                prepared.first_block,
                prepared.last_block,
                prepared.uncompressed_size,
                prepared.compressed_size,
                &prepared.block_tx_counts,
                receipt.l1_block,
            );
        }

        tracing::info!(
            batch_number,
            block_count = prepared.block_count,
            first_block = prepared.first_block,
            last_block = prepared.last_block,
            tx_hash = hex::encode(receipt.tx_hash),
            l1_block = receipt.l1_block,
            "Batch submitted to L1"
        );

        Ok(())
    }

    /// Accumulates a single incoming block without triggering batch submission.
    ///
    /// This method adds the block to the pending list, emits events, and invokes
    /// callbacks, but does NOT check or trigger batch submission. The caller
    /// (typically `tick()`) is responsible for deciding when to submit.
    ///
    /// This separation allows `tick()` to accumulate multiple blocks when backlogged
    /// before submitting a batch, enabling proper batching of blocks.
    ///
    /// # Arguments
    ///
    /// * `block` - The full OpBlock to accumulate
    fn accumulate_block(&mut self, block: OpBlock) {
        let start = Instant::now();
        let block_number = block.header.number;
        let tx_count = block.transactions.len();
        let gas_used = block.header.gas_used;

        // Calculate the serialized size of this block
        let block_size = serde_json::to_vec(&block).map(|v| v.len()).unwrap_or(0);

        tracing::debug!(
            block_number,
            tx_count,
            block_size,
            gas_used,
            "Accumulating block in sequencer"
        );

        // Add block to pending list
        self.pending_blocks.push(block);
        self.current_batch_size += block_size;

        // Calculate execution time
        let execution_time_ms = start.elapsed().as_millis() as u64;
        self.blocks_executed += 1;

        // Emit block executed event
        self.emit(SequencerEvent::BlockExecuted {
            block_number,
            block_hash: [0u8; 32], // TODO: compute actual block hash
        });

        // Invoke execution callback for TUI visibility
        if let Some(ref callback) = self.execution_callback {
            callback.on_block_executed(block_number, execution_time_ms, gas_used);
        }

        tracing::debug!(
            pending_count = self.pending_blocks.len(),
            current_size = self.current_batch_size,
            "Block accumulated, deferring submission check to tick()"
        );
    }
}

#[async_trait]
impl<S, C> Role for Sequencer<S, C>
where
    S: BatchSink,
    C: Compressor + 'static,
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
        // =====================================================================
        // PARALLEL BATCH PREPARATION PIPELINE
        //
        // This tick() implementation uses a pipelined approach:
        // 1. Collect completed preparations from workers
        // 2. Submit any ready batches (in order)
        // 3. Accumulate new blocks from the channel
        // 4. Spawn new preparation workers (up to MAX_CONCURRENT_PREPARATIONS)
        //
        // The key insight is that compression is CPU-intensive and can be
        // parallelized, while submission must be sequential for ordering.
        // =====================================================================

        let mut made_progress = false;

        // Phase 1: Collect completed preparations from background workers
        self.collect_prepared_batches();

        // Phase 2: Submit any ready batches that are in order
        let submitted = self.submit_ready_batches().await?;
        if submitted > 0 {
            made_progress = true;
            tracing::debug!(submitted, "Submitted ready batches");
        }

        // Phase 3: Accumulate blocks from the channel
        let backlog = self.estimate_backlog();
        let dynamic_max = self.dynamic_max_blocks(backlog);
        let mut blocks_accumulated = 0;
        let mut channel_disconnected = false;

        // Accumulate blocks until we have enough for a batch
        loop {
            match self.block_rx.try_recv() {
                Ok(block) => {
                    self.accumulate_block(block);
                    blocks_accumulated += 1;

                    // Stop when we have enough for a dynamically-sized batch
                    if self.pending_blocks.len() >= dynamic_max {
                        break;
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    channel_disconnected = true;
                    break;
                }
            }
        }

        if blocks_accumulated > 0 {
            made_progress = true;
        }

        // Phase 4: Spawn preparation workers for pending blocks
        // Calculate how many workers we can spawn
        let available_slots =
            MAX_CONCURRENT_PREPARATIONS.saturating_sub(self.in_flight_preparations);

        if available_slots > 0 && !self.pending_blocks.is_empty() {
            let current_backlog = self.estimate_backlog();
            let is_backlogged = current_backlog >= LIGHT_BACKLOG_THRESHOLD;

            if is_backlogged {
                // CATCH-UP MODE: Spawn multiple preparation workers in parallel
                //
                // When backlogged, we aggressively spawn workers to prepare
                // multiple batches concurrently. This parallelizes the expensive
                // compression work.
                let mut workers_spawned = 0;

                while workers_spawned < available_slots && !self.pending_blocks.is_empty() {
                    // Calculate batch size based on remaining backlog
                    let remaining_backlog = self.pending_blocks.len() + self.block_rx.len();
                    let effective_max = self.dynamic_max_blocks(remaining_backlog);

                    // Require at least half a batch to spawn a worker
                    let min_for_spawn = (effective_max / 2).max(1);
                    if self.pending_blocks.len() < min_for_spawn {
                        break;
                    }

                    // Take blocks for this batch
                    let batch_size = self.pending_blocks.len().min(effective_max);
                    let blocks: Vec<OpBlock> = self.pending_blocks.drain(..batch_size).collect();

                    // Assign batch number and spawn worker
                    let batch_number = self.next_batch_number;
                    self.next_batch_number += 1;

                    tracing::info!(
                        batch_number,
                        block_count = blocks.len(),
                        backlog = remaining_backlog,
                        in_flight = self.in_flight_preparations,
                        "CATCH-UP MODE: spawning compression worker"
                    );

                    self.spawn_preparation_worker(blocks, batch_number);
                    self.in_flight_preparations += 1;
                    workers_spawned += 1;
                    made_progress = true;

                    // Recalculate batch size after taking blocks
                    self.current_batch_size = self
                        .pending_blocks
                        .iter()
                        .map(|b| serde_json::to_vec(b).map(|v| v.len()).unwrap_or(0))
                        .sum();
                }
            } else {
                // NORMAL MODE: Only spawn if we meet submission criteria
                if self.should_submit() && available_slots > 0 {
                    let batch_size = self
                        .pending_blocks
                        .len()
                        .min(self.batcher_config.max_blocks_per_batch as usize);
                    let blocks: Vec<OpBlock> = self.pending_blocks.drain(..batch_size).collect();

                    let batch_number = self.next_batch_number;
                    self.next_batch_number += 1;

                    tracing::debug!(
                        batch_number,
                        block_count = blocks.len(),
                        "Normal mode: spawning compression worker"
                    );

                    self.spawn_preparation_worker(blocks, batch_number);
                    self.in_flight_preparations += 1;
                    made_progress = true;

                    // Recalculate batch size
                    self.current_batch_size = self
                        .pending_blocks
                        .iter()
                        .map(|b| serde_json::to_vec(b).map(|v| v.len()).unwrap_or(0))
                        .sum();
                }
            }
        }

        // Handle channel disconnection
        if channel_disconnected {
            // Block producer finished - flush remaining blocks
            if !self.pending_blocks.is_empty() {
                let blocks = std::mem::take(&mut self.pending_blocks);
                let batch_number = self.next_batch_number;
                self.next_batch_number += 1;

                tracing::info!(
                    batch_number,
                    block_count = blocks.len(),
                    "Channel disconnected, spawning final compression worker"
                );

                self.spawn_preparation_worker(blocks, batch_number);
                self.in_flight_preparations += 1;
                self.current_batch_size = 0;
            }

            // Wait for all in-flight preparations to complete
            while self.in_flight_preparations > 0 || !self.ready_batches.is_empty() {
                // Collect any completed preparations
                self.collect_prepared_batches();

                // Submit ready batches
                self.submit_ready_batches().await?;

                // If still waiting, yield briefly
                if self.in_flight_preparations > 0 && self.ready_batches.is_empty() {
                    tokio::task::yield_now().await;
                }
            }

            return Ok(TickResult::Complete);
        }

        if made_progress { Ok(TickResult::Progress) } else { Ok(TickResult::Idle) }
    }

    fn checkpoint(&self) -> RoleCheckpoint {
        RoleCheckpoint {
            last_batch_submitted: Some(self.checkpoint.last_batch_submitted),
            last_block_processed: Some(self.checkpoint.last_block_executed),
            last_batch_derived: None,
        }
    }
}
