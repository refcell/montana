//! Local file-based batch source implementation.

use std::path::PathBuf;

use async_trait::async_trait;
use montana_pipeline::{BatchSource, Bytes, L2BlockData, SourceError};
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

/// JSON representation of the source data file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSourceData {
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
                    .map(|tx| Self::hex_to_bytes(&tx.data).map(Bytes::from))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(L2BlockData { timestamp: block.timestamp, transactions })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    const SAMPLE_JSON: &str = r#"{
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
