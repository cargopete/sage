//! RFC-0011: Tool implementations for Sage agents.
//!
//! This module provides the built-in tools that agents can use via
//! `use ToolName` declarations and `ToolName.method()` calls.
//!
//! # WASM Compatibility
//!
//! - `HttpClient`: Works on both native and WASM (reqwest has a Fetch backend)
//! - `DatabaseClient`: Native only (requires SQLx)
//! - `FsClient`: Native only (requires filesystem access)
//! - `ShellClient`: Native only (requires process spawning)

#[cfg(not(target_arch = "wasm32"))]
mod database;
#[cfg(not(target_arch = "wasm32"))]
mod filesystem;
mod http;
#[cfg(not(target_arch = "wasm32"))]
mod shell;

#[cfg(not(target_arch = "wasm32"))]
pub use database::{DatabaseClient, DbRow};
#[cfg(not(target_arch = "wasm32"))]
pub use filesystem::FsClient;
pub use http::{HttpClient, HttpResponse};
#[cfg(not(target_arch = "wasm32"))]
pub use shell::{ShellClient, ShellResult};

// ---------------------------------------------------------------------------
// WASM stubs for native-only tools
// ---------------------------------------------------------------------------
// These provide the same types so generated code compiles, but return errors
// at runtime if invoked in the browser.

#[cfg(target_arch = "wasm32")]
mod wasm_stubs {
    use crate::error::{SageError, SageResult};

    /// A row returned from a database query.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct DbRow {
        pub columns: Vec<String>,
        pub values: Vec<String>,
    }

    /// Database client stub for WASM (not supported).
    #[derive(Debug, Clone)]
    pub struct DatabaseClient;

    impl DatabaseClient {
        pub fn new() -> Self {
            Self
        }

        pub fn from_env() -> Self {
            Self
        }

        pub async fn query(&self, _sql: String) -> SageResult<Vec<DbRow>> {
            Err(SageError::Tool(
                "Database tool is not available in the WASM target".to_string(),
            ))
        }

        pub async fn execute(&self, _sql: String) -> SageResult<i64> {
            Err(SageError::Tool(
                "Database tool is not available in the WASM target".to_string(),
            ))
        }
    }

    impl Default for DatabaseClient {
        fn default() -> Self {
            Self::new()
        }
    }

    /// FileSystem client stub for WASM (not supported).
    #[derive(Debug, Clone)]
    pub struct FsClient;

    impl FsClient {
        pub fn new() -> Self {
            Self
        }

        pub fn from_env() -> Self {
            Self
        }

        pub async fn read(&self, _path: String) -> SageResult<String> {
            Err(SageError::Tool(
                "Fs tool is not available in the WASM target".to_string(),
            ))
        }

        pub async fn write(&self, _path: String, _content: String) -> SageResult<()> {
            Err(SageError::Tool(
                "Fs tool is not available in the WASM target".to_string(),
            ))
        }

        pub async fn list(&self, _path: String) -> SageResult<Vec<String>> {
            Err(SageError::Tool(
                "Fs tool is not available in the WASM target".to_string(),
            ))
        }

        pub async fn exists(&self, _path: String) -> SageResult<bool> {
            Err(SageError::Tool(
                "Fs tool is not available in the WASM target".to_string(),
            ))
        }
    }

    impl Default for FsClient {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Result of running a shell command.
    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ShellResult {
        pub exit_code: i64,
        pub stdout: String,
        pub stderr: String,
    }

    /// Shell client stub for WASM (not supported).
    #[derive(Debug, Clone, Default)]
    pub struct ShellClient;

    impl ShellClient {
        pub fn new() -> Self {
            Self
        }

        pub fn from_env() -> Self {
            Self
        }

        pub async fn run(&self, _command: String) -> SageResult<ShellResult> {
            Err(SageError::Tool(
                "Shell tool is not available in the WASM target".to_string(),
            ))
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_stubs::{DatabaseClient, DbRow, FsClient, ShellClient, ShellResult};
