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
//! - v2.0: Supervision trees for agent lifecycle management
//! - Phase 3: Session types for protocol verification

#![forbid(unsafe_code)]

mod agent;
mod error;
mod llm;
pub mod mock;
pub mod persistence;
pub mod session;
pub mod stdlib;
pub mod supervisor;
pub mod tools;
pub mod tracing;

pub use agent::{spawn, spawn_with_llm_config, AgentContext, AgentHandle, Message};
pub use error::{ErrorKind, SageError, SageResult};
pub use llm::{LlmClient, LlmConfig};
pub use mock::{
    try_get_mock, with_mock_tools, MockLlmClient, MockQueue, MockResponse, MockToolRegistry,
};
pub use persistence::{CheckpointStore, Persisted};
pub use session::{
    ProtocolStateMachine, ProtocolViolation, SenderHandle, SessionId, SessionRegistry,
    SessionState, SharedSessionRegistry,
};
pub use supervisor::{RestartConfig, RestartPolicy, Strategy, Supervisor};
pub use tools::{
    DatabaseClient, DbRow, FsClient, HttpClient, HttpResponse, ShellClient, ShellResult,
};
pub use tracing as trace;

/// Prelude for generated code.
pub mod prelude {
    pub use crate::agent::{spawn, spawn_with_llm_config, AgentContext, AgentHandle, Message};
    pub use crate::error::{ErrorKind, SageError, SageResult};
    pub use crate::llm::{LlmClient, LlmConfig};
    pub use crate::mock::{
        try_get_mock, with_mock_tools, MockLlmClient, MockQueue, MockResponse, MockToolRegistry,
    };
    pub use crate::persistence::{CheckpointStore, Persisted};
    pub use crate::session::{
        ProtocolStateMachine, ProtocolViolation, SenderHandle, SessionId, SessionRegistry,
        SessionState, SharedSessionRegistry,
    };
    pub use crate::supervisor::{RestartConfig, RestartPolicy, Strategy, Supervisor};
    pub use crate::tools::{
        DatabaseClient, DbRow, FsClient, HttpClient, HttpResponse, ShellClient, ShellResult,
    };
    pub use crate::tracing as trace;
}
