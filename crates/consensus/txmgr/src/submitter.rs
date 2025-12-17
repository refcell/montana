//! Transaction submission and retry logic.

use std::sync::Arc;

use alloy::{primitives::B256, providers::Provider};

use crate::{config::TxManagerConfig, error::TxError, state::SendState};

/// Transaction submitter for publishing transactions to the mempool.
pub struct TxSubmitter<P> {
    /// RPC provider for transaction submission.
    provider: Arc<P>,
    /// Transaction manager configuration.
    config: TxManagerConfig,
}

impl<P> TxSubmitter<P> {
    /// Creates a new transaction submitter.
    ///
    /// # Arguments
    ///
    /// * `provider` - RPC provider for transaction submission
    /// * `config` - Transaction manager configuration
    pub const fn new(provider: Arc<P>, config: TxManagerConfig) -> Self {
        Self { provider, config }
    }

    /// Classifies an RPC error string into the appropriate TxError variant.
    ///
    /// # Arguments
    ///
    /// * `error` - The RPC error message to classify
    ///
    /// # Returns
    ///
    /// The appropriate TxError variant based on the error message content
    pub fn classify_error(&self, error: &str) -> TxError {
        TxError::from_rpc_error(error)
    }

    /// Determines if we should abort retrying based on the error and state.
    ///
    /// # Arguments
    ///
    /// * `error` - The error that occurred
    /// * `state` - Current send state tracking retry attempts
    ///
    /// # Returns
    ///
    /// true if we should give up and abort, false if we should retry
    pub const fn should_abort(&self, error: &TxError, state: &SendState) -> bool {
        match error {
            // Always abort on these errors
            TxError::AlreadyReserved | TxError::InsufficientFunds => true,

            // Abort if we've exceeded the nonce too low threshold
            TxError::NonceTooLowAbort => true,

            // Check threshold for nonce too low
            TxError::NonceTooLow => {
                state.should_abort_nonce(self.config.safe_abort_nonce_too_low_count)
            }

            // All other errors are retryable
            _ => false,
        }
    }
}

impl<P> TxSubmitter<P>
where
    P: Provider + Clone + Send + Sync,
{
    /// Publishes a raw transaction to the mempool.
    ///
    /// # Arguments
    ///
    /// * `raw_tx` - Raw transaction bytes to publish
    ///
    /// # Returns
    ///
    /// Transaction hash on success, or TxError on failure
    ///
    /// # Errors
    ///
    /// Returns a TxError if:
    /// - The RPC call fails
    /// - The transaction is rejected by the node
    /// - The error is classified as non-retryable
    pub async fn publish(&self, raw_tx: &[u8]) -> Result<B256, TxError> {
        let pending_tx = self.provider.send_raw_transaction(raw_tx).await.map_err(|e| {
            let error_msg = e.to_string();
            // Handle "already known" as success - transaction is already in mempool
            if error_msg.to_lowercase().contains("already known") {
                return TxError::AlreadyReserved;
            }
            TxError::from_rpc_error(&error_msg)
        })?;

        Ok(*pending_tx.tx_hash())
    }
}

