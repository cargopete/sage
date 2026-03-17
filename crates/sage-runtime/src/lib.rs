//! Runtime library for compiled Sage programs.
//!
//! This crate provides the types and functions that generated Rust code
//! depends on. It handles:
//!
//! - Agent spawning and lifecycle
//! - Message passing between agents
//! - LLM inference calls
//! - RFC-0011: Tool execution (Http, Fs, etc.)
//! - RFC-0012: Mock infrastructure for testing
//! - Tracing and observability
//! - Error handling
//! - v2.0: Persistence for @persistent agent beliefs

#![forbid(unsafe_code)]

mod agent;
mod error;
mod llm;
pub mod mock;
pub mod persistence;
pub mod stdlib;
pub mod tools;
pub mod tracing;

pub use agent::{spawn, AgentContext, AgentHandle};
pub use error::{ErrorKind, SageError, SageResult};
pub use llm::LlmClient;
pub use mock::{MockLlmClient, MockQueue, MockResponse, MockToolRegistry};
pub use persistence::{CheckpointStore, Persisted};
pub use tools::{DatabaseClient, DbRow, FsClient, HttpClient, HttpResponse, ShellClient, ShellResult};
pub use tracing as trace;

/// Prelude for generated code.
pub mod prelude {
    pub use crate::agent::{spawn, AgentContext, AgentHandle};
    pub use crate::error::{ErrorKind, SageError, SageResult};
    pub use crate::llm::LlmClient;
    pub use crate::mock::{MockLlmClient, MockQueue, MockResponse, MockToolRegistry};
    pub use crate::persistence::{CheckpointStore, Persisted};
    pub use crate::tools::{DatabaseClient, DbRow, FsClient, HttpClient, HttpResponse, ShellClient, ShellResult};
    pub use crate::tracing as trace;
}
