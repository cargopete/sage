//! RFC-0011: Shell tool for Sage agents.
//!
//! Provides the `Shell` tool with command execution capabilities.

use crate::error::{SageError, SageResult};
use crate::mock::{try_get_mock, MockResponse};

/// Result of running a shell command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShellResult {
    /// Exit code from the command.
    pub exit_code: i64,
    /// Standard output from the command.
    pub stdout: String,
    /// Standard error from the command.
    pub stderr: String,
}

/// Shell client for Sage agents.
///
/// Provides command execution via the system shell.
#[derive(Debug, Clone, Default)]
pub struct ShellClient;

impl ShellClient {
    /// Create a new shell client.
    pub fn new() -> Self {
        Self
    }

    /// Create a new shell client from environment variables.
    ///
    /// Currently no environment configuration is needed.
    pub fn from_env() -> Self {
        Self
    }

    /// Run a shell command.
    ///
    /// # Arguments
    /// * `command` - The command to run (passed to `sh -c`)
    ///
    /// # Returns
    /// A `ShellResult` with exit code, stdout, and stderr.
    pub async fn run(&self, command: String) -> SageResult<ShellResult> {
        // Check for mock response first
        if let Some(mock_response) = try_get_mock("Shell", "run") {
            return Self::apply_mock(mock_response);
        }

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .output()
            .await?;

        Ok(ShellResult {
            exit_code: output.status.code().unwrap_or(-1) as i64,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    /// Apply a mock response, deserializing it to ShellResult.
    fn apply_mock(mock_response: MockResponse) -> SageResult<ShellResult> {
        match mock_response {
            MockResponse::Value(v) => serde_json::from_value(v)
                .map_err(|e| SageError::Tool(format!("mock deserialize: {e}"))),
            MockResponse::Fail(msg) => Err(SageError::Tool(msg)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_client_creates() {
        let _client = ShellClient::new();
    }

    #[tokio::test]
    async fn shell_run_echo() {
        let client = ShellClient::new();
        let result = client.run("echo hello".to_string()).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello");
        assert!(result.stderr.is_empty());
    }

    #[tokio::test]
    async fn shell_run_exit_code() {
        let client = ShellClient::new();
        let result = client.run("exit 42".to_string()).await.unwrap();
        assert_eq!(result.exit_code, 42);
    }

    #[tokio::test]
    async fn shell_run_stderr() {
        let client = ShellClient::new();
        let result = client.run("echo error >&2".to_string()).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.is_empty());
        assert_eq!(result.stderr.trim(), "error");
    }

    #[tokio::test]
    async fn shell_run_complex_command() {
        let client = ShellClient::new();
        let result = client
            .run("echo 'line1'; echo 'line2'".to_string())
            .await
            .unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("line1"));
        assert!(result.stdout.contains("line2"));
    }
}
