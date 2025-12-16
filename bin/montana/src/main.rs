//! Optimism (Base) block executor using op-revm
//!
//! This binary fetches blocks from an Optimism (Base) RPC and executes all transactions
//! using op-revm with an in-memory database that falls back to RPC for missing state.

mod cli;

use std::time::Duration;

use alloy::{
    consensus::BlockHeader,
    eips::BlockId,
    network::{BlockResponse, ReceiptResponse},
    providers::{Provider, ProviderBuilder, RootProvider},
};
use blocksource::{BlockProducer, HistoricalRangeProducer, LiveRpcProducer, OpBlock};
use chainspec::BASE_MAINNET;
use clap::Parser;
use cli::{Args, ProducerMode};
use database::{CachedDatabase, RPCDatabase};
use execution::BlockExecutor;
use eyre::Result;
use op_alloy::network::Optimism;
use tokio::{runtime::Handle, sync::mpsc};
use tracing::{error, info, warn};

const CHANNEL_CAPACITY: usize = 256;

/// Execute a block using the `BlockExecutor` and verify against RPC receipts
async fn execute_block(block: OpBlock, provider: &RootProvider<Optimism>) -> Result<()> {
    let block_number = block.header().number();

    // Create the database backed by RPC (state at block - 1)
    let state_block = block_number.saturating_sub(1);
    let rpc_db = RPCDatabase::new(provider.clone(), state_block, Handle::current());
    let db = CachedDatabase::new(rpc_db);

    let mut executor = BlockExecutor::new(db, BASE_MAINNET);
    let result = executor.execute_block(block)?;

    // Fetch receipts from RPC for verification
    let receipts =
        provider.get_block_receipts(BlockId::number(block_number)).await?.unwrap_or_default();

    // Verify execution results against RPC receipts
    let mut verified = 0;
    let mut mismatched = 0;

    for (idx, (tx_result, receipt)) in result.tx_results.iter().zip(receipts.iter()).enumerate() {
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
            result.tx_results.len()
        );
    } else {
        warn!(
            "Block {} verification FAILED: {} mismatched, {} verified out of {} transactions",
            block_number,
            mismatched,
            verified,
            result.tx_results.len()
        );
    }

    Ok(())
}

async fn run(args: Args) -> Result<()> {
    // Create channel for producer -> consumer communication
    let (tx, mut rx) = mpsc::channel::<OpBlock>(CHANNEL_CAPACITY);

    let provider = ProviderBuilder::new()
        .disable_recommended_fillers()
        .network::<Optimism>()
        .connect(args.rpc_url.as_str())
        .await?;

    let producer: Box<dyn BlockProducer> = match args.mode {
        ProducerMode::Live { poll_interval_ms, start_block } => {
            let poll_interval = Duration::from_millis(poll_interval_ms);
            Box::new(LiveRpcProducer::new(provider.clone(), poll_interval, start_block))
        }
        ProducerMode::Historical { start, end } => {
            Box::new(HistoricalRangeProducer::new(provider.clone(), start, end))
        }
    };

    let producer_handle = tokio::spawn(async move {
        if let Err(e) = producer.produce(tx).await {
            error!("Producer error: {e}");
        }
    });

    // Consumer loop: process blocks as they arrive
    while let Some(block) = rx.recv().await {
        if let Err(e) = execute_block(block, &provider).await {
            error!("Block execution error: {e}");
        }
    }

    // Wait for producer to finish
    producer_handle.await?;

    info!("All blocks processed");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::level_filters::LevelFilter::INFO.into()),
        )
        .init();

    let args = Args::parse();
    run(args).await
}
