//! Compressed batch type.

/// Compressed batch ready for submission.
#[derive(Clone, Debug)]
pub struct CompressedBatch {
    /// Batch number.
    pub batch_number: u64,
    /// Compressed data.
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn compressed_batch_new() {
        let batch = CompressedBatch { batch_number: 42, data: vec![1, 2, 3, 4, 5] };
        assert_eq!(batch.batch_number, 42);
        assert_eq!(batch.data, vec![1, 2, 3, 4, 5]);
    }

    #[rstest]
    #[case(0, vec![], "empty batch")]
    #[case(1, vec![0u8; 128 * 1024], "max size batch")]
    #[case(u64::MAX, vec![0xFF], "max batch number")]
    fn compressed_batch_various(
        #[case] batch_number: u64,
        #[case] data: Vec<u8>,
        #[case] _description: &str,
    ) {
        let batch = CompressedBatch { batch_number, data: data.clone() };
        assert_eq!(batch.batch_number, batch_number);
        assert_eq!(batch.data.len(), data.len());
    }

    #[test]
    fn compressed_batch_clone() {
        let batch = CompressedBatch { batch_number: 1, data: vec![1, 2, 3] };
        let cloned = batch.clone();
        assert_eq!(cloned.batch_number, batch.batch_number);
        assert_eq!(cloned.data, batch.data);
    }

    #[test]
    fn compressed_batch_debug() {
        let batch = CompressedBatch { batch_number: 1, data: vec![1, 2, 3] };
        let debug_str = format!("{:?}", batch);
        assert!(debug_str.contains("CompressedBatch"));
        assert!(debug_str.contains("batch_number"));
        assert!(debug_str.contains("data"));
    }

    #[rstest]
    #[case(0, "first batch")]
    #[case(1, "second batch")]
    #[case(1000, "many batches")]
    #[case(u64::MAX, "max batch number")]
    fn compressed_batch_numbers(#[case] batch_number: u64, #[case] _description: &str) {
        let batch = CompressedBatch { batch_number, data: vec![] };
        assert_eq!(batch.batch_number, batch_number);
    }

    #[rstest]
    #[case(0, "empty data")]
    #[case(1, "single byte")]
    #[case(1024, "1KB")]
    #[case(128 * 1024, "128KB - max blob")]
    fn compressed_batch_data_sizes(#[case] size: usize, #[case] _description: &str) {
        let data = vec![0xAB; size];
        let batch = CompressedBatch { batch_number: 1, data };
        assert_eq!(batch.data.len(), size);
    }

    #[test]
    fn compressed_batch_access_data() {
        let batch = CompressedBatch { batch_number: 5, data: vec![0x01, 0x02, 0x03] };
        assert_eq!(batch.data[0], 0x01);
        assert_eq!(batch.data[1], 0x02);
        assert_eq!(batch.data[2], 0x03);
    }

    #[test]
    fn compressed_batch_sequential_numbers() {
        let batches: Vec<CompressedBatch> =
            (0..10).map(|i| CompressedBatch { batch_number: i, data: vec![i as u8] }).collect();
        for (i, batch) in batches.iter().enumerate() {
            assert_eq!(batch.batch_number, i as u64);
        }
    }
}
