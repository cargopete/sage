//! Dependency specification parsing.

use crate::error::PackageError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A dependency specification from grove.toml.
///
/// Can be either a git dependency or a local path dependency.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencySpec {
    /// A git-based dependency with URL and ref.
    Git(GitDependency),
    /// A local path dependency.
    Path(PathDependency),
}

/// A git-based dependency specification.
///
/// Requires a git URL and exactly one of: tag, branch, or rev.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GitDependency {
    /// Git repository URL.
    pub git: String,
    /// Git tag (e.g., "v1.0.0").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Git branch (e.g., "main").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Git revision (full or short SHA).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
}

/// A local path dependency specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PathDependency {
    /// Path to the local package (relative or absolute).
    pub path: String,
}

impl DependencySpec {
    /// Create a new git dependency spec with a tag.
    pub fn with_tag(git: impl Into<String>, tag: impl Into<String>) -> Self {
        Self::Git(GitDependency {
            git: git.into(),
            tag: Some(tag.into()),
            branch: None,
            rev: None,
        })
    }

    /// Create a new git dependency spec with a branch.
    pub fn with_branch(git: impl Into<String>, branch: impl Into<String>) -> Self {
        Self::Git(GitDependency {
            git: git.into(),
            tag: None,
            branch: Some(branch.into()),
            rev: None,
        })
    }

    /// Create a new git dependency spec with a revision.
    pub fn with_rev(git: impl Into<String>, rev: impl Into<String>) -> Self {
        Self::Git(GitDependency {
            git: git.into(),
            tag: None,
            branch: None,
            rev: Some(rev.into()),
        })
    }

    /// Create a new path dependency spec.
    pub fn with_path(path: impl Into<String>) -> Self {
        Self::Path(PathDependency { path: path.into() })
    }

    /// Check if this is a path dependency.
    pub fn is_path(&self) -> bool {
        matches!(self, Self::Path(_))
    }

    /// Check if this is a git dependency.
    pub fn is_git(&self) -> bool {
        matches!(self, Self::Git(_))
    }

    /// Get the git URL if this is a git dependency.
    pub fn git_url(&self) -> Option<&str> {
        match self {
            Self::Git(g) => Some(&g.git),
            Self::Path(_) => None,
        }
    }

    /// Get the path if this is a path dependency.
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::Path(p) => Some(&p.path),
            Self::Git(_) => None,
        }
    }

    /// Validate the dependency specification.
    pub fn validate(&self, package_name: &str) -> Result<(), PackageError> {
        match self {
            Self::Git(g) => {
                let count = [&g.tag, &g.branch, &g.rev]
                    .iter()
                    .filter(|x| x.is_some())
                    .count();

                if count != 1 {
                    return Err(PackageError::InvalidDependencySpec {
                        package: package_name.to_string(),
                    });
                }
                Ok(())
            }
            Self::Path(_) => Ok(()), // Path deps are always valid if they exist
        }
    }

    /// Get the ref string (tag, branch, or rev) for git deps.
    pub fn ref_string(&self) -> &str {
        match self {
            Self::Git(g) => g
                .tag
                .as_deref()
                .or(g.branch.as_deref())
                .or(g.rev.as_deref())
                .unwrap_or("HEAD"),
            Self::Path(_) => "path",
        }
    }

    /// Get the ref type for display.
    pub fn ref_type(&self) -> &'static str {
        match self {
            Self::Git(g) => {
                if g.tag.is_some() {
                    "tag"
                } else if g.branch.is_some() {
                    "branch"
                } else if g.rev.is_some() {
                    "rev"
                } else {
                    "HEAD"
                }
            }
            Self::Path(_) => "path",
        }
    }
}

impl GitDependency {
    /// Get the ref string (tag, branch, or rev).
    pub fn ref_string(&self) -> &str {
        self.tag
            .as_deref()
            .or(self.branch.as_deref())
            .or(self.rev.as_deref())
            .unwrap_or("HEAD")
    }
}

/// Parse dependencies from a TOML table.
pub fn parse_dependencies(
    table: &toml::Table,
) -> Result<HashMap<String, DependencySpec>, PackageError> {
    let mut deps = HashMap::new();

    for (name, value) in table {
        let spec = parse_dependency_value(name, value)?;
        deps.insert(name.clone(), spec);
    }

    Ok(deps)
}

fn parse_dependency_value(name: &str, value: &toml::Value) -> Result<DependencySpec, PackageError> {
    match value {
        toml::Value::Table(t) => {
            // Check if it's a path dependency
            if let Some(path) = t.get("path").and_then(|v| v.as_str()) {
                return Ok(DependencySpec::Path(PathDependency {
                    path: path.to_string(),
                }));
            }

            // Otherwise it's a git dependency
            let git = t
                .get("git")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PackageError::MissingGitUrl {
                    package: name.to_string(),
                })?
                .to_string();

            let tag = t.get("tag").and_then(|v| v.as_str()).map(String::from);
            let branch = t.get("branch").and_then(|v| v.as_str()).map(String::from);
            let rev = t.get("rev").and_then(|v| v.as_str()).map(String::from);

            let spec = DependencySpec::Git(GitDependency {
                git,
                tag,
                branch,
                rev,
            });
            spec.validate(name)?;
            Ok(spec)
        }
        _ => Err(PackageError::InvalidDependencySpec {
            package: name.to_string(),
        }),
    }
}

