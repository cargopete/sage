//! Error types for the Sage runtime.

use thiserror::Error;

/// Result type for Sage operations.
pub type SageResult<T> = Result<T, SageError>;

/// Error type for Sage runtime errors.
#[derive(Debug, Error)]
pub enum SageError {
    /// Error from LLM inference.
    #[error("LLM error: {0}")]
    Llm(String),

    /// Error from agent execution.
    #[error("Agent error: {0}")]
    Agent(String),

    /// Type mismatch at runtime.
    #[error("Type error: expected {expected}, got {got}")]
    Type { expected: String, got: String },

    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON parsing error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Agent task was cancelled or panicked.
    #[error("Agent task failed: {0}")]
    JoinError(String),
}

impl From<tokio::task::JoinError> for SageError {
    fn from(e: tokio::task::JoinError) -> Self {
        SageError::JoinError(e.to_string())
    }
}
