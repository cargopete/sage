//! Error types for the Sage runtime.
//!
//! RFC-0007: This module provides the `SageError` type and `ErrorKind` enum
//! that are exposed to Sage programs through the `Error` type.

use thiserror::Error;

/// Result type for Sage operations.
pub type SageResult<T> = Result<T, SageError>;

/// RFC-0007: Error kind classification for Sage errors.
///
/// This enum is exposed to Sage programs as `ErrorKind` and can be matched
/// in `on error(e)` handlers via `e.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// Error from LLM inference (network, parsing, rate limits).
    Llm,
    /// Error from agent execution (panics, message failures).
    Agent,
    /// Runtime errors (type mismatches, I/O, etc.).
    Runtime,
    /// RFC-0011: Error from tool execution (Http, Fs, etc.).
    Tool,
    /// User-raised error via `fail` expression.
    User,
    /// Phase 3: Protocol violation in session types.
    Protocol,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Llm => write!(f, "Llm"),
            ErrorKind::Agent => write!(f, "Agent"),
            ErrorKind::Runtime => write!(f, "Runtime"),
            ErrorKind::Tool => write!(f, "Tool"),
            ErrorKind::User => write!(f, "User"),
            ErrorKind::Protocol => write!(f, "Protocol"),
        }
    }
}

/// Error type for Sage runtime errors.
///
/// RFC-0007: This is exposed to Sage programs as the `Error` type with
/// `.message` and `.kind` field accessors.
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

    /// RFC-0011: Error from tool execution.
    #[error("Tool error: {0}")]
    Tool(String),

    /// I/O error (file operations, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// User-raised error via `fail` expression.
    #[error("{0}")]
    User(String),

    /// Error from supervisor (restart intensity exceeded, etc.).
    #[error("Supervisor error: {0}")]
    Supervisor(String),

    /// Phase 3: Protocol violation in session types.
    #[error("Protocol error: {0}")]
    Protocol(String),
}

impl SageError {
    /// RFC-0007: Get the error message as a String.
    ///
    /// This is exposed to Sage programs as `e.message`.
    #[must_use]
    pub fn message(&self) -> String {
        self.to_string()
    }

    /// RFC-0007: Get the error kind classification.
    ///
    /// This is exposed to Sage programs as `e.kind`.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        match self {
            SageError::Llm(_) | SageError::Json(_) => ErrorKind::Llm,
            SageError::Agent(_) | SageError::JoinError(_) | SageError::Supervisor(_) => {
                ErrorKind::Agent
            }
            SageError::Type { .. } => ErrorKind::Runtime,
            // RFC-0011: Http, Io, and Tool errors are tool errors
            SageError::Http(_) | SageError::Tool(_) | SageError::Io(_) => ErrorKind::Tool,
            SageError::User(_) => ErrorKind::User,
            // Phase 3: Protocol errors
            SageError::Protocol(_) => ErrorKind::Protocol,
        }
    }

    /// Create an LLM error with a message.
    #[must_use]
    pub fn llm(msg: impl Into<String>) -> Self {
        SageError::Llm(msg.into())
    }

    /// Create an agent error with a message.
    #[must_use]
    pub fn agent(msg: impl Into<String>) -> Self {
        SageError::Agent(msg.into())
    }

    /// Create a type error.
    #[must_use]
    pub fn type_error(expected: impl Into<String>, got: impl Into<String>) -> Self {
        SageError::Type {
            expected: expected.into(),
            got: got.into(),
        }
    }

    /// RFC-0011: Create a tool error with a message.
    #[must_use]
    pub fn tool(msg: impl Into<String>) -> Self {
        SageError::Tool(msg.into())
    }

    /// Create a user error via `fail` expression.
    #[must_use]
    pub fn user(msg: impl Into<String>) -> Self {
        SageError::User(msg.into())
    }

    /// Phase 3: Create a protocol violation error.
    #[must_use]
    pub fn protocol(msg: impl Into<String>) -> Self {
        SageError::Protocol(msg.into())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<tokio::task::JoinError> for SageError {
    fn from(e: tokio::task::JoinError) -> Self {
        SageError::JoinError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_kind_classification() {
        assert_eq!(SageError::llm("test").kind(), ErrorKind::Llm);
        assert_eq!(SageError::agent("test").kind(), ErrorKind::Agent);
        assert_eq!(
            SageError::type_error("Int", "String").kind(),
            ErrorKind::Runtime
        );
    }

    #[test]
    fn error_message() {
        let err = SageError::llm("inference failed");
        assert_eq!(err.message(), "LLM error: inference failed");
    }

    #[test]
    fn error_kind_display() {
        assert_eq!(format!("{}", ErrorKind::Llm), "Llm");
        assert_eq!(format!("{}", ErrorKind::Agent), "Agent");
        assert_eq!(format!("{}", ErrorKind::Runtime), "Runtime");
        assert_eq!(format!("{}", ErrorKind::Tool), "Tool");
        assert_eq!(format!("{}", ErrorKind::Protocol), "Protocol");
    }

    #[test]
    fn tool_error_classification() {
        assert_eq!(SageError::tool("http failed").kind(), ErrorKind::Tool);
        assert_eq!(SageError::tool("timeout").message(), "Tool error: timeout");
    }

    #[test]
    fn protocol_error_classification() {
        assert_eq!(
            SageError::protocol("unexpected message").kind(),
            ErrorKind::Protocol
        );
        assert_eq!(
            SageError::protocol("wrong sender").message(),
            "Protocol error: wrong sender"
        );
    }
}
