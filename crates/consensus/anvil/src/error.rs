//! Error types for the Anvil crate.

use montana_pipeline::{SinkError, SourceError};

/// Errors that can occur when working with Anvil.
#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    /// Failed to spawn Anvil process.
    #[error("Failed to spawn Anvil: {0}")]
    Spawn(String),

    /// Failed to connect to Anvil.
    #[error("Failed to connect to Anvil: {0}")]
    Connection(String),

    /// Transaction submission failed.
    #[error("Transaction failed: {0}")]
    Transaction(String),

    /// Failed to get transaction receipt.
    #[error("Failed to get receipt: {0}")]
    Receipt(String),

    /// Block not found.
    #[error("Block not found: {0}")]
    BlockNotFound(u64),

    /// Provider error.
    #[error("Provider error: {0}")]
    Provider(String),
}

impl From<AnvilError> for SinkError {
    fn from(err: AnvilError) -> Self {
        match err {
            AnvilError::Spawn(msg) | AnvilError::Connection(msg) => Self::Connection(msg),
            AnvilError::Transaction(msg) | AnvilError::Receipt(msg) => Self::TxFailed(msg),
            AnvilError::BlockNotFound(n) => Self::TxFailed(format!("Block not found: {}", n)),
            AnvilError::Provider(msg) => Self::TxFailed(msg),
        }
    }
}

impl From<AnvilError> for SourceError {
    fn from(err: AnvilError) -> Self {
        match err {
            AnvilError::Spawn(msg)
            | AnvilError::Connection(msg)
            | AnvilError::Provider(msg)
            | AnvilError::Transaction(msg)
            | AnvilError::Receipt(msg) => Self::Connection(msg),
            AnvilError::BlockNotFound(_) => Self::Empty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anvil_error_spawn_display() {
        let err = AnvilError::Spawn("command not found".into());
        assert_eq!(err.to_string(), "Failed to spawn Anvil: command not found");
    }

    #[test]
    fn anvil_error_connection_display() {
        let err = AnvilError::Connection("connection refused".into());
        assert_eq!(err.to_string(), "Failed to connect to Anvil: connection refused");
    }

    #[test]
    fn anvil_error_transaction_display() {
        let err = AnvilError::Transaction("nonce too low".into());
        assert_eq!(err.to_string(), "Transaction failed: nonce too low");
    }

    #[test]
    fn anvil_error_to_sink_error() {
        let err = AnvilError::Transaction("failed".into());
        let sink_err: SinkError = err.into();
        assert!(matches!(sink_err, SinkError::TxFailed(_)));
    }

    #[test]
    fn anvil_error_to_source_error() {
        let err = AnvilError::BlockNotFound(100);
        let source_err: SourceError = err.into();
        assert!(matches!(source_err, SourceError::Empty));
    }
}
