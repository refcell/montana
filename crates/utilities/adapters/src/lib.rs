#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use montana_harness::HarnessProgressReporter;
use montana_roles::{BatchCallback, DerivationCallback, ExecutionCallback};
use montana_tui::{TuiEvent, TuiHandle};
use tokio::sync::mpsc;

/// Shared state for tracking per-block transaction counts between batch submission and derivation.
///
/// This allows the derivation callback to know the transaction count for each block
/// when it's derived, even though that information is only available at batch submission time.
#[derive(Debug, Default)]
pub struct BlockTxCountStore {
    /// Map of block number to transaction count
    tx_counts: RwLock<HashMap<u64, usize>>,
}

impl BlockTxCountStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self { tx_counts: RwLock::new(HashMap::new()) }
    }

    /// Record transaction counts for a range of blocks.
    ///
    /// # Arguments
    /// * `first_block` - First block number in the batch
    /// * `block_tx_counts` - Transaction counts for each block (in order)
    pub fn record_batch(&self, first_block: u64, block_tx_counts: &[usize]) {
        let mut counts = self.tx_counts.write().unwrap();
        for (i, &tx_count) in block_tx_counts.iter().enumerate() {
            counts.insert(first_block + i as u64, tx_count);
        }
    }

    /// Get the transaction count for a block.
    ///
    /// Returns 0 if the block is not found (e.g., in validator-only mode).
    pub fn get_tx_count(&self, block_number: u64) -> usize {
        self.tx_counts.read().unwrap().get(&block_number).copied().unwrap_or(0)
    }

    /// Remove the transaction count for a block (to prevent unbounded growth).
    pub fn remove(&self, block_number: u64) {
        self.tx_counts.write().unwrap().remove(&block_number);
    }
}

/// Boxed block producer for type erasure.
pub type BoxedBlockProducer = Box<dyn blocksource::BlockProducer>;

/// Debug wrapper for boxed block producer.
pub struct BlockProducerWrapper {
    inner: BoxedBlockProducer,
    #[allow(dead_code)]
    description: &'static str,
}

impl std::fmt::Debug for BlockProducerWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BlockProducerWrapper({})", self.description)
    }
}

impl BlockProducerWrapper {
    /// Create a new block producer wrapper.
    pub fn new<P: blocksource::BlockProducer + 'static>(
        producer: P,
        description: &'static str,
    ) -> Self {
        Self { inner: Box::new(producer), description }
    }
}

#[async_trait]
impl blocksource::BlockProducer for BlockProducerWrapper {
    async fn produce(&self, tx: mpsc::Sender<blocksource::OpBlock>) -> eyre::Result<()> {
        self.inner.produce(tx).await
    }

    async fn get_chain_tip(&self) -> eyre::Result<u64> {
        self.inner.get_chain_tip().await
    }

    async fn get_block(&self, number: u64) -> eyre::Result<Option<blocksource::OpBlock>> {
        self.inner.get_block(number).await
    }
}

/// Adapter that bridges montana_batch_context::BatchSink to montana_pipeline::BatchSink.
pub struct BatchSinkAdapter {
    inner: Arc<Box<dyn montana_batch_context::BatchSink>>,
}

impl std::fmt::Debug for BatchSinkAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchSinkAdapter").finish_non_exhaustive()
    }
}

