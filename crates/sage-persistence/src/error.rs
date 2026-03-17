//! Error types for the persistence layer.

use thiserror::Error;

/// Result type for persistence operations.
pub type Result<T> = std::result::Result<T, PersistenceError>;

/// Errors that can occur during persistence operations.
#[derive(Debug, Error)]
pub enum PersistenceError {
    /// Failed to connect to the storage backend.
    #[error("failed to connect to storage: {0}")]
    ConnectionFailed(String),

    /// Failed to read from storage.
    #[error("failed to read checkpoint: {0}")]
    ReadFailed(String),

    /// Failed to write to storage.
    #[error("failed to write checkpoint: {0}")]
    WriteFailed(String),

    /// Failed to serialize data.
    #[error("serialization failed: {0}")]
    SerializationFailed(#[from] serde_json::Error),

    /// Failed to delete checkpoint data.
    #[error("failed to delete checkpoint: {0}")]
    DeleteFailed(String),

    /// I/O error (for file backend).
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// SQLite error.
    #[cfg(feature = "sqlite")]
    #[error("SQLite error: {0}")]
    SqliteError(#[from] rusqlite::Error),

    /// PostgreSQL error.
    #[cfg(feature = "postgres")]
    #[error("PostgreSQL error: {0}")]
    PostgresError(#[from] sqlx::Error),
}
