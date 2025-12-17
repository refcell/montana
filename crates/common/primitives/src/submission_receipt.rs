//! Submission receipt type.

/// Result of a successful submission.
#[derive(Clone, Debug)]
pub struct SubmissionReceipt {
    /// Batch number.
    pub batch_number: u64,
    /// Transaction hash.
    pub tx_hash: [u8; 32],
    /// L1 block number.
    pub l1_block: u64,
    /// Blob hash (if blob submission).
    pub blob_hash: Option<[u8; 32]>,
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn submission_receipt_new() {
        let receipt = SubmissionReceipt {
            batch_number: 1,
            tx_hash: [0xABu8; 32],
            l1_block: 100,
            blob_hash: Some([0xCDu8; 32]),
        };
        assert_eq!(receipt.batch_number, 1);
        assert_eq!(receipt.tx_hash, [0xABu8; 32]);
        assert_eq!(receipt.l1_block, 100);
        assert_eq!(receipt.blob_hash, Some([0xCDu8; 32]));
    }

    #[rstest]
    #[case(0, 0, None, "genesis calldata")]
    #[case(1, 100, Some([0u8; 32]), "blob submission")]
    #[case(u64::MAX, u64::MAX, None, "max values")]
    fn submission_receipt_various(
        #[case] batch_number: u64,
        #[case] l1_block: u64,
        #[case] blob_hash: Option<[u8; 32]>,
        #[case] _description: &str,
    ) {
        let receipt = SubmissionReceipt { batch_number, tx_hash: [0u8; 32], l1_block, blob_hash };
        assert_eq!(receipt.batch_number, batch_number);
        assert_eq!(receipt.l1_block, l1_block);
        assert_eq!(receipt.blob_hash, blob_hash);
    }

    #[test]
    fn submission_receipt_clone() {
        let receipt = SubmissionReceipt {
            batch_number: 1,
            tx_hash: [0xABu8; 32],
            l1_block: 100,
            blob_hash: Some([0xCDu8; 32]),
        };
        let cloned = receipt.clone();
        assert_eq!(cloned.batch_number, receipt.batch_number);
        assert_eq!(cloned.tx_hash, receipt.tx_hash);
        assert_eq!(cloned.l1_block, receipt.l1_block);
        assert_eq!(cloned.blob_hash, receipt.blob_hash);
    }

    #[test]
    fn submission_receipt_debug() {
        let receipt = SubmissionReceipt {
            batch_number: 1,
            tx_hash: [0u8; 32],
            l1_block: 100,
            blob_hash: None,
        };
        let debug_str = format!("{:?}", receipt);
        assert!(debug_str.contains("SubmissionReceipt"));
        assert!(debug_str.contains("tx_hash"));
    }

    #[test]
    fn submission_receipt_without_blob() {
        let receipt = SubmissionReceipt {
            batch_number: 1,
            tx_hash: [0xFFu8; 32],
            l1_block: 50,
            blob_hash: None,
        };
        assert!(receipt.blob_hash.is_none());
    }

    #[test]
    fn submission_receipt_with_blob() {
        let blob_hash = [0x12u8; 32];
        let receipt = SubmissionReceipt {
            batch_number: 2,
            tx_hash: [0xAAu8; 32],
            l1_block: 200,
            blob_hash: Some(blob_hash),
        };
        assert_eq!(receipt.blob_hash, Some(blob_hash));
    }

    #[rstest]
    #[case([0x00u8; 32], "zero hash")]
    #[case([0xFFu8; 32], "max hash")]
    #[case([0xABu8; 32], "arbitrary hash")]
    fn submission_receipt_tx_hashes(#[case] tx_hash: [u8; 32], #[case] _description: &str) {
        let receipt =
            SubmissionReceipt { batch_number: 1, tx_hash, l1_block: 100, blob_hash: None };
        assert_eq!(receipt.tx_hash, tx_hash);
    }

    #[rstest]
    #[case(0, "genesis block")]
    #[case(18_000_000, "recent mainnet block")]
    #[case(u64::MAX, "max block number")]
    fn submission_receipt_l1_blocks(#[case] l1_block: u64, #[case] _description: &str) {
        let receipt =
            SubmissionReceipt { batch_number: 1, tx_hash: [0u8; 32], l1_block, blob_hash: None };
        assert_eq!(receipt.l1_block, l1_block);
    }

    #[test]
    fn submission_receipt_sequential_batches() {
        let receipts: Vec<SubmissionReceipt> = (0..5)
            .map(|i| SubmissionReceipt {
                batch_number: i,
                tx_hash: [i as u8; 32],
                l1_block: 100 + i,
                blob_hash: None,
            })
            .collect();
        for (i, receipt) in receipts.iter().enumerate() {
            assert_eq!(receipt.batch_number, i as u64);
            assert_eq!(receipt.l1_block, 100 + i as u64);
        }
    }
}
