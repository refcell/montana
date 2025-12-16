//! Error types for database operations

use std::fmt;

use revm::database_interface::DBErrorMarker;

/// Error type for database operations
#[derive(Debug)]
pub struct DbError(pub String);

impl DbError {
    /// Create a new database error with the given message
    #[must_use]
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for DbError {}

impl DBErrorMarker for DbError {}
