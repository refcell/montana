//! Local error types.

/// Errors from local file operations.
#[derive(Debug, thiserror::Error)]
pub enum LocalError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parse error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// File not found.
    #[error("File not found: {0}")]
    NotFound(String),
}
