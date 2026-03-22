//! Project manifest (grove.toml) parsing.

use crate::error::LoadError;
use sage_package::{parse_dependencies, DependencySpec};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A Sage project manifest (grove.toml).
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectManifest {
    pub project: ProjectConfig,
    #[serde(default)]
    pub dependencies: toml::Table,
    #[serde(default)]
    pub test: TestConfig,
    #[serde(default)]
    pub tools: ToolsConfig,
    #[serde(default)]
    pub persistence: PersistenceConfig,
    #[serde(default)]
    pub supervision: SupervisionConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
}

/// Tool configuration section of grove.toml.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolsConfig {
    pub database: Option<DatabaseToolConfig>,
    pub http: Option<HttpToolConfig>,
    pub filesystem: Option<FileSystemToolConfig>,
}

/// Database tool configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseToolConfig {
    /// Database driver: "postgres", "sqlite", etc.
    pub driver: String,
    /// Connection URL.
    pub url: String,
    /// Connection pool size.
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
}

fn default_pool_size() -> u32 {
    5
}

/// HTTP tool configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct HttpToolConfig {
    /// Request timeout in milliseconds.
    #[serde(default = "default_http_timeout")]
    pub timeout_ms: u64,
}

fn default_http_timeout() -> u64 {
    30_000 // 30 seconds
}

/// FileSystem tool configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct FileSystemToolConfig {
    /// Root directory for filesystem operations.
    pub root: PathBuf,
}

/// Persistence configuration for @persistent agent fields.
#[derive(Debug, Clone, Deserialize)]
pub struct PersistenceConfig {
    /// Storage backend: "sqlite" (default), "postgres", or "file".
    #[serde(default = "default_persistence_backend")]
    pub backend: String,
    /// Path for file-based backends (sqlite, file).
    #[serde(default = "default_persistence_path")]
    pub path: String,
    /// Connection URL for postgres backend.
    #[serde(default)]
    pub url: Option<String>,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            backend: default_persistence_backend(),
            path: default_persistence_path(),
            url: None,
        }
    }
}

fn default_persistence_backend() -> String {
    "sqlite".to_string()
}

fn default_persistence_path() -> String {
    ".sage/checkpoints.db".to_string()
}

/// Supervision configuration for supervisor restart intensity limiting.
#[derive(Debug, Clone, Deserialize)]
pub struct SupervisionConfig {
    /// Maximum number of restarts allowed within the time window.
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
    /// Time window in seconds for restart counting.
    #[serde(default = "default_within_seconds")]
    pub within_seconds: u64,
}

impl Default for SupervisionConfig {
    fn default() -> Self {
        Self {
            max_restarts: default_max_restarts(),
            within_seconds: default_within_seconds(),
        }
    }
}

fn default_max_restarts() -> u32 {
    5
}

fn default_within_seconds() -> u64 {
    60
}

/// Observability configuration for tracing and metrics export.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    /// Tracing backend: "ndjson" (default), "otlp", or "none".
    #[serde(default = "default_observability_backend")]
    pub backend: String,
    /// OTLP endpoint for trace export (when backend = "otlp").
    #[serde(default)]
    pub otlp_endpoint: Option<String>,
    /// Service name for trace attribution.
    #[serde(default = "default_service_name_option")]
    pub service_name: Option<String>,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            backend: default_observability_backend(),
            otlp_endpoint: None,
            service_name: default_service_name_option(),
        }
    }
}

fn default_observability_backend() -> String {
    "ndjson".to_string()
}

fn default_service_name_option() -> Option<String> {
    Some("sage-agent".to_string())
}

/// The [test] section of grove.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct TestConfig {
    /// Test timeout in milliseconds (default: 10000)
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            timeout_ms: default_timeout_ms(),
        }
    }
}

fn default_timeout_ms() -> u64 {
    10_000 // 10 seconds
}

/// The [project] section of grove.toml.
#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_entry")]
    pub entry: PathBuf,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_entry() -> PathBuf {
    PathBuf::from("src/main.sg")
}

impl ProjectManifest {
    /// Load a manifest from a grove.toml file.
    pub fn load(path: &Path) -> Result<Self, Box<LoadError>> {
        let contents = std::fs::read_to_string(path).map_err(|e| {
            Box::new(LoadError::IoError {
                path: path.to_path_buf(),
                source: e,
            })
        })?;

        toml::from_str(&contents).map_err(|e| {
            Box::new(LoadError::InvalidManifest {
                path: path.to_path_buf(),
                source: e,
            })
        })
    }

    /// Find a grove.toml file by searching upward from the given directory.
    /// Falls back to sage.toml for backwards compatibility.
    pub fn find(start_dir: &Path) -> Option<PathBuf> {
        let mut current = start_dir.to_path_buf();
        loop {
            // Try grove.toml first
            let grove_path = current.join("grove.toml");
            if grove_path.exists() {
                return Some(grove_path);
            }
            // Fall back to sage.toml (deprecated)
            let sage_path = current.join("sage.toml");
            if sage_path.exists() {
                return Some(sage_path);
            }
            if !current.pop() {
                return None;
            }
        }
    }

