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

use std::{sync::Arc, time::Duration};

use alloy::{
    eips::BlockId,
    network::ReceiptResponse,
    providers::{Provider, ProviderBuilder},
};
use blocksource::{BlockProducer, HistoricalRangeProducer, LiveRpcProducer};
use clap::Parser;
use cli::{Args, ProducerMode};
use database::{CachedDatabase, RPCDatabase};
use eyre::Result;
use montana_batcher::{Address, BatchContext, BatchSink, BatchSubmissionMode, BatcherConfig};
use montana_brotli::BrotliCompressor;
use montana_cli::MontanaMode;
use montana_pipeline::{CompressedBatch, Compressor};
use op_alloy::network::Optimism;
use runner::{ExecutedBlock, Execution};
use tokio::sync::mpsc;
use tracing::{info, warn};

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
            run_executor(db, producer, provider).await
        }
        (MontanaMode::Executor, ProducerMode::Historical { start, end }) => {
            let producer = HistoricalRangeProducer::new(provider.clone(), *start, *end);
            run_executor(db, producer, provider).await
        }
        (MontanaMode::Sequencer, ProducerMode::Live { poll_interval_ms }) => {
            let producer = LiveRpcProducer::new(
                provider.clone(),
                Duration::from_millis(*poll_interval_ms),
                Some(start_block),
            );
            run_sequencer(db, producer, args.batch_mode).await
        }
        (MontanaMode::Sequencer, ProducerMode::Historical { start, end }) => {
            let producer = HistoricalRangeProducer::new(provider.clone(), *start, *end);
            run_sequencer(db, producer, args.batch_mode).await
        }
        (MontanaMode::Validator, _) => {
            unimplemented!("Validator mode is not yet implemented")
        }
    }
}

/// Run in executor mode: execute and verify blocks against RPC receipts.
async fn run_executor<DB, P, Prov>(db: DB, producer: P, provider: Prov) -> Result<()>
where
    DB: database::Database,
    P: BlockProducer + 'static,
    Prov: Provider<Optimism> + Clone + Send + Sync + 'static,
{
    info!("Starting in executor mode");

    // Create a channel for execution output
    let (block_tx, mut block_rx) = mpsc::channel::<ExecutedBlock>(16);

    let execution = Execution::new(db, producer, block_tx);

    // Spawn a task to verify executed blocks against RPC receipts
    let verifier_handle = tokio::spawn(async move {
        while let Some(executed) = block_rx.recv().await {
            let block_number = executed.result.block_number;

            // Fetch receipts from RPC for verification
            let receipts = match provider.get_block_receipts(BlockId::number(block_number)).await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    warn!(block = block_number, "No receipts found for block");
                    continue;
                }
                Err(e) => {
                    warn!(block = block_number, error = %e, "Failed to fetch receipts");
                    continue;
                }
            };

            // Verify execution results against receipts
            let mut verified = 0;
            let mut mismatched = 0;

            for (idx, (tx_result, receipt)) in
                executed.result.tx_results.iter().zip(receipts.iter()).enumerate()
            {
                let receipt_gas = receipt.gas_used();
                let receipt_success = receipt.status();

                if tx_result.gas_used == receipt_gas && tx_result.success == receipt_success {
                    verified += 1;
                } else {
                    mismatched += 1;
                    warn!(
                        "Block {} tx {}: gas mismatch (exec={}, receipt={}) or status mismatch (exec={}, receipt={})",
                        block_number,
                        idx,
                        tx_result.gas_used,
                        receipt_gas,
                        tx_result.success,
                        receipt_success
                    );
                }
            }

            if mismatched == 0 {
                info!(
                    "Block {} verification PASSED: {}/{} transactions verified",
                    block_number,
                    verified,
                    executed.result.tx_results.len()
                );
            } else {
                warn!(
                    "Block {} verification FAILED: {} mismatched, {} verified out of {} transactions",
                    block_number,
                    mismatched,
                    verified,
                    executed.result.tx_results.len()
                );
            }
        }
    });

    // Run execution
    let result = execution.start().await;

    // Wait for verifier to complete
    let _ = verifier_handle.await;

    result
}

/// Run in sequencer mode: execute blocks and submit batches to L1.
async fn run_sequencer<DB, P>(db: DB, producer: P, batch_mode: BatchSubmissionMode) -> Result<()>
where
    DB: database::Database,
    P: BlockProducer + 'static,
{
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
    let (block_tx, mut block_rx) = mpsc::channel::<ExecutedBlock>(SEQUENCER_BUFFER_CAPACITY);

    // Create execution client with output channel
    let execution = Execution::new(db, producer, block_tx);

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

        while let Some(executed) = block_rx.recv().await {
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
                                    total_batches, total_blocks, "Batch submitted successfully"
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
                    "Note: {} blocks remain unbatched (below threshold)", remaining_count
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
            info!(total_blocks, total_batches, "Sequencer completed successfully");
        }
        Err(e) => {
            tracing::error!(error = %e, "Batcher task failed");
        }
    }

    execution_result
}
