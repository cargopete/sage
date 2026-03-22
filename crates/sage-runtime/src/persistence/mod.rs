//! Persistence support for @persistent agent beliefs.
//!
//! This module provides the runtime support for persistent agent state:
//! - `CheckpointStore` trait for storage backends
//! - `Persisted<T>` wrapper for auto-checkpointing fields
//! - `AgentCheckpoint` for managing agent-level persistence
//!
//! # Backends
//!
//! The following backends are available via feature flags:
//! - `persistence-sqlite`: SQLite database (recommended for local development)
//! - `persistence-postgres`: PostgreSQL (recommended for production)
//! - `persistence-file`: JSON files (useful for debugging)
//!
//! Without any persistence feature, only `MemoryCheckpointStore` is available.

// Sync adapters for async persistence backends
#[cfg(any(
    feature = "persistence-sqlite",
    feature = "persistence-postgres",
    feature = "persistence-file"
))]
mod backends;

#[cfg(feature = "persistence-file")]
pub use backends::SyncFileStore;
#[cfg(feature = "persistence-postgres")]
pub use backends::SyncPostgresStore;
#[cfg(feature = "persistence-sqlite")]
pub use backends::SyncSqliteStore;

use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A checkpoint store for persisting agent state.
///
/// This is a re-export of the trait from sage-persistence, simplified
/// for use in generated code.
pub trait CheckpointStore: Send + Sync {
    /// Save a field value synchronously (blocks on async).
    fn save_sync(&self, agent_key: &str, field: &str, value: serde_json::Value);

    /// Load a field value synchronously.
    fn load_sync(&self, agent_key: &str, field: &str) -> Option<serde_json::Value>;

    /// Load all fields for an agent.
    fn load_all_sync(&self, agent_key: &str) -> HashMap<String, serde_json::Value>;

    /// Save all fields atomically.
    fn save_all_sync(&self, agent_key: &str, fields: &HashMap<String, serde_json::Value>);

    /// Check if any checkpoint exists for an agent.
    fn exists_sync(&self, agent_key: &str) -> bool;
}

/// In-memory checkpoint store for testing.
#[derive(Default)]
pub struct MemoryCheckpointStore {
    data: RwLock<HashMap<String, HashMap<String, serde_json::Value>>>,
}

impl MemoryCheckpointStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CheckpointStore for MemoryCheckpointStore {
    fn save_sync(&self, agent_key: &str, field: &str, value: serde_json::Value) {
        let mut data = self.data.write().unwrap();
        data.entry(agent_key.to_string())
            .or_default()
            .insert(field.to_string(), value);
    }

    fn load_sync(&self, agent_key: &str, field: &str) -> Option<serde_json::Value> {
        self.data
            .read()
            .unwrap()
            .get(agent_key)
            .and_then(|fields| fields.get(field).cloned())
    }

    fn load_all_sync(&self, agent_key: &str) -> HashMap<String, serde_json::Value> {
        self.data
            .read()
            .unwrap()
            .get(agent_key)
            .cloned()
            .unwrap_or_default()
    }

    fn save_all_sync(&self, agent_key: &str, fields: &HashMap<String, serde_json::Value>) {
        let mut data = self.data.write().unwrap();
        data.insert(agent_key.to_string(), fields.clone());
    }

    fn exists_sync(&self, agent_key: &str) -> bool {
        self.data.read().unwrap().contains_key(agent_key)
    }
}

/// A wrapper for @persistent fields that auto-checkpoints on modification.
///
/// This provides interior mutability and automatic persistence when the
/// value is modified via `set()`.
pub struct Persisted<T> {
    value: RwLock<T>,
    store: Arc<dyn CheckpointStore>,
    agent_key: String,
    field_name: String,
}

