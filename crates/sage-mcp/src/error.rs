//! Error types for MCP client operations.

use thiserror::Error;

/// Errors that can occur during MCP client operations.
#[derive(Debug, Error)]
pub enum McpError {
    /// Failed to connect to the MCP server.
    #[error("MCP connection failed: {0}")]
    Connection(String),

    /// MCP protocol error (invalid JSON-RPC, version mismatch, etc.).
    #[error("MCP protocol error: {0}")]
    Protocol(String),

    /// The requested tool was not found on the server.
    #[error("MCP tool not found: {0}")]
    ToolNotFound(String),

    /// The tool call returned an error from the server.
    #[error("MCP tool error: {0}")]
    ToolExecution(String),

    /// Failed to deserialise the tool result into the expected type.
    #[error("MCP deserialisation error: {0}")]
    Deserialisation(String),

    /// Authentication failed.
    #[error("MCP auth error: {0}")]
    Auth(String),

    /// The tool call timed out.
    #[error("MCP timeout: {0}")]
    Timeout(String),

    /// I/O error (subprocess, network, etc.).
    #[error("MCP I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP error from reqwest.
    #[error("MCP HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialisation/deserialisation error.
    #[error("MCP JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The server process exited unexpectedly.
    #[error("MCP server exited: {0}")]
    ServerExited(String),
}

/// Result type for MCP operations.
pub type McpResult<T> = Result<T, McpError>;
