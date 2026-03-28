//! High-level per-tool MCP client for generated code.
//!
//! `McpToolClient` is the type that codegen produces as a field on each
//! agent struct. It wraps the connection pool, knows which MCP server to
//! talk to, and maps Sage function names to MCP tool names.

use std::collections::HashMap;
use std::sync::Arc;

use crate::client::ToolCallResult;
use crate::error::{McpError, McpResult};
use crate::pool::McpConnectionPool;

/// A per-tool MCP client used by generated agent code.
///
/// Each typed MCP tool declaration in Sage (e.g. `tool Github { ... }`)
/// gets an `McpToolClient` field on the agent struct. It provides:
///
/// - Function name → MCP tool name mapping (via `#[mcp_name]`)
/// - Lazy connection through the shared pool
/// - A clean `call()` interface for generated dispatch code
#[derive(Clone)]
pub struct McpToolClient {
    /// The tool name as declared in grove.toml (e.g. "github").
    tool_name: String,
    /// Shared connection pool.
    pool: Arc<McpConnectionPool>,
    /// Maps Sage function names to MCP tool names.
    /// e.g. "list_issues" → "list_issues", "create_pr" → "create_pull_request"
    name_map: HashMap<String, String>,
}

impl McpToolClient {
    /// Create a new tool client.
    ///
    /// # Arguments
    /// * `tool_name` — The tool name matching the `[tools.X]` section in grove.toml.
    /// * `pool` — The shared connection pool.
    /// * `name_map` — Mapping from Sage function names to MCP tool names.
    ///   If empty, function names are used as-is.
    pub fn new(
        tool_name: impl Into<String>,
        pool: Arc<McpConnectionPool>,
        name_map: HashMap<String, String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            pool,
            name_map,
        }
    }

    /// Call an MCP tool function.
    ///
    /// Resolves the Sage function name to the MCP tool name, then dispatches
    /// through the connection pool. Returns the raw `ToolCallResult`.
    ///
    /// # Arguments
    /// * `function_name` — The Sage-side function name.
    /// * `arguments` — JSON object of arguments.
    pub async fn call(
        &self,
        function_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<ToolCallResult> {
        // Resolve MCP tool name
        let mcp_name = self
            .name_map
            .get(function_name)
            .map(|s| s.as_str())
            .unwrap_or(function_name);

        // Get the client from the pool (lazy connect + init)
        let client = self.pool.get(&self.tool_name).await?;

        // Dispatch the call
        client.call_tool(mcp_name, arguments).await
    }

    /// Call an MCP tool function and extract the text result.
    pub async fn call_text(
        &self,
        function_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<String> {
        let result = self.call(function_name, arguments).await?;
        Ok(result.text())
    }

    /// Call an MCP tool function and parse the result as JSON.
    pub async fn call_json<T: serde::de::DeserializeOwned>(
        &self,
        function_name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<T> {
        let result = self.call(function_name, arguments).await?;
        result.json()
    }

    /// Get the tool name this client is configured for.
    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    /// Resolve a Sage function name to the MCP tool name.
    pub fn resolve_name<'a>(&'a self, function_name: &'a str) -> &'a str {
        self.name_map
            .get(function_name)
            .map(|s| s.as_str())
            .unwrap_or(function_name)
    }

    /// List the available MCP tools on the server.
    ///
    /// Useful for dynamic discovery and the `sage tools list` CLI command.
    pub async fn discover_tools(&self) -> McpResult<Vec<crate::client::McpToolInfo>> {
        let client = self.pool.get(&self.tool_name).await?;
        client.list_tools().await
    }
}

impl std::fmt::Debug for McpToolClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpToolClient")
            .field("tool_name", &self.tool_name)
            .field("name_map", &self.name_map)
            .finish()
    }
}

/// Builder for constructing an `McpToolClient` with name mappings.
///
/// Used by generated code to set up the function name → MCP tool name
/// mappings from `#[mcp_name]` attributes.
pub struct McpToolClientBuilder {
    tool_name: String,
    pool: Arc<McpConnectionPool>,
    name_map: HashMap<String, String>,
}

impl McpToolClientBuilder {
    /// Create a new builder.
    pub fn new(tool_name: impl Into<String>, pool: Arc<McpConnectionPool>) -> Self {
        Self {
            tool_name: tool_name.into(),
            pool,
            name_map: HashMap::new(),
        }
    }

    /// Add a name mapping from a Sage function name to an MCP tool name.
    pub fn map(mut self, sage_name: impl Into<String>, mcp_name: impl Into<String>) -> Self {
        self.name_map.insert(sage_name.into(), mcp_name.into());
        self
    }

    /// Build the `McpToolClient`.
    pub fn build(self) -> McpToolClient {
        McpToolClient::new(self.tool_name, self.pool, self.name_map)
    }
}

/// Create an `McpToolClient` by calling `mcp_connect` at runtime.
///
/// This is used by the dynamic MCP stdlib functions rather than
/// the typed tool system.
pub async fn connect_dynamic(
    tool_name: &str,
    pool: Arc<McpConnectionPool>,
) -> McpResult<McpToolClient> {
    // Verify the tool exists in the pool
    if !pool.has_tool(tool_name) {
        return Err(McpError::ToolNotFound(format!(
            "No MCP tool '{}' configured in grove.toml",
            tool_name
        )));
    }

    Ok(McpToolClient::new(
        tool_name,
        pool,
        HashMap::new(), // No name mapping for dynamic tools
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_name_with_mapping() {
        let pool = Arc::new(McpConnectionPool::new(HashMap::new()));
        let client = McpToolClientBuilder::new("github", pool)
            .map("create_pr", "create_pull_request")
            .map("list_issues", "list_repository_issues")
            .build();

        assert_eq!(client.resolve_name("create_pr"), "create_pull_request");
        assert_eq!(client.resolve_name("list_issues"), "list_repository_issues");
        // Unmapped names pass through as-is
        assert_eq!(client.resolve_name("get_user"), "get_user");
    }

    #[test]
    fn builder_produces_correct_client() {
        let pool = Arc::new(McpConnectionPool::new(HashMap::new()));
        let client = McpToolClientBuilder::new("slack", pool)
            .map("send", "chat_postMessage")
            .build();

        assert_eq!(client.tool_name(), "slack");
        assert_eq!(client.resolve_name("send"), "chat_postMessage");
    }

    #[tokio::test]
    async fn connect_dynamic_missing_tool() {
        let pool = Arc::new(McpConnectionPool::new(HashMap::new()));
        let result = connect_dynamic("nonexistent", pool).await;
        assert!(result.is_err());
    }
}
