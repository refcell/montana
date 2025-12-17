//! Error types for payload execution.

/// Error type for payload execution failures.
#[derive(Debug, thiserror::Error)]
pub enum ExecutePayloadError {
    /// The payload failed to execute.
    #[error("Payload execution failed: {0}")]
    ExecutionFailed(String),
    /// Channel send error.
    #[error("Channel send error: {0}")]
    ChannelSend(String),
}
