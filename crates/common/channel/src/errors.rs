//! Error types for payload execution.

/// Error type for payload execution failures.
#[derive(Debug, derive_more::Display)]
pub enum ExecutePayloadError {
    /// The payload failed to execute.
    #[display("Payload execution failed: {_0}")]
    ExecutionFailed(String),
    /// Channel send error.
    #[display("Channel send error: {_0}")]
    ChannelSend(String),
}

impl std::error::Error for ExecutePayloadError {}
