//! Optimism (Base) block executor using op-revm
//!
//! This binary fetches blocks from an Optimism (Base) RPC and executes all transactions
//! using op-revm with an in-memory database that falls back to RPC for missing state.
//!
//! # Operating Modes
//!
//! - **Executor**: Execute blocks and verify against RPC receipts
//! - **Sequencer**: Execute blocks and submit batches to L1
//! - **Validator**: Derive and validate blocks from L1
//! - **Dual** (default): Run both sequencer and validator concurrently

mod cli;

use std::{io, sync::Arc, time::Duration};

use alloy::providers::{Provider, ProviderBuilder};
use montana_batch_context::{BatchContext, BatchSubmissionMode, L1BatchSourceAdapter};
use blocksource::{BlockProducer, HistoricalRangeProducer, LiveRpcProducer};
use clap::Parser;
use cli::{Args, ProducerMode};
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use database::{CachedDatabase, RPCDatabase};
use eyre::Result;
use montana_batcher::{Address, BatcherConfig};
use montana_brotli::BrotliCompressor;
use montana_cli::MontanaMode;
use montana_derivation_runner::{DerivationConfig, DerivationRunner};
use montana_pipeline::{CompressedBatch, Compressor, NoopExecutor};
use montana_tui::{TuiEvent, TuiHandle, create_tui};
use op_alloy::network::Optimism;
use runner::{ExecutedBlock, Execution, verification};
use tokio::sync::mpsc;
use tracing::info;

/// Buffer capacity for the sequencer mode.
const SEQUENCER_BUFFER_CAPACITY: usize = 256;

/// Default poll interval for derivation in milliseconds.
const DERIVATION_POLL_INTERVAL_MS: u64 = 50;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if !args.no_tui {
        run_with_tui(args)
    } else {
        // Initialize tracing only in non-TUI mode
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive(tracing::level_filters::LevelFilter::INFO.into()),
            )
            .init();
        run_without_tui(args).await
    }
}

/// Run without TUI (existing tracing-based behavior).
async fn run_without_tui(args: Args) -> Result<()> {
    run_with_handle(args, None).await
}

/// Run with TUI interface.
fn run_with_tui(args: Args) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // Create TUI and handle
    let (tui, handle) = create_tui();

    // Create a new tokio runtime for async work
    let rt = tokio::runtime::Runtime::new()?;

    // Clone args for the async task
    let async_args = args;
    let async_handle = handle;

    // Spawn the async work in a separate thread
    std::thread::spawn(move || {
        rt.block_on(async move {
            if let Err(e) = run_with_handle(async_args, Some(async_handle)).await {
                tracing::error!(error = %e, "Montana node error");
            }
        });
    });

    // Run TUI (blocking) in main thread
    let result = tui.run();

    // Cleanup
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    ratatui::restore();

    result.map_err(|e| eyre::eyre!("TUI error: {}", e))
}

/// Core logic that takes optional TUI handle.
async fn run_with_handle(args: Args, tui_handle: Option<TuiHandle>) -> Result<()> {
    let provider = ProviderBuilder::new()
        .disable_recommended_fillers()
        .network::<Optimism>()
        .connect(args.rpc_url.as_str())
        .await?;

    let start_block = match &args.producer {
        ProducerMode::Live { .. } => provider.get_block_number().await?,
        ProducerMode::Historical { start, .. } => *start,
    };

    let db = CachedDatabase::new(RPCDatabase::new(
        provider.clone(),
        start_block.saturating_sub(1),
        tokio::runtime::Handle::current(),
    ));

    match (&args.mode, &args.producer) {
        (MontanaMode::Executor, ProducerMode::Live { poll_interval_ms }) => {
            let producer = LiveRpcProducer::new(
                provider.clone(),
                Duration::from_millis(*poll_interval_ms),
                Some(start_block),
            );
            run_executor(db, producer, provider, tui_handle).await
        }
        (MontanaMode::Executor, ProducerMode::Historical { start, end }) => {
            let producer = HistoricalRangeProducer::new(provider.clone(), *start, *end);
            run_executor(db, producer, provider, tui_handle).await
        }
        (MontanaMode::Sequencer, ProducerMode::Live { poll_interval_ms }) => {
            let producer = LiveRpcProducer::new(
                provider.clone(),
                Duration::from_millis(*poll_interval_ms),
                Some(start_block),
            );
            run_sequencer(db, producer, args.batch_mode, tui_handle).await
        }
        (MontanaMode::Sequencer, ProducerMode::Historical { start, end }) => {
            let producer = HistoricalRangeProducer::new(provider.clone(), *start, *end);
            run_sequencer(db, producer, args.batch_mode, tui_handle).await
        }
        (MontanaMode::Validator, ProducerMode::Live { .. }) => {
            run_validator(args.batch_mode, tui_handle).await
        }
        (MontanaMode::Validator, ProducerMode::Historical { .. }) => {
            run_validator(args.batch_mode, tui_handle).await
        }
        (MontanaMode::Dual, ProducerMode::Live { poll_interval_ms }) => {
            let producer = LiveRpcProducer::new(
                provider.clone(),
                Duration::from_millis(*poll_interval_ms),
                Some(start_block),
            );
            run_dual(db, producer, args.batch_mode, tui_handle).await
        }
        (MontanaMode::Dual, ProducerMode::Historical { start, end }) => {
            let producer = HistoricalRangeProducer::new(provider.clone(), *start, *end);
            run_dual(db, producer, args.batch_mode, tui_handle).await
        }
    }
}

