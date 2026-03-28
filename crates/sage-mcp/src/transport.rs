//! Transport trait for MCP communication.

use async_trait::async_trait;

use crate::error::McpResult;
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};

/// Transport layer for MCP JSON-RPC communication.
///
/// Implementations handle the actual message delivery over stdio or HTTP.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC request and receive the response.
    ///
    /// For notifications (requests without an ID), the response may be
    /// a synthetic empty response.
    async fn send(&self, request: &JsonRpcRequest) -> McpResult<JsonRpcResponse>;

    /// Send a notification (no response expected).
    async fn notify(&self, request: &JsonRpcRequest) -> McpResult<()>;

    /// Close the transport connection.
    async fn close(&self) -> McpResult<()>;
}
