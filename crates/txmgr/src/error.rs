//! Transaction manager error types.

use thiserror::Error;

/// Transaction manager errors.
#[derive(Debug, Clone, Error)]
pub enum TxError {
    /// RPC error.
    #[error("RPC error: {0}")]
    Rpc(String),

    /// Transaction underpriced.
    #[error("Transaction underpriced")]
    Underpriced,

    /// Nonce too low.
    #[error("Nonce too low")]
    NonceTooLow,

    /// Nonce too low count exceeded threshold.
    #[error("Nonce too low count exceeded threshold")]
    NonceTooLowAbort,

    /// Transaction not in mempool after timeout.
    #[error("Transaction not in mempool after timeout")]
    NotInMempool,

    /// Transaction type conflict: already reserved.
    #[error("Transaction type conflict: already reserved")]
    AlreadyReserved,

    /// Blob gas too expensive.
    #[error("Blob gas too expensive: {current} > {max}")]
    BlobGasTooExpensive {
        /// Current blob gas price.
        current: u128,
        /// Maximum blob gas price.
        max: u128,
    },

    /// Insufficient funds.
    #[error("Insufficient funds")]
    InsufficientFunds,

    /// Timeout waiting for confirmation.
    #[error("Timeout waiting for confirmation")]
    ConfirmationTimeout,

    /// Transaction reverted.
    #[error("Transaction reverted")]
    Reverted,

    /// Signing error.
    #[error("Signing error: {0}")]
    Signing(String),

    /// Gas estimation failed.
    #[error("Gas estimation failed: {0}")]
    GasEstimation(String),
}

impl TxError {
    /// Classifies an RPC error string into the appropriate TxError variant.
    ///
    /// # Arguments
    ///
    /// * `msg` - The RPC error message to classify
    ///
    /// # Returns
    ///
    /// The appropriate TxError variant based on the error message content
    pub fn from_rpc_error(msg: &str) -> Self {
        let lower = msg.to_lowercase();

        if lower.contains("underpriced") || lower.contains("replacement transaction underpriced") {
            Self::Underpriced
        } else if lower.contains("nonce too low") {
            Self::NonceTooLow
        } else if lower.contains("insufficient funds") || lower.contains("insufficient balance") {
            Self::InsufficientFunds
        } else if lower.contains("transaction reverted")
            || lower.contains("execution reverted")
            || lower.contains("revert")
        {
            Self::Reverted
        } else if lower.contains("already known") || lower.contains("already reserved") {
            Self::AlreadyReserved
        } else {
            Self::Rpc(msg.to_string())
        }
    }
}

/// Trait for determining if an error is retryable.
pub trait Retryable {
    /// Returns true if the error is retryable.
    fn is_retryable(&self) -> bool;
}