impl<T: Clone + Serialize + DeserializeOwned + Default + Send> Persisted<T> {
    /// Create a new persisted field, loading from checkpoint if available.
    pub fn new(
        store: Arc<dyn CheckpointStore>,
        agent_key: impl Into<String>,
        field_name: impl Into<String>,
    ) -> Self {
        let agent_key = agent_key.into();
        let field_name = field_name.into();

        // Try to load from checkpoint
        let value = store
            .load_sync(&agent_key, &field_name)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        Self {
            value: RwLock::new(value),
            store,
            agent_key,
            field_name,
        }
    }

    /// Create with an explicit initial value (used when no checkpoint exists).
    pub fn with_initial(
        store: Arc<dyn CheckpointStore>,
        agent_key: impl Into<String>,
        field_name: impl Into<String>,
        initial: T,
    ) -> Self {
        let agent_key = agent_key.into();
        let field_name = field_name.into();

        // Try to load from checkpoint, fall back to initial
        let value = store
            .load_sync(&agent_key, &field_name)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or(initial);

        Self {
            value: RwLock::new(value),
            store,
            agent_key,
            field_name,
        }
    }

    /// Get the current value.
    pub fn get(&self) -> T {
        self.value.read().unwrap().clone()
    }

    /// Set the value and checkpoint it.
    pub fn set(&self, new_value: T) {
        *self.value.write().unwrap() = new_value.clone();
        if let Ok(json) = serde_json::to_value(&new_value) {
            self.store
                .save_sync(&self.agent_key, &self.field_name, json);
        }
    }

    /// Checkpoint the current value without modifying it.
    pub fn checkpoint(&self) {
        let value = self.value.read().unwrap().clone();
        if let Ok(json) = serde_json::to_value(&value) {
            self.store
                .save_sync(&self.agent_key, &self.field_name, json);
        }
    }
}

/// Helper to generate a unique checkpoint key for an agent instance.
pub fn agent_checkpoint_key(agent_name: &str, beliefs: &serde_json::Value) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    agent_name.hash(&mut hasher);
    beliefs.to_string().hash(&mut hasher);
    format!("{}_{:016x}", agent_name, hasher.finish())
}

/// Helper to save all @persistent fields atomically before yield.
pub fn checkpoint_all<S: CheckpointStore + ?Sized>(
    store: &S,
    agent_key: &str,
    fields: Vec<(&str, serde_json::Value)>,
) {
    let map: HashMap<String, serde_json::Value> = fields
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    store.save_all_sync(agent_key, &map);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> Arc<dyn CheckpointStore> {
        Arc::new(MemoryCheckpointStore::new())
    }

    #[test]
    fn memory_store_save_load() {
        let store = MemoryCheckpointStore::new();
        store.save_sync("agent1", "count", serde_json::json!(42));

        let loaded = store.load_sync("agent1", "count");
        assert_eq!(loaded, Some(serde_json::json!(42)));
    }

    #[test]
    fn persisted_field_loads_from_checkpoint() {
        let store = make_store();
        store.save_sync("agent1", "count", serde_json::json!(100));

        let field: Persisted<i64> = Persisted::new(store, "agent1", "count");
        assert_eq!(field.get(), 100);
    }

    #[test]
    fn persisted_field_defaults_when_no_checkpoint() {
        let store = make_store();
        let field: Persisted<i64> = Persisted::new(store, "agent1", "count");
        assert_eq!(field.get(), 0); // Default for i64
    }

    #[test]
    fn persisted_field_auto_checkpoints_on_set() {
        let store = make_store();
        let field: Persisted<i64> = Persisted::new(Arc::clone(&store), "agent1", "count");

        field.set(42);

        // Verify it was persisted
        let loaded = store.load_sync("agent1", "count");
        assert_eq!(loaded, Some(serde_json::json!(42)));
    }

    #[test]
    fn checkpoint_key_varies_with_beliefs() {
        let key1 = agent_checkpoint_key("Agent", &serde_json::json!({"x": 1}));
        let key2 = agent_checkpoint_key("Agent", &serde_json::json!({"x": 2}));
        assert_ne!(key1, key2);
    }
}
