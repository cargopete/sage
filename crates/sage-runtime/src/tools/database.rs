//! RFC-0011: Database tool for Sage agents.
//!
//! Provides the `Database` tool with SQL query capabilities.
//! Requires the `database` feature to be enabled.

use crate::error::{SageError, SageResult};
use crate::mock::{try_get_mock, MockResponse};

#[cfg(feature = "database")]
use sqlx::{any::AnyRow, AnyPool, Column, Row};

/// A row returned from a database query.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DbRow {
    /// Column names.
    pub columns: Vec<String>,
    /// Values as strings.
    pub values: Vec<String>,
}

/// Database client for Sage agents.
///
/// Requires the `database` feature to be enabled.
#[derive(Debug, Clone)]
pub struct DatabaseClient {
    #[cfg(feature = "database")]
    pool: AnyPool,
    #[cfg(not(feature = "database"))]
    _marker: std::marker::PhantomData<()>,
}

impl DatabaseClient {
    /// Create a new database client by connecting to the given URL.
    ///
    /// # Arguments
    /// * `url` - Database connection URL (e.g., "postgres://localhost/db" or "sqlite::memory:")
    #[cfg(feature = "database")]
    pub async fn connect(url: &str) -> SageResult<Self> {
        // Install default drivers
        sqlx::any::install_default_drivers();

        let pool = AnyPool::connect(url)
            .await
            .map_err(|e| SageError::Tool(format!("Database connection failed: {e}")))?;
        Ok(Self { pool })
    }

    /// Create a new database client by connecting to the given URL.
    #[cfg(not(feature = "database"))]
    pub async fn connect(_url: &str) -> SageResult<Self> {
        Err(SageError::Tool(
            "Database support not enabled. Compile with the 'database' feature.".to_string(),
        ))
    }

    /// Create a new database client from environment variables.
    ///
    /// Reads:
    /// - `SAGE_DATABASE_URL`: Database connection URL (required)
    #[cfg(feature = "database")]
    pub async fn from_env() -> SageResult<Self> {
        let url = std::env::var("SAGE_DATABASE_URL").map_err(|_| {
            SageError::Tool("SAGE_DATABASE_URL environment variable not set".to_string())
        })?;
        Self::connect(&url).await
    }

    /// Create a new database client from environment variables.
    #[cfg(not(feature = "database"))]
    pub async fn from_env() -> SageResult<Self> {
        Err(SageError::Tool(
            "Database support not enabled. Compile with the 'database' feature.".to_string(),
        ))
    }

    /// Execute a SQL query and return the results.
    ///
    /// # Arguments
    /// * `sql` - The SQL query to execute
    ///
    /// # Returns
    /// A list of rows, each containing column names and values.
    #[cfg(feature = "database")]
    pub async fn query(&self, sql: String) -> SageResult<Vec<DbRow>> {
        // Check for mock response first
        if let Some(mock_response) = try_get_mock("Database", "query") {
            return Self::apply_mock_vec(mock_response);
        }

        let rows: Vec<AnyRow> = sqlx::query(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SageError::Tool(format!("Query failed: {e}")))?;

        let result: Vec<DbRow> = rows
            .iter()
            .map(|row| {
                let columns: Vec<String> =
                    row.columns().iter().map(|c| c.name().to_string()).collect();
                let values: Vec<String> = (0..row.columns().len())
                    .map(|i| {
                        // Try to get the value as different types
                        if let Ok(v) = row.try_get::<String, _>(i) {
                            v
                        } else if let Ok(v) = row.try_get::<i64, _>(i) {
                            v.to_string()
                        } else if let Ok(v) = row.try_get::<i32, _>(i) {
                            v.to_string()
                        } else if let Ok(v) = row.try_get::<f64, _>(i) {
                            v.to_string()
                        } else if let Ok(v) = row.try_get::<bool, _>(i) {
                            v.to_string()
                        } else {
                            // Fallback: try to get raw value as Option<String>
                            row.try_get::<Option<String>, _>(i)
                                .ok()
                                .flatten()
                                .unwrap_or_else(|| "null".to_string())
                        }
                    })
                    .collect();
                DbRow { columns, values }
            })
            .collect();

        Ok(result)
    }

