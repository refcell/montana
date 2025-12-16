//! Optimism (Base) block executor using op-revm
//!
//! This binary fetches blocks from an Optimism (Base) RPC and executes all transactions
//! using op-revm with an in-memory database that falls back to RPC for missing state.

mod cli;

use std::time::Duration;

use alloy::{
    consensus::BlockHeader,
    network::BlockResponse,
    providers::{ProviderBuilder, RootProvider},
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
use tracing::{error, info};

const CHANNEL_CAPACITY: usize = 256;

/// Execute a block using the `BlockExecutor`
fn execute_block(block: OpBlock, provider: &RootProvider<Optimism>) -> Result<()> {
    let block_number = block.header().number();

    // Create the database backed by RPC (state at block - 1)
    let state_block = block_number.saturating_sub(1);
    let rpc_db = RPCDatabase::new(provider.clone(), state_block, Handle::current());
    let db = CachedDatabase::new(rpc_db);

    let mut executor = BlockExecutor::new(db, BASE_MAINNET);
    let _result = executor.execute_block(block);

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
        if let Err(e) = execute_block(block, &provider) {
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
