#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/base/montana/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::path::PathBuf;

use clap::Parser;
use montana_cli::init_tracing;
use serde::{Deserialize, Serialize};

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
#[derive(Debug, Deserialize, derive_more::Display)]
#[display("{message} (code: {code})")]
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
    #[allow(dead_code)]
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

/// JSON representation of the source data file (Montana format).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonSourceData {
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

    for block_num in start..=end {
        tracing::debug!("Fetching block {}", block_num);

        let block = fetch_block(&client, &cli.rpc, block_num).await?;

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
    let output = JsonSourceData { blocks };

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
