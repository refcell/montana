//! Transaction candidates and receipts.

use alloy::{
    consensus::BlobTransactionSidecar,
    primitives::{Address, B256, Bytes, U256},
};

/// A transaction candidate ready for submission.
#[derive(Clone, Debug)]
pub struct TxCandidate {
    /// Recipient address (batch inbox)
    pub to: Address,
    /// Transaction value (typically 0 for batch submission)
    pub value: U256,
    /// Calldata (for calldata mode)
    pub data: Bytes,
    /// Blob sidecar (for blob mode)
    pub blob_sidecar: Option<BlobTransactionSidecar>,
    /// Gas limit override (estimated if None)
    pub gas_limit: Option<u64>,
}

impl TxCandidate {
    /// Creates a new calldata transaction candidate.
    ///
    /// # Arguments
    ///
    /// * `to` - The recipient address (batch inbox)
    /// * `data` - The calldata to include in the transaction
    ///
    /// # Returns
    ///
    /// A new [`TxCandidate`] configured for calldata mode with zero value.
    pub fn calldata(to: Address, data: impl Into<Bytes>) -> Self {
        Self { to, value: U256::ZERO, data: data.into(), blob_sidecar: None, gas_limit: None }
    }

    /// Creates a new blob transaction candidate.
    ///
    /// # Arguments
    ///
    /// * `to` - The recipient address (batch inbox)
    /// * `sidecar` - The blob transaction sidecar containing blob data
    ///
    /// # Returns
    ///
    /// A new [`TxCandidate`] configured for blob mode with zero value and empty calldata.
    pub const fn blob(to: Address, sidecar: BlobTransactionSidecar) -> Self {
        Self {
            to,
            value: U256::ZERO,
            data: Bytes::new(),
            blob_sidecar: Some(sidecar),
            gas_limit: None,
        }
    }

    /// Returns `true` if this is a blob transaction.
    ///
    /// A transaction is considered a blob transaction if it has a blob sidecar attached.
    #[must_use]
    pub const fn is_blob(&self) -> bool {
        self.blob_sidecar.is_some()
    }

    /// Sets the transaction value.
    ///
    /// # Arguments
    ///
    /// * `value` - The value to send with the transaction
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    #[must_use]
    pub const fn with_value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }

    /// Sets the gas limit override.
    ///
    /// # Arguments
    ///
    /// * `gas_limit` - The gas limit to use instead of estimation
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    #[must_use]
    pub const fn with_gas_limit(mut self, gas_limit: u64) -> Self {
        self.gas_limit = Some(gas_limit);
        self
    }
}

/// Result of a successful transaction submission.
#[derive(Clone, Debug)]
pub struct TxReceipt {
    /// Transaction hash
    pub tx_hash: B256,
    /// Block number where transaction was included
    pub block_number: u64,
    /// Block hash
    pub block_hash: B256,
    /// Gas used by the transaction
    pub gas_used: u64,
    /// Effective gas price paid
    pub effective_gas_price: u128,
    /// Blob gas used (for EIP-4844)
    pub blob_gas_used: Option<u64>,
    /// Blob gas price (for EIP-4844)
    pub blob_gas_price: Option<u128>,
}