impl Retryable for TxError {
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Rpc(_) | Self::Underpriced | Self::NonceTooLow)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(TxError::Rpc("test error".to_string()), true)]
    #[case(TxError::Underpriced, true)]
    #[case(TxError::NonceTooLow, true)]
    #[case(TxError::NonceTooLowAbort, false)]
    #[case(TxError::NotInMempool, false)]
    #[case(TxError::AlreadyReserved, false)]
    #[case(TxError::BlobGasTooExpensive { current: 100, max: 50 }, false)]
    #[case(TxError::InsufficientFunds, false)]
    #[case(TxError::ConfirmationTimeout, false)]
    #[case(TxError::Reverted, false)]
    #[case(TxError::Signing("test".to_string()), false)]
    #[case(TxError::GasEstimation("test".to_string()), false)]
    fn test_is_retryable(#[case] error: TxError, #[case] expected: bool) {
        assert_eq!(error.is_retryable(), expected);
    }

    #[test]
    fn test_rpc_error_display() {
        let err = TxError::Rpc("connection timeout".to_string());
        assert_eq!(err.to_string(), "RPC error: connection timeout");
    }

    #[test]
    fn test_underpriced_display() {
        let err = TxError::Underpriced;
        assert_eq!(err.to_string(), "Transaction underpriced");
    }

    #[test]
    fn test_nonce_too_low_display() {
        let err = TxError::NonceTooLow;
        assert_eq!(err.to_string(), "Nonce too low");
    }

    #[test]
    fn test_nonce_too_low_abort_display() {
        let err = TxError::NonceTooLowAbort;
        assert_eq!(err.to_string(), "Nonce too low count exceeded threshold");
    }

    #[test]
    fn test_not_in_mempool_display() {
        let err = TxError::NotInMempool;
        assert_eq!(err.to_string(), "Transaction not in mempool after timeout");
    }

    #[test]
    fn test_already_reserved_display() {
        let err = TxError::AlreadyReserved;
        assert_eq!(err.to_string(), "Transaction type conflict: already reserved");
    }

    #[test]
    fn test_blob_gas_too_expensive_display() {
        let err = TxError::BlobGasTooExpensive { current: 100, max: 50 };
        assert_eq!(err.to_string(), "Blob gas too expensive: 100 > 50");
    }

    #[test]
    fn test_insufficient_funds_display() {
        let err = TxError::InsufficientFunds;
        assert_eq!(err.to_string(), "Insufficient funds");
    }

    #[test]
    fn test_confirmation_timeout_display() {
        let err = TxError::ConfirmationTimeout;
        assert_eq!(err.to_string(), "Timeout waiting for confirmation");
    }

    #[test]
    fn test_reverted_display() {
        let err = TxError::Reverted;
        assert_eq!(err.to_string(), "Transaction reverted");
    }

    #[test]
    fn test_signing_error_display() {
        let err = TxError::Signing("invalid key".to_string());
        assert_eq!(err.to_string(), "Signing error: invalid key");
    }

    #[test]
    fn test_gas_estimation_error_display() {
        let err = TxError::GasEstimation("estimation failed".to_string());
        assert_eq!(err.to_string(), "Gas estimation failed: estimation failed");
    }

    #[rstest]
    #[case(TxError::Rpc("test".to_string()))]
    #[case(TxError::Underpriced)]
    #[case(TxError::NonceTooLow)]
    #[case(TxError::NonceTooLowAbort)]
    #[case(TxError::NotInMempool)]
    #[case(TxError::AlreadyReserved)]
    #[case(TxError::BlobGasTooExpensive { current: 100, max: 50 })]
    #[case(TxError::InsufficientFunds)]
    #[case(TxError::ConfirmationTimeout)]
    #[case(TxError::Reverted)]
    #[case(TxError::Signing("test".to_string()))]
    #[case(TxError::GasEstimation("test".to_string()))]
    fn test_error_variants_are_debug(#[case] err: TxError) {
        let _ = format!("{:?}", err);
    }

    #[test]
    fn test_error_is_clone() {
        let err = TxError::Rpc("test".to_string());
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[test]
    fn test_blob_gas_too_expensive_clone() {
        let err = TxError::BlobGasTooExpensive { current: 200, max: 100 };
        let cloned = err.clone();
        assert_eq!(err.to_string(), cloned.to_string());
    }

    #[rstest]
    #[case("transaction underpriced", TxError::Underpriced)]
    #[case("replacement transaction underpriced", TxError::Underpriced)]
    #[case("Transaction underpriced", TxError::Underpriced)]
    fn test_from_rpc_error_underpriced(#[case] msg: &str, #[case] expected: TxError) {
        let err = TxError::from_rpc_error(msg);
        assert_eq!(err.to_string(), expected.to_string());
    }

    #[rstest]
    #[case("nonce too low", TxError::NonceTooLow)]
    #[case("Nonce too low", TxError::NonceTooLow)]
    #[case("NONCE TOO LOW", TxError::NonceTooLow)]
    fn test_from_rpc_error_nonce_too_low(#[case] msg: &str, #[case] expected: TxError) {
        let err = TxError::from_rpc_error(msg);
        assert_eq!(err.to_string(), expected.to_string());
    }

    #[rstest]
    #[case("insufficient funds", TxError::InsufficientFunds)]
    #[case("Insufficient funds for gas * price + value", TxError::InsufficientFunds)]
    #[case("insufficient balance", TxError::InsufficientFunds)]
    fn test_from_rpc_error_insufficient_funds(#[case] msg: &str, #[case] expected: TxError) {
        let err = TxError::from_rpc_error(msg);
        assert_eq!(err.to_string(), expected.to_string());
    }

    #[rstest]
    #[case("transaction reverted", TxError::Reverted)]
    #[case("execution reverted", TxError::Reverted)]
    #[case("revert", TxError::Reverted)]
    #[case("Transaction reverted without a reason", TxError::Reverted)]
    fn test_from_rpc_error_reverted(#[case] msg: &str, #[case] expected: TxError) {
        let err = TxError::from_rpc_error(msg);
        assert_eq!(err.to_string(), expected.to_string());
    }

    #[rstest]
    #[case("already known", TxError::AlreadyReserved)]
    #[case("already reserved", TxError::AlreadyReserved)]
    fn test_from_rpc_error_already_reserved(#[case] msg: &str, #[case] expected: TxError) {
        let err = TxError::from_rpc_error(msg);
        assert_eq!(err.to_string(), expected.to_string());
    }

    #[rstest]
    #[case("unknown error")]
    #[case("connection timeout")]
    #[case("network unreachable")]
    fn test_from_rpc_error_generic(#[case] msg: &str) {
        let err = TxError::from_rpc_error(msg);
        match err {
            TxError::Rpc(s) => assert_eq!(s, msg),
            _ => panic!("Expected TxError::Rpc variant"),
        }
    }

    #[test]
    fn test_from_rpc_error_case_insensitive() {
        let lower = TxError::from_rpc_error("nonce too low");
        let upper = TxError::from_rpc_error("NONCE TOO LOW");
        let mixed = TxError::from_rpc_error("Nonce Too Low");

        assert!(matches!(lower, TxError::NonceTooLow));
        assert!(matches!(upper, TxError::NonceTooLow));
        assert!(matches!(mixed, TxError::NonceTooLow));
    }

    #[test]
    fn test_from_rpc_error_empty_string() {
        let err = TxError::from_rpc_error("");
        match err {
            TxError::Rpc(s) => assert_eq!(s, ""),
            _ => panic!("Expected TxError::Rpc variant"),
        }
    }

    #[test]
    fn test_from_rpc_error_with_context() {
        let err = TxError::from_rpc_error(
            "failed to send transaction: replacement transaction underpriced",
        );
        assert!(matches!(err, TxError::Underpriced));
    }

    #[test]
    fn test_retryable_trait_rpc_generic() {
        let err = TxError::Rpc("connection refused".to_string());
        assert!(err.is_retryable());
    }

    #[test]
    fn test_retryable_trait_non_retryable_errors() {
        let errors = vec![
            TxError::InsufficientFunds,
            TxError::Reverted,
            TxError::NonceTooLowAbort,
            TxError::NotInMempool,
            TxError::AlreadyReserved,
            TxError::ConfirmationTimeout,
            TxError::Signing("error".to_string()),
            TxError::GasEstimation("error".to_string()),
            TxError::BlobGasTooExpensive { current: 100, max: 50 },
        ];

        for err in errors {
            assert!(!err.is_retryable(), "Expected {} to not be retryable", err);
        }
    }

    #[test]
    fn test_blob_gas_too_expensive_fields() {
        let err = TxError::BlobGasTooExpensive { current: 150, max: 100 };

        if let TxError::BlobGasTooExpensive { current, max } = err {
            assert_eq!(current, 150);
            assert_eq!(max, 100);
        } else {
            panic!("Expected BlobGasTooExpensive variant");
        }
    }
}
