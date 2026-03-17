//! RFC-0011: Tool implementations for Sage agents.
//!
//! This module provides the built-in tools that agents can use via
//! `use ToolName` declarations and `ToolName.method()` calls.

mod database;
mod filesystem;
mod http;
mod shell;

pub use database::{DatabaseClient, DbRow};
pub use filesystem::FsClient;
pub use http::{HttpClient, HttpResponse};
pub use shell::{ShellClient, ShellResult};
