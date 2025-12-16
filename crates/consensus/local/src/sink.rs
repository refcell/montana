//! Local file-based batch sink implementation.

use std::{path::PathBuf, sync::Mutex};

use async_trait::async_trait;
use montana_pipeline::{BatchSink, CompressedBatch, SinkError, SubmissionReceipt};
use serde::{Deserialize, Serialize};

/// JSON representation of a submission receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonReceipt {
    /// Batch number.
    pub batch_number: u64,
    /// Transaction hash (hex-encoded).
    pub tx_hash: String,
    /// L1 block number.
    pub l1_block: u64,
    /// Blob hash (hex-encoded, optional).
    pub blob_hash: Option<String>,
}

/// JSON representation of a compressed batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonBatch {
    /// Batch number.
    pub batch_number: u64,
    /// Compressed data (hex-encoded).
    pub data: String,
}

/// JSON representation of the sink output file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JsonSinkData {
    /// Submitted batches.
    pub batches: Vec<JsonBatch>,
    /// Submission receipts.
    pub receipts: Vec<JsonReceipt>,
}

/// A batch sink that writes to a local JSON file.
#[derive(Debug)]
pub struct LocalBatchSink {
    output_path: Option<PathBuf>,
    data: Mutex<JsonSinkData>,
    next_l1_block: Mutex<u64>,
    capacity: usize,
}

impl LocalBatchSink {
    /// Create a new local batch sink that writes to a file.
    pub fn new(output_path: impl Into<PathBuf>) -> Self {
        Self {
            output_path: Some(output_path.into()),
            data: Mutex::new(JsonSinkData::default()),
            next_l1_block: Mutex::new(1),
            capacity: 128 * 1024,
        }
    }

    /// Create a new in-memory local batch sink (no file output).
    pub fn in_memory() -> Self {
        Self {
            output_path: None,
            data: Mutex::new(JsonSinkData::default()),
            next_l1_block: Mutex::new(1),
            capacity: 128 * 1024,
        }
    }

    /// Create a new local batch sink with custom capacity.
    pub const fn with_capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }

    /// Get the current sink data.
    pub fn data(&self) -> JsonSinkData {
        self.data.lock().unwrap().clone()
    }

    /// Get the number of submitted batches.
    pub fn batch_count(&self) -> usize {
        self.data.lock().unwrap().batches.len()
    }

    /// Convert bytes to hex string.
    fn bytes_to_hex(bytes: &[u8]) -> String {
        format!("0x{}", bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>())
    }

    /// Write data to file if output path is set.
    fn write_to_file(&self) -> Result<(), SinkError> {
        if let Some(ref path) = self.output_path {
            let data = self.data.lock().unwrap();
            let json = serde_json::to_string_pretty(&*data)
                .map_err(|e| SinkError::TxFailed(format!("JSON serialization error: {}", e)))?;
            std::fs::write(path, json)
                .map_err(|e| SinkError::TxFailed(format!("File write error: {}", e)))?;
        }
        Ok(())
    }
}

#[async_trait]
impl BatchSink for LocalBatchSink {
    async fn submit(&mut self, batch: CompressedBatch) -> Result<SubmissionReceipt, SinkError> {
        let batch_number = batch.batch_number;
        let data_hex = Self::bytes_to_hex(&batch.data);

        // Generate a deterministic tx hash based on batch number
        let mut tx_hash = [0u8; 32];
        tx_hash[0..8].copy_from_slice(&batch_number.to_le_bytes());

        // Generate a deterministic blob hash
        let mut blob_hash = [0u8; 32];
        blob_hash[0] = 0x01; // Versioned hash prefix
        blob_hash[1..9].copy_from_slice(&batch_number.to_le_bytes());

        let l1_block = {
            let mut next = self.next_l1_block.lock().unwrap();
            let current = *next;
            *next += 1;
            current
        };

        let json_batch = JsonBatch { batch_number, data: data_hex };

        let json_receipt = JsonReceipt {
            batch_number,
            tx_hash: Self::bytes_to_hex(&tx_hash),
            l1_block,
            blob_hash: Some(Self::bytes_to_hex(&blob_hash)),
        };

        {
            let mut data = self.data.lock().unwrap();
            data.batches.push(json_batch);
            data.receipts.push(json_receipt);
        }

        self.write_to_file()?;

        Ok(SubmissionReceipt { batch_number, tx_hash, l1_block, blob_hash: Some(blob_hash) })
    }

    async fn capacity(&self) -> Result<usize, SinkError> {
        Ok(self.capacity)
    }

    async fn health_check(&self) -> Result<(), SinkError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn local_batch_sink_in_memory() {
        let sink = LocalBatchSink::in_memory();
        assert_eq!(sink.batch_count(), 0);
    }

    #[tokio::test]
    async fn local_batch_sink_submit() {
        let mut sink = LocalBatchSink::in_memory();

        let batch = CompressedBatch { batch_number: 1, data: vec![1, 2, 3, 4, 5] };

        let receipt = sink.submit(batch).await.unwrap();
        assert_eq!(receipt.batch_number, 1);
        assert_eq!(receipt.l1_block, 1);
        assert!(receipt.blob_hash.is_some());
        assert_eq!(sink.batch_count(), 1);
    }

    #[tokio::test]
    async fn local_batch_sink_multiple_submissions() {
        let mut sink = LocalBatchSink::in_memory();

        for i in 0..5 {
            let batch = CompressedBatch { batch_number: i, data: vec![i as u8] };
            let receipt = sink.submit(batch).await.unwrap();
            assert_eq!(receipt.batch_number, i);
            assert_eq!(receipt.l1_block, (i + 1) as u64);
        }

        assert_eq!(sink.batch_count(), 5);
    }

    #[tokio::test]
    async fn local_batch_sink_capacity() {
        let sink = LocalBatchSink::in_memory();
        assert_eq!(sink.capacity().await.unwrap(), 128 * 1024);

        let sink = LocalBatchSink::in_memory().with_capacity(64 * 1024);
        assert_eq!(sink.capacity().await.unwrap(), 64 * 1024);
    }

    #[tokio::test]
    async fn local_batch_sink_health_check() {
        let sink = LocalBatchSink::in_memory();
        assert!(sink.health_check().await.is_ok());
    }

    #[rstest]
    #[case(&[0x01, 0x02], "0x0102")]
    #[case(&[0xab, 0xcd, 0xef], "0xabcdef")]
    #[case(&[], "0x")]
    fn bytes_to_hex_conversion(#[case] bytes: &[u8], #[case] expected: &str) {
        assert_eq!(LocalBatchSink::bytes_to_hex(bytes), expected);
    }

    #[tokio::test]
    async fn local_batch_sink_data_retrieval() {
        let mut sink = LocalBatchSink::in_memory();

        let batch = CompressedBatch { batch_number: 42, data: vec![0xde, 0xad, 0xbe, 0xef] };
        sink.submit(batch).await.unwrap();

        let data = sink.data();
        assert_eq!(data.batches.len(), 1);
        assert_eq!(data.batches[0].batch_number, 42);
        assert_eq!(data.batches[0].data, "0xdeadbeef");
        assert_eq!(data.receipts.len(), 1);
        assert_eq!(data.receipts[0].batch_number, 42);
    }
}
