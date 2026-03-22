//! Persistence layer for Sage agent checkpoints.
//!
//! This crate provides durable storage for `@persistent` agent fields, enabling
//! agents to recover their state after restarts, crashes, or process exits.
//!
//! # Backends
//!
//! - `sqlite` (default): Local SQLite database
//! - `postgres`: PostgreSQL for production deployments
//! - `file`: JSON files for development/debugging
//!
//! # Example
//!
//! ```ignore
//! use sage_persistence::{CheckpointStore, SqliteStore};
//!
//! let store = SqliteStore::open(".sage/checkpoints.db").await?;
//! store.save("agent_key", "field", serde_json::json!(42)).await?;
//! let value = store.load("agent_key", "field").await?;
//! ```

#![forbid(unsafe_code)]

mod error;
#[cfg(feature = "file")]
mod file;
#[cfg(feature = "postgres")]
mod postgres;
#[cfg(feature = "sqlite")]
mod sqlite;

pub use error::{PersistenceError, Result};

#[cfg(feature = "file")]
pub use file::FileStore;
#[cfg(feature = "postgres")]
pub use postgres::PostgresStore;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStore;

use async_trait::async_trait;
use serde_json::Value;

/// A checkpoint store for persisting agent state.
///
/// Implementations provide durable storage with atomic checkpoint semantics.
/// A failed checkpoint should not corrupt previously stored data.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Save a field value for an agent.
    ///
    /// The operation is atomic — either the entire value is persisted or nothing is.
    async fn save(&self, agent_key: &str, field: &str, value: Value) -> Result<()>;

    /// Load a field value for an agent.
    ///
    /// Returns `None` if the field has never been persisted.
    async fn load(&self, agent_key: &str, field: &str) -> Result<Option<Value>>;

    /// Load all persistent fields for an agent.
    ///
    /// Returns a map of field names to their values.
    async fn load_all(&self, agent_key: &str) -> Result<std::collections::HashMap<String, Value>>;

    /// Save all persistent fields for an agent atomically.
    ///
    /// This is the preferred method for checkpoint operations as it ensures
    /// consistency across all fields.
    async fn save_all(
        &self,
        agent_key: &str,
        fields: &std::collections::HashMap<String, Value>,
    ) -> Result<()>;

    /// Delete all checkpointed data for an agent.
    async fn delete(&self, agent_key: &str) -> Result<()>;

    /// Check if any checkpoint exists for an agent.
    ///
    /// Used for first-run detection.
    async fn exists(&self, agent_key: &str) -> Result<bool>;
}

/// Configuration for the persistence layer.
#[derive(Debug, Clone)]
pub struct PersistenceConfig {
    /// The backend type to use.
    pub backend: Backend,
    /// Path for file-based backends (sqlite, file).
    pub path: Option<String>,
    /// Connection URL for networked backends (postgres).
    pub url: Option<String>,
}

/// Available persistence backends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    Sqlite,
    Postgres,
    File,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            backend: Backend::Sqlite,
            path: Some(".sage/checkpoints.db".to_string()),
            url: None,
        }
    }
}

/// Generate a unique checkpoint key for an agent instance.
///
/// The key is derived from the agent name and its initial belief values,
/// ensuring that agents with different initial state have separate namespaces.
pub fn agent_checkpoint_key(agent_name: &str, initial_beliefs: &Value) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    agent_name.hash(&mut hasher);
    initial_beliefs.to_string().hash(&mut hasher);
    format!("{}_{:016x}", agent_name, hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn checkpoint_key_different_for_different_beliefs() {
        let key1 = agent_checkpoint_key("Agent", &json!({"x": 1}));
        let key2 = agent_checkpoint_key("Agent", &json!({"x": 2}));
        assert_ne!(key1, key2);
    }

    #[test]
    fn checkpoint_key_same_for_same_beliefs() {
        let key1 = agent_checkpoint_key("Agent", &json!({"x": 1}));
        let key2 = agent_checkpoint_key("Agent", &json!({"x": 1}));
        assert_eq!(key1, key2);
    }

    #[test]
    fn checkpoint_key_different_for_different_agents() {
        let key1 = agent_checkpoint_key("Agent1", &json!({"x": 1}));
        let key2 = agent_checkpoint_key("Agent2", &json!({"x": 1}));
        assert_ne!(key1, key2);
    }
}
