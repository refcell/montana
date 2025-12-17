//! L2 block data type.

use alloy_primitives::Bytes;

/// A block's worth of transactions with metadata.
///
/// Generic over the transaction type `T`, which defaults to `Bytes` for raw
/// RLP-encoded transaction bytes. This allows flexibility for typed transactions
/// (e.g., `L2BlockData<OpTxEnvelope>`) when needed.
#[derive(Clone, Debug)]
pub struct L2BlockData<T = Bytes> {
    /// Block timestamp.
    pub timestamp: u64,
    /// Block transactions.
    pub transactions: Vec<T>,
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn l2_block_data_new() {
        let block: L2BlockData = L2BlockData { timestamp: 1234567890, transactions: vec![] };
        assert_eq!(block.timestamp, 1234567890);
        assert!(block.transactions.is_empty());
    }

    #[rstest]
    #[case(0, 0, "genesis block")]
    #[case(1234567890, 1, "single tx block")]
    #[case(u64::MAX, 100, "far future with many txs")]
    fn l2_block_data_various(
        #[case] timestamp: u64,
        #[case] tx_count: usize,
        #[case] _description: &str,
    ) {
        let transactions: Vec<Bytes> = (0..tx_count).map(|i| Bytes::from(vec![i as u8])).collect();
        let block = L2BlockData { timestamp, transactions };
        assert_eq!(block.timestamp, timestamp);
        assert_eq!(block.transactions.len(), tx_count);
    }

    #[test]
    fn l2_block_data_clone() {
        let block = L2BlockData { timestamp: 1000, transactions: vec![Bytes::from(vec![1, 2, 3])] };
        let cloned = block.clone();
        assert_eq!(cloned.timestamp, block.timestamp);
        assert_eq!(cloned.transactions.len(), block.transactions.len());
        assert_eq!(cloned.transactions[0], block.transactions[0]);
    }

    #[test]
    fn l2_block_data_debug() {
        let block: L2BlockData = L2BlockData { timestamp: 1000, transactions: vec![] };
        let debug_str = format!("{:?}", block);
        assert!(debug_str.contains("L2BlockData"));
        assert!(debug_str.contains("timestamp"));
        assert!(debug_str.contains("transactions"));
    }

    #[rstest]
    #[case(0, "genesis")]
    #[case(1699000000, "recent timestamp")]
    #[case(u64::MAX, "max timestamp")]
    fn l2_block_data_timestamps(#[case] timestamp: u64, #[case] _description: &str) {
        let block: L2BlockData = L2BlockData { timestamp, transactions: vec![] };
        assert_eq!(block.timestamp, timestamp);
    }

    #[test]
    fn l2_block_data_with_multiple_transactions() {
        let txs = vec![
            Bytes::from(vec![0x01]),
            Bytes::from(vec![0x02, 0x03]),
            Bytes::from(vec![0x04, 0x05, 0x06]),
        ];
        let block = L2BlockData { timestamp: 1000, transactions: txs };
        assert_eq!(block.transactions.len(), 3);
        assert_eq!(block.transactions[0].as_ref(), &[0x01]);
        assert_eq!(block.transactions[1].as_ref(), &[0x02, 0x03]);
        assert_eq!(block.transactions[2].as_ref(), &[0x04, 0x05, 0x06]);
    }

    #[test]
    fn l2_block_data_empty_transactions() {
        let block: L2BlockData = L2BlockData { timestamp: 1000, transactions: vec![] };
        assert!(block.transactions.is_empty());
    }

    #[rstest]
    #[case(1, "single transaction")]
    #[case(10, "ten transactions")]
    #[case(1000, "many transactions")]
    fn l2_block_data_transaction_counts(#[case] count: usize, #[case] _description: &str) {
        let transactions: Vec<Bytes> = (0..count).map(|_| Bytes::from(vec![0xAB])).collect();
        let block = L2BlockData { timestamp: 1000, transactions };
        assert_eq!(block.transactions.len(), count);
    }
}
