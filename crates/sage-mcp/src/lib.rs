//! MCP (Model Context Protocol) client for Sage agents.
//!
//! This crate implements the MCP client protocol (spec 2025-03-26) for
//! connecting Sage agents to external tool servers. It supports:
//!
//! - **stdio transport**: Launch MCP servers as subprocesses
//! - **Streamable HTTP transport**: Connect to remote MCP servers
//! - **Bearer token auth**: For authenticated HTTP endpoints
//! - **Connection pooling**: Lazy, shared connections across agents
//! - **Typed tool clients**: Per-tool wrappers with name mapping
//!
//! # Architecture
//!
//! ```text
//! McpToolClient  ←  what generated agent code uses
//!       ↓
//! McpConnectionPool  ←  lazy, shared connections
//!       ↓
//! McpClient  ←  protocol lifecycle (init, list, call, shutdown)
//!       ↓
//! McpTransport  ←  stdio or HTTP
//! ```

pub mod client;
pub mod config;
pub mod error;
pub mod http;
pub mod jsonrpc;
pub mod pool;
pub mod stdio;
pub mod tool_client;
pub mod transport;

// Re-export the main public types.
pub use client::{ContentItem, McpToolInfo, ToolCallResult};
pub use config::{AuthConfig, McpToolConfig, TransportType};
pub use error::{McpError, McpResult};
pub use pool::{global_pool, set_global_pool, McpConnectionPool};
pub use tool_client::{McpToolClient, McpToolClientBuilder};