impl TxReceipt {
    /// Calculates the total cost of the transaction.
    ///
    /// The total cost includes:
    /// - Base gas cost: `gas_used * effective_gas_price`
    /// - Blob gas cost (if applicable): `blob_gas_used * blob_gas_price`
    ///
    /// # Returns
    ///
    /// The total cost in wei.
    #[must_use]
    pub const fn total_cost(&self) -> u128 {
        let base_cost = self.gas_used as u128 * self.effective_gas_price;
        let blob_cost = match (self.blob_gas_used, self.blob_gas_price) {
            (Some(blob_gas_used), Some(blob_gas_price)) => blob_gas_used as u128 * blob_gas_price,
            _ => 0,
        };
        base_cost + blob_cost
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    // TxCandidate::calldata tests

    #[test]
    fn tx_candidate_calldata_creates_valid_candidate() {
        let to = Address::ZERO;
        let data = Bytes::from_static(&[1, 2, 3, 4]);
        let candidate = TxCandidate::calldata(to, data.clone());

        assert_eq!(candidate.to, to);
        assert_eq!(candidate.value, U256::ZERO);
        assert_eq!(candidate.data, data);
        assert!(candidate.blob_sidecar.is_none());
        assert!(candidate.gas_limit.is_none());
    }

    #[test]
    fn tx_candidate_calldata_from_vec() {
        let to = Address::ZERO;
        let data = vec![5, 6, 7, 8];
        let candidate = TxCandidate::calldata(to, data.clone());

        assert_eq!(candidate.data, Bytes::from(data));
    }

    #[test]
    fn tx_candidate_calldata_empty_data() {
        let to = Address::ZERO;
        let candidate = TxCandidate::calldata(to, Bytes::new());

        assert_eq!(candidate.data.len(), 0);
        assert!(!candidate.is_blob());
    }

    // TxCandidate::blob tests

    #[test]
    fn tx_candidate_blob_creates_valid_candidate() {
        let to = Address::ZERO;
        let sidecar = BlobTransactionSidecar::default();
        let candidate = TxCandidate::blob(to, sidecar);

        assert_eq!(candidate.to, to);
        assert_eq!(candidate.value, U256::ZERO);
        assert_eq!(candidate.data.len(), 0);
        assert!(candidate.blob_sidecar.is_some());
        assert!(candidate.gas_limit.is_none());
    }

    // TxCandidate::is_blob tests

    #[rstest]
    #[case(true, "blob transaction")]
    #[case(false, "calldata transaction")]
    fn tx_candidate_is_blob(#[case] has_blob: bool, #[case] _description: &str) {
        let to = Address::ZERO;
        let candidate = if has_blob {
            TxCandidate::blob(to, BlobTransactionSidecar::default())
        } else {
            TxCandidate::calldata(to, Bytes::new())
        };

        assert_eq!(candidate.is_blob(), has_blob);
    }

    // TxCandidate::with_value tests

    #[rstest]
    #[case(U256::ZERO, "zero value")]
    #[case(U256::from(1000), "small value")]
    #[case(U256::from(u128::MAX), "large value")]
    fn tx_candidate_with_value(#[case] value: U256, #[case] _description: &str) {
        let to = Address::ZERO;
        let candidate = TxCandidate::calldata(to, Bytes::new()).with_value(value);

        assert_eq!(candidate.value, value);
    }

    #[test]
    fn tx_candidate_with_value_chaining() {
        let to = Address::ZERO;
        let value = U256::from(500);
        let candidate = TxCandidate::calldata(to, Bytes::from_static(&[1, 2, 3]))
            .with_value(value)
            .with_gas_limit(21000);

        assert_eq!(candidate.value, value);
        assert_eq!(candidate.gas_limit, Some(21000));
    }

    // TxCandidate::with_gas_limit tests

    #[rstest]
    #[case(21000, "minimum gas")]
    #[case(100000, "typical gas")]
    #[case(10000000, "high gas")]
    fn tx_candidate_with_gas_limit(#[case] gas_limit: u64, #[case] _description: &str) {
        let to = Address::ZERO;
        let candidate = TxCandidate::calldata(to, Bytes::new()).with_gas_limit(gas_limit);

        assert_eq!(candidate.gas_limit, Some(gas_limit));
    }

    #[test]
    fn tx_candidate_with_gas_limit_chaining() {
        let to = Address::ZERO;
        let gas_limit = 50000;
        let value = U256::from(100);
        let candidate = TxCandidate::calldata(to, Bytes::from_static(&[1, 2, 3]))
            .with_gas_limit(gas_limit)
            .with_value(value);

        assert_eq!(candidate.gas_limit, Some(gas_limit));
        assert_eq!(candidate.value, value);
    }

    // TxCandidate::clone and Debug tests

    #[test]
    fn tx_candidate_clone() {
        let to = Address::ZERO;
        let data = Bytes::from_static(&[1, 2, 3]);
        let candidate = TxCandidate::calldata(to, data.clone())
            .with_value(U256::from(100))
            .with_gas_limit(50000);

        let cloned = candidate.clone();
        assert_eq!(cloned.to, candidate.to);
        assert_eq!(cloned.value, candidate.value);
        assert_eq!(cloned.data, candidate.data);
        assert_eq!(cloned.gas_limit, candidate.gas_limit);
    }

    #[test]
    fn tx_candidate_debug() {
        let to = Address::ZERO;
        let candidate = TxCandidate::calldata(to, Bytes::from_static(&[1, 2, 3]));
        let debug_str = format!("{:?}", candidate);

        assert!(debug_str.contains("TxCandidate"));
        assert!(debug_str.contains("to"));
        assert!(debug_str.contains("value"));
    }

    // TxReceipt::total_cost tests

    #[rstest]
    #[case(21000, 1_000_000_000, None, None, 21_000_000_000_000, "calldata only")]
    #[case(100000, 50_000_000_000, None, None, 5_000_000_000_000_000, "higher gas")]
    #[case(
        21000,
        1_000_000_000,
        Some(131072),
        Some(1),
        21_000_000_000_000 + 131072,
        "with blob"
    )]
    #[case(
        50000,
        2_000_000_000,
        Some(262144),
        Some(10),
        50000 * 2_000_000_000 + 262144 * 10,
        "blob with higher prices"
    )]
    fn tx_receipt_total_cost(
        #[case] gas_used: u64,
        #[case] effective_gas_price: u128,
        #[case] blob_gas_used: Option<u64>,
        #[case] blob_gas_price: Option<u128>,
        #[case] expected_cost: u128,
        #[case] _description: &str,
    ) {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used,
            effective_gas_price,
            blob_gas_used,
            blob_gas_price,
        };

        assert_eq!(receipt.total_cost(), expected_cost);
    }

    #[test]
    fn tx_receipt_total_cost_blob_gas_only() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 1_000_000_000,
            blob_gas_used: Some(131072),
            blob_gas_price: None,
        };

        // Should only count base gas cost when blob price is None
        assert_eq!(receipt.total_cost(), 21_000_000_000_000);
    }

    #[test]
    fn tx_receipt_total_cost_blob_price_only() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 1_000_000_000,
            blob_gas_used: None,
            blob_gas_price: Some(1),
        };

        // Should only count base gas cost when blob gas used is None
        assert_eq!(receipt.total_cost(), 21_000_000_000_000);
    }

    #[test]
    fn tx_receipt_total_cost_zero_values() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 0,
            effective_gas_price: 0,
            blob_gas_used: Some(0),
            blob_gas_price: Some(0),
        };

        assert_eq!(receipt.total_cost(), 0);
    }

    #[test]
    fn tx_receipt_total_cost_large_values() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 10_000_000,
            effective_gas_price: 100_000_000_000,
            blob_gas_used: Some(393216),
            blob_gas_price: Some(1_000_000),
        };

        let expected = 10_000_000u128 * 100_000_000_000u128 + 393216u128 * 1_000_000u128;
        assert_eq!(receipt.total_cost(), expected);
    }

    // TxReceipt::clone and Debug tests

    #[test]
    fn tx_receipt_clone() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 1_000_000_000,
            blob_gas_used: Some(131072),
            blob_gas_price: Some(1),
        };

        let cloned = receipt.clone();
        assert_eq!(cloned.tx_hash, receipt.tx_hash);
        assert_eq!(cloned.block_number, receipt.block_number);
        assert_eq!(cloned.block_hash, receipt.block_hash);
        assert_eq!(cloned.gas_used, receipt.gas_used);
        assert_eq!(cloned.effective_gas_price, receipt.effective_gas_price);
        assert_eq!(cloned.blob_gas_used, receipt.blob_gas_used);
        assert_eq!(cloned.blob_gas_price, receipt.blob_gas_price);
    }

    #[test]
    fn tx_receipt_debug() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 1_000_000_000,
            blob_gas_used: None,
            blob_gas_price: None,
        };
        let debug_str = format!("{:?}", receipt);

        assert!(debug_str.contains("TxReceipt"));
        assert!(debug_str.contains("tx_hash"));
        assert!(debug_str.contains("block_number"));
    }

    // Integration tests

    #[test]
    fn tx_candidate_blob_mode_workflow() {
        let to = Address::ZERO;
        let sidecar = BlobTransactionSidecar::default();
        let candidate =
            TxCandidate::blob(to, sidecar).with_gas_limit(1_000_000).with_value(U256::from(0));

        assert!(candidate.is_blob());
        assert_eq!(candidate.gas_limit, Some(1_000_000));
        assert_eq!(candidate.value, U256::ZERO);
    }

    #[test]
    fn tx_candidate_calldata_mode_workflow() {
        let to = Address::ZERO;
        let data = Bytes::from_static(&[0x01, 0x02, 0x03]);
        let candidate = TxCandidate::calldata(to, data.clone())
            .with_gas_limit(100_000)
            .with_value(U256::from(1000));

        assert!(!candidate.is_blob());
        assert_eq!(candidate.data, data);
        assert_eq!(candidate.gas_limit, Some(100_000));
        assert_eq!(candidate.value, U256::from(1000));
    }

    #[test]
    fn tx_receipt_calldata_transaction_cost() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 50000,
            effective_gas_price: 20_000_000_000,
            blob_gas_used: None,
            blob_gas_price: None,
        };

        let cost = receipt.total_cost();
        assert_eq!(cost, 50000u128 * 20_000_000_000u128);
    }

    #[test]
    fn tx_receipt_blob_transaction_cost() {
        let receipt = TxReceipt {
            tx_hash: B256::ZERO,
            block_number: 1000,
            block_hash: B256::ZERO,
            gas_used: 21000,
            effective_gas_price: 10_000_000_000,
            blob_gas_used: Some(131072),
            blob_gas_price: Some(5),
        };

        let base_cost = 21000u128 * 10_000_000_000u128;
        let blob_cost = 131072u128 * 5u128;
        assert_eq!(receipt.total_cost(), base_cost + blob_cost);
    }
}
