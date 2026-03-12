//! Runtime library for compiled Sage programs.
//!
//! This crate provides the types and functions that generated Rust code
//! depends on. It handles:
//!
//! - Agent spawning and lifecycle
//! - Message passing between agents
//! - LLM inference calls
//! - Error handling

#![forbid(unsafe_code)]

mod agent;
mod error;
mod llm;

pub use agent::{spawn, AgentContext, AgentHandle};
pub use error::{SageError, SageResult};
pub use llm::LlmClient;

/// Prelude for generated code.
pub mod prelude {
    pub use crate::agent::{spawn, AgentContext, AgentHandle};
    pub use crate::error::{SageError, SageResult};
    pub use crate::llm::LlmClient;
}
