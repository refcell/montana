//! # Fetcher Binary
//!
//! A command-line tool for fetching Base L2 blocks and transactions from an RPC endpoint
//! and converting them to Montana's JSON input format.
//!
//! This is a fully functional implementation that:
//! - Fetches blocks via JSON-RPC (eth_getBlockByNumber)
//! - Converts RPC responses to Montana's LocalBatchSource format
//! - Supports configurable block ranges, output paths, and RPC endpoints
//! - Provides verbose logging for debugging
//!
//! **Note**: L1 origin data is currently stubbed with placeholder values (block_number: 0).
//! A complete implementation would require additional L1 block lookups.

#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::path::PathBuf;

use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing::Level;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Default Base mainnet RPC URL.
const DEFAULT_RPC_URL: &str = "https://mainnet.base.org";

/// Default number of blocks to fetch.
const DEFAULT_BLOCK_COUNT: u64 = 10;

/// Fetcher CLI.
#[derive(Debug, Parser)]
#[command(name = "fetcher")]
#[command(about = "Fetch Base L2 blocks and output in Montana JSON format")]
struct Cli {
    /// Verbosity level (can be specified multiple times)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Starting block number to fetch.
    #[arg(short, long)]
    start: u64,

    /// Ending block number (inclusive). Defaults to start + 10.
    #[arg(short, long)]
    end: Option<u64>,

    /// Output JSON file path.
    #[arg(short, long, default_value = "fetched_blocks.json")]
    output: PathBuf,

    /// RPC URL for Base mainnet.
    #[arg(short, long, default_value = DEFAULT_RPC_URL)]
    rpc: String,
}

/// JSON-RPC request structure.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    params: serde_json::Value,
    id: u64,
}

/// JSON-RPC response structure.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// Block response from eth_getBlockByNumber.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcBlock {
    #[allow(dead_code)]
    number: String,
    timestamp: String,
    #[allow(dead_code)]
    hash: String,
    parent_hash: String,
    transactions: Vec<RpcTransaction>,
}

/// Transaction response from eth_getBlockByNumber (full transactions).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcTransaction {
    #[allow(dead_code)]
    hash: String,
    input: String,
}

// Output format matching Montana's LocalBatchSource input

/// JSON representation of a raw transaction (Montana format).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonTransaction {
    /// Hex-encoded transaction data.
    data: String,
}

/// JSON representation of an L2 block (Montana format).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonBlock {
    /// Block timestamp.
    timestamp: u64,
    /// Block transactions.
    transactions: Vec<JsonTransaction>,
}

/// JSON representation of L1 origin data (Montana format).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonL1Origin {
    /// L1 block number (placeholder - would need L1 origin lookup).
    block_number: u64,
    /// L1 block hash prefix (hex-encoded, 20 bytes).
    hash_prefix: String,
}

/// JSON representation of the source data file (Montana format).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonSourceData {
    /// L1 origin information.
    l1_origin: JsonL1Origin,
    /// Parent L2 block hash prefix (hex-encoded, 20 bytes).
    parent_hash: String,
    /// L2 blocks to process.
    blocks: Vec<JsonBlock>,
}

fn main() {
    let cli = Cli::parse();

    // Initialize tracing
    init_tracing(cli.verbose);

    if let Err(e) = run(cli) {
        tracing::error!("Fetcher failed: {}", e);
        std::process::exit(1);
    }
}

fn init_tracing(verbosity: u8) {
    let level = match verbosity {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let filter = EnvFilter::builder().with_default_directive(level.into()).from_env_lossy();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true).with_thread_ids(false))
        .init();
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let end = cli.end.unwrap_or(cli.start + DEFAULT_BLOCK_COUNT - 1);

    if end < cli.start {
        return Err("End block must be >= start block".into());
    }

    let block_count = end - cli.start + 1;
    tracing::info!(
        "Fetching {} blocks from {} to {} from {}",
        block_count,
        cli.start,
        end,
        cli.rpc
    );

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { fetch_blocks(&cli, cli.start, end).await })
}

async fn fetch_blocks(cli: &Cli, start: u64, end: u64) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let mut blocks = Vec::new();
    let mut first_parent_hash = String::new();

    for block_num in start..=end {
        tracing::debug!("Fetching block {}", block_num);

        let block = fetch_block(&client, &cli.rpc, block_num).await?;

        if block_num == start {
            // Truncate parent hash to 20 bytes (40 hex chars + 0x prefix = 42 chars)
            first_parent_hash = truncate_hash(&block.parent_hash);
        }

        // Parse timestamp from hex
        let timestamp = parse_hex_u64(&block.timestamp)?;

        // Convert transactions
        let transactions: Vec<JsonTransaction> = block
            .transactions
            .iter()
            .map(|tx| JsonTransaction { data: tx.input.clone() })
            .collect();

        tracing::debug!(
            "Block {} has {} transactions at timestamp {}",
            block_num,
            transactions.len(),
            timestamp
        );

        blocks.push(JsonBlock { timestamp, transactions });
    }

    tracing::info!("Fetched {} blocks successfully", blocks.len());

    // Create output data
    // Note: L1 origin would need additional lookups in a real implementation
    let output = JsonSourceData {
        l1_origin: JsonL1Origin {
            block_number: 0, // Placeholder - would need L1 origin lookup
            hash_prefix: "0x0000000000000000000000000000000000000000".to_string(),
        },
        parent_hash: first_parent_hash,
        blocks,
    };

    // Write to file
    let json = serde_json::to_string_pretty(&output)?;
    std::fs::write(&cli.output, &json)?;

    tracing::info!("Output written to {:?}", cli.output);
    Ok(())
}

async fn fetch_block(
    client: &reqwest::Client,
    rpc_url: &str,
    block_num: u64,
) -> Result<RpcBlock, Box<dyn std::error::Error>> {
    let request = JsonRpcRequest {
        jsonrpc: "2.0",
        method: "eth_getBlockByNumber",
        params: serde_json::json!([format!("0x{:x}", block_num), true]),
        id: 1,
    };

    let response = client
        .post(rpc_url)
        .json(&request)
        .send()
        .await?
        .json::<JsonRpcResponse<RpcBlock>>()
        .await?;

    if let Some(error) = response.error {
        return Err(format!("RPC error {}: {}", error.code, error.message).into());
    }

    response.result.ok_or_else(|| format!("Block {} not found", block_num).into())
}

/// Parse a hex string (with 0x prefix) to u64.
fn parse_hex_u64(hex: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    Ok(u64::from_str_radix(hex, 16)?)
}

/// Truncate a 32-byte hash to 20 bytes (for Montana format).
fn truncate_hash(hash: &str) -> String {
    let hash = hash.strip_prefix("0x").unwrap_or(hash);
    // Take first 40 hex chars (20 bytes)
    format!("0x{}", &hash[..40.min(hash.len())])
}
