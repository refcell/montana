//! Payload execution traits.

use std::marker::PhantomData;

use tokio::sync::mpsc::Sender;

use crate::errors::ExecutePayloadError;

/// Trait for executing derived block payloads.
///
/// Implementations define how a validator should execute
/// blocks derived from the consensus layer.
pub trait ExecutePayload {
    /// The payload type to execute.
    type Payload;

    /// Execute the given payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload fails to execute.
    fn execute(&self, payload: Self::Payload) -> Result<(), ExecutePayloadError>;
}

/// A no-op executor that does nothing with payloads.
///
/// This is useful for testing or when payload execution
/// is not yet implemented.
///
/// The type parameter `P` specifies what payload type this executor accepts.
#[derive(Debug, Clone, Copy)]
pub struct NoopExecutor<P = Vec<u8>> {
    _marker: PhantomData<P>,
}

impl<P> Default for NoopExecutor<P> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P> NoopExecutor<P> {
    /// Creates a new no-op executor.
    pub const fn new() -> Self {
        Self { _marker: PhantomData }
    }
}

impl<P> ExecutePayload for NoopExecutor<P> {
    type Payload = P;

    fn execute(&self, _payload: Self::Payload) -> Result<(), ExecutePayloadError> {
        Ok(())
    }
}

/// Executor that sends payloads to a channel.
///
/// This is used in validator mode to bridge the derivation runner
/// output to the execution service input. The payload bytes are
/// sent directly to a channel receiver.
pub struct ChannelExecutor {
    tx: Sender<Vec<u8>>,
}

impl ChannelExecutor {
    /// Create a new channel executor.
    pub const fn new(tx: Sender<Vec<u8>>) -> Self {
        Self { tx }
    }

    /// Get a reference to the sender.
    pub const fn sender(&self) -> &Sender<Vec<u8>> {
        &self.tx
    }
}

impl std::fmt::Debug for ChannelExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelExecutor").finish()
    }
}

impl ExecutePayload for ChannelExecutor {
    type Payload = Vec<u8>;

    fn execute(&self, payload: Self::Payload) -> Result<(), ExecutePayloadError> {
        // Use try_send for non-blocking send in sync context
        self.tx.try_send(payload).map_err(|e| {
            ExecutePayloadError::ChannelSend(format!("Failed to send payload to channel: {}", e))
        })
    }
}
