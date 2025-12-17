//! Montana Base stack Node binary.
//!
//! This is a thin binary that wires up components from library crates.
//! All business logic lives in the library crates; this file only handles
//! configuration parsing, component construction, and runtime setup.

use std::{io, sync::Arc, time::Duration};

use alloy::providers::{Provider, ProviderBuilder};
use async_trait::async_trait;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use eyre::Result;
use montana_batch_context::BatchContext;
use montana_batcher::{Address, BatchDriver, BatcherConfig};
use montana_brotli::BrotliCompressor;
use montana_cli::{MontanaCli, MontanaMode, init_tracing_with_level};
use montana_node::{Node, NodeBuilder, NodeConfig, NodeRole, SyncConfig, SyncStage};
use montana_pipeline::{Bytes, L2BlockData, NoopExecutor};
use montana_roles::{ExecutionCallback, Sequencer, Validator};
use montana_tui::{TuiEvent, TuiHandle, TuiObserver, create_tui};
use op_alloy::network::Optimism;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = MontanaCli::parse();

    // Check for unsupported executor mode
    if cli.mode == MontanaMode::Executor {
        eprintln!("Error: Executor mode is not yet supported in the new node architecture.");
        eprintln!("Please use 'sequencer', 'validator', or 'dual' mode instead.");
        std::process::exit(1);
    }

    if cli.headless {
        init_tracing_with_level(&cli.log_level);
        run_headless(cli).await
    } else {
        run_with_tui(cli)
    }
}

async fn run_headless(cli: MontanaCli) -> Result<()> {
    let mut node = build_node(cli, None, None).await?;
    node.run().await
}

fn run_with_tui(cli: MontanaCli) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let (tui, handle) = create_tui();

    // Send mode info to TUI immediately after creation
    let node_role = match cli.mode {
        MontanaMode::Sequencer => "Sequencer",
        MontanaMode::Validator => "Validator",
        MontanaMode::Dual => "Dual",
        MontanaMode::Executor => "Executor",
    }
    .to_string();

    // Build range description for display
    let range_display = cli.range_description();

    handle.send(montana_tui::TuiEvent::ModeInfo {
        node_role,
        producer_mode: range_display,
        skip_sync: cli.skip_sync,
        historical_range: cli.start.zip(cli.end),
    });

    // Clone handle for block feeder before creating observer
    let block_feeder_handle = handle.clone();
    let tui_observer = Arc::new(TuiObserver::new(handle));

    let rt = tokio::runtime::Runtime::new()?;

    std::thread::spawn(move || {
        rt.block_on(async move {
            let result = match build_node(cli, Some(tui_observer), Some(block_feeder_handle)).await
            {
                Ok(mut node) => node.run().await,
                Err(e) => Err(e),
            };

            if let Err(e) = result {
                tracing::error!(error = %e, "Node error");
            }
        });
    });

    let result = tui.run();

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    ratatui::restore();

    result.map_err(|e| eyre::eyre!("TUI error: {}", e))
}

/// Build the node with the appropriate block producer based on CLI args.
async fn build_node(
    cli: MontanaCli,
    tui_observer: Option<Arc<TuiObserver>>,
    tui_handle: Option<TuiHandle>,
) -> Result<Node<BlockProducerWrapper>> {
    // Build provider
    let provider = ProviderBuilder::new()
        .disable_recommended_fillers()
        .network::<Optimism>()
        .connect(&cli.rpc_url)
        .await?;

    // Determine start block
    let start_block = if let Some(start) = cli.start {
        start
    } else {
        // If no start specified, get current block for live following
        provider.get_block_number().await?
    };

    // Create block producer based on whether we have an end block
    let block_producer = if let Some(end_block) = cli.end {
        // Bounded range - process specific blocks
        let historical =
            blocksource::HistoricalRangeProducer::new(provider.clone(), start_block, end_block);
        BlockProducerWrapper::new(historical, "historical")
    } else {
        // Unbounded - follow chain tip
        let live = blocksource::LiveRpcProducer::new(
            provider.clone(),
            Duration::from_millis(cli.poll_interval_ms),
            Some(start_block),
        );
        BlockProducerWrapper::new(live, "live")
    };

    build_node_common(cli, block_producer, tui_observer, tui_handle, provider).await
}

