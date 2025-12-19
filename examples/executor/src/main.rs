//! Block Executor Example
//!
//! This example demonstrates how to set up a block executor with a cached database,
//! trie database, and RocksDB key-value store. It fetches blocks from an RPC endpoint
//! and executes them, collecting performance metrics.
//!
//! Usage:
//!   cargo run -p executor -- --rpc-url <RPC_URL> --start-block <NUM> --num-blocks <NUM>

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use alloy::{genesis::Genesis, providers::ProviderBuilder};
use blocksource::{BlockSource, RpcBlockSource};
use chainspec::BASE_MAINNET;
use clap::Parser;
use database::{CachedDatabase, KeyValueDatabase, RocksDbKvDatabase, TrieDatabase};
use eyre::Result;
use op_alloy::network::Optimism;
use tracing::{info, warn};
use vm::{BlockExecutor, BlockMetrics};

/// CLI arguments for the executor example.
#[derive(Parser, Debug)]
#[command(name = "executor")]
#[command(about = "Execute blocks from RPC with a cached trie database and RocksDB storage")]
struct Args {
    /// RPC URL to fetch blocks from (e.g., https://mainnet.base.org)
    #[arg(long, env = "RPC_URL")]
    rpc_url: String,

    /// Starting block number to execute
    #[arg(long)]
    start_block: u64,

    /// Number of blocks to execute
    #[arg(long, default_value = "10")]
    num_blocks: u64,

    /// Path to genesis JSON file (alloy Genesis format)
    #[arg(long)]
    genesis: PathBuf,

    /// Data directory for RocksDB and TrieDB storage
    #[arg(long, default_value = "./executor_data")]
    data_dir: PathBuf,
}

/// Metrics collected during block execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionMetrics {
    /// Total gas used across all executed blocks.
    pub total_gas_used: u64,
    /// Total time spent processing blocks (includes fetch, execute, commit).
    pub total_process_time: Duration,
    /// Total time spent on EVM execution only.
    pub evm_execution_time: Duration,
    /// Total time spent committing state to disk.
    pub commit_time: Duration,
    /// Total number of successful transactions.
    pub successful_txs: usize,
    /// Total number of failed transactions.
    pub failed_txs: usize,
}

impl ExecutionMetrics {
    /// Calculate throughput in gigagas per second.
    #[must_use]
    pub fn gigagas_per_second(&self) -> f64 {
        if self.total_process_time.is_zero() {
            return 0.0;
        }
        let gas_billions = self.total_gas_used as f64 / 1_000_000_000.0;
        let seconds = self.total_process_time.as_secs_f64();
        gas_billions / seconds
    }

    /// Calculate EVM-only throughput in gigagas per second.
    #[must_use]
    pub fn evm_gigagas_per_second(&self) -> f64 {
        if self.evm_execution_time.is_zero() {
            return 0.0;
        }
        let gas_billions = self.total_gas_used as f64 / 1_000_000_000.0;
        let seconds = self.evm_execution_time.as_secs_f64();
        gas_billions / seconds
    }

    /// Accumulate metrics from a single block execution.
    pub fn accumulate(&mut self, block_metrics: &BlockMetrics) {
        self.total_gas_used += block_metrics.total_gas_used;
        self.evm_execution_time += Duration::from_micros(block_metrics.execution_time_us);
        self.commit_time += Duration::from_micros(block_metrics.state_root_time_us);
        self.successful_txs += block_metrics.successful_txs;
        self.failed_txs += block_metrics.failed_txs;
    }

