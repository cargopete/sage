//! SQLite backend for persistence.
//!
//! This is the default backend for local development and single-instance deployments.

use crate::{CheckpointStore, PersistenceError, Result};
use async_trait::async_trait;
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

/// SQLite-based checkpoint store.
///
/// Uses a single table with (agent_key, field, value) schema.
/// All operations are atomic via SQLite transactions.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open or create a SQLite checkpoint database.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(path).map_err(|e| {
            PersistenceError::ConnectionFailed(format!("failed to open SQLite: {e}"))
        })?;

        // Create table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                agent_key TEXT NOT NULL,
                field TEXT NOT NULL,
                value TEXT NOT NULL,
                updated_at INTEGER DEFAULT (strftime('%s', 'now')),
                PRIMARY KEY (agent_key, field)
            )",
            [],
        )
        .map_err(|e| PersistenceError::WriteFailed(format!("failed to create table: {e}")))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Create an in-memory SQLite store for testing.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| PersistenceError::ConnectionFailed(format!("in-memory SQLite: {e}")))?;

        conn.execute(
            "CREATE TABLE checkpoints (
                agent_key TEXT NOT NULL,
                field TEXT NOT NULL,
                value TEXT NOT NULL,
                updated_at INTEGER DEFAULT (strftime('%s', 'now')),
                PRIMARY KEY (agent_key, field)
            )",
            [],
        )
        .map_err(|e| PersistenceError::WriteFailed(format!("failed to create table: {e}")))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

#[async_trait]
impl CheckpointStore for SqliteStore {
    async fn save(&self, agent_key: &str, field: &str, value: Value) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let value_str = serde_json::to_string(&value)?;

        conn.execute(
            "INSERT OR REPLACE INTO checkpoints (agent_key, field, value, updated_at)
             VALUES (?1, ?2, ?3, strftime('%s', 'now'))",
            params![agent_key, field, value_str],
        )
        .map_err(|e| PersistenceError::WriteFailed(format!("save failed: {e}")))?;

        Ok(())
    }

    async fn load(&self, agent_key: &str, field: &str) -> Result<Option<Value>> {
        let conn = self.conn.lock().unwrap();

        let result: std::result::Result<String, rusqlite::Error> = conn.query_row(
            "SELECT value FROM checkpoints WHERE agent_key = ?1 AND field = ?2",
            params![agent_key, field],
            |row| row.get(0),
        );

        match result {
            Ok(value_str) => {
                let value = serde_json::from_str(&value_str)?;
                Ok(Some(value))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(PersistenceError::ReadFailed(format!("load failed: {e}"))),
        }
    }

    async fn load_all(&self, agent_key: &str) -> Result<HashMap<String, Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT field, value FROM checkpoints WHERE agent_key = ?1")
            .map_err(|e| PersistenceError::ReadFailed(format!("prepare failed: {e}")))?;

        let rows = stmt
            .query_map(params![agent_key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| PersistenceError::ReadFailed(format!("query failed: {e}")))?;

        let mut fields = HashMap::new();
        for row in rows {
            let (field, value_str) =
                row.map_err(|e| PersistenceError::ReadFailed(format!("row error: {e}")))?;
            let value = serde_json::from_str(&value_str)?;
            fields.insert(field, value);
        }

        Ok(fields)
    }

    async fn save_all(&self, agent_key: &str, fields: &HashMap<String, Value>) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn
            .transaction()
            .map_err(|e| PersistenceError::WriteFailed(format!("transaction failed: {e}")))?;

        for (field, value) in fields {
            let value_str = serde_json::to_string(value)?;
            tx.execute(
                "INSERT OR REPLACE INTO checkpoints (agent_key, field, value, updated_at)
                 VALUES (?1, ?2, ?3, strftime('%s', 'now'))",
                params![agent_key, field, value_str],
            )
            .map_err(|e| PersistenceError::WriteFailed(format!("save_all failed: {e}")))?;
        }

        tx.commit()
            .map_err(|e| PersistenceError::WriteFailed(format!("commit failed: {e}")))?;

        Ok(())
    }

    async fn delete(&self, agent_key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM checkpoints WHERE agent_key = ?1",
            params![agent_key],
        )
        .map_err(|e| PersistenceError::DeleteFailed(format!("delete failed: {e}")))?;

        Ok(())
    }

    async fn exists(&self, agent_key: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM checkpoints WHERE agent_key = ?1",
                params![agent_key],
                |row| row.get(0),
            )
            .map_err(|e| PersistenceError::ReadFailed(format!("exists check failed: {e}")))?;

        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn save_and_load_field() {
        let store = SqliteStore::in_memory().unwrap();

        store.save("agent1", "count", json!(42)).await.unwrap();
        let loaded = store.load("agent1", "count").await.unwrap();

        assert_eq!(loaded, Some(json!(42)));
    }

    #[tokio::test]
    async fn load_missing_field_returns_none() {
        let store = SqliteStore::in_memory().unwrap();

        let loaded = store.load("agent1", "nonexistent").await.unwrap();
        assert_eq!(loaded, None);
    }

    #[tokio::test]
    async fn save_all_is_atomic() {
        let store = SqliteStore::in_memory().unwrap();

        let mut fields = HashMap::new();
        fields.insert("a".to_string(), json!(1));
        fields.insert("b".to_string(), json!(2));
        fields.insert("c".to_string(), json!(3));

        store.save_all("agent1", &fields).await.unwrap();

        let loaded = store.load_all("agent1").await.unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.get("a"), Some(&json!(1)));
        assert_eq!(loaded.get("b"), Some(&json!(2)));
        assert_eq!(loaded.get("c"), Some(&json!(3)));
    }

    #[tokio::test]
    async fn exists_returns_false_for_new_agent() {
        let store = SqliteStore::in_memory().unwrap();

        assert!(!store.exists("new_agent").await.unwrap());
    }

    #[tokio::test]
    async fn exists_returns_true_after_save() {
        let store = SqliteStore::in_memory().unwrap();

        store.save("agent1", "field", json!("value")).await.unwrap();
        assert!(store.exists("agent1").await.unwrap());
    }

    #[tokio::test]
    async fn delete_removes_all_fields() {
        let store = SqliteStore::in_memory().unwrap();

        store.save("agent1", "a", json!(1)).await.unwrap();
        store.save("agent1", "b", json!(2)).await.unwrap();

        store.delete("agent1").await.unwrap();

        assert!(!store.exists("agent1").await.unwrap());
    }

    #[tokio::test]
    async fn complex_values() {
        let store = SqliteStore::in_memory().unwrap();

        let complex = json!({
            "list": [1, 2, 3],
            "nested": {"a": "b"},
            "null": null
        });

        store.save("agent1", "data", complex.clone()).await.unwrap();
        let loaded = store.load("agent1", "data").await.unwrap();

        assert_eq!(loaded, Some(complex));
    }
}
