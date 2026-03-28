//! Configuration types for MCP tool connections.
//!
//! These types map directly to the `[tools.X]` sections in grove.toml.

use serde::Deserialize;
use std::collections::HashMap;

/// Transport type for MCP connections.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransportType {
    /// Launch server as subprocess, communicate via stdin/stdout.
    Stdio,
    /// Connect to remote server via HTTP.
    Http,
}

/// Authentication configuration for HTTP MCP servers.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "auth", rename_all = "lowercase")]
pub enum AuthConfig {
    /// Bearer token authentication.
    Bearer {
        /// Environment variable containing the token.
        token_env: String,
    },
    /// OAuth 2.1 + PKCE authentication.
    OAuth {
        /// Environment variable containing the client ID.
        client_id_env: String,
        /// OAuth authorization endpoint.
        authorization_url: String,
        /// OAuth token endpoint.
        token_url: String,
        /// Requested scopes.
        #[serde(default)]
        scopes: Vec<String>,
    },
}

/// MCP tool configuration from grove.toml `[tools.X]` section.
#[derive(Debug, Clone, Deserialize)]
pub struct McpToolConfig {
    /// Transport type: "stdio" or "http".
    pub transport: TransportType,

    // --- stdio fields ---
    /// Command to launch (stdio transport).
    #[serde(default)]
    pub command: Option<String>,

    /// Arguments to the command (stdio transport).
    #[serde(default)]
    pub args: Option<Vec<String>>,

    /// Environment variables for the subprocess (stdio transport).
    /// Values starting with `$` are resolved from the host environment.
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,

    // --- HTTP fields ---
    /// MCP server endpoint URL (http transport).
    #[serde(default)]
    pub url: Option<String>,

    // --- Auth ---
    /// Authentication type and config.
    #[serde(default, flatten)]
    pub auth_config: Option<AuthConfig>,

    // --- Timeouts ---
    /// Per-call timeout in milliseconds. Default: 30000.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Connection timeout in milliseconds. Default: 10000.
    #[serde(default = "default_connect_timeout_ms")]
    pub connect_timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    30_000
}

fn default_connect_timeout_ms() -> u64 {
    10_000
}

impl McpToolConfig {
    /// Validate the configuration for completeness.
    pub fn validate(&self, tool_name: &str) -> Result<(), String> {
        match self.transport {
            TransportType::Stdio => {
                if self.command.is_none() {
                    return Err(format!(
                        "MCP tool '{}': stdio transport requires 'command' field",
                        tool_name
                    ));
                }
            }
            TransportType::Http => {
                if self.url.is_none() {
                    return Err(format!(
                        "MCP tool '{}': http transport requires 'url' field",
                        tool_name
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stdio_config() {
        let toml = r#"
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
timeout_ms = 60000
"#;
        let config: McpToolConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.transport, TransportType::Stdio);
        assert_eq!(config.command.unwrap(), "npx");
        assert_eq!(config.args.unwrap(), vec!["-y", "@modelcontextprotocol/server-github"]);
        assert_eq!(config.timeout_ms, 60000);
    }

    #[test]
    fn parse_http_config() {
        let toml = r#"
transport = "http"
url = "https://mcp.example.com/mcp"
timeout_ms = 5000
"#;
        let config: McpToolConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.transport, TransportType::Http);
        assert_eq!(config.url.unwrap(), "https://mcp.example.com/mcp");
        assert_eq!(config.timeout_ms, 5000);
    }

    #[test]
    fn validate_stdio_missing_command() {
        let config = McpToolConfig {
            transport: TransportType::Stdio,
            command: None,
            args: None,
            env: None,
            url: None,
            auth_config: None,
            timeout_ms: 30_000,
            connect_timeout_ms: 10_000,
        };
        assert!(config.validate("TestTool").is_err());
    }

    #[test]
    fn validate_http_missing_url() {
        let config = McpToolConfig {
            transport: TransportType::Http,
            command: None,
            args: None,
            env: None,
            url: None,
            auth_config: None,
            timeout_ms: 30_000,
            connect_timeout_ms: 10_000,
        };
        assert!(config.validate("TestTool").is_err());
    }

    #[test]
    fn default_timeouts() {
        let toml = r#"
transport = "stdio"
command = "test"
"#;
        let config: McpToolConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.timeout_ms, 30_000);
        assert_eq!(config.connect_timeout_ms, 10_000);
    }

    // RFC-0023: JSON deserialization (for dynamic mcp_connect)
    #[test]
    fn parse_stdio_config_from_json() {
        let json = r#"{
            "transport": "stdio",
            "command": "npx",
            "args": ["-y", "@mcp/server-github"],
            "timeout_ms": 60000
        }"#;
        let config: McpToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.transport, TransportType::Stdio);
        assert_eq!(config.command.unwrap(), "npx");
        assert_eq!(config.timeout_ms, 60000);
        assert_eq!(config.connect_timeout_ms, 10_000); // default
    }

    #[test]
    fn parse_http_config_from_json() {
        let json = r#"{
            "transport": "http",
            "url": "https://mcp.example.com/mcp"
        }"#;
        let config: McpToolConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.transport, TransportType::Http);
        assert_eq!(config.url.unwrap(), "https://mcp.example.com/mcp");
        assert_eq!(config.timeout_ms, 30_000);
    }

    #[test]
    fn parse_config_with_env_from_json() {
        let json = r#"{
            "transport": "stdio",
            "command": "npx",
            "args": ["-y", "@mcp/server-github"],
            "env": {"GITHUB_TOKEN": "abc123"}
        }"#;
        let config: McpToolConfig = serde_json::from_str(json).unwrap();
        let env = config.env.unwrap();
        assert_eq!(env.get("GITHUB_TOKEN").unwrap(), "abc123");
    }

    #[test]
    fn validate_json_parsed_config() {
        let json = r#"{"transport": "stdio"}"#;
        let config: McpToolConfig = serde_json::from_str(json).unwrap();
        assert!(config.validate("dynamic").is_err()); // missing command
    }
}