impl<P> std::fmt::Debug for TxSubmitter<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TxSubmitter").field("config", &self.config).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_classify_error_underpriced() {
        let submitter = create_test_submitter();

        let err = submitter.classify_error("transaction underpriced");
        assert!(matches!(err, TxError::Underpriced));

        let err = submitter.classify_error("replacement transaction underpriced");
        assert!(matches!(err, TxError::Underpriced));
    }

    #[test]
    fn test_classify_error_nonce_too_low() {
        let submitter = create_test_submitter();

        let err = submitter.classify_error("nonce too low");
        assert!(matches!(err, TxError::NonceTooLow));

        let err = submitter.classify_error("Nonce too low");
        assert!(matches!(err, TxError::NonceTooLow));
    }

    #[test]
    fn test_classify_error_insufficient_funds() {
        let submitter = create_test_submitter();

        let err = submitter.classify_error("insufficient funds");
        assert!(matches!(err, TxError::InsufficientFunds));

        let err = submitter.classify_error("insufficient balance");
        assert!(matches!(err, TxError::InsufficientFunds));
    }

    #[test]
    fn test_classify_error_reverted() {
        let submitter = create_test_submitter();

        let err = submitter.classify_error("transaction reverted");
        assert!(matches!(err, TxError::Reverted));

        let err = submitter.classify_error("execution reverted");
        assert!(matches!(err, TxError::Reverted));
    }

    #[test]
    fn test_classify_error_already_reserved() {
        let submitter = create_test_submitter();

        let err = submitter.classify_error("already known");
        assert!(matches!(err, TxError::AlreadyReserved));

        let err = submitter.classify_error("already reserved");
        assert!(matches!(err, TxError::AlreadyReserved));
    }

    #[test]
    fn test_classify_error_generic_rpc() {
        let submitter = create_test_submitter();

        let err = submitter.classify_error("connection timeout");
        match err {
            TxError::Rpc(msg) => assert_eq!(msg, "connection timeout"),
            _ => panic!("Expected TxError::Rpc variant"),
        }
    }

    #[rstest]
    #[case(TxError::AlreadyReserved, SendState::new(), true, "always abort")]
    #[case(TxError::InsufficientFunds, SendState::new(), true, "always abort")]
    #[case(TxError::NonceTooLowAbort, SendState::new(), true, "always abort")]
    fn test_should_abort_always_abort_errors(
        #[case] error: TxError,
        #[case] state: SendState,
        #[case] expected: bool,
        #[case] _description: &str,
    ) {
        let submitter = create_test_submitter();
        assert_eq!(submitter.should_abort(&error, &state), expected);
    }

    #[test]
    fn test_should_abort_nonce_too_low_below_threshold() {
        let submitter = create_test_submitter();
        let mut state = SendState::new();

        // First nonce too low - should not abort
        state.nonce_too_low();
        assert!(!submitter.should_abort(&TxError::NonceTooLow, &state));

        // Second nonce too low - should not abort
        state.nonce_too_low();
        assert!(!submitter.should_abort(&TxError::NonceTooLow, &state));
    }

    #[test]
    fn test_should_abort_nonce_too_low_at_threshold() {
        let submitter = create_test_submitter();
        let mut state = SendState::new();

        // Hit the threshold (default is 3)
        state.nonce_too_low();
        state.nonce_too_low();
        state.nonce_too_low();

        assert!(submitter.should_abort(&TxError::NonceTooLow, &state));
    }

    #[test]
    fn test_should_abort_nonce_too_low_above_threshold() {
        let submitter = create_test_submitter();
        let mut state = SendState::new();

        // Exceed the threshold
        state.nonce_too_low();
        state.nonce_too_low();
        state.nonce_too_low();
        state.nonce_too_low();

        assert!(submitter.should_abort(&TxError::NonceTooLow, &state));
    }

    #[rstest]
    #[case(TxError::Rpc("test".to_string()), false, "retryable RPC error")]
    #[case(TxError::Underpriced, false, "retryable underpriced")]
    #[case(TxError::NotInMempool, false, "retryable not in mempool")]
    #[case(TxError::BlobGasTooExpensive { current: 100, max: 50 }, false, "retryable blob gas")]
    #[case(TxError::ConfirmationTimeout, false, "retryable timeout")]
    #[case(TxError::Reverted, false, "retryable reverted")]
    #[case(TxError::Signing("test".to_string()), false, "retryable signing error")]
    #[case(TxError::GasEstimation("test".to_string()), false, "retryable gas estimation")]
    fn test_should_abort_retryable_errors(
        #[case] error: TxError,
        #[case] expected: bool,
        #[case] _description: &str,
    ) {
        let submitter = create_test_submitter();
        let state = SendState::new();
        assert_eq!(submitter.should_abort(&error, &state), expected);
    }

    #[test]
    fn test_should_abort_with_custom_threshold() {
        let config = TxManagerConfig::builder().safe_abort_nonce_too_low_count(5).build();
        let submitter = TxSubmitter::new(Arc::new(MockProvider), config);
        let mut state = SendState::new();

        // Add 4 nonce too low errors - should not abort
        for _ in 0..4 {
            state.nonce_too_low();
        }
        assert!(!submitter.should_abort(&TxError::NonceTooLow, &state));

        // Add 5th error - should abort
        state.nonce_too_low();
        assert!(submitter.should_abort(&TxError::NonceTooLow, &state));
    }

    #[test]
    fn test_debug_impl() {
        let submitter = create_test_submitter();
        let debug_str = format!("{:?}", submitter);
        assert!(debug_str.contains("TxSubmitter"));
        assert!(debug_str.contains("config"));
    }

    #[test]
    fn test_classify_error_case_insensitive() {
        let submitter = create_test_submitter();

        let lower = submitter.classify_error("nonce too low");
        let upper = submitter.classify_error("NONCE TOO LOW");
        let mixed = submitter.classify_error("Nonce Too Low");

        assert!(matches!(lower, TxError::NonceTooLow));
        assert!(matches!(upper, TxError::NonceTooLow));
        assert!(matches!(mixed, TxError::NonceTooLow));
    }

    #[test]
    fn test_classify_error_with_context() {
        let submitter = create_test_submitter();

        let err = submitter
            .classify_error("failed to send transaction: replacement transaction underpriced");
        assert!(matches!(err, TxError::Underpriced));
    }

    #[test]
    fn test_should_abort_state_independence() {
        let submitter = create_test_submitter();
        let state = SendState::new();

        // These errors should abort regardless of state
        assert!(submitter.should_abort(&TxError::AlreadyReserved, &state));
        assert!(submitter.should_abort(&TxError::InsufficientFunds, &state));
        assert!(submitter.should_abort(&TxError::NonceTooLowAbort, &state));
    }

    #[rstest]
    #[case(0, false, "no nonce too low errors")]
    #[case(1, false, "one nonce too low error")]
    #[case(2, false, "two nonce too low errors")]
    #[case(3, true, "at threshold")]
    #[case(5, true, "above threshold")]
    fn test_should_abort_nonce_too_low_threshold_variants(
        #[case] count: u32,
        #[case] expected: bool,
        #[case] _description: &str,
    ) {
        let submitter = create_test_submitter();
        let mut state = SendState::new();

        for _ in 0..count {
            state.nonce_too_low();
        }

        assert_eq!(submitter.should_abort(&TxError::NonceTooLow, &state), expected);
    }

    #[test]
    fn test_multiple_error_types_with_same_state() {
        let submitter = create_test_submitter();
        let mut state = SendState::new();

        // Add some nonce too low errors
        state.nonce_too_low();
        state.nonce_too_low();

        // Non-abort errors should still not abort
        assert!(!submitter.should_abort(&TxError::Underpriced, &state));
        assert!(!submitter.should_abort(&TxError::Rpc("error".to_string()), &state));

        // Abort errors should still abort
        assert!(submitter.should_abort(&TxError::InsufficientFunds, &state));
        assert!(submitter.should_abort(&TxError::AlreadyReserved, &state));
    }

    #[test]
    fn test_new_submitter() {
        let config = TxManagerConfig::default();
        let submitter = TxSubmitter::new(Arc::new(MockProvider), config.clone());
        assert_eq!(
            submitter.config.safe_abort_nonce_too_low_count,
            config.safe_abort_nonce_too_low_count
        );
    }

    #[test]
    fn test_classify_empty_error() {
        let submitter = create_test_submitter();
        let err = submitter.classify_error("");
        match err {
            TxError::Rpc(msg) => assert_eq!(msg, ""),
            _ => panic!("Expected TxError::Rpc variant"),
        }
    }

    #[test]
    fn test_should_abort_with_published_state() {
        let submitter = create_test_submitter();
        let mut state = SendState::new();

        // Publish transaction
        state.tx_published();

        // Nonce too low count still matters
        state.nonce_too_low();
        assert!(!submitter.should_abort(&TxError::NonceTooLow, &state));

        state.nonce_too_low();
        state.nonce_too_low();
        assert!(submitter.should_abort(&TxError::NonceTooLow, &state));
    }

    #[test]
    fn test_should_abort_with_mined_transaction() {
        let submitter = create_test_submitter();
        let mut state = SendState::new();

        // Mine a transaction
        let hash = B256::from([1u8; 32]);
        state.tx_mined(hash);

        // Abort errors should still abort
        assert!(submitter.should_abort(&TxError::InsufficientFunds, &state));
        assert!(submitter.should_abort(&TxError::AlreadyReserved, &state));

        // Retryable errors should not abort
        assert!(!submitter.should_abort(&TxError::Underpriced, &state));
    }

    // Helper function to create a test submitter
    fn create_test_submitter() -> TxSubmitter<MockProvider> {
        let config = TxManagerConfig::default();
        TxSubmitter::new(Arc::new(MockProvider), config)
    }

    // Mock provider for testing
    #[derive(Clone)]
    struct MockProvider;

    impl std::fmt::Debug for MockProvider {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("MockProvider").finish()
        }
    }
}