impl BatchSinkAdapter {
    /// Create a new batch sink adapter.
    pub fn new(inner: Arc<Box<dyn montana_batch_context::BatchSink>>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl montana_pipeline::BatchSink for BatchSinkAdapter {
    async fn submit(
        &mut self,
        batch: montana_pipeline::CompressedBatch,
    ) -> Result<montana_pipeline::SubmissionReceipt, montana_pipeline::SinkError> {
        self.inner
            .submit(batch)
            .await
            .map_err(|e| montana_pipeline::SinkError::Connection(e.to_string()))
    }

    async fn capacity(&self) -> Result<usize, montana_pipeline::SinkError> {
        // Return a reasonable default capacity
        // In a real implementation, this would query the underlying sink
        Ok(1000)
    }

    async fn health_check(&self) -> Result<(), montana_pipeline::SinkError> {
        // For now, always return healthy
        // In a real implementation, this would check the underlying sink
        Ok(())
    }
}

/// Adapter that bridges BatchContext source to montana_pipeline::L1BatchSource.
#[derive(Debug)]
pub struct BatchSourceAdapter {
    inner: Arc<montana_batch_context::BatchContext>,
}

impl BatchSourceAdapter {
    /// Create a new batch source adapter.
    pub const fn new(inner: Arc<montana_batch_context::BatchContext>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl montana_pipeline::L1BatchSource for BatchSourceAdapter {
    async fn next_batch(
        &mut self,
    ) -> Result<Option<montana_pipeline::CompressedBatch>, montana_pipeline::SourceError> {
        self.inner
            .source()
            .next_batch()
            .await
            .map_err(|e| montana_pipeline::SourceError::Connection(e.to_string()))
    }

    async fn l1_head(&self) -> Result<u64, montana_pipeline::SourceError> {
        self.inner
            .source()
            .l1_head()
            .await
            .map_err(|e| montana_pipeline::SourceError::Connection(e.to_string()))
    }
}

/// Adapter that implements ExecutionCallback and sends TUI events.
///
/// This allows the sequencer to report execution metrics to the TUI
/// without depending on the TUI crate directly.
#[derive(Debug)]
pub struct TuiExecutionCallback {
    handle: TuiHandle,
}

impl TuiExecutionCallback {
    /// Create a new TUI execution callback.
    pub const fn new(handle: TuiHandle) -> Self {
        Self { handle }
    }
}

impl ExecutionCallback for TuiExecutionCallback {
    fn on_block_executed(&self, block_number: u64, execution_time_ms: u64) {
        self.handle.send(TuiEvent::BlockExecuted { block_number, execution_time_ms });
    }
}

/// Adapter that implements BatchCallback and sends TUI events.
///
/// This allows the sequencer to report batch submissions to the TUI
/// without depending on the TUI crate directly.
#[derive(Debug)]
pub struct TuiBatchCallback {
    handle: TuiHandle,
    /// Shared store for block transaction counts
    tx_count_store: Arc<BlockTxCountStore>,
}

impl TuiBatchCallback {
    /// Create a new TUI batch callback with a shared tx count store.
    pub const fn new(handle: TuiHandle, tx_count_store: Arc<BlockTxCountStore>) -> Self {
        Self { handle, tx_count_store }
    }
}

impl BatchCallback for TuiBatchCallback {
    fn on_batch_submitted(
        &self,
        batch_number: u64,
        block_count: usize,
        first_block: u64,
        last_block: u64,
        uncompressed_size: usize,
        compressed_size: usize,
        block_tx_counts: &[usize],
    ) {
        // Store the tx counts for later retrieval during derivation
        self.tx_count_store.record_batch(first_block, block_tx_counts);

        self.handle.send(TuiEvent::BatchSubmitted {
            batch_number,
            block_count,
            first_block,
            last_block,
            uncompressed_size,
            compressed_size,
        });
    }
}

/// Adapter that implements DerivationCallback and sends TUI events.
///
/// This allows the validator to report batch derivation events to the TUI
/// without depending on the TUI crate directly.
#[derive(Debug)]
pub struct TuiDerivationCallback {
    handle: TuiHandle,
    /// Shared store for block transaction counts
    tx_count_store: Arc<BlockTxCountStore>,
}

impl TuiDerivationCallback {
    /// Create a new TUI derivation callback with a shared tx count store.
    pub const fn new(handle: TuiHandle, tx_count_store: Arc<BlockTxCountStore>) -> Self {
        Self { handle, tx_count_store }
    }
}

impl DerivationCallback for TuiDerivationCallback {
    fn on_batch_derived(
        &self,
        _batch_number: u64,
        _block_count: usize,
        _first_block: u64,
        _last_block: u64,
    ) {
        // Block-level events are now sent via on_block_derived
        // This callback is used only for batch-level processing
    }

    fn on_block_derived(
        &self,
        block_number: u64,
        _tx_count: usize,
        derivation_time_ms: u64,
        execution_time_ms: u64,
    ) {
        // Look up the actual tx count from the shared store (populated by batch callback)
        // This will return 0 if not found (e.g., in validator-only mode)
        let actual_tx_count = self.tx_count_store.get_tx_count(block_number);

        // Remove the entry to prevent unbounded growth
        self.tx_count_store.remove(block_number);

        self.handle.send(TuiEvent::BlockDerived {
            number: block_number,
            tx_count: actual_tx_count,
            derivation_time_ms,
            execution_time_ms,
        });
    }
}

/// Adapter that implements HarnessProgressReporter for TUI feedback.
///
/// This bridges the harness progress reporting to TUI events.
#[derive(Debug)]
pub struct HarnessProgressAdapter {
    handle: TuiHandle,
}

impl HarnessProgressAdapter {
    /// Create a new harness progress adapter.
    pub const fn new(handle: TuiHandle) -> Self {
        Self { handle }
    }
}

impl HarnessProgressReporter for HarnessProgressAdapter {
    fn report_progress(&self, current_block: u64, total_blocks: u64, message: &str) {
        self.handle.send(TuiEvent::HarnessInitProgress {
            current_block,
            total_blocks,
            message: message.to_string(),
        });
    }

    fn report_init_complete(&self, final_block: u64) {
        self.handle.send(TuiEvent::HarnessInitComplete { final_block });
    }
}
