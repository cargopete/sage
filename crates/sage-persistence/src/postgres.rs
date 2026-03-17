//! PostgreSQL backend for persistence.
//!
//! For production deployments requiring durability and concurrent access.

use crate::{CheckpointStore, PersistenceError, Result};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;

/// PostgreSQL-based checkpoint store.
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    /// Connect to a PostgreSQL database.
    ///
    /// The connection string should be a standard PostgreSQL URL:
    /// `postgres://user:password@host/database`
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(|e| PersistenceError::ConnectionFailed(format!("PostgreSQL: {e}")))?;

        // Create table if it doesn't exist
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sage_checkpoints (
                agent_key TEXT NOT NULL,
                field TEXT NOT NULL,
                value JSONB NOT NULL,
                updated_at TIMESTAMPTZ DEFAULT NOW(),
                PRIMARY KEY (agent_key, field)
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| PersistenceError::WriteFailed(format!("create table: {e}")))?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl CheckpointStore for PostgresStore {
    async fn save(&self, agent_key: &str, field: &str, value: Value) -> Result<()> {
        sqlx::query(
            "INSERT INTO sage_checkpoints (agent_key, field, value, updated_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (agent_key, field) DO UPDATE SET value = $3, updated_at = NOW()",
        )
        .bind(agent_key)
        .bind(field)
        .bind(&value)
        .execute(&self.pool)
        .await
        .map_err(|e| PersistenceError::WriteFailed(format!("save: {e}")))?;

        Ok(())
    }

    async fn load(&self, agent_key: &str, field: &str) -> Result<Option<Value>> {
        let row: Option<(Value,)> = sqlx::query_as(
            "SELECT value FROM sage_checkpoints WHERE agent_key = $1 AND field = $2",
        )
        .bind(agent_key)
        .bind(field)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::ReadFailed(format!("load: {e}")))?;

        Ok(row.map(|(v,)| v))
    }

    async fn load_all(&self, agent_key: &str) -> Result<HashMap<String, Value>> {
        let rows: Vec<(String, Value)> =
            sqlx::query_as("SELECT field, value FROM sage_checkpoints WHERE agent_key = $1")
                .bind(agent_key)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PersistenceError::ReadFailed(format!("load_all: {e}")))?;

        Ok(rows.into_iter().collect())
    }

    async fn save_all(&self, agent_key: &str, fields: &HashMap<String, Value>) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PersistenceError::WriteFailed(format!("begin tx: {e}")))?;

        for (field, value) in fields {
            sqlx::query(
                "INSERT INTO sage_checkpoints (agent_key, field, value, updated_at)
                 VALUES ($1, $2, $3, NOW())
                 ON CONFLICT (agent_key, field) DO UPDATE SET value = $3, updated_at = NOW()",
            )
            .bind(agent_key)
            .bind(field)
            .bind(value)
            .execute(&mut *tx)
            .await
            .map_err(|e| PersistenceError::WriteFailed(format!("save_all: {e}")))?;
        }

        tx.commit()
            .await
            .map_err(|e| PersistenceError::WriteFailed(format!("commit: {e}")))?;

        Ok(())
    }

    async fn delete(&self, agent_key: &str) -> Result<()> {
        sqlx::query("DELETE FROM sage_checkpoints WHERE agent_key = $1")
            .bind(agent_key)
            .execute(&self.pool)
            .await
            .map_err(|e| PersistenceError::DeleteFailed(format!("delete: {e}")))?;

        Ok(())
    }

    async fn exists(&self, agent_key: &str) -> Result<bool> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM sage_checkpoints WHERE agent_key = $1 LIMIT 1",
        )
        .bind(agent_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PersistenceError::ReadFailed(format!("exists: {e}")))?;

        Ok(row.map(|(c,)| c > 0).unwrap_or(false))
    }
}
