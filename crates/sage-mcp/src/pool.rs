//! Connection pool for managing MCP client connections.
//!
//! Each tool name maps to a single MCP server connection, created lazily
//! on first use and shared across all agents in the program via `Arc`.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

use crate::client::McpClient;
use crate::config::McpToolConfig;
use crate::error::{McpError, McpResult};

// ---------------------------------------------------------------------------
// Global pool — set once in main(), shared by all summoned agents
// ---------------------------------------------------------------------------

static GLOBAL_POOL: LazyLock<Mutex<Option<Arc<McpConnectionPool>>>> =
    LazyLock::new(|| Mutex::new(None));

/// Register the process-wide MCP connection pool.
///
/// Called once from generated `main()`. Subsequent calls replace the pool.
pub async fn set_global_pool(pool: Arc<McpConnectionPool>) {
    let mut guard = GLOBAL_POOL.lock().await;
    *guard = Some(pool);
}

/// Retrieve the global MCP connection pool, if one has been registered.
///
/// Used by summoned agents to share connections with the root agent
/// instead of creating redundant pools.
pub async fn global_pool() -> Option<Arc<McpConnectionPool>> {
    let guard = GLOBAL_POOL.lock().await;
    guard.as_ref().map(Arc::clone)
}

/// A pool of MCP connections, keyed by tool name.
///
/// Connections are created lazily when first requested and reused for
/// subsequent calls. The pool is designed to be wrapped in `Arc` and
/// shared across agents.
pub struct McpConnectionPool {
    configs: HashMap<String, McpToolConfig>,
    clients: Mutex<HashMap<String, Arc<McpClient>>>,
}

impl McpConnectionPool {
    /// Create a new pool from tool configurations.
    ///
    /// No connections are established until `get()` is called.
    pub fn new(configs: HashMap<String, McpToolConfig>) -> Self {
        Self {
            configs,
            clients: Mutex::new(HashMap::new()),
        }
    }

    /// Get or create an MCP client for the given tool name.
    ///
    /// On first call, this launches the MCP server (stdio) or connects
    /// to it (HTTP), performs the initialize handshake, and caches the
    /// connection for future use.
    pub async fn get(&self, tool_name: &str) -> McpResult<Arc<McpClient>> {
        let mut clients = self.clients.lock().await;

        // Return cached client if available
        if let Some(client) = clients.get(tool_name) {
            return Ok(Arc::clone(client));
        }

        // Look up configuration
        let config = self.configs.get(tool_name).ok_or_else(|| {
            McpError::ToolNotFound(format!("No MCP configuration for tool '{tool_name}'"))
        })?;

        // Create and initialize the client
        tracing::info!(tool = %tool_name, "Connecting to MCP server");
        let client = McpClient::from_config(config).await?;
        client.initialize().await?;

        let client = Arc::new(client);
        clients.insert(tool_name.to_string(), Arc::clone(&client));

        Ok(client)
    }

    /// Check if a tool name has a registered configuration.
    pub fn has_tool(&self, tool_name: &str) -> bool {
        self.configs.contains_key(tool_name)
    }

    /// Get the list of configured tool names.
    pub fn tool_names(&self) -> Vec<&str> {
        self.configs.keys().map(|s| s.as_str()).collect()
    }

    /// Shut down all active connections.
    pub async fn shutdown(&self) {
        let mut clients = self.clients.lock().await;
        for (name, client) in clients.drain() {
            if let Err(e) = client.shutdown().await {
                tracing::warn!(tool = %name, error = %e, "Error shutting down MCP connection");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TransportType;

    #[test]
    fn pool_has_tool() {
        let mut configs = HashMap::new();
        configs.insert(
            "github".to_string(),
            McpToolConfig {
                transport: TransportType::Stdio,
                command: Some("npx".to_string()),
                args: Some(vec!["-y".to_string(), "@mcp/server-github".to_string()]),
                env: None,
                url: None,
                auth_config: None,
                timeout_ms: 30_000,
                connect_timeout_ms: 10_000,
            },
        );

        let pool = McpConnectionPool::new(configs);
        assert!(pool.has_tool("github"));
        assert!(!pool.has_tool("slack"));
    }

    #[test]
    fn pool_tool_names() {
        let mut configs = HashMap::new();
        configs.insert(
            "github".to_string(),
            McpToolConfig {
                transport: TransportType::Stdio,
                command: Some("npx".to_string()),
                args: None,
                env: None,
                url: None,
                auth_config: None,
                timeout_ms: 30_000,
                connect_timeout_ms: 10_000,
            },
        );
        configs.insert(
            "slack".to_string(),
            McpToolConfig {
                transport: TransportType::Http,
                command: None,
                args: None,
                env: None,
                url: Some("https://mcp.slack.example.com".to_string()),
                auth_config: None,
                timeout_ms: 30_000,
                connect_timeout_ms: 10_000,
            },
        );

        let pool = McpConnectionPool::new(configs);
        let mut names = pool.tool_names();
        names.sort();
        assert_eq!(names, vec!["github", "slack"]);
    }

    #[tokio::test]
    async fn pool_get_missing_tool_returns_error() {
        let pool = McpConnectionPool::new(HashMap::new());
        let result = pool.get("nonexistent").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::ToolNotFound(_)));
    }
}