    /// Check if the project has any dependencies declared.
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }

    /// Parse the dependencies table into structured specs.
    pub fn parse_dependencies(&self) -> Result<HashMap<String, DependencySpec>, Box<LoadError>> {
        parse_dependencies(&self.dependencies)
            .map_err(|e| Box::new(LoadError::PackageError { source: e }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let toml = r#"
[project]
name = "test"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.project.name, "test");
        assert_eq!(manifest.project.version, "0.1.0");
        assert_eq!(manifest.project.entry, PathBuf::from("src/main.sg"));
    }

    #[test]
    fn parse_full_manifest() {
        let toml = r#"
[project]
name = "research"
version = "1.2.3"
entry = "src/app.sg"

[dependencies]
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.project.name, "research");
        assert_eq!(manifest.project.version, "1.2.3");
        assert_eq!(manifest.project.entry, PathBuf::from("src/app.sg"));
    }

    #[test]
    fn parse_test_config_default() {
        let toml = r#"
[project]
name = "test"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.test.timeout_ms, 10_000);
    }

    #[test]
    fn parse_test_config_custom_timeout() {
        let toml = r#"
[project]
name = "test"

[test]
timeout_ms = 30000
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.test.timeout_ms, 30_000);
    }

    #[test]
    fn parse_tools_config_default() {
        let toml = r#"
[project]
name = "test"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert!(manifest.tools.database.is_none());
        assert!(manifest.tools.http.is_none());
        assert!(manifest.tools.filesystem.is_none());
    }

    #[test]
    fn parse_tools_database_config() {
        let toml = r#"
[project]
name = "test"

[tools.database]
driver = "postgres"
url = "postgresql://localhost/mydb"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        let db = manifest.tools.database.unwrap();
        assert_eq!(db.driver, "postgres");
        assert_eq!(db.url, "postgresql://localhost/mydb");
        assert_eq!(db.pool_size, 5); // default
    }

    #[test]
    fn parse_tools_database_config_with_pool() {
        let toml = r#"
[project]
name = "test"

[tools.database]
driver = "sqlite"
url = ":memory:"
pool_size = 10
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        let db = manifest.tools.database.unwrap();
        assert_eq!(db.driver, "sqlite");
        assert_eq!(db.pool_size, 10);
    }

    #[test]
    fn parse_tools_http_config() {
        let toml = r#"
[project]
name = "test"

[tools.http]
timeout_ms = 60000
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        let http = manifest.tools.http.unwrap();
        assert_eq!(http.timeout_ms, 60_000);
    }

    #[test]
    fn parse_tools_filesystem_config() {
        let toml = r#"
[project]
name = "test"

[tools.filesystem]
root = "/var/data"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        let fs = manifest.tools.filesystem.unwrap();
        assert_eq!(fs.root, PathBuf::from("/var/data"));
    }

    #[test]
    fn parse_tools_all_configs() {
        let toml = r#"
[project]
name = "full-project"

[tools.database]
driver = "postgres"
url = "postgresql://localhost/db"
pool_size = 20

[tools.http]
timeout_ms = 5000

[tools.filesystem]
root = "./data"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert!(manifest.tools.database.is_some());
        assert!(manifest.tools.http.is_some());
        assert!(manifest.tools.filesystem.is_some());
    }

    #[test]
    fn parse_persistence_default() {
        let toml = r#"
[project]
name = "test"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.persistence.backend, "sqlite");
        assert_eq!(manifest.persistence.path, ".sage/checkpoints.db");
        assert!(manifest.persistence.url.is_none());
    }

    #[test]
    fn parse_persistence_sqlite() {
        let toml = r#"
[project]
name = "test"

[persistence]
backend = "sqlite"
path = "./checkpoints/data.db"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.persistence.backend, "sqlite");
        assert_eq!(manifest.persistence.path, "./checkpoints/data.db");
    }

    #[test]
    fn parse_persistence_postgres() {
        let toml = r#"
[project]
name = "test"

[persistence]
backend = "postgres"
url = "postgresql://user:pass@localhost/mydb"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.persistence.backend, "postgres");
        assert_eq!(
            manifest.persistence.url,
            Some("postgresql://user:pass@localhost/mydb".to_string())
        );
    }

    #[test]
    fn parse_persistence_file() {
        let toml = r#"
[project]
name = "test"

[persistence]
backend = "file"
path = "./state"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.persistence.backend, "file");
        assert_eq!(manifest.persistence.path, "./state");
    }

    #[test]
    fn parse_supervision_default() {
        let toml = r#"
[project]
name = "test"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.supervision.max_restarts, 5);
        assert_eq!(manifest.supervision.within_seconds, 60);
    }

    #[test]
    fn parse_supervision_custom() {
        let toml = r#"
[project]
name = "test"

[supervision]
max_restarts = 10
within_seconds = 120
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.supervision.max_restarts, 10);
        assert_eq!(manifest.supervision.within_seconds, 120);
    }

    #[test]
    fn parse_observability_default() {
        let toml = r#"
[project]
name = "test"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.observability.backend, "ndjson");
        assert!(manifest.observability.otlp_endpoint.is_none());
        assert_eq!(
            manifest.observability.service_name,
            Some("sage-agent".to_string())
        );
    }

    #[test]
    fn parse_observability_otlp() {
        let toml = r#"
[project]
name = "test"

[observability]
backend = "otlp"
otlp_endpoint = "http://localhost:4317"
service_name = "my-service"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.observability.backend, "otlp");
        assert_eq!(
            manifest.observability.otlp_endpoint,
            Some("http://localhost:4317".to_string())
        );
        assert_eq!(
            manifest.observability.service_name,
            Some("my-service".to_string())
        );
    }

    #[test]
    fn parse_observability_none() {
        let toml = r#"
[project]
name = "test"

[observability]
backend = "none"
"#;
        let manifest: ProjectManifest = toml::from_str(toml).unwrap();
        assert_eq!(manifest.observability.backend, "none");
    }
}
