//! MCP client implementing the protocol lifecycle.
//!
//! Handles the initialize handshake, tool discovery, tool invocation,
//! and graceful shutdown per the MCP spec (2025-03-26).

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::{McpToolConfig, TransportType};
use crate::error::{McpError, McpResult};
use crate::http::{HttpConfig, HttpTransport};
use crate::jsonrpc::JsonRpcRequest;
use crate::stdio::{StdioConfig, StdioTransport};
use crate::transport::McpTransport;

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
const CLIENT_NAME: &str = "sage";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Information about a tool exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    /// The tool name as the server advertises it.
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// JSON Schema for the tool's input parameters.
    #[serde(rename = "inputSchema", default)]
    pub input_schema: Option<serde_json::Value>,
}

/// A single content item in a tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentItem {
    /// Plain text content.
    #[serde(rename = "text")]
    Text { text: String },
    /// Base64-encoded image content.
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// Embedded resource content.
    #[serde(rename = "resource")]
    Resource { resource: serde_json::Value },
}

/// Result of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Content items returned by the tool.
    #[serde(default)]
    pub content: Vec<ContentItem>,
    /// Whether the tool call resulted in an error.
    #[serde(rename = "isError", default)]
    pub is_error: bool,
}

/// Server information returned during initialization.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    #[serde(default)]
    capabilities: serde_json::Value,
    #[serde(rename = "serverInfo", default)]
    server_info: Option<ServerInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct ServerInfo {
    name: String,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ToolsListResult {
    tools: Vec<McpToolInfo>,
}

/// MCP client wrapping a transport with protocol lifecycle management.
pub struct McpClient {
    transport: Box<dyn McpTransport>,
    state: Arc<Mutex<ClientState>>,
}

impl std::fmt::Debug for McpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpClient").finish_non_exhaustive()
    }
}

#[derive(Debug, Default)]
struct ClientState {
    initialized: bool,
    server_name: Option<String>,
}

impl McpClient {
    /// Create an McpClient from a tool configuration.
    ///
    /// This builds the appropriate transport (stdio or HTTP) and returns
    /// an uninitialised client. Call `initialize()` before making tool calls.
    pub async fn from_config(config: &McpToolConfig) -> McpResult<Self> {
        let transport: Box<dyn McpTransport> = match config.transport {
            TransportType::Stdio => {
                let stdio_config = StdioConfig {
                    command: config
                        .command
                        .clone()
                        .ok_or_else(|| McpError::Protocol("stdio transport requires 'command'".into()))?,
                    args: config.args.clone().unwrap_or_default(),
                    env: config.env.clone().unwrap_or_default(),
                };
                Box::new(StdioTransport::new(&stdio_config).await?)
            }
            TransportType::Http => {
                let http_config = HttpConfig {
                    url: config
                        .url
                        .clone()
                        .ok_or_else(|| McpError::Protocol("http transport requires 'url'".into()))?,
                    auth: config.auth_config.clone(),
                    timeout_ms: config.timeout_ms,
                    connect_timeout_ms: config.connect_timeout_ms,
                };
                Box::new(HttpTransport::new(http_config)?)
            }
        };

        Ok(Self {
            transport,
            state: Arc::new(Mutex::new(ClientState::default())),
        })
    }

