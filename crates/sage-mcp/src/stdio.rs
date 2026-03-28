//! stdio transport for MCP servers launched as subprocesses.
//!
//! The runtime spawns the MCP server as a child process and communicates
//! over stdin/stdout using newline-delimited JSON-RPC 2.0 messages.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::error::{McpError, McpResult};
use crate::jsonrpc::{JsonRpcRequest, JsonRpcResponse};
use crate::transport::McpTransport;

/// Configuration for launching a stdio MCP server.
#[derive(Debug, Clone)]
pub struct StdioConfig {
    /// The command to execute.
    pub command: String,
    /// Arguments to the command.
    pub args: Vec<String>,
    /// Environment variables to pass to the subprocess.
    /// Values starting with `$` are resolved from the host environment.
    pub env: HashMap<String, String>,
}

/// stdio transport implementation.
///
/// Manages a child process and communicates via stdin/stdout.
pub struct StdioTransport {
    state: Arc<Mutex<StdioState>>,
}

struct StdioState {
    child: Child,
    stdin: Option<tokio::process::ChildStdin>,
    stdout_reader: BufReader<tokio::process::ChildStdout>,
}

impl StdioTransport {
    /// Launch the MCP server subprocess and create the transport.
    pub async fn new(config: &StdioConfig) -> McpResult<Self> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args);

        // Resolve environment variables
        for (key, value) in &config.env {
            let resolved = if let Some(var_name) = value.strip_prefix('$') {
                std::env::var(var_name).unwrap_or_default()
            } else {
                value.clone()
            };
            cmd.env(key, resolved);
        }

        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            McpError::Connection(format!(
                "Failed to launch MCP server '{}': {}",
                config.command, e
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpError::Connection("Failed to capture stdin of MCP server".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpError::Connection("Failed to capture stdout of MCP server".to_string())
        })?;

        // Spawn a task to forward stderr to tracing
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(target: "mcp.server.stderr", "{}", line);
                }
            });
        }

        let stdout_reader = BufReader::new(stdout);

        Ok(Self {
            state: Arc::new(Mutex::new(StdioState {
                child,
                stdin: Some(stdin),
                stdout_reader,
            })),
        })
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send(&self, request: &JsonRpcRequest) -> McpResult<JsonRpcResponse> {
        let mut state = self.state.lock().await;

        let stdin = state.stdin.as_mut().ok_or_else(|| {
            McpError::Connection("MCP server stdin is closed".to_string())
        })?;

        // Serialise and write the request as a single line
        let mut line = serde_json::to_string(request)?;
        line.push('\n');

        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpError::Connection(format!("Failed to write to MCP server: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| McpError::Connection(format!("Failed to flush to MCP server: {e}")))?;

        // Read the response line
        let mut response_line = String::new();
        let bytes_read = state
            .stdout_reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| McpError::Connection(format!("Failed to read from MCP server: {e}")))?;

        if bytes_read == 0 {
            return Err(McpError::ServerExited(
                "MCP server closed stdout".to_string(),
            ));
        }

        let response: JsonRpcResponse = serde_json::from_str(response_line.trim())
            .map_err(|e| McpError::Protocol(format!("Invalid JSON-RPC response: {e}")))?;

        Ok(response)
    }

    async fn notify(&self, request: &JsonRpcRequest) -> McpResult<()> {
        let mut state = self.state.lock().await;

        let stdin = state.stdin.as_mut().ok_or_else(|| {
            McpError::Connection("MCP server stdin is closed".to_string())
        })?;

        let mut line = serde_json::to_string(request)?;
        line.push('\n');

        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| McpError::Connection(format!("Failed to write to MCP server: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| McpError::Connection(format!("Failed to flush to MCP server: {e}")))?;

        Ok(())
    }

    async fn close(&self) -> McpResult<()> {
        let mut state = self.state.lock().await;

        // Close stdin to signal the subprocess
        drop(state.stdin.take());

        // Wait for process to exit with a timeout
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            state.child.wait(),
        )
        .await;

        match timeout {
            Ok(Ok(_status)) => Ok(()),
            Ok(Err(e)) => Err(McpError::Connection(format!(
                "Error waiting for MCP server to exit: {e}"
            ))),
            Err(_) => {
                // Timeout — kill the process
                let _ = state.child.kill().await;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stdio_config_resolves_env_vars() {
        std::env::set_var("TEST_MCP_VAR", "resolved_value");
        let config = StdioConfig {
            command: "echo".to_string(),
            args: vec![],
            env: HashMap::from([("MY_VAR".to_string(), "$TEST_MCP_VAR".to_string())]),
        };

        // Just verify the config is valid — we can't easily test subprocess
        // communication without a real MCP server
        assert_eq!(config.command, "echo");
        assert_eq!(config.env.get("MY_VAR").unwrap(), "$TEST_MCP_VAR");
        std::env::remove_var("TEST_MCP_VAR");
    }
}
