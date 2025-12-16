//! Local file-based batch source implementation.

use std::path::PathBuf;

use async_trait::async_trait;
use montana_pipeline::{BatchSource, L2BlockData, RawTransaction, SourceError};
use serde::{Deserialize, Serialize};

use crate::LocalError;

/// JSON representation of a raw transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonTransaction {
    /// Hex-encoded transaction data.
    pub data: String,
}

/// JSON representation of an L2 block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonBlock {
    /// Block timestamp.
    pub timestamp: u64,
    /// Block transactions.
    pub transactions: Vec<JsonTransaction>,
}

/// JSON representation of L1 origin data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonL1Origin {
    /// L1 block number.
    pub block_number: u64,
    /// L1 block hash prefix (hex-encoded, 20 bytes).
    pub hash_prefix: String,
}

/// JSON representation of the source data file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSourceData {
    /// L1 origin information.
    pub l1_origin: JsonL1Origin,
    /// Parent L2 block hash prefix (hex-encoded, 20 bytes).
    pub parent_hash: String,
    /// L2 blocks to process.
    pub blocks: Vec<JsonBlock>,
}

/// A batch source that reads from a local JSON file.
#[derive(Debug)]
pub struct LocalBatchSource {
    data: JsonSourceData,
    blocks_consumed: bool,
}

impl LocalBatchSource {
    /// Create a new local batch source from a JSON file path.
    pub fn from_file(path: impl Into<PathBuf>) -> Result<Self, LocalError> {
        let path = path.into();
        let content = std::fs::read_to_string(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LocalError::NotFound(path.display().to_string())
            } else {
                LocalError::Io(e)
            }
        })?;
        let data: JsonSourceData = serde_json::from_str(&content)?;
        Ok(Self { data, blocks_consumed: false })
    }

    /// Create a new local batch source from JSON string.
    pub fn from_json(json: &str) -> Result<Self, LocalError> {
        let data: JsonSourceData = serde_json::from_str(json)?;
        Ok(Self { data, blocks_consumed: false })
    }

    /// Create a new local batch source from data directly.
    pub const fn from_data(data: JsonSourceData) -> Self {
        Self { data, blocks_consumed: false }
    }

    /// Parse a hex string to bytes.
    fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, SourceError> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        (0..hex.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&hex[i..i + 2], 16)
                    .map_err(|_| SourceError::Connection("Invalid hex string".to_string()))
            })
            .collect()
    }

    /// Parse a hex string to a 20-byte array.
    fn hex_to_hash20(hex: &str) -> Result<[u8; 20], SourceError> {
        let bytes = Self::hex_to_bytes(hex)?;
        if bytes.len() != 20 {
            return Err(SourceError::Connection(format!("Expected 20 bytes, got {}", bytes.len())));
        }
        let mut arr = [0u8; 20];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

#[async_trait]
impl BatchSource for LocalBatchSource {
    async fn pending_blocks(&mut self) -> Result<Vec<L2BlockData>, SourceError> {
        if self.blocks_consumed {
            return Ok(vec![]);
        }
        self.blocks_consumed = true;

        self.data
            .blocks
            .iter()
            .map(|block| {
                let transactions = block
                    .transactions
                    .iter()
                    .map(|tx| Self::hex_to_bytes(&tx.data).map(RawTransaction))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(L2BlockData { timestamp: block.timestamp, transactions })
            })
            .collect()
    }

    async fn l1_origin(&self) -> Result<u64, SourceError> {
        Ok(self.data.l1_origin.block_number)
    }

    async fn l1_origin_hash(&self) -> Result<[u8; 20], SourceError> {
        Self::hex_to_hash20(&self.data.l1_origin.hash_prefix)
    }

    async fn parent_hash(&self) -> Result<[u8; 20], SourceError> {
        Self::hex_to_hash20(&self.data.parent_hash)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const SAMPLE_JSON: &str = r#"{
        "l1_origin": {
            "block_number": 12345,
            "hash_prefix": "0x0102030405060708091011121314151617181920"
        },
        "parent_hash": "0x2122232425262728293031323334353637383940",
        "blocks": [
            {
                "timestamp": 1000,
                "transactions": [
                    {"data": "0xf86c0a8502540be400825208"}
                ]
            },
            {
                "timestamp": 1012,
                "transactions": []
            }
        ]
    }"#;

    #[test]
    fn local_batch_source_from_json() {
        let source = LocalBatchSource::from_json(SAMPLE_JSON).unwrap();
        assert_eq!(source.data.l1_origin.block_number, 12345);
        assert_eq!(source.data.blocks.len(), 2);
    }

    #[tokio::test]
    async fn local_batch_source_pending_blocks() {
        let mut source = LocalBatchSource::from_json(SAMPLE_JSON).unwrap();

        let blocks = source.pending_blocks().await.unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].timestamp, 1000);
        assert_eq!(blocks[0].transactions.len(), 1);
        assert_eq!(blocks[1].timestamp, 1012);
        assert!(blocks[1].transactions.is_empty());

        // Second call returns empty
        let blocks = source.pending_blocks().await.unwrap();
        assert!(blocks.is_empty());
    }

    #[tokio::test]
    async fn local_batch_source_l1_origin() {
        let source = LocalBatchSource::from_json(SAMPLE_JSON).unwrap();
        assert_eq!(source.l1_origin().await.unwrap(), 12345);
    }

    #[tokio::test]
    async fn local_batch_source_hashes() {
        let source = LocalBatchSource::from_json(SAMPLE_JSON).unwrap();

        let l1_hash = source.l1_origin_hash().await.unwrap();
        assert_eq!(l1_hash[0], 0x01);
        assert_eq!(l1_hash[19], 0x20);

        let parent_hash = source.parent_hash().await.unwrap();
        assert_eq!(parent_hash[0], 0x21);
        assert_eq!(parent_hash[19], 0x40);
    }

    #[rstest]
    #[case("0x0102", vec![0x01, 0x02])]
    #[case("0102", vec![0x01, 0x02])]
    #[case("0xabcd", vec![0xab, 0xcd])]
    #[case("ABCD", vec![0xab, 0xcd])]
    fn hex_to_bytes_parsing(#[case] hex: &str, #[case] expected: Vec<u8>) {
        let result = LocalBatchSource::hex_to_bytes(hex).unwrap();
        assert_eq!(result, expected);
    }
}
