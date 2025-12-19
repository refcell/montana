//! Validator role implementation.
//!
//! The validator derives and validates batches from L1.

use std::{path::PathBuf, sync::Arc};
// TODO: Re-enable when block execution is restored
#[allow(unused_imports)]
use std::time::Instant;

use async_trait::async_trait;
use montana_checkpoint::Checkpoint;
use primitives::OpBlockBatch;
use tokio::sync::mpsc;
use vm::Executor;

use crate::{Role, RoleCheckpoint, TickResult};

/// Callback trait for derivation events.
///
/// This allows external components (like a TUI) to receive derivation
/// notifications without the validator depending on them directly.
pub trait DerivationCallback: Send + Sync {
    /// Called when a batch has been derived and validated.
    ///
    /// # Arguments
    /// * `batch_number` - The batch number that was derived
    /// * `block_count` - Number of blocks in the batch
    /// * `first_block` - First block number in the batch
    /// * `last_block` - Last block number in the batch
    fn on_batch_derived(
        &self,
        batch_number: u64,
        block_count: usize,
        first_block: u64,
        last_block: u64,
    );

    /// Called when an individual block has been derived and executed.
    ///
    /// # Arguments
    /// * `block_number` - The block number that was derived
    /// * `tx_count` - Number of transactions in the block
    /// * `derivation_time_ms` - Time spent deriving the block (in milliseconds)
    /// * `execution_time_ms` - Time spent executing the block (in milliseconds)
    fn on_block_derived(
        &self,
        block_number: u64,
        tx_count: usize,
        derivation_time_ms: u64,
        execution_time_ms: u64,
    ) {
        // Default no-op implementation for backwards compatibility
        let _ = (block_number, tx_count, derivation_time_ms, execution_time_ms);
    }

    /// Called when a new L1 block is detected.
    ///
    /// This is called when polling L1 detects a new head block. It allows
    /// the TUI to visualize the L1 chain progression.
    ///
    /// # Arguments
    /// * `l1_block_number` - The new L1 head block number
    fn on_l1_block_produced(&self, l1_block_number: u64) {
        // Default no-op implementation for backwards compatibility
        let _ = l1_block_number;
    }
}

/// Events emitted by the validator.
///
/// These events follow the observer pattern, allowing external components
/// (like a TUI or metrics system) to monitor validator progress without
/// tight coupling.
#[derive(Debug, Clone)]
pub enum ValidatorEvent {
    /// A batch was successfully derived from L1.
    ///
    /// Contains the batch number and count of blocks in the batch.
    BatchDerived {
        /// The batch number that was derived.
        batch_number: u64,
        /// Number of blocks contained in this batch.
        block_count: usize,
    },
    /// A derived batch was successfully validated.
    ///
    /// This event is emitted after the batch payload has been executed
    /// and the state transition validated.
    BatchValidated {
        /// The batch number that was validated.
        batch_number: u64,
    },
}

/// The validator role: derives and validates batches from L1.
///
/// Generic over two traits:
/// - `S: L1BatchSource` - Source for reading compressed batches from L1
/// - `C: Compressor` - For decompressing batch data
///
/// The validator polls L1 for new batches, decompresses them, and validates
/// the state transitions. Progress is checkpointed to enable resumption after
/// restarts.
pub struct Validator<S, C> {
    /// Source for reading batches from L1.
    batch_source: S,
    /// Compressor for decompressing batch data.
    compressor: C,
    /// Block executor for EVM execution.
    executor: Box<dyn Executor>,
    /// Checkpoint for resumption.
    checkpoint: Checkpoint,
    /// Path to checkpoint file (None disables checkpoint persistence).
    checkpoint_path: Option<PathBuf>,
    /// Poll interval in milliseconds.
    poll_interval_ms: u64,
    /// Event sender for observer pattern.
    event_tx: Option<mpsc::UnboundedSender<ValidatorEvent>>,
    /// Callback for derivation events (TUI visibility).
    derivation_callback: Option<Arc<dyn DerivationCallback>>,
    /// Last known L1 head for change detection.
    last_l1_head: u64,
}

impl<S: std::fmt::Debug, C: std::fmt::Debug> std::fmt::Debug for Validator<S, C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Validator")
            .field("batch_source", &self.batch_source)
            .field("compressor", &self.compressor)
            .field("executor", &"Box<dyn Executor>")
            .field("checkpoint", &self.checkpoint)
            .field("checkpoint_path", &self.checkpoint_path)
            .field("poll_interval_ms", &self.poll_interval_ms)
            .field("event_tx", &self.event_tx.is_some())
            .field("derivation_callback", &self.derivation_callback.is_some())
            .field("last_l1_head", &self.last_l1_head)
            .finish()
    }
}

