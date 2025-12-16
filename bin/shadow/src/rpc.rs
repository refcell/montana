//! RPC client for fetching blocks from the chain.
//!
//! This module provides a simple JSON-RPC client for fetching
//! L2 blocks from an Ethereum-compatible RPC endpoint.

use montana_pipeline::{L2BlockData, RawTransaction};
use serde::{Deserialize, Serialize};

/// JSON-RPC request structure.
#[derive(Debug, Serialize)]
pub(crate) struct JsonRpcRequest<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    params: serde_json::Value,
    id: u64,
}

/// JSON-RPC response structure.
#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure.
#[derive(Debug, Deserialize)]
pub(crate) struct JsonRpcError {
    code: i64,
    message: String,
}

/// Block response from eth_getBlockByNumber.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RpcBlock {
    /// Block number (hex).
    #[allow(dead_code)]
    pub(crate) number: String,
    /// Block timestamp (hex).
    pub(crate) timestamp: String,
    /// Block hash.
    #[allow(dead_code)]
    pub(crate) hash: String,
    /// Parent block hash.
    #[allow(dead_code)]
    pub(crate) parent_hash: String,
    /// Transactions in the block.
    pub(crate) transactions: Vec<RpcTransaction>,
}

/// Transaction response from eth_getBlockByNumber (full transactions).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RpcTransaction {
    /// Transaction hash.
    #[allow(dead_code)]
    pub(crate) hash: String,
    /// Transaction input data.
    pub(crate) input: String,
}

/// RPC client for fetching blocks.
#[derive(Debug, Clone)]
pub(crate) struct RpcClient {
    client: reqwest::Client,
    url: String,
}

/// RPC error type.
#[derive(Debug)]
pub(crate) enum RpcError {
    /// HTTP request failed.
    Http(String),
    /// JSON-RPC error response.
    Rpc { code: i64, message: String },
    /// Block not found.
    BlockNotFound(u64),
    /// Failed to parse response.
    Parse(String),
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(e) => write!(f, "HTTP error: {}", e),
            Self::Rpc { code, message } => write!(f, "RPC error {}: {}", code, message),
            Self::BlockNotFound(n) => write!(f, "Block {} not found", n),
            Self::Parse(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl RpcClient {
    /// Create a new RPC client.
    pub(crate) fn new(url: String) -> Self {
        Self { client: reqwest::Client::new(), url }
    }

    /// Get the latest block number.
    pub(crate) async fn get_block_number(&self) -> Result<u64, RpcError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: "eth_blockNumber",
            params: serde_json::json!([]),
            id: 1,
        };

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| RpcError::Http(e.to_string()))?
            .json::<JsonRpcResponse<String>>()
            .await
            .map_err(|e| RpcError::Parse(e.to_string()))?;

        if let Some(error) = response.error {
            return Err(RpcError::Rpc { code: error.code, message: error.message });
        }

        let hex = response.result.ok_or_else(|| RpcError::Parse("No result".into()))?;
        parse_hex_u64(&hex)
    }

    /// Fetch a block by number.
    pub(crate) async fn get_block(&self, block_num: u64) -> Result<RpcBlock, RpcError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: "eth_getBlockByNumber",
            params: serde_json::json!([format!("0x{:x}", block_num), true]),
            id: 1,
        };

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| RpcError::Http(e.to_string()))?
            .json::<JsonRpcResponse<RpcBlock>>()
            .await
            .map_err(|e| RpcError::Parse(e.to_string()))?;

        if let Some(error) = response.error {
            return Err(RpcError::Rpc { code: error.code, message: error.message });
        }

        response.result.ok_or(RpcError::BlockNotFound(block_num))
    }

    /// Fetch a block and convert it to L2BlockData.
    pub(crate) async fn get_l2_block(&self, block_num: u64) -> Result<L2BlockData, RpcError> {
        let block = self.get_block(block_num).await?;

        let timestamp =
            parse_hex_u64(&block.timestamp).map_err(|e| RpcError::Parse(e.to_string()))?;

        let transactions = block
            .transactions
            .iter()
            .filter_map(|tx| hex_to_bytes(&tx.input).ok().map(RawTransaction))
            .collect();

        Ok(L2BlockData { timestamp, transactions })
    }
}

/// Parse a hex string (with 0x prefix) to u64.
fn parse_hex_u64(hex: &str) -> Result<u64, RpcError> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    u64::from_str_radix(hex, 16).map_err(|e| RpcError::Parse(e.to_string()))
}

/// Parse hex string to bytes.
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, RpcError> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    if hex.is_empty() {
        return Ok(Vec::new());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16)
                .map_err(|e| RpcError::Parse(format!("Invalid hex at position {}: {}", i, e)))
        })
        .collect()
}
