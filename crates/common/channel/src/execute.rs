//! Payload execution traits.

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
