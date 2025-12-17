//! Optimism (Base) block executor using op-revm
//!
//! This binary fetches blocks from an Optimism (Base) RPC and executes all transactions
//! using op-revm with an in-memory database that falls back to RPC for missing state.
//!
//! # Operating Modes
//!
//! - **Executor** (default): Execute blocks and verify against RPC receipts
//! - **Sequencer**: Execute blocks and submit batches to L1
//! - **Validator**: Derive and validate blocks from L1 (unimplemented)

mod cli;

use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use clap::Parser;
use cli::{Args, ProducerMode};
use eyre::Result;
use montana_batcher::BatcherConfig;
use montana_brotli::BrotliCompressor;
use montana_cli::MontanaMode;
use montana_pipeline::{BatchSink, CompressedBatch, SinkError, SubmissionReceipt};
use runner::{Execution, ProducerMode as RunnerMode};
use sequencer::{ExecutedBlockBuffer, OpBlock};
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
        MontanaMode::Sequencer => run_sequencer(args.rpc_url, producer_mode).await,
        MontanaMode::Validator => {
            unimplemented!("Validator mode is not yet implemented")
        }
    }
}

/// Run in executor mode (default): execute and verify blocks only.
async fn run_executor(rpc_url: String, mode: RunnerMode) -> Result<()> {
    info!("Starting in executor mode");
    let execution = Execution::new(rpc_url, mode);
    execution.start().await
}

/// Run in sequencer mode: execute blocks and submit batches to L1.
async fn run_sequencer(rpc_url: String, mode: RunnerMode) -> Result<()> {
    info!("Starting in sequencer mode");

    // Create the buffer that bridges execution to batching
    let buffer = Arc::new(ExecutedBlockBuffer::new(SEQUENCER_BUFFER_CAPACITY));

    // Create channel for execution → buffer communication
    let (block_tx, mut block_rx) = mpsc::channel::<OpBlock>(SEQUENCER_BUFFER_CAPACITY);

    // Spawn task to ingest blocks into the buffer
    let buffer_clone = buffer.clone();
    let ingestion_handle = tokio::spawn(async move {
        while let Some(block) = block_rx.recv().await {
            if let Err(e) = buffer_clone.push(block).await {
                tracing::error!("Failed to push block to buffer: {e}");
            }
        }
        info!("Block ingestion task completed");
    });

    // Create execution client with output channel
    let execution = Execution::new(rpc_url, mode).with_output(block_tx);

    // Create batcher components
    // Note: The batcher service would normally consume the buffer, but since we're
    // sharing it with the ingestion task, we create a wrapper source here.
    // For now, we just demonstrate the wiring - full batcher integration is TODO.
    let _compressor = BrotliCompressor::default();
    let _sink = NoopBatchSink; // TODO: Replace with real L1 sink
    let _config = BatcherConfig::default();

    // TODO: Integrate batcher service with shared buffer
    // The batcher needs to periodically call buffer.pending_blocks() to drain
    // blocks and submit batches. This requires either:
    // 1. A channel-based approach where buffer sends to batcher
    // 2. A shared Arc<Mutex<ExecutedBlockBuffer>> with periodic polling
    // For now, we just run execution and the buffer fills up.

    info!(
        buffer_capacity = SEQUENCER_BUFFER_CAPACITY,
        "Sequencer buffer created, running execution"
    );

    // Run execution
    let execution_result = execution.start().await;

    // Wait for ingestion to complete
    ingestion_handle.await?;

    // Log buffer stats
    info!(buffered_blocks = buffer.len().await, "Sequencer completed");

    execution_result
}

/// A no-op batch sink for development/testing.
///
/// This sink accepts batches but doesn't actually submit them anywhere.
/// Replace with a real L1 sink implementation for production.
struct NoopBatchSink;

#[async_trait]
impl BatchSink for NoopBatchSink {
    async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        info!(
            batch_number = batch.batch_number,
            size = batch.data.len(),
            "NoopBatchSink: would submit batch"
        );
        Ok(SubmissionReceipt {
            batch_number: batch.batch_number,
            tx_hash: [0u8; 32],
            l1_block: 0,
            blob_hash: None,
        })
    }

    async fn capacity(&self) -> Result<usize, SinkError> {
        Ok(128 * 1024) // 128 KB
    }

    async fn health_check(&self) -> Result<(), SinkError> {
        Ok(())
    }
}
