//! Optimism (Base) block executor using op-revm
//!
//! This binary fetches blocks from an Optimism (Base) RPC and executes all transactions
//! using op-revm with an in-memory database that falls back to RPC for missing state.

mod cli;

use std::time::Duration;

use clap::Parser;
use cli::{Args, ProducerMode};
use eyre::Result;
use runner::{Execution, ProducerMode as RunnerMode};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::level_filters::LevelFilter::INFO.into()),
        )
        .init();

    let args = Args::parse();

    let mode = match args.mode {
        ProducerMode::Live { poll_interval_ms, start_block } => {
            RunnerMode::Live { poll_interval: Duration::from_millis(poll_interval_ms), start_block }
        }
        ProducerMode::Historical { start, end } => RunnerMode::Historical { start, end },
    };

    let execution = Execution::new(args.rpc_url, mode);
    execution.start().await
}
