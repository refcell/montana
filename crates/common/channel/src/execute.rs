//! Payload execution traits.

use std::marker::PhantomData;

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
