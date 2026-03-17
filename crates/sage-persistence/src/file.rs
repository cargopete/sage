//! File-based backend for persistence.
//!
//! Stores checkpoints as JSON files, one per agent. Useful for debugging
//! as the checkpoint data is human-readable.

use crate::{CheckpointStore, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;

/// File-based checkpoint store.
///
/// Each agent's state is stored in a separate JSON file named `{agent_key}.json`.
/// Uses an in-memory cache with write-through to disk.
pub struct FileStore {
    dir: PathBuf,
    cache: RwLock<HashMap<String, HashMap<String, Value>>>,
}

impl FileStore {
    /// Create a new file store in the given directory.
    pub async fn open<P: Into<PathBuf>>(dir: P) -> Result<Self> {
        let dir = dir.into();
        tokio::fs::create_dir_all(&dir).await?;

        Ok(Self {
            dir,
            cache: RwLock::new(HashMap::new()),
        })
    }

    fn agent_path(&self, agent_key: &str) -> PathBuf {
        // Sanitize agent key for filename safety
        let safe_key: String = agent_key
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
            .collect();
        self.dir.join(format!("{safe_key}.json"))
    }

    async fn read_from_disk(&self, agent_key: &str) -> Result<Option<HashMap<String, Value>>> {
        let path = self.agent_path(agent_key);

        match tokio::fs::read_to_string(&path).await {
            Ok(content) => {
                let data: HashMap<String, Value> = serde_json::from_str(&content)?;
                Ok(Some(data))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    async fn write_to_disk(&self, agent_key: &str, data: &HashMap<String, Value>) -> Result<()> {
        let path = self.agent_path(agent_key);
        let content = serde_json::to_string_pretty(data)?;

        // Write atomically via temp file + rename
        let temp_path = path.with_extension("json.tmp");
        tokio::fs::write(&temp_path, &content).await?;
        tokio::fs::rename(&temp_path, &path).await?;

        Ok(())
    }

    /// Load from disk into cache if not already cached.
    async fn ensure_cached(&self, agent_key: &str) -> Result<()> {
        let mut cache = self.cache.write().await;
        if !cache.contains_key(agent_key) {
            if let Some(data) = self.read_from_disk(agent_key).await? {
                cache.insert(agent_key.to_string(), data);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl CheckpointStore for FileStore {
    async fn save(&self, agent_key: &str, field: &str, value: Value) -> Result<()> {
        // Ensure we have the latest from disk
        self.ensure_cached(agent_key).await?;

        let mut cache = self.cache.write().await;
        let fields = cache.entry(agent_key.to_string()).or_default();
        fields.insert(field.to_string(), value);

        // Write through to disk
        self.write_to_disk(agent_key, fields).await?;

        Ok(())
    }

    async fn load(&self, agent_key: &str, field: &str) -> Result<Option<Value>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(fields) = cache.get(agent_key) {
                return Ok(fields.get(field).cloned());
            }
        }

        // Load from disk
        if let Some(fields) = self.read_from_disk(agent_key).await? {
            let value = fields.get(field).cloned();

            // Update cache
            let mut cache = self.cache.write().await;
            cache.insert(agent_key.to_string(), fields);

            return Ok(value);
        }

        Ok(None)
    }

    async fn load_all(&self, agent_key: &str) -> Result<HashMap<String, Value>> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(fields) = cache.get(agent_key) {
                return Ok(fields.clone());
            }
        }

        // Load from disk
        if let Some(fields) = self.read_from_disk(agent_key).await? {
            let mut cache = self.cache.write().await;
            cache.insert(agent_key.to_string(), fields.clone());
            return Ok(fields);
        }

        Ok(HashMap::new())
    }

    async fn save_all(&self, agent_key: &str, fields: &HashMap<String, Value>) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.insert(agent_key.to_string(), fields.clone());
        self.write_to_disk(agent_key, fields).await?;
        Ok(())
    }

    async fn delete(&self, agent_key: &str) -> Result<()> {
        let mut cache = self.cache.write().await;
        cache.remove(agent_key);

        let path = self.agent_path(agent_key);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    async fn exists(&self, agent_key: &str) -> Result<bool> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if cache.contains_key(agent_key) {
                return Ok(true);
            }
        }

        // Check disk
        let path = self.agent_path(agent_key);
        Ok(path.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    async fn temp_store() -> (FileStore, TempDir) {
        let dir = TempDir::new().unwrap();
        let store = FileStore::open(dir.path()).await.unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn save_and_load_field() {
        let (store, _dir) = temp_store().await;

        store.save("agent1", "count", json!(42)).await.unwrap();
        let loaded = store.load("agent1", "count").await.unwrap();

        assert_eq!(loaded, Some(json!(42)));
    }

    #[tokio::test]
    async fn load_missing_field_returns_none() {
        let (store, _dir) = temp_store().await;

        let loaded = store.load("agent1", "nonexistent").await.unwrap();
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn persists_to_disk() {
        let dir = TempDir::new().unwrap();

        // Write with one store instance
        {
            let store = FileStore::open(dir.path()).await.unwrap();
            store.save("agent1", "count", json!(42)).await.unwrap();
        }

        // Read with a new store instance
        {
            let store = FileStore::open(dir.path()).await.unwrap();
            let loaded = store.load("agent1", "count").await.unwrap();
            assert_eq!(loaded, Some(json!(42)));
        }
    }

    #[tokio::test]
    async fn exists_returns_false_for_new_agent() {
        let (store, _dir) = temp_store().await;

        assert!(!store.exists("new_agent").await.unwrap());
    }

    #[tokio::test]
    async fn delete_removes_file() {
        let (store, dir) = temp_store().await;

        store.save("agent1", "field", json!(1)).await.unwrap();
        assert!(store.exists("agent1").await.unwrap());

        store.delete("agent1").await.unwrap();
        assert!(!store.exists("agent1").await.unwrap());
        assert!(!dir.path().join("agent1.json").exists());
    }
}