    /// Perform the MCP initialize handshake.
    ///
    /// Sends the `initialize` request, validates the server's protocol version,
    /// then sends the `notifications/initialized` notification.
    pub async fn initialize(&self) -> McpResult<()> {
        let params = serde_json::json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": CLIENT_NAME,
                "version": CLIENT_VERSION,
            }
        });

        let request = JsonRpcRequest::new("initialize", Some(params));
        let response = self.transport.send(&request).await?;

        let result_value = response.into_result().map_err(|e| {
            McpError::Protocol(format!("Initialize failed: {e}"))
        })?;

        let init_result: InitializeResult = serde_json::from_value(result_value)
            .map_err(|e| McpError::Protocol(format!("Invalid initialize response: {e}")))?;

        // Update state
        let mut state = self.state.lock().await;
        state.initialized = true;
        state.server_name = init_result
            .server_info
            .as_ref()
            .map(|s| s.name.clone());

        tracing::info!(
            server = ?state.server_name,
            protocol = %init_result.protocol_version,
            "MCP server initialized"
        );

        // Send the initialized notification
        let notification = JsonRpcRequest::notification("notifications/initialized", None);
        self.transport.notify(&notification).await?;

        Ok(())
    }

    /// Discover available tools on the server.
    pub async fn list_tools(&self) -> McpResult<Vec<McpToolInfo>> {
        self.ensure_initialized().await?;

        let request = JsonRpcRequest::new("tools/list", None);
        let response = self.transport.send(&request).await?;

        let result_value = response.into_result().map_err(|e| {
            McpError::Protocol(format!("tools/list failed: {e}"))
        })?;

        let list_result: ToolsListResult = serde_json::from_value(result_value)
            .map_err(|e| McpError::Protocol(format!("Invalid tools/list response: {e}")))?;

        Ok(list_result.tools)
    }

    /// Call a tool on the MCP server.
    ///
    /// # Arguments
    /// * `name` — The tool name as advertised by the server.
    /// * `arguments` — JSON object of arguments matching the tool's input schema.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> McpResult<ToolCallResult> {
        self.ensure_initialized().await?;

        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });

        let request = JsonRpcRequest::new("tools/call", Some(params));
        let response = self.transport.send(&request).await?;

        let result_value = response.into_result().map_err(|e| {
            McpError::ToolExecution(format!("Tool '{}' error: {}", name, e))
        })?;

        let tool_result: ToolCallResult = serde_json::from_value(result_value)
            .map_err(|e| McpError::Protocol(format!("Invalid tool result: {e}")))?;

        if tool_result.is_error {
            let error_text = tool_result
                .content
                .iter()
                .filter_map(|c| match c {
                    ContentItem::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            return Err(McpError::ToolExecution(format!(
                "Tool '{}' returned error: {}",
                name, error_text
            )));
        }

        Ok(tool_result)
    }

    /// Gracefully shut down the connection.
    pub async fn shutdown(&self) -> McpResult<()> {
        self.transport.close().await
    }

    /// Ensure the client has been initialised before making requests.
    async fn ensure_initialized(&self) -> McpResult<()> {
        let state = self.state.lock().await;
        if !state.initialized {
            return Err(McpError::Protocol(
                "MCP client not initialized — call initialize() first".into(),
            ));
        }
        Ok(())
    }
}

impl ToolCallResult {
    /// Extract all text content from the result, concatenated.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|c| match c {
                ContentItem::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Try to parse the text content as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> McpResult<T> {
        let text = self.text();
        serde_json::from_str(&text).map_err(|e| {
            McpError::Deserialisation(format!("Failed to parse tool result as JSON: {e}"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_result_text_extraction() {
        let result = ToolCallResult {
            content: vec![
                ContentItem::Text {
                    text: "Hello".to_string(),
                },
                ContentItem::Text {
                    text: "World".to_string(),
                },
            ],
            is_error: false,
        };
        assert_eq!(result.text(), "Hello\nWorld");
    }

    #[test]
    fn tool_call_result_json_parsing() {
        let result = ToolCallResult {
            content: vec![ContentItem::Text {
                text: r#"{"name":"test","value":42}"#.to_string(),
            }],
            is_error: false,
        };

        #[derive(Debug, Deserialize, PartialEq)]
        struct Data {
            name: String,
            value: i32,
        }

        let data: Data = result.json().unwrap();
        assert_eq!(data.name, "test");
        assert_eq!(data.value, 42);
    }

    #[test]
    fn mcp_tool_info_deserialise() {
        let json = r#"{
            "name": "get_weather",
            "description": "Get weather for a city",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                },
                "required": ["city"]
            }
        }"#;

        let info: McpToolInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.name, "get_weather");
        assert_eq!(info.description.unwrap(), "Get weather for a city");
        assert!(info.input_schema.is_some());
    }

    #[test]
    fn content_item_variants() {
        let text_json = r#"{"type":"text","text":"hello"}"#;
        let item: ContentItem = serde_json::from_str(text_json).unwrap();
        assert!(matches!(item, ContentItem::Text { .. }));

        let image_json = r#"{"type":"image","data":"base64data","mimeType":"image/png"}"#;
        let item: ContentItem = serde_json::from_str(image_json).unwrap();
        assert!(matches!(item, ContentItem::Image { .. }));
    }
}
