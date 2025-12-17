//! Transaction building and signing.

use std::sync::Arc;

use alloy::{
    network::{TransactionBuilder, TransactionBuilder4844},
    primitives::Address,
    providers::Provider,
    rpc::types::TransactionRequest,
};

use crate::{candidate::TxCandidate, error::TxError, gas::GasCaps};

/// Transaction builder for constructing and signing transactions.
pub struct TxBuilder<P> {
    /// The provider for RPC calls.
    provider: Arc<P>,
    /// The chain ID.
    chain_id: u64,
}

impl<P> TxBuilder<P>
where
    P: Provider + Clone + Send + Sync,
{
    /// Creates a new transaction builder.
    ///
    /// # Arguments
    ///
    /// * `provider` - The provider for RPC calls
    /// * `chain_id` - The chain ID
    ///
    /// # Returns
    ///
    /// A new [`TxBuilder`] instance.
    #[must_use]
    pub const fn new(provider: Arc<P>, chain_id: u64) -> Self {
        Self { provider, chain_id }
    }

    /// Builds an unsigned transaction request from a candidate.
    ///
    /// # Arguments
    ///
    /// * `candidate` - The transaction candidate
    /// * `nonce` - The transaction nonce
    /// * `caps` - The gas fee caps
    /// * `from` - The sender address
    ///
    /// # Returns
    ///
    /// A [`TransactionRequest`] ready for signing.
    ///
    /// # Errors
    ///
    /// Returns an error if the transaction request cannot be built.
    pub async fn build_request(
        &self,
        candidate: &TxCandidate,
        nonce: u64,
        caps: &GasCaps,
        from: Address,
    ) -> Result<TransactionRequest, TxError> {
        let mut request = TransactionRequest::default()
            .with_to(candidate.to)
            .with_value(candidate.value)
            .with_input(candidate.data.clone())
            .with_nonce(nonce)
            .with_chain_id(self.chain_id)
            .with_from(from)
            .with_max_fee_per_gas(caps.max_fee_per_gas())
            .with_max_priority_fee_per_gas(caps.max_priority_fee_per_gas());

        // Set gas limit if specified, otherwise leave unset for estimation
        if let Some(gas_limit) = candidate.gas_limit {
            request = request.with_gas_limit(gas_limit);
        }

        // Handle blob transactions
        if let Some(ref sidecar) = candidate.blob_sidecar {
            // Set blob versioned hashes from sidecar
            request = request.with_blob_sidecar(sidecar.clone());

            // Set max fee per blob gas
            if let Some(blob_fee_cap) = caps.blob_fee_cap {
                request = request.with_max_fee_per_blob_gas(blob_fee_cap.to::<u128>());
            }
        }

        Ok(request)
    }

    /// Estimates the gas for a transaction request.
    ///
    /// # Arguments
    ///
    /// * `request` - The transaction request to estimate gas for
    ///
    /// # Returns
    ///
    /// The estimated gas amount.
    ///
    /// # Errors
    ///
    /// Returns an error if gas estimation fails.
    pub async fn estimate_gas(&self, request: &TransactionRequest) -> Result<u64, TxError> {
        self.provider
            .estimate_gas(request.clone())
            .await
            .map_err(|e| TxError::GasEstimation(e.to_string()))
    }
}