impl<S, C> Validator<S, C>
where
    S: montana_pipeline::L1BatchSource + Send + Sync,
    C: montana_pipeline::Compressor + Send + Sync,
{
    /// Creates a new validator instance.
    ///
    /// Loads the checkpoint from the specified path if it exists, otherwise
    /// starts with a default checkpoint.
    ///
    /// # Arguments
    /// * `batch_source` - Source for reading batches from L1
    /// * `compressor` - Compressor for decompressing batch data
    /// * `checkpoint_path` - Optional path to checkpoint file. If `None`, checkpointing
    ///   is disabled (useful for harness/demo mode where fresh starts are expected).
    /// * `executor` - Block executor for EVM execution
    ///
    /// # Errors
    /// Returns an error if the checkpoint file exists but cannot be loaded.
    pub fn new(
        batch_source: S,
        compressor: C,
        checkpoint_path: Option<PathBuf>,
        executor: Box<dyn Executor>,
    ) -> eyre::Result<Self> {
        let checkpoint = if let Some(ref path) = checkpoint_path {
            Checkpoint::load(path)?.map_or_else(
                || {
                    tracing::info!("No checkpoint found at {:?}, starting fresh", path);
                    Checkpoint::default()
                },
                |cp| {
                    tracing::info!(
                        "Loaded checkpoint from {:?}, last batch derived: {}",
                        path,
                        cp.last_batch_derived
                    );
                    cp
                },
            )
        } else {
            tracing::info!("Checkpoint disabled, starting fresh");
            Checkpoint::default()
        };

        Ok(Self {
            batch_source,
            compressor,
            executor,
            checkpoint,
            checkpoint_path,
            poll_interval_ms: 50,
            event_tx: None,
            derivation_callback: None,
            last_l1_head: 0,
        })
    }

    /// Sets the poll interval for checking L1 for new batches.
    ///
    /// # Arguments
    /// * `ms` - Poll interval in milliseconds
    pub const fn with_poll_interval(mut self, ms: u64) -> Self {
        self.poll_interval_ms = ms;
        self
    }

    /// Sets the event sender for observer notifications.
    ///
    /// # Arguments
    /// * `tx` - Unbounded sender for validator events
    pub fn with_event_sender(mut self, tx: mpsc::UnboundedSender<ValidatorEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Sets the derivation callback for reporting batch derivation events.
    ///
    /// This callback is invoked each time a batch is derived and validated,
    /// allowing external components (like the TUI) to track derivation progress
    /// without the validator depending on them directly.
    ///
    /// # Arguments
    ///
    /// * `callback` - The callback to invoke on batch derivation
    pub fn with_derivation_callback(mut self, callback: Arc<dyn DerivationCallback>) -> Self {
        self.derivation_callback = Some(callback);
        self
    }

    /// Emits an event to observers if an event sender is configured.
    ///
    /// # Arguments
    /// * `event` - The event to emit
    fn emit(&self, event: ValidatorEvent) {
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Processes a single derived batch.
    ///
    /// This method:
    /// 1. Checks if the batch was already processed (via checkpoint)
    /// 2. Decompresses the batch data
    /// 3. Counts blocks in the decompressed data
    /// 4. Executes the payload
    /// 5. Updates the checkpoint
    /// 6. Emits events for observers
    ///
    /// # Arguments
    /// * `batch` - The compressed batch to process
    ///
    /// # Errors
    /// Returns an error if decompression, execution, or checkpoint saving fails.
    async fn process_batch(
        &mut self,
        batch: montana_pipeline::CompressedBatch,
    ) -> eyre::Result<()> {
        // Skip if already derived
        if batch.batch_number <= self.checkpoint.last_batch_derived {
            tracing::debug!(batch_number = batch.batch_number, "Skipping already-derived batch");
            return Ok(());
        }

        // Use actual block metadata from the batch
        let block_count = batch.block_count as usize;
        let first_block = batch.first_block;
        let last_block = batch.last_block;

        tracing::info!(
            batch_number = batch.batch_number,
            block_count,
            first_block,
            last_block,
            "Processing batch"
        );

        // Decompress
        let decompressed = self.compressor.decompress(&batch.data)?;

        // Deserialize the OpBlockBatch from the decompressed data
        let block_batch = OpBlockBatch::from_bytes(&decompressed)
            .map_err(|e| eyre::eyre!("Failed to deserialize OpBlockBatch: {}", e))?;

        self.emit(ValidatorEvent::BatchDerived { batch_number: batch.batch_number, block_count });

        // Execute each block in the batch
        for block in block_batch.blocks {
            let block_number = block.header.number;
            let tx_count = block.transactions.len();

            let start = Instant::now();
            let _result = self.executor.execute_block(block);
            let execution_time_ms = start.elapsed().as_millis() as u64;

            // Invoke per-block callback for TUI visibility
            if let Some(ref callback) = self.derivation_callback {
                callback.on_block_derived(block_number, tx_count, 0, execution_time_ms);
            }
        }

        // Update checkpoint (only save if persistence is enabled)
        self.checkpoint.record_batch_derived(batch.batch_number);
        self.checkpoint.touch();
        if let Some(ref path) = self.checkpoint_path {
            self.checkpoint.save(path)?;
        }

        self.emit(ValidatorEvent::BatchValidated { batch_number: batch.batch_number });

        // Invoke batch-level derivation callback for TUI visibility
        if let Some(ref callback) = self.derivation_callback {
            tracing::debug!(
                batch_number = batch.batch_number,
                block_count,
                first_block,
                last_block,
                "Invoking derivation callback for TUI"
            );
            callback.on_batch_derived(batch.batch_number, block_count, first_block, last_block);
        }

        tracing::info!(
            batch_number = batch.batch_number,
            block_count,
            first_block,
            last_block,
            "Batch executed and validated successfully"
        );

        Ok(())
    }
}

#[async_trait]
impl<S, C> Role for Validator<S, C>
where
    S: montana_pipeline::L1BatchSource + Send + Sync,
    C: montana_pipeline::Compressor + Send + Sync,
{
    fn name(&self) -> &'static str {
        "validator"
    }

    async fn resume(&mut self, checkpoint: &Checkpoint) -> eyre::Result<()> {
        self.checkpoint = checkpoint.clone();

        tracing::info!(
            last_batch = checkpoint.last_batch_derived,
            "Validator resuming from batch {}",
            checkpoint.last_batch_derived + 1
        );

        Ok(())
    }

    async fn tick(&mut self) -> eyre::Result<TickResult> {
        // Check for L1 head changes and emit L1BlockProduced events
        if let Some(ref callback) = self.derivation_callback
            && let Ok(l1_head) = self.batch_source.l1_head().await
            && l1_head > self.last_l1_head
        {
            // Emit events for all new L1 blocks since last check
            for block_num in (self.last_l1_head + 1)..=l1_head {
                callback.on_l1_block_produced(block_num);
            }
            self.last_l1_head = l1_head;
        }

        // Process all available batches (non-blocking drain)
        // This allows the validator to catch up quickly when there's a backlog
        let mut batches_processed = 0;
        const MAX_BATCHES_PER_TICK: usize = 10; // Limit to avoid starving other tasks

        loop {
            match self.batch_source.next_batch().await {
                Ok(Some(batch)) => {
                    self.process_batch(batch).await?;
                    batches_processed += 1;

                    // Limit batches per tick to avoid blocking the event loop too long
                    if batches_processed >= MAX_BATCHES_PER_TICK {
                        break;
                    }
                }
                Ok(None) => {
                    // No more batches available
                    break;
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "Error fetching next batch, will retry");
                    break;
                }
            }
        }

        // If we processed batches, report progress
        if batches_processed > 0 {
            return Ok(TickResult::Progress);
        }

        // No batches were available, idle
        tokio::time::sleep(tokio::time::Duration::from_millis(self.poll_interval_ms)).await;
        Ok(TickResult::Idle)
    }

    fn checkpoint(&self) -> RoleCheckpoint {
        RoleCheckpoint {
            last_batch_derived: Some(self.checkpoint.last_batch_derived),
            last_block_processed: Some(self.checkpoint.last_block_executed),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_event_debug() {
        let event1 = ValidatorEvent::BatchDerived { batch_number: 1, block_count: 5 };
        let event2 = ValidatorEvent::BatchValidated { batch_number: 1 };

        let debug1 = format!("{:?}", event1);
        let debug2 = format!("{:?}", event2);

        assert!(debug1.contains("BatchDerived"));
        assert!(debug1.contains("batch_number"));
        assert!(debug1.contains("block_count"));

        assert!(debug2.contains("BatchValidated"));
        assert!(debug2.contains("batch_number"));
    }

    #[test]
    fn test_validator_event_clone() {
        let event = ValidatorEvent::BatchDerived { batch_number: 42, block_count: 10 };
        let cloned = event.clone();

        if let ValidatorEvent::BatchDerived { batch_number, block_count } = cloned {
            assert_eq!(batch_number, 42);
            assert_eq!(block_count, 10);
        } else {
            panic!("Event type mismatch after clone");
        }
    }
}