/// Resolve a path dependency to an absolute path.
pub fn resolve_path(base_dir: &std::path::Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tag_dependency() {
        let toml_str = r#"
git = "https://github.com/example/package"
tag = "v1.0.0"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let spec = parse_dependency_value("test", &value).unwrap();

        match spec {
            DependencySpec::Git(g) => {
                assert_eq!(g.git, "https://github.com/example/package");
                assert_eq!(g.tag, Some("v1.0.0".to_string()));
                assert_eq!(g.branch, None);
                assert_eq!(g.rev, None);
            }
            _ => panic!("Expected git dependency"),
        }
    }

    #[test]
    fn parse_branch_dependency() {
        let toml_str = r#"
git = "https://github.com/example/package"
branch = "develop"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let spec = parse_dependency_value("test", &value).unwrap();

        match spec {
            DependencySpec::Git(g) => {
                assert_eq!(g.branch, Some("develop".to_string()));
            }
            _ => panic!("Expected git dependency"),
        }
    }

    #[test]
    fn parse_rev_dependency() {
        let toml_str = r#"
git = "https://github.com/example/package"
rev = "abc123"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let spec = parse_dependency_value("test", &value).unwrap();

        match spec {
            DependencySpec::Git(g) => {
                assert_eq!(g.rev, Some("abc123".to_string()));
            }
            _ => panic!("Expected git dependency"),
        }
    }

    #[test]
    fn parse_path_dependency() {
        let toml_str = r#"
path = "../my-local-lib"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let spec = parse_dependency_value("test", &value).unwrap();

        match spec {
            DependencySpec::Path(p) => {
                assert_eq!(p.path, "../my-local-lib");
            }
            _ => panic!("Expected path dependency"),
        }
    }

    #[test]
    fn parse_absolute_path_dependency() {
        let toml_str = r#"
path = "/Users/someone/projects/my-lib"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let spec = parse_dependency_value("test", &value).unwrap();

        assert!(spec.is_path());
        assert_eq!(spec.path(), Some("/Users/someone/projects/my-lib"));
    }

    #[test]
    fn reject_missing_git_for_git_dep() {
        let toml_str = r#"
tag = "v1.0.0"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let result = parse_dependency_value("test", &value);

        assert!(matches!(result, Err(PackageError::MissingGitUrl { .. })));
    }

    #[test]
    fn reject_multiple_refs() {
        let toml_str = r#"
git = "https://github.com/example/package"
tag = "v1.0.0"
branch = "main"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let result = parse_dependency_value("test", &value);

        assert!(matches!(
            result,
            Err(PackageError::InvalidDependencySpec { .. })
        ));
    }

    #[test]
    fn reject_no_ref() {
        let toml_str = r#"
git = "https://github.com/example/package"
"#;
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        let result = parse_dependency_value("test", &value);

        assert!(matches!(
            result,
            Err(PackageError::InvalidDependencySpec { .. })
        ));
    }

    #[test]
    fn parse_multiple_dependencies() {
        let table: toml::Table = toml::from_str(
            r#"
[foo]
git = "https://github.com/example/foo"
tag = "v1.0.0"

[bar]
git = "https://github.com/example/bar"
branch = "main"

[local]
path = "../local-lib"
"#,
        )
        .unwrap();

        let deps = parse_dependencies(&table).unwrap();
        assert_eq!(deps.len(), 3);
        assert!(deps.contains_key("foo"));
        assert!(deps.contains_key("bar"));
        assert!(deps.contains_key("local"));
        assert!(deps.get("local").unwrap().is_path());
    }

    #[test]
    fn resolve_relative_path() {
        use std::path::Path;
        let base = Path::new("/home/user/project");
        let resolved = resolve_path(base, "../lib");
        assert_eq!(resolved, PathBuf::from("/home/user/project/../lib"));
    }

    #[test]
    fn resolve_absolute_path() {
        use std::path::Path;
        let base = Path::new("/home/user/project");
        let resolved = resolve_path(base, "/opt/libs/mylib");
        assert_eq!(resolved, PathBuf::from("/opt/libs/mylib"));
    }

    #[test]
    fn dependency_spec_helpers() {
        let git = DependencySpec::with_tag("https://example.com", "v1.0");
        assert!(git.is_git());
        assert!(!git.is_path());
        assert_eq!(git.git_url(), Some("https://example.com"));
        assert_eq!(git.path(), None);

        let path = DependencySpec::with_path("../lib");
        assert!(path.is_path());
        assert!(!path.is_git());
        assert_eq!(path.git_url(), None);
        assert_eq!(path.path(), Some("../lib"));
    }
}
