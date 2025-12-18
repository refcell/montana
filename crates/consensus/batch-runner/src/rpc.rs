//! RPC block source implementation.
//!
//! This module provides an RPC-based implementation of the BlockSource trait
//! for fetching L2 blocks from an Ethereum-compatible RPC endpoint.

use async_trait::async_trait;
use montana_pipeline::{Bytes, L2BlockData};
use serde::{Deserialize, Serialize};

use crate::{BlockSource, BlockSourceError};

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
    /// Block number (hex).
    #[allow(dead_code)]
    number: String,
    /// Block timestamp (hex).
    timestamp: String,
    /// Block hash.
    #[allow(dead_code)]
    hash: String,
    /// Parent block hash.
    #[allow(dead_code)]
    parent_hash: String,
    /// Transactions in the block.
    transactions: Vec<RpcTransaction>,
}

/// Transaction response from eth_getBlockByNumber (full transactions).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcTransaction {
    /// Transaction hash.
    #[allow(dead_code)]
    hash: String,
    /// Transaction input data.
    input: String,
}

/// RPC block source for fetching L2 blocks from an RPC endpoint.
///
/// This implementation fetches blocks using the standard Ethereum JSON-RPC API.
#[derive(Clone)]
pub struct RpcBlockSource {
    client: reqwest::Client,
    url: String,
}

impl RpcBlockSource {
    /// Create a new RPC block source.
    pub fn new(url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), url: url.into() }
    }
}

impl std::fmt::Debug for RpcBlockSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcBlockSource").field("url", &self.url).finish()
    }
}

#[async_trait]
impl BlockSource for RpcBlockSource {
    async fn get_block(&mut self, block_number: u64) -> Result<L2BlockData, BlockSourceError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: "eth_getBlockByNumber",
            params: serde_json::json!([format!("0x{:x}", block_number), true]),
            id: 1,
        };

        let response = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| BlockSourceError::ConnectionError(e.to_string()))?
            .json::<JsonRpcResponse<RpcBlock>>()
            .await
            .map_err(|e| BlockSourceError::RpcError(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = response.error {
            return Err(BlockSourceError::RpcError(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        let block = response.result.ok_or(BlockSourceError::BlockNotFound(block_number))?;

        let timestamp = parse_hex_u64(&block.timestamp)
            .map_err(|e| BlockSourceError::RpcError(format!("Invalid timestamp: {}", e)))?;

        let transactions = block
            .transactions
            .iter()
            .filter_map(|tx| hex_to_bytes(&tx.input).ok().map(Bytes::from))
            .collect();

        Ok(L2BlockData { block_number, timestamp, transactions })
    }

    async fn get_head(&self) -> Result<u64, BlockSourceError> {
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
            .map_err(|e| BlockSourceError::ConnectionError(e.to_string()))?
            .json::<JsonRpcResponse<String>>()
            .await
            .map_err(|e| BlockSourceError::RpcError(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = response.error {
            return Err(BlockSourceError::RpcError(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        let hex = response
            .result
            .ok_or_else(|| BlockSourceError::RpcError("No result in response".to_string()))?;

        parse_hex_u64(&hex)
    }
}

/// Parse a hex string (with 0x prefix) to u64.
fn parse_hex_u64(hex: &str) -> Result<u64, BlockSourceError> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    u64::from_str_radix(hex, 16)
        .map_err(|e| BlockSourceError::RpcError(format!("Invalid hex number: {}", e)))
}

/// Parse hex string to bytes.
fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, BlockSourceError> {
    let hex = hex.strip_prefix("0x").unwrap_or(hex);
    if hex.is_empty() {
        return Ok(Vec::new());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(hex.get(i..i + 2).unwrap_or("00"), 16).map_err(|e| {
                BlockSourceError::RpcError(format!("Invalid hex at position {}: {}", i, e))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_u64() {
        assert_eq!(parse_hex_u64("0x10").unwrap(), 16);
        assert_eq!(parse_hex_u64("0xff").unwrap(), 255);
        assert_eq!(parse_hex_u64("ff").unwrap(), 255);
        assert!(parse_hex_u64("invalid").is_err());
    }

    #[test]
    fn test_hex_to_bytes() {
        assert_eq!(hex_to_bytes("0x").unwrap(), Vec::<u8>::new());
        assert_eq!(hex_to_bytes("0x1234").unwrap(), vec![0x12, 0x34]);
        assert_eq!(hex_to_bytes("1234").unwrap(), vec![0x12, 0x34]);
        assert_eq!(hex_to_bytes("0xabcd").unwrap(), vec![0xab, 0xcd]);
    }

    #[test]
    fn rpc_source_debug() {
        let source = RpcBlockSource::new("https://example.com");
        let debug_str = format!("{:?}", source);
        assert!(debug_str.contains("RpcBlockSource"));
        assert!(debug_str.contains("https://example.com"));
    }
}