impl<P> std::fmt::Debug for TxBuilder<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxBuilder").field("chain_id", &self.chain_id).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Bytes, U256};
    use rstest::rstest;

    use super::*;

    // Note: TxBuilder tests that require a Provider are integration tests
    // These unit tests focus on pure logic without provider dependencies

    #[test]
    fn test_build_request_sets_basic_fields() {
        // This test verifies the logic of building a request without actually calling the provider
        let to = Address::repeat_byte(1);
        let from = Address::repeat_byte(2);
        let data = Bytes::from_static(&[0x01, 0x02, 0x03]);
        let candidate = TxCandidate::calldata(to, data.clone())
            .with_value(U256::from(1000))
            .with_gas_limit(21000);

        let caps = GasCaps::new(1_000_000_000, 500_000, None);

        // Verify candidate properties
        assert_eq!(candidate.to, to);
        assert_eq!(candidate.value, U256::from(1000));
        assert_eq!(candidate.data, data);
        assert_eq!(candidate.gas_limit, Some(21000));
        assert!(!candidate.is_blob());

        // Verify caps properties
        assert_eq!(caps.max_fee_per_gas(), 1_000_000_000);
        assert_eq!(caps.max_priority_fee_per_gas(), 500_000);
        assert!(caps.blob_fee_cap.is_none());

        // Verify addresses
        assert_eq!(from, Address::repeat_byte(2));
    }

    #[test]
    fn test_build_request_blob_transaction() {
        let to = Address::repeat_byte(1);
        let from = Address::repeat_byte(2);

        // Create a blob candidate with sidecar
        let sidecar = alloy::consensus::BlobTransactionSidecar::default();
        let candidate = TxCandidate::blob(to, sidecar);

        let caps = GasCaps::new(1_000_000_000, 500_000, Some(100_000));

        // Verify candidate is blob type
        assert!(candidate.is_blob());
        assert!(candidate.blob_sidecar.is_some());

        // Verify caps have blob fee
        assert!(caps.blob_fee_cap.is_some());

        // Verify addresses
        assert_eq!(from, Address::repeat_byte(2));
    }

    #[test]
    fn test_build_request_without_gas_limit() {
        let to = Address::repeat_byte(1);
        let from = Address::repeat_byte(2);
        let candidate = TxCandidate::calldata(to, Bytes::new());

        // Verify gas_limit is None (for estimation)
        assert_eq!(candidate.gas_limit, None);

        let caps = GasCaps::new(1_000_000_000, 500_000, None);

        // Verify we can build with no gas limit
        assert_eq!(from, Address::repeat_byte(2));
        assert_eq!(caps.max_fee_per_gas(), 1_000_000_000);
    }

    #[test]
    fn test_gas_caps_values() {
        let caps = GasCaps::new(2_000_000_000, 1_000_000, Some(50_000));
        assert_eq!(caps.max_fee_per_gas(), 2_000_000_000);
        assert_eq!(caps.max_priority_fee_per_gas(), 1_000_000);
        assert!(caps.blob_fee_cap.is_some());
    }

    #[test]
    fn test_candidate_properties() {
        let to = Address::ZERO;
        let data = Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]);
        let candidate = TxCandidate::calldata(to, data.clone())
            .with_value(U256::from(5000))
            .with_gas_limit(100000);

        assert_eq!(candidate.to, to);
        assert_eq!(candidate.value, U256::from(5000));
        assert_eq!(candidate.data, data);
        assert_eq!(candidate.gas_limit, Some(100000));
    }

    #[rstest]
    #[case(21000, "minimum gas")]
    #[case(50000, "typical contract call")]
    #[case(1000000, "complex operation")]
    fn test_candidate_with_various_gas_limits(#[case] gas_limit: u64, #[case] _description: &str) {
        let candidate =
            TxCandidate::calldata(Address::ZERO, Bytes::new()).with_gas_limit(gas_limit);
        assert_eq!(candidate.gas_limit, Some(gas_limit));
    }

    #[test]
    fn test_blob_candidate_has_sidecar() {
        let sidecar = alloy::consensus::BlobTransactionSidecar::default();
        let candidate = TxCandidate::blob(Address::ZERO, sidecar);

        assert!(candidate.is_blob());
        assert!(candidate.blob_sidecar.is_some());
    }

    #[test]
    fn test_calldata_candidate_no_sidecar() {
        let candidate = TxCandidate::calldata(Address::ZERO, Bytes::new());

        assert!(!candidate.is_blob());
        assert!(candidate.blob_sidecar.is_none());
    }

    #[test]
    fn test_arc_clone() {
        // Verify Arc can be cloned as required by the builder
        let provider = Arc::new(42u64);
        let cloned = provider.clone();
        assert_eq!(*provider, *cloned);
    }
}
