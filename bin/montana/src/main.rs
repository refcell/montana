//! Montana Base stack Node binary.
//!
//! This is a thin binary that wires up components from library crates.
//! All business logic lives in the library crates; this file only handles
//! configuration parsing, component construction, and runtime setup.

use std::{io, sync::Arc};

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
use montana_cli::{MontanaCli, MontanaMode, ProducerMode};
use montana_node::{Node, NodeBuilder, NodeConfig, NodeRole, SyncConfig, SyncStage};
use montana_pipeline::NoopExecutor;
use montana_roles::{Sequencer, Validator};
use montana_tui::{TuiObserver, create_tui};
use op_alloy::network::Optimism;

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
        init_tracing(&cli.log_level);
        run_headless(cli).await
    } else {
        run_with_tui(cli)
    }
}

fn init_tracing(log_level: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();
}

async fn run_headless(cli: MontanaCli) -> Result<()> {
    match &cli.producer {
        ProducerMode::Live { .. } => {
            let mut node = build_node_live(cli, None).await?;
            node.run().await
        }
        ProducerMode::Historical { .. } => {
            let mut node = build_node_historical(cli, None).await?;
            node.run().await
        }
    }
}

fn run_with_tui(cli: MontanaCli) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let (tui, handle) = create_tui();
    let tui_observer = Arc::new(TuiObserver::new(handle));

    let rt = tokio::runtime::Runtime::new()?;
    let is_live = matches!(cli.producer, ProducerMode::Live { .. });

    std::thread::spawn(move || {
        rt.block_on(async move {
            let result = if is_live {
                match build_node_live(cli, Some(tui_observer)).await {
                    Ok(mut node) => node.run().await,
                    Err(e) => Err(e),
                }
            } else {
                match build_node_historical(cli, Some(tui_observer)).await {
                    Ok(mut node) => node.run().await,
                    Err(e) => Err(e),
                }
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

async fn build_node_live(
    cli: MontanaCli,
    tui_observer: Option<Arc<TuiObserver>>,
) -> Result<Node<blocksource::LiveRpcProducer<impl Provider<Optimism> + Clone + 'static>>> {
    use std::time::Duration;

    use blocksource::LiveRpcProducer;

    // Build provider
    let provider = ProviderBuilder::new()
        .disable_recommended_fillers()
        .network::<Optimism>()
        .connect(&cli.rpc_url)
        .await?;

    // Get start block
    let start_block = provider.get_block_number().await?;

    // Build block producer
    let ProducerMode::Live { poll_interval_ms } = &cli.producer else {
        unreachable!("build_node_live called with non-live producer");
    };

    let block_producer =
        LiveRpcProducer::new(provider, Duration::from_millis(*poll_interval_ms), Some(start_block));

    build_node_common(cli, block_producer, tui_observer).await
}

async fn build_node_historical(
    cli: MontanaCli,
    tui_observer: Option<Arc<TuiObserver>>,
) -> Result<Node<blocksource::HistoricalRangeProducer<impl Provider<Optimism> + Clone + 'static>>> {
    use blocksource::HistoricalRangeProducer;

    // Build provider
    let provider = ProviderBuilder::new()
        .disable_recommended_fillers()
        .network::<Optimism>()
        .connect(&cli.rpc_url)
        .await?;

    // Build block producer
    let ProducerMode::Historical { start, end } = &cli.producer else {
        unreachable!("build_node_historical called with non-historical producer");
    };

    let block_producer = HistoricalRangeProducer::new(provider, *start, *end);

    build_node_common(cli, block_producer, tui_observer).await
}

async fn build_node_common<P: blocksource::BlockProducer + 'static>(
    cli: MontanaCli,
    block_producer: P,
    tui_observer: Option<Arc<TuiObserver>>,
) -> Result<Node<P>> {
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

    // Add sync stage unless skipped
    if !cli.skip_sync {
        let sync_stage = SyncStage::new(block_producer, sync_config);
        builder = builder.with_sync_stage(sync_stage);
    }

    // Add sequencer if role requires it
    if config.role.runs_sequencer() {
        let (_block_tx, block_rx) = tokio::sync::mpsc::channel(256);

        let batch_driver = BatchDriver::new(BatcherConfig::default());
        let compressor = BrotliCompressor::default();

        // Wrap the batch context sink in an adapter to bridge the two BatchSink traits
        let sink_adapter = BatchSinkAdapter { inner: batch_ctx.sink() };

        let sequencer = Sequencer::new(
            sink_adapter,
            compressor,
            batch_driver,
            cli.checkpoint_path.clone(),
            block_rx,
        )?;
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