/// Run in executor mode: execute and verify blocks against RPC receipts.
async fn run_executor<DB, P, Prov>(
    db: DB,
    producer: P,
    provider: Prov,
    _tui_handle: Option<TuiHandle>,
) -> Result<()>
where
    DB: database::Database,
    P: BlockProducer + 'static,
    Prov: Provider<Optimism> + Clone + Send + Sync + 'static,
{
    info!("Starting in executor mode");

    // Create a channel for execution output
    let (block_tx, block_rx) = mpsc::channel::<ExecutedBlock>(16);

    let execution = Execution::new(db, producer, block_tx);

    // Spawn a task to verify executed blocks against RPC receipts
    let verifier_handle = tokio::spawn(verification::run_verifier(block_rx, provider));

    // Run execution
    let result = execution.start().await;

    // Wait for verifier to complete
    let _ = verifier_handle.await;

    result
}

/// Run in sequencer mode: execute blocks and submit batches to L1.
async fn run_sequencer<DB, P>(
    db: DB,
    producer: P,
    batch_mode: BatchSubmissionMode,
    tui_handle: Option<TuiHandle>,
) -> Result<()>
where
    DB: database::Database,
    P: BlockProducer + 'static,
{
    info!(batch_mode = %batch_mode, "Starting in sequencer mode");

    // Create the batch context based on the submission mode
    let batch_inbox = Address::repeat_byte(0x42);
    let batch_ctx = BatchContext::new(batch_mode, batch_inbox)
        .await
        .map_err(|e| eyre::eyre!("Failed to create batch context: {}", e))?;

    if let Some(endpoint) = batch_ctx.anvil_endpoint() {
        info!(endpoint = %endpoint, "Anvil L1 endpoint available");
    }

    // Create channel for execution → batcher communication
    let (block_tx, mut block_rx) = mpsc::channel::<ExecutedBlock>(SEQUENCER_BUFFER_CAPACITY);

    // Create execution client with output channel
    let execution = Execution::new(db, producer, block_tx);

    // Create batcher components
    let compressor = BrotliCompressor::default();
    let sink = batch_ctx.sink();
    let config = BatcherConfig::default();

    // Create the batch driver to manage batching decisions
    let mut driver = montana_batcher::BatchDriver::new(config.clone());

    info!(
        buffer_capacity = SEQUENCER_BUFFER_CAPACITY,
        batch_interval = ?config.batch_interval,
        min_batch_size = config.min_batch_size,
        max_blocks_per_batch = config.max_blocks_per_batch,
        "Sequencer initialized, running execution with batch submission"
    );

    // Spawn the batcher task that processes blocks and submits batches
    let batcher_handle = tokio::spawn(async move {
        let mut total_blocks = 0u64;
        let mut total_batches = 0u64;
        let mut last_safe_block = 0u64;

        while let Some(executed) = block_rx.recv().await {
            let block_number = executed.block.header.number;

            // Emit block built event
            if let Some(ref handle) = tui_handle {
                handle.send(TuiEvent::BlockBuilt {
                    number: block_number,
                    tx_count: executed.block.transactions.len(),
                    size_bytes: 0, // TODO: calculate actual size
                    gas_used: executed.block.header.gas_used,
                });
                handle.send(TuiEvent::UnsafeHeadUpdated(block_number));
            }

            // Convert block to L2BlockData for batching
            let l2_data = sequencer::op_block_to_l2_data(&executed.block);
            driver.add_blocks(vec![l2_data]);
            total_blocks += 1;

            tracing::debug!(
                block_number = executed.block.header.number,
                pending_blocks = driver.pending_count(),
                current_size = driver.current_size(),
                "Block added to batcher"
            );

            // Check if we should submit a batch
            while let Some(pending_batch) = driver.build_batch() {
                // Calculate block range for this batch
                let first_block_num = last_safe_block + 1;
                let last_block_num = first_block_num + pending_batch.blocks.len() as u64 - 1;

                info!(
                    batch_number = pending_batch.batch_number,
                    blocks = pending_batch.blocks.len(),
                    uncompressed_size = pending_batch.uncompressed_size,
                    "Building batch for submission"
                );

                // Encode the batch data
                let mut batch_data = Vec::new();
                for block_data in &pending_batch.blocks {
                    for tx in &block_data.transactions {
                        batch_data.extend_from_slice(&tx.0);
                    }
                }

                // Compress the batch
                match compressor.compress(&batch_data) {
                    Ok(compressed) => {
                        let compressed_size = compressed.len();
                        let compression_ratio =
                            1.0 - (compressed_size as f64 / batch_data.len().max(1) as f64);
                        info!(
                            batch_number = pending_batch.batch_number,
                            uncompressed = batch_data.len(),
                            compressed = compressed_size,
                            compression_ratio = format!("{:.1}%", compression_ratio * 100.0),
                            "Batch compressed"
                        );

                        // Submit the batch
                        let compressed_batch = CompressedBatch {
                            batch_number: pending_batch.batch_number,
                            data: compressed,
                        };

                        match sink.submit(compressed_batch).await {
                            Ok(receipt) => {
                                total_batches += 1;
                                last_safe_block = last_block_num;
                                info!(
                                    batch_number = receipt.batch_number,
                                    total_batches, total_blocks, "Batch submitted successfully"
                                );

                                // Emit batch submitted event
                                if let Some(ref handle) = tui_handle {
                                    handle.send(TuiEvent::BatchSubmitted {
                                        batch_number: pending_batch.batch_number,
                                        block_count: pending_batch.blocks.len(),
                                        first_block: first_block_num,
                                        last_block: last_block_num,
                                        uncompressed_size: batch_data.len(),
                                        compressed_size,
                                    });
                                    handle.send(TuiEvent::SafeHeadUpdated(last_block_num));
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    batch_number = pending_batch.batch_number,
                                    error = %e,
                                    "Failed to submit batch"
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            batch_number = pending_batch.batch_number,
                            error = %e,
                            "Failed to compress batch"
                        );
                    }
                }
            }
        }

        info!(
            total_blocks,
            total_batches,
            remaining_pending = driver.pending_count(),
            "Block ingestion completed"
        );

        // Log any remaining blocks
        if driver.pending_count() > 0 {
            info!(
                remaining_blocks = driver.pending_count(),
                "Note: {} blocks remain unbatched (below threshold)",
                driver.pending_count()
            );
        }

        (total_blocks, total_batches)
    });

    // Run execution
    let execution_result = execution.start().await;

    // Wait for batcher to complete and get stats
    match batcher_handle.await {
        Ok((total_blocks, total_batches)) => {
            info!(total_blocks, total_batches, "Sequencer completed successfully");
        }
        Err(e) => {
            tracing::error!(error = %e, "Batcher task failed");
        }
    }

    execution_result
}

/// Run in validator mode: derive and validate blocks from L1.
async fn run_validator(
    batch_mode: BatchSubmissionMode,
    tui_handle: Option<TuiHandle>,
) -> Result<()> {
    info!(batch_mode = %batch_mode, "Starting in validator mode");

    // Create the batch context for reading from L1
    let batch_inbox = Address::repeat_byte(0x42);
    let batch_ctx = BatchContext::new(batch_mode, batch_inbox)
        .await
        .map_err(|e| eyre::eyre!("Failed to create batch context: {}", e))?;

    if let Some(endpoint) = batch_ctx.anvil_endpoint() {
        info!(endpoint = %endpoint, "Anvil L1 endpoint available");
    }

    // Create derivation components
    let source = L1BatchSourceAdapter::new(&batch_ctx);
    let compressor = BrotliCompressor::default();
    let executor = NoopExecutor::new();
    let config = DerivationConfig::builder().poll_interval_ms(DERIVATION_POLL_INTERVAL_MS).build();

    let mut runner = DerivationRunner::new(source, compressor, executor, config);

    info!(
        poll_interval_ms = DERIVATION_POLL_INTERVAL_MS,
        "Validator initialized, running derivation loop"
    );

    // Run the derivation loop
    let mut total_batches = 0u64;
    loop {
        match runner.tick().await {
            Ok(Some(metrics)) => {
                total_batches += 1;
                info!(
                    batches_derived = metrics.batches_derived,
                    blocks_derived = metrics.blocks_derived,
                    first_block = metrics.first_block_in_batch,
                    last_block = metrics.last_block_in_batch,
                    bytes_decompressed = metrics.bytes_decompressed,
                    "Derived batch from L1"
                );

                // Emit batch derived event (for metrics tracking)
                if let Some(ref handle) = tui_handle {
                    handle.send(TuiEvent::BatchDerived {
                        batch_number: metrics.current_batch_number,
                        block_count: metrics.blocks_in_current_batch,
                        first_block: metrics.first_block_in_batch,
                        last_block: metrics.last_block_in_batch,
                    });
                }
            }
            Ok(None) => {
                // No batch available, continue polling
            }
            Err(e) => {
                tracing::error!(error = %e, "Derivation error");
                // Continue trying
            }
        }

        // Exit condition: no more batches expected (in historical mode)
        // For now, just run indefinitely - this would need refinement
        // based on actual use case requirements
        if total_batches > 0 {
            // Small delay before next poll
            tokio::time::sleep(Duration::from_millis(DERIVATION_POLL_INTERVAL_MS)).await;
        }
    }
}

/// Information about a submitted batch, sent from sequencer to derivation task.
struct BatchInfo {
    batch_number: u64,
    block_count: u64,
}

/// Run in dual mode: execute blocks, submit batches, and derive/validate concurrently.
async fn run_dual<DB, P>(
    db: DB,
    producer: P,
    batch_mode: BatchSubmissionMode,
    tui_handle: Option<TuiHandle>,
) -> Result<()>
where
    DB: database::Database,
    P: BlockProducer + 'static,
{
    info!(batch_mode = %batch_mode, "Starting in dual mode (sequencer + validator)");

    // Create the batch context
    let batch_inbox = Address::repeat_byte(0x42);
    let batch_ctx = Arc::new(
        BatchContext::new(batch_mode, batch_inbox)
            .await
            .map_err(|e| eyre::eyre!("Failed to create batch context: {}", e))?,
    );

    if let Some(endpoint) = batch_ctx.anvil_endpoint() {
        info!(endpoint = %endpoint, "Anvil L1 endpoint available");
    }

    // Create channel for execution → batcher communication
    let (block_tx, mut block_rx) = mpsc::channel::<ExecutedBlock>(SEQUENCER_BUFFER_CAPACITY);

    // Create channel for batcher → derivation communication (batch block counts)
    let (batch_info_tx, mut batch_info_rx) = mpsc::channel::<BatchInfo>(SEQUENCER_BUFFER_CAPACITY);

    // Create execution client with output channel
    let execution = Execution::new(db, producer, block_tx);

    // Create batcher components
    let compressor = BrotliCompressor::default();
    let sink = batch_ctx.sink();
    let batcher_config = BatcherConfig::default();

    // Create the batch driver to manage batching decisions
    let mut driver = montana_batcher::BatchDriver::new(batcher_config.clone());

    info!(
        buffer_capacity = SEQUENCER_BUFFER_CAPACITY,
        batch_interval = ?batcher_config.batch_interval,
        min_batch_size = batcher_config.min_batch_size,
        max_blocks_per_batch = batcher_config.max_blocks_per_batch,
        "Dual mode initialized"
    );

    // Spawn the batcher task (sequencer side)
    let batcher_compressor = compressor.clone();
    let batcher_tui_handle = tui_handle.clone();
    let batcher_handle = tokio::spawn(async move {
        let mut total_blocks = 0u64;
        let mut total_batches = 0u64;
        let mut last_safe_block = 0u64;

        while let Some(executed) = block_rx.recv().await {
            let block_number = executed.block.header.number;

            // Emit block built event
            if let Some(ref handle) = batcher_tui_handle {
                handle.send(TuiEvent::BlockBuilt {
                    number: block_number,
                    tx_count: executed.block.transactions.len(),
                    size_bytes: 0, // TODO: calculate actual size
                    gas_used: executed.block.header.gas_used,
                });
                handle.send(TuiEvent::UnsafeHeadUpdated(block_number));
            }

            let l2_data = sequencer::op_block_to_l2_data(&executed.block);
            driver.add_blocks(vec![l2_data]);
            total_blocks += 1;

            tracing::debug!(
                block_number = executed.block.header.number,
                pending_blocks = driver.pending_count(),
                "Block added to batcher"
            );

            while let Some(pending_batch) = driver.build_batch() {
                // Calculate block range for this batch
                let first_block_num = last_safe_block + 1;
                let last_block_num = first_block_num + pending_batch.blocks.len() as u64 - 1;
                let block_count = pending_batch.blocks.len() as u64;

                info!(
                    batch_number = pending_batch.batch_number,
                    blocks = pending_batch.blocks.len(),
                    "Building batch for submission"
                );

                let mut batch_data = Vec::new();
                for block_data in &pending_batch.blocks {
                    for tx in &block_data.transactions {
                        batch_data.extend_from_slice(&tx.0);
                    }
                }

                match batcher_compressor.compress(&batch_data) {
                    Ok(compressed) => {
                        let compressed_batch = CompressedBatch {
                            batch_number: pending_batch.batch_number,
                            data: compressed.clone(),
                        };

                        match sink.submit(compressed_batch).await {
                            Ok(receipt) => {
                                total_batches += 1;
                                last_safe_block = last_block_num;
                                info!(
                                    batch_number = receipt.batch_number,
                                    total_batches, "Batch submitted"
                                );

                                // Send batch info to derivation task
                                let _ = batch_info_tx
                                    .send(BatchInfo {
                                        batch_number: pending_batch.batch_number,
                                        block_count,
                                    })
                                    .await;

                                // Emit batch submitted event
                                if let Some(ref handle) = batcher_tui_handle {
                                    handle.send(TuiEvent::BatchSubmitted {
                                        batch_number: pending_batch.batch_number,
                                        block_count: pending_batch.blocks.len(),
                                        first_block: first_block_num,
                                        last_block: last_block_num,
                                        uncompressed_size: batch_data.len(),
                                        compressed_size: compressed.len(),
                                    });
                                    handle.send(TuiEvent::SafeHeadUpdated(last_block_num));
                                }
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "Failed to submit batch");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to compress batch");
                    }
                }
            }
        }

        info!(total_blocks, total_batches, "Sequencer side completed");
        (total_blocks, total_batches)
    });

    // Spawn the derivation task (validator side)
    let derivation_batch_ctx = Arc::clone(&batch_ctx);
    let derivation_compressor = compressor;
    let derivation_tui_handle = tui_handle;
    let derivation_handle = tokio::spawn(async move {
        let source = L1BatchSourceAdapter::new(&derivation_batch_ctx);
        let executor = NoopExecutor::new();
        let config =
            DerivationConfig::builder().poll_interval_ms(DERIVATION_POLL_INTERVAL_MS).build();

        let mut runner = DerivationRunner::new(source, derivation_compressor, executor, config);
        let mut _total_derived = 0u64;

        loop {
            // Check for batch info from sequencer (non-blocking)
            while let Ok(batch_info) = batch_info_rx.try_recv() {
                runner.record_batch_block_count(batch_info.batch_number, batch_info.block_count);
            }

            match runner.tick().await {
                Ok(Some(metrics)) => {
                    _total_derived += 1;
                    info!(
                        batches_derived = metrics.batches_derived,
                        blocks_derived = metrics.blocks_derived,
                        first_block = metrics.first_block_in_batch,
                        last_block = metrics.last_block_in_batch,
                        bytes_decompressed = metrics.bytes_decompressed,
                        "Derived batch from L1"
                    );

                    // Emit batch derived event (for metrics tracking)
                    if let Some(ref handle) = derivation_tui_handle {
                        handle.send(TuiEvent::BatchDerived {
                            batch_number: metrics.current_batch_number,
                            block_count: metrics.blocks_in_current_batch,
                            first_block: metrics.first_block_in_batch,
                            last_block: metrics.last_block_in_batch,
                        });
                    }
                }
                Ok(None) => {
                    // No batch available
                }
                Err(e) => {
                    tracing::error!(error = %e, "Derivation error");
                }
            }

            // Small delay between polls
            tokio::time::sleep(Duration::from_millis(DERIVATION_POLL_INTERVAL_MS)).await;
        }

        #[allow(unreachable_code)]
        _total_derived
    });

    // Run execution
    let execution_result = execution.start().await;

    // Wait for batcher to complete
    match batcher_handle.await {
        Ok((total_blocks, total_batches)) => {
            info!(total_blocks, total_batches, "Batcher completed");
        }
        Err(e) => {
            tracing::error!(error = %e, "Batcher task failed");
        }
    }

    // Abort derivation (it runs indefinitely)
    derivation_handle.abort();

    info!("Dual mode completed");
    execution_result
}