async fn build_node_common<P: Provider<Optimism> + Clone + 'static>(
    cli: MontanaCli,
    block_producer: BlockProducerWrapper,
    tui_observer: Option<Arc<TuiObserver>>,
    tui_handle: Option<TuiHandle>,
    _provider: P,
) -> Result<Node<BlockProducerWrapper>> {
    // Build node config
    let config = NodeConfig {
        role: mode_to_node_role(&cli.mode),
        checkpoint_path: cli.checkpoint_path.clone(),
        checkpoint_interval_secs: cli.checkpoint_interval,
        skip_sync: cli.skip_sync,
    };

    // Build sync config
    let sync_config = SyncConfig { sync_threshold: cli.sync_threshold, ..Default::default() };

    // Use batch mode from CLI directly (it's already BatchSubmissionMode)
    let batch_mode = cli.batch_mode;

    // Build batch context
    let batch_inbox = Address::repeat_byte(0x42);
    let batch_ctx = Arc::new(
        BatchContext::new(batch_mode, batch_inbox)
            .await
            .map_err(|e| eyre::eyre!("Failed to create batch context: {}", e))?,
    );

    // Create node builder
    let mut builder = NodeBuilder::new().with_config(config.clone());

    // Create unbounded channel for blocks to decouple block fetching from execution
    // This allows the block feeder to continuously fetch blocks without blocking on executor pace
    let (block_tx, block_rx) = mpsc::unbounded_channel::<L2BlockData>();

    // Add sync stage OR spawn block feeder based on skip_sync
    if cli.skip_sync {
        // When skipping sync, we need to feed blocks directly to the sequencer
        // Spawn a DEDICATED THREAD with its own runtime for the block feeder.
        // This ensures true parallelism - the block feeder runs completely independently
        // of the node's main loop, preventing any blocking or starvation issues.
        let start = cli.start.unwrap_or(0);
        let end = cli.end;
        let poll_ms = cli.poll_interval_ms;
        let feeder_tui_handle = tui_handle.clone();
        let rpc_url = cli.rpc_url.clone();

        std::thread::spawn(move || {
            // Create a dedicated runtime for the block feeder
            let feeder_rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create block feeder runtime");

            feeder_rt.block_on(async move {
                // Create a new provider for this thread
                let feeder_provider = match ProviderBuilder::new()
                    .disable_recommended_fillers()
                    .network::<Optimism>()
                    .connect(&rpc_url)
                    .await
                {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(error = %e, "Block feeder failed to connect to RPC");
                        return;
                    }
                };

                if let Err(e) = run_block_feeder(
                    feeder_provider,
                    start,
                    end,
                    poll_ms,
                    block_tx,
                    feeder_tui_handle,
                )
                .await
                {
                    tracing::error!(error = %e, "Block feeder error");
                }
            });
        });
    } else {
        // Normal sync mode - add sync stage
        let sync_stage = SyncStage::new(block_producer, sync_config);
        builder = builder.with_sync_stage(sync_stage);
    }

    // Add sequencer if role requires it
    if config.role.runs_sequencer() {
        let batch_driver = BatchDriver::new(BatcherConfig::default());
        let compressor = BrotliCompressor::default();

        // Wrap the batch context sink in an adapter to bridge the two BatchSink traits
        let sink_adapter = BatchSinkAdapter { inner: batch_ctx.sink() };

        let mut sequencer = Sequencer::new(
            sink_adapter,
            compressor,
            batch_driver,
            cli.checkpoint_path.clone(),
            block_rx,
        )?;

        // Wire up execution callback for TUI visibility if available
        if let Some(ref handle) = tui_handle {
            let callback = Arc::new(TuiExecutionCallback { handle: handle.clone() });
            sequencer = sequencer.with_execution_callback(callback);
        }

        builder = builder.with_sequencer(sequencer);
    }

    // Add validator if role requires it
    if config.role.runs_validator() {
        // Create a batch source adapter that wraps the batch context source
        // We need to create an owned adapter, not a reference-based one
        let source = BatchSourceAdapter { inner: Arc::clone(&batch_ctx) };
        let compressor = BrotliCompressor::default();
        let executor = NoopExecutor::new();
        let validator = Validator::new(source, compressor, executor, cli.checkpoint_path.clone())?;
        builder = builder.with_validator(validator);
    }

    // Add TUI observer if present
    if let Some(observer) = tui_observer {
        builder = builder.with_observer(observer);
    }

    builder.build()
}

