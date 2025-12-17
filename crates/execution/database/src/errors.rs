//! Error types for database operations

use derive_more::Display;
use revm::database_interface::DBErrorMarker;

/// Error type for database operations
#[derive(Debug, Display)]
pub struct DbError(pub String);

impl DbError {
    /// Create a new database error with the given message
    #[must_use]
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl std::error::Error for DbError {}

impl DBErrorMarker for DbError {}
