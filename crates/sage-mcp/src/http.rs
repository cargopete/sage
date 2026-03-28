//! Streamable HTTP transport for remote MCP servers.
//!
//! Implements the MCP Streamable HTTP transport (spec 2025-03-26):
//! - Client sends JSON-RPC as HTTP POST to the server endpoint
//! - Server responds with `application/json` or `text/event-stream`
//! - Session management via `MCP-Session-Id` header
//! - Bearer token and OAuth 2.1 auth support

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::AuthConfig;
use crate::error::{McpError, McpResult};
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};
use crate::transport::McpTransport;

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

/// Configuration for connecting to an HTTP MCP server.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// The MCP server endpoint URL.
    pub url: String,
    /// Authentication configuration.
    pub auth: Option<AuthConfig>,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: u64,
}

/// Streamable HTTP transport implementation.
pub struct HttpTransport {
    config: HttpConfig,
    client: reqwest::Client,
    state: Arc<Mutex<HttpState>>,
}

struct HttpState {
    /// Session ID assigned by the server after initialization.
    session_id: Option<String>,
    /// Bearer token for authentication.
    bearer_token: Option<String>,
}

impl HttpTransport {
    /// Create a new HTTP transport.
    pub fn new(config: HttpConfig) -> McpResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .connect_timeout(std::time::Duration::from_millis(config.connect_timeout_ms))
            .build()
            .map_err(|e| McpError::Connection(format!("Failed to build HTTP client: {e}")))?;

        // Resolve bearer token from environment
        let bearer_token = match &config.auth {
            Some(AuthConfig::Bearer { token_env }) => {
                let token = std::env::var(token_env).map_err(|_| {
                    McpError::Auth(format!(
                        "Environment variable '{}' not set for MCP bearer token",
                        token_env
                    ))
                })?;
                Some(token)
            }
            Some(AuthConfig::OAuth { .. }) => {
                // OAuth flow is more complex — for now, check for cached token
                // Full OAuth 2.1 + PKCE flow will be implemented in a follow-up
                None
            }
            None => None,
        };

        Ok(Self {
            config,
            client,
            state: Arc::new(Mutex::new(HttpState {
                session_id: None,
                bearer_token,
            })),
        })
    }

    /// Build the HTTP request with standard MCP headers.
    async fn build_request(&self, body: &str) -> McpResult<reqwest::RequestBuilder> {
        let state = self.state.lock().await;
        let mut req = self
            .client
            .post(&self.config.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", MCP_PROTOCOL_VERSION);

        if let Some(session_id) = &state.session_id {
            req = req.header("MCP-Session-Id", session_id);
        }

        if let Some(token) = &state.bearer_token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }

        Ok(req.body(body.to_string()))
    }

    /// Parse SSE response and extract the JSON-RPC message.
    fn parse_sse_response(body: &str) -> McpResult<JsonRpcResponse> {
        // SSE format: lines starting with "data: " contain the payload
        for line in body.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if !data.is_empty() {
                    return serde_json::from_str(data).map_err(|e| {
                        McpError::Protocol(format!("Invalid JSON in SSE data: {e}"))
                    });
                }
            }
        }
        Err(McpError::Protocol(
            "No data found in SSE response".to_string(),
        ))
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn send(&self, request: &JsonRpcRequest) -> McpResult<JsonRpcResponse> {
        let body = serde_json::to_string(request)?;
        let req = self.build_request(&body).await?;

        let response = req.send().await.map_err(|e| {
            if e.is_timeout() {
                McpError::Timeout(format!(
                    "MCP request timed out after {}ms",
                    self.config.timeout_ms
                ))
            } else {
                McpError::Http(e)
            }
        })?;

        // Extract session ID from response headers
        if let Some(session_id) = response.headers().get("mcp-session-id") {
            if let Ok(id) = session_id.to_str() {
                let mut state = self.state.lock().await;
                state.session_id = Some(id.to_string());
            }
        }

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(McpError::Protocol(format!(
                "MCP server returned HTTP {status}: {body}"
            )));
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let response_body = response.text().await.map_err(|e| {
            McpError::Protocol(format!("Failed to read response body: {e}"))
        })?;

        if content_type.contains("text/event-stream") {
            Self::parse_sse_response(&response_body)
        } else {
            serde_json::from_str(&response_body)
                .map_err(|e| McpError::Protocol(format!("Invalid JSON-RPC response: {e}")))
        }
    }

    async fn notify(&self, request: &JsonRpcRequest) -> McpResult<()> {
        let body = serde_json::to_string(request)?;
        let req = self.build_request(&body).await?;

        // Fire and forget — we don't wait for a meaningful response
        let _ = req.send().await.map_err(|e| {
            McpError::Connection(format!("Failed to send notification: {e}"))
        })?;

        Ok(())
    }

    async fn close(&self) -> McpResult<()> {
        let state = self.state.lock().await;

        // Send DELETE to terminate the session if we have a session ID
        if let Some(session_id) = &state.session_id {
            let mut req = self.client.delete(&self.config.url);
            req = req.header("MCP-Session-Id", session_id);

            if let Some(token) = &state.bearer_token {
                req = req.header("Authorization", format!("Bearer {token}"));
            }

            // Best-effort — don't fail if the server is already gone
            let _ = req.send().await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_response_basic() {
        let sse = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"tools\":[]}}\n\n";
        let resp = HttpTransport::parse_sse_response(sse).unwrap();
        assert!(!resp.is_error());
    }

    #[test]
    fn parse_sse_response_no_data() {
        let sse = "event: ping\n\n";
        let result = HttpTransport::parse_sse_response(sse);
        assert!(result.is_err());
    }
}