/// Fetches blocks and sends them to the sequencer via channel.
/// This is used when sync is skipped.
///
/// Uses an unbounded channel to decouple block fetching from execution - the feeder
/// continuously pulls blocks from the RPC while the sequencer processes them at its
/// own pace. The TUI shows both blocks fetched (backlog) and blocks executed.
async fn run_block_feeder<P: Provider<Optimism> + Clone>(
    provider: P,
    start: u64,
    end: Option<u64>,
    poll_interval_ms: u64,
    tx: mpsc::UnboundedSender<L2BlockData>,
    tui_handle: Option<TuiHandle>,
) -> Result<()> {
    use alloy::eips::eip2718::Encodable2718;

    let mut current = start;
    let mut blocks_fetched: u64 = 0;

    loop {
        // Get chain tip
        let chain_tip = provider.get_block_number().await?;

        // Determine end block for this iteration
        let target = end.unwrap_or(chain_tip).min(chain_tip);

        // Fetch blocks up to target
        while current <= target {
            // Fetch block with full transactions
            let block = provider
                .get_block_by_number(current.into())
                .full()
                .await?
                .ok_or_else(|| eyre::eyre!("Block {} not found", current))?;

            let tx_count = block.transactions.len();
            let gas_used = block.header.gas_used;

            // Convert to L2BlockData
            let block_data = L2BlockData {
                block_number: current,
                timestamp: block.header.timestamp,
                transactions: block
                    .transactions
                    .txns()
                    .map(|rpc_tx| {
                        // Get the inner OpTxEnvelope and encode it
                        let envelope = rpc_tx.inner.inner.inner();
                        Bytes::from(envelope.encoded_2718())
                    })
                    .collect(),
            };

            // Calculate approximate block size from transaction data
            let size_bytes: usize = block_data.transactions.iter().map(|tx| tx.len()).sum();

            // Emit TUI events for the block fetched
            if let Some(ref handle) = tui_handle {
                handle.send(TuiEvent::BlockBuilt {
                    number: current,
                    tx_count,
                    size_bytes,
                    gas_used,
                });
                handle.send(TuiEvent::UnsafeHeadUpdated(current));
            }

            // Send to sequencer (unbounded - never blocks)
            if tx.send(block_data).is_err() {
                tracing::info!("Block receiver closed, stopping feeder");
                return Ok(());
            }

            blocks_fetched += 1;

            // Emit backlog update event for TUI
            if let Some(ref handle) = tui_handle {
                handle
                    .send(TuiEvent::BacklogUpdated { blocks_fetched, last_fetched_block: current });
            }

            tracing::debug!(block = current, blocks_fetched, "Fed block to sequencer");
            current += 1;

            // Yield to allow other tasks to run (e.g., TUI event processing)
            tokio::task::yield_now().await;
        }

        // If we have a bounded range and reached the end, we're done
        if let Some(end_block) = end
            && current > end_block
        {
            tracing::info!("Completed block range {}-{}", start, end_block);
            break;
        }

        // Wait before polling for new blocks
        tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
    }

    Ok(())
}

/// Convert MontanaMode to NodeRole.
///
/// # Panics
/// Panics if called with Executor mode, which is not supported in the new architecture.
fn mode_to_node_role(mode: &MontanaMode) -> NodeRole {
    match mode {
        MontanaMode::Executor => {
            // Executor mode is not supported in the new architecture
            // This should never be reached due to the check in main()
            panic!("Executor mode is not supported")
        }
        MontanaMode::Sequencer => NodeRole::Sequencer,
        MontanaMode::Validator => NodeRole::Validator,
        MontanaMode::Dual => NodeRole::Dual,
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
    fn new<P: blocksource::BlockProducer + 'static>(
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
struct BatchSinkAdapter {
    inner: Arc<Box<dyn montana_batch_context::BatchSink>>,
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
struct BatchSourceAdapter {
    inner: Arc<BatchContext>,
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
struct TuiExecutionCallback {
    handle: TuiHandle,
}

impl ExecutionCallback for TuiExecutionCallback {
    fn on_block_executed(&self, block_number: u64, execution_time_ms: u64) {
        self.handle.send(TuiEvent::BlockExecuted { block_number, execution_time_ms });
    }
}