    /// Execute a SQL query and return the results.
    #[cfg(not(feature = "database"))]
    pub async fn query(&self, _sql: String) -> SageResult<Vec<DbRow>> {
        // Check for mock response first (allows testing without database feature)
        if let Some(mock_response) = try_get_mock("Database", "query") {
            return Self::apply_mock_vec(mock_response);
        }

        Err(SageError::Tool(
            "Database support not enabled. Compile with the 'database' feature.".to_string(),
        ))
    }

    /// Execute a SQL statement (INSERT, UPDATE, DELETE) and return affected row count.
    ///
    /// # Arguments
    /// * `sql` - The SQL statement to execute
    ///
    /// # Returns
    /// Number of rows affected.
    #[cfg(feature = "database")]
    pub async fn execute(&self, sql: String) -> SageResult<i64> {
        // Check for mock response first
        if let Some(mock_response) = try_get_mock("Database", "execute") {
            return Self::apply_mock_i64(mock_response);
        }

        let result = sqlx::query(&sql)
            .execute(&self.pool)
            .await
            .map_err(|e| SageError::Tool(format!("Execute failed: {e}")))?;

        Ok(result.rows_affected() as i64)
    }

    /// Execute a SQL statement and return affected row count.
    #[cfg(not(feature = "database"))]
    pub async fn execute(&self, _sql: String) -> SageResult<i64> {
        // Check for mock response first (allows testing without database feature)
        if let Some(mock_response) = try_get_mock("Database", "execute") {
            return Self::apply_mock_i64(mock_response);
        }

        Err(SageError::Tool(
            "Database support not enabled. Compile with the 'database' feature.".to_string(),
        ))
    }

    /// Apply a mock response for Vec<DbRow>.
    fn apply_mock_vec(mock_response: MockResponse) -> SageResult<Vec<DbRow>> {
        match mock_response {
            MockResponse::Value(v) => serde_json::from_value(v)
                .map_err(|e| SageError::Tool(format!("mock deserialize: {e}"))),
            MockResponse::Fail(msg) => Err(SageError::Tool(msg)),
        }
    }

    /// Apply a mock response for i64.
    fn apply_mock_i64(mock_response: MockResponse) -> SageResult<i64> {
        match mock_response {
            MockResponse::Value(v) => serde_json::from_value(v)
                .map_err(|e| SageError::Tool(format!("mock deserialize: {e}"))),
            MockResponse::Fail(msg) => Err(SageError::Tool(msg)),
        }
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn database_connect_sqlite() {
        // Use shared cache mode for in-memory database
        let client = DatabaseClient::connect("sqlite:file::memory:?mode=memory&cache=shared")
            .await
            .unwrap();
        drop(client);
    }

    #[tokio::test]
    async fn database_execute_and_query() {
        // Use a temporary file-based database for this test to avoid pool issues
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        // Create the file first
        std::fs::write(&db_path, "").unwrap();
        let url = format!("sqlite:{}?mode=rwc", db_path.display());

        let client = DatabaseClient::connect(&url).await.unwrap();

        // Create a table
        client
            .execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)".to_string())
            .await
            .unwrap();

        // Insert data
        let affected = client
            .execute("INSERT INTO test (id, name) VALUES (1, 'Alice'), (2, 'Bob')".to_string())
            .await
            .unwrap();
        assert_eq!(affected, 2);

        // Query data
        let rows = client
            .query("SELECT id, name FROM test ORDER BY id".to_string())
            .await
            .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].columns, vec!["id", "name"]);
        assert_eq!(rows[0].values, vec!["1", "Alice"]);
        assert_eq!(rows[1].values, vec!["2", "Bob"]);
    }

    #[tokio::test]
    async fn database_query_select_one() {
        let client = DatabaseClient::connect("sqlite:file::memory:?mode=memory&cache=shared")
            .await
            .unwrap();
        let rows = client.query("SELECT 1 as value".to_string()).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].columns, vec!["value"]);
        assert_eq!(rows[0].values, vec!["1"]);
    }
}