    /// Log the metrics summary.
    pub fn log_summary(&self, blocks_executed: u64) {
        info!("=== Execution Metrics Summary ===");
        info!("Blocks executed: {}", blocks_executed);
        info!(
            "Total gas used: {} ({:.2} Ggas)",
            self.total_gas_used,
            self.total_gas_used as f64 / 1_000_000_000.0
        );
        info!("Successful transactions: {}", self.successful_txs);
        info!("Failed transactions: {}", self.failed_txs);
        info!("Total process time: {:?}", self.total_process_time);
        info!("EVM execution time: {:?}", self.evm_execution_time);
        info!("State root time: {:?}", self.commit_time);
        info!("Throughput (total): {:.4} Ggas/s", self.gigagas_per_second());
        info!("Throughput (EVM only): {:.4} Ggas/s", self.evm_gigagas_per_second());
        info!("=================================");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?),
        )
        .init();

    let args = Args::parse();

    info!("Starting block executor example");
    info!("RPC URL: {}", args.rpc_url);
    info!("Start block: {}", args.start_block);
    info!("Number of blocks: {}", args.num_blocks);
    info!("Genesis file: {:?}", args.genesis);
    info!("Data directory: {:?}", args.data_dir);

    // Load genesis from file
    info!("Loading genesis from {:?}...", args.genesis);
    let genesis_json = std::fs::read_to_string(&args.genesis)?;
    let genesis: Genesis = serde_json::from_str(&genesis_json)?;
    info!("Loaded genesis with {} accounts", genesis.alloc.len());

    // Create data directories
    std::fs::create_dir_all(&args.data_dir)?;
    let kvdb_path = args.data_dir.join("kvdb");
    let trie_path = args.data_dir.join("triedb");

    // Set up RocksDB key-value database
    info!("Opening RocksDB at {:?}", kvdb_path);
    let kvdb = RocksDbKvDatabase::open_or_create(&kvdb_path, &genesis)?;
    let kvdb_ref = kvdb.clone();

    // Set up TrieDB with RocksDB backend
    info!("Opening TrieDB at {:?}", trie_path);
    let trie_db = TrieDatabase::open_or_create(&trie_path, &genesis, kvdb)
        .map_err(|e| eyre::eyre!("Failed to open TrieDB: {e:?}"))?;

    // Wrap in cache layer for performance
    let cached_db = CachedDatabase::new(trie_db);

    // Create block executor with Base Mainnet chain config
    let mut executor = BlockExecutor::new(cached_db, BASE_MAINNET);

    // Set up RPC provider and block source
    info!("Connecting to RPC endpoint...");
    let provider = ProviderBuilder::new()
        .disable_recommended_fillers()
        .network::<Optimism>()
        .connect(&args.rpc_url)
        .await?;

    let block_source = RpcBlockSource::new(provider);

    // Initialize metrics
    let mut metrics = ExecutionMetrics::default();
    let mut blocks_executed = 0u64;

    let overall_start = Instant::now();

    // Execute blocks in a loop
    for block_num in args.start_block..(args.start_block + args.num_blocks) {
        info!("Fetching block {}...", block_num);

        // Fetch block from RPC
        let block = match block_source.get_block(block_num).await {
            Ok(b) => b,
            Err(e) => {
                warn!("Failed to fetch block {}: {}", block_num, e);
                continue;
            }
        };

        let tx_count = block.transactions.len();
        info!("Block {} has {} transactions", block_num, tx_count);

        // Execute block
        let result = match executor.execute_block(block) {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to execute block {}: {}", block_num, e);
                continue;
            }
        };

        // Update metrics using the block execution metrics
        metrics.accumulate(&result.metrics);
        blocks_executed += 1;

        info!(
            "Block {} executed: {} txs ({} ok, {} failed), {} gas, state_root: {}, exec_time: {}us, state_root_time: {}us",
            block_num,
            result.tx_results.len(),
            result.metrics.successful_txs,
            result.metrics.failed_txs,
            result.metrics.total_gas_used,
            result.state_root,
            result.metrics.execution_time_us,
            result.metrics.state_root_time_us
        );

        // Write block_number - 1 to the kvdb
        if block_num > 0 {
            kvdb_ref.set_unsafe(block_num - 1)?;
            info!("Wrote block {} - 1 = {} to kvdb", block_num, block_num - 1);
        }
    }

    // Finalize total process time
    metrics.total_process_time = overall_start.elapsed();

    // Log final summary
    println!();
    metrics.log_summary(blocks_executed);

    Ok(())
}
