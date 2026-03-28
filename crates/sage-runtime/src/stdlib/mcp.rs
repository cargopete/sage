//! Dynamic MCP stdlib functions for Sage (RFC-0023).
//!
//! These functions allow agents to connect to MCP servers at runtime
//! without declaring typed tool blocks. Connections are managed in a
//! global registry keyed by handle ID.

use sage_mcp::client::McpClient;
use sage_mcp::config::McpToolConfig;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;

/// Global registry of dynamic MCP connections.
static CONNECTIONS: LazyLock<Mutex<HashMap<String, Arc<McpClient>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Counter for generating unique handle IDs.
static HANDLE_COUNTER: LazyLock<std::sync::atomic::AtomicU64> =
    LazyLock::new(|| std::sync::atomic::AtomicU64::new(1));

fn next_handle() -> String {
    let id = HANDLE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("mcp:{id}")
}

/// Connect to an MCP server dynamically.
///
/// Takes a JSON config string with the same shape as a `[tools.X]` section
/// in grove.toml:
///
/// ```json
/// {
///   "transport": "stdio",
///   "command": "npx",
///   "args": ["-y", "@modelcontextprotocol/server-github"],
///   "env": {"GITHUB_TOKEN": "..."}
/// }
/// ```
///
/// Returns a handle string that can be passed to `mcp_list_tools`,
/// `mcp_call`, and `mcp_disconnect`.
pub async fn mcp_connect(config_json: &str) -> Result<String, String> {
    let config: McpToolConfig =
        serde_json::from_str(config_json).map_err(|e| format!("invalid MCP config: {e}"))?;

    config
        .validate("dynamic")
        .map_err(|e| format!("invalid MCP config: {e}"))?;

    let client = McpClient::from_config(&config)
        .await
        .map_err(|e| format!("MCP connection failed: {e}"))?;

    client
        .initialize()
        .await
        .map_err(|e| format!("MCP initialization failed: {e}"))?;

    let handle = next_handle();
    let mut conns = CONNECTIONS.lock().await;
    conns.insert(handle.clone(), Arc::new(client));

    Ok(handle)
}

/// List available tools on a connected MCP server.
///
/// Returns a JSON array of tool descriptions:
/// ```json
/// [{"name": "tool_name", "description": "...", "input_schema": {...}}, ...]
/// ```
pub async fn mcp_list_tools(handle: &str) -> Result<String, String> {
    let client = get_client(handle).await?;

    let tools = client
        .list_tools()
        .await
        .map_err(|e| format!("mcp_list_tools failed: {e}"))?;

    serde_json::to_string(&tools).map_err(|e| format!("failed to serialize tool list: {e}"))
}

/// Call a tool on a connected MCP server.
///
/// `args_json` should be a JSON object matching the tool's input schema.
/// Returns the tool result as a string.
pub async fn mcp_call(handle: &str, tool_name: &str, args_json: &str) -> Result<String, String> {
    let client = get_client(handle).await?;

    let arguments: serde_json::Value =
        serde_json::from_str(args_json).map_err(|e| format!("invalid arguments JSON: {e}"))?;

    let result = client
        .call_tool(tool_name, arguments)
        .await
        .map_err(|e| format!("mcp_call '{tool_name}' failed: {e}"))?;

    Ok(result.text())
}

/// Disconnect from an MCP server and release the handle.
pub async fn mcp_disconnect(handle: &str) -> Result<(), String> {
    let mut conns = CONNECTIONS.lock().await;
    let client = conns
        .remove(handle)
        .ok_or_else(|| format!("unknown MCP handle: {handle}"))?;

    client
        .shutdown()
        .await
        .map_err(|e| format!("MCP shutdown failed: {e}"))?;

    Ok(())
}

/// Look up a client by handle.
async fn get_client(handle: &str) -> Result<Arc<McpClient>, String> {
    let conns = CONNECTIONS.lock().await;
    conns
        .get(handle)
        .cloned()
        .ok_or_else(|| format!("unknown MCP handle: {handle}"))
}
