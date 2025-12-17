//! Optimism (Base) block executor using op-revm
//!
//! This binary fetches blocks from an Optimism (Base) RPC and executes all transactions
//! using op-revm with an in-memory database that falls back to RPC for missing state.
//!
//! # Operating Modes
//!
//! - **Executor**: Execute blocks and verify against RPC receipts
//! - **Sequencer** (default): Execute blocks and submit batches to L1
//! - **Validator**: Derive and validate blocks from L1 (unimplemented)

mod cli;

use std::time::Duration;

use clap::Parser;
use cli::{Args, ProducerMode};
use eyre::Result;
use montana_batcher::{
    Address, BatchContext, BatchSubmissionMode, BatcherConfig, BatchSink,
};
use montana_brotli::BrotliCompressor;
use montana_cli::MontanaMode;
use montana_pipeline::{CompressedBatch, Compressor};
use runner::{Execution, ProducerMode as RunnerMode};
use sequencer::OpBlock;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

/// Buffer capacity for the sequencer mode.
const SEQUENCER_BUFFER_CAPACITY: usize = 256;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::level_filters::LevelFilter::INFO.into()),
        )
        .init();

    let args = Args::parse();

    let producer_mode = match args.producer {
        ProducerMode::Live { poll_interval_ms, start_block } => {
            RunnerMode::Live { poll_interval: Duration::from_millis(poll_interval_ms), start_block }
        }
        ProducerMode::Historical { start, end } => RunnerMode::Historical { start, end },
    };

    match args.mode {
        MontanaMode::Executor => run_executor(args.rpc_url, producer_mode).await,
        MontanaMode::Sequencer => run_sequencer(args.rpc_url, producer_mode, args.batch_mode).await,
        MontanaMode::Validator => {
            unimplemented!("Validator mode is not yet implemented")
        }
    }
}

/// Run in executor mode: execute and verify blocks only.
async fn run_executor(rpc_url: String, mode: RunnerMode) -> Result<()> {
    info!("Starting in executor mode");
    let execution = Execution::new(rpc_url, mode);
    execution.start().await
}

/// Run in sequencer mode: execute blocks and submit batches to L1.
async fn run_sequencer(
    rpc_url: String,
    mode: RunnerMode,
    batch_mode: BatchSubmissionMode,
) -> Result<()> {
    info!(batch_mode = %batch_mode, "Starting in sequencer mode");

    // Create the batch context based on the submission mode
    // Use the default Base batch inbox address
    let batch_inbox = Address::repeat_byte(0x42);
    let batch_ctx = BatchContext::new(batch_mode, batch_inbox)
        .await
        .map_err(|e| eyre::eyre!("Failed to create batch context: {}", e))?;

    if let Some(endpoint) = batch_ctx.anvil_endpoint() {
        info!(endpoint = %endpoint, "Anvil L1 endpoint available");
    }

    // Create channel for execution → batcher communication
    let (block_tx, mut block_rx) = mpsc::channel::<OpBlock>(SEQUENCER_BUFFER_CAPACITY);

    // Create execution client with output channel
    let execution = Execution::new(rpc_url, mode).with_output(block_tx);

    // Create batcher components
    let compressor = BrotliCompressor::default();
    let sink: Arc<dyn BatchSink> = batch_ctx.sink();
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

        while let Some(block) = block_rx.recv().await {
            // Convert block to L2BlockData for batching
            let l2_data = sequencer::op_block_to_l2_data(&block);
            driver.add_blocks(vec![l2_data]);
            total_blocks += 1;

            tracing::debug!(
                block_number = block.header.number,
                pending_blocks = driver.pending_count(),
                current_size = driver.current_size(),
                "Block added to batcher"
            );

            // Check if we should submit a batch
            while let Some(pending_batch) = driver.build_batch() {
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
                        let compression_ratio =
                            1.0 - (compressed.len() as f64 / batch_data.len().max(1) as f64);
                        info!(
                            batch_number = pending_batch.batch_number,
                            uncompressed = batch_data.len(),
                            compressed = compressed.len(),
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
                                info!(
                                    batch_number = receipt.batch_number,
                                    total_batches,
                                    total_blocks,
                                    "Batch submitted successfully"
                                );
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

        // Submit any remaining blocks as a final batch
        if driver.pending_count() > 0 {
            info!(
                pending_blocks = driver.pending_count(),
                "Submitting final batch with remaining blocks"
            );

            // Force submit remaining blocks by checking if there are any pending
            // The driver may not submit if thresholds aren't met, so we drain manually
            let remaining_count = driver.pending_count();
            if remaining_count > 0 {
                // Manually trigger batch build for remaining blocks
                // by lowering the threshold check - for now just log
                info!(
                    remaining_blocks = remaining_count,
                    "Note: {} blocks remain unbatched (below threshold)",
                    remaining_count
                );
            }
        }

        (total_blocks, total_batches)
    });

    // Run execution
    let execution_result = execution.start().await;

    // Wait for batcher to complete and get stats
    match batcher_handle.await {
        Ok((total_blocks, total_batches)) => {
            info!(
                total_blocks,
                total_batches,
                "Sequencer completed successfully"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "Batcher task failed");
        }
    }

    execution_result
}
