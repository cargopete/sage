//! Dependency resolution.

use crate::cache::{PackageCache, ResolvedPackage, ResolvedPackagesMap};
use crate::dependency::{parse_dependencies, resolve_path, DependencySpec, GitDependency};
use crate::error::PackageError;
use crate::lock::{LockFile, LockedPackage};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Result of dependency resolution.
#[derive(Debug)]
pub struct ResolvedPackages {
    /// All resolved packages (including transitive).
    pub packages: ResolvedPackagesMap,
    /// The generated lock file.
    pub lock_file: LockFile,
}

/// Minimal manifest for reading package info.
#[derive(Debug, Deserialize)]
struct PackageManifest {
    project: ProjectInfo,
    #[serde(default)]
    dependencies: toml::Table,
}

#[derive(Debug, Deserialize)]
struct ProjectInfo {
    name: String,
    #[serde(default = "default_version")]
    version: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

/// Resolve all dependencies from a project's grove.toml.
pub fn resolve_dependencies(
    project_root: &Path,
    deps: &HashMap<String, DependencySpec>,
    existing_lock: Option<&LockFile>,
) -> Result<ResolvedPackages, PackageError> {
    let cache = PackageCache::new()?;
    let mut resolver = Resolver::new(cache, existing_lock, project_root);

    // Resolve all direct dependencies
    for (name, spec) in deps {
        resolver.resolve(name, spec, "root")?;
    }

    // Build the result
    let packages = resolver.resolved;
    let lock_file = LockFile {
        version: 1,
        packages: packages
            .values()
            .map(|p| {
                if let Some(ref path) = p.source_path {
                    LockedPackage::path(
                        p.name.clone(),
                        p.version.clone(),
                        path.clone(),
                        p.dependencies.clone(),
                    )
                } else {
                    LockedPackage::git(
                        p.name.clone(),
                        p.version.clone(),
                        p.git.clone().unwrap_or_default(),
                        p.rev.clone().unwrap_or_default(),
                        p.dependencies.clone(),
                    )
                }
            })
            .collect(),
    };

    // Save lock file
    let lock_path = project_root.join("grove.lock");
    lock_file.save(&lock_path)?;

    Ok(ResolvedPackages {
        packages,
        lock_file,
    })
}

/// Check if dependencies are up-to-date with lock file.
pub fn check_lock_freshness(deps: &HashMap<String, DependencySpec>, lock: &LockFile) -> bool {
    lock.matches_dependencies(deps)
}

/// Install dependencies from an existing lock file.
pub fn install_from_lock(
    project_root: &Path,
    lock: &LockFile,
) -> Result<ResolvedPackagesMap, PackageError> {
    let cache = PackageCache::new()?;
    let mut packages = ResolvedPackagesMap::new();

    for locked in lock.in_dependency_order() {
        if locked.is_path() {
            // Path dependency - resolve directly
            let path_str = locked.path.as_ref().unwrap();
            let resolved_path = resolve_path(project_root, path_str);

            if !resolved_path.exists() {
                return Err(PackageError::IoError {
                    message: format!(
                        "path dependency '{}' not found at {}",
                        locked.name,
                        resolved_path.display()
                    ),
                    source: std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "path dependency not found",
                    ),
                });
            }

            packages.insert(
                locked.name.clone(),
                ResolvedPackage {
                    name: locked.name.clone(),
                    version: locked.version.clone(),
                    path: resolved_path,
                    rev: None,
                    git: None,
                    source_path: Some(path_str.clone()),
                    dependencies: locked.dependencies.clone(),
                },
            );
        } else {
            // Git dependency - fetch via cache
            let git_url = locked.git.as_ref().unwrap();
            let rev = locked.rev.as_ref().unwrap();
            let spec = GitDependency {
                git: git_url.clone(),
                tag: None,
                branch: None,
                rev: Some(rev.clone()),
            };

            // Fetch (will use cache if available)
            let (path, _) = cache.fetch(&locked.name, &spec)?;

            packages.insert(
                locked.name.clone(),
                ResolvedPackage {
                    name: locked.name.clone(),
                    version: locked.version.clone(),
                    path,
                    rev: Some(rev.clone()),
                    git: Some(git_url.clone()),
                    source_path: None,
                    dependencies: locked.dependencies.clone(),
                },
            );
        }
    }

    Ok(packages)
}

struct Resolver<'a> {
    cache: PackageCache,
    resolved: ResolvedPackagesMap,
    in_progress: HashSet<String>,
    existing_lock: Option<&'a LockFile>,
    project_root: &'a Path,
}

impl<'a> Resolver<'a> {
    fn new(
        cache: PackageCache,
        existing_lock: Option<&'a LockFile>,
        project_root: &'a Path,
    ) -> Self {
        Self {
            cache,
            resolved: ResolvedPackagesMap::new(),
            in_progress: HashSet::new(),
            existing_lock,
            project_root,
        }
    }

    fn resolve(
        &mut self,
        name: &str,
        spec: &DependencySpec,
        requirer: &str,
    ) -> Result<(), PackageError> {
        // Check for circular dependency
        if self.in_progress.contains(name) {
            // Circular deps in packages are allowed as long as we've already
            // resolved this package - just return
            return Ok(());
        }

        // Already resolved?
        if let Some(existing) = self.resolved.get(name) {
            // Check for source conflict
            match spec {
                DependencySpec::Git(g) => {
                    if existing.git.as_ref() != Some(&g.git) {
                        return Err(PackageError::IncompatibleVersions {
                            package: name.to_string(),
                            version_a: existing.rev.clone().unwrap_or_default(),
                            requirer_a: "previously resolved".to_string(),
                            version_b: g.ref_string().to_string(),
                            requirer_b: requirer.to_string(),
                        });
                    }
                }
                DependencySpec::Path(p) => {
                    if existing.source_path.as_ref() != Some(&p.path) {
                        return Err(PackageError::IncompatibleVersions {
                            package: name.to_string(),
                            version_a: existing.source_path.clone().unwrap_or_default(),
                            requirer_a: "previously resolved".to_string(),
                            version_b: p.path.clone(),
                            requirer_b: requirer.to_string(),
                        });
                    }
                }
            }
            return Ok(());
        }

        self.in_progress.insert(name.to_string());

        let (path, rev, git, source_path) = match spec {
            DependencySpec::Path(p) => {
                // Resolve path dependency directly
                let resolved_path = resolve_path(self.project_root, &p.path);

                if !resolved_path.exists() {
                    return Err(PackageError::IoError {
                        message: format!(
                            "path dependency '{}' not found at {}",
                            name,
                            resolved_path.display()
                        ),
                        source: std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "path dependency not found",
                        ),
                    });
                }

                (resolved_path, None, None, Some(p.path.clone()))
            }
            DependencySpec::Git(g) => {
                // Check if we can use the lock file
                let (path, rev) = if let Some(lock) = self.existing_lock {
                    if let Some(locked) = lock.find(name) {
                        if locked.git.as_ref() == Some(&g.git) {
                            // Use locked version
                            let locked_spec = GitDependency {
                                git: g.git.clone(),
                                tag: None,
                                branch: None,
                                rev: locked.rev.clone(),
                            };
                            self.cache.fetch(name, &locked_spec)?
                        } else {
                            // Git URL changed - resolve fresh
                            self.cache.fetch(name, g)?
                        }
                    } else {
                        // Not in lock file - resolve fresh
                        self.cache.fetch(name, g)?
                    }
                } else {
                    // No lock file - resolve fresh
                    self.cache.fetch(name, g)?
                };

                (path, Some(rev), Some(g.git.clone()), None)
            }
        };

        // Read the package's manifest
        let manifest = self.read_manifest(&path, name)?;

        // Verify package name matches
        if manifest.project.name != name {
            return Err(PackageError::PackageNameMismatch {
                expected: name.to_string(),
                found: manifest.project.name,
            });
        }

        // Parse transitive dependencies
        let trans_deps = parse_dependencies(&manifest.dependencies)?;
        let dep_names: Vec<String> = trans_deps.keys().cloned().collect();

        // Store the resolved package
        self.resolved.insert(
            name.to_string(),
            ResolvedPackage {
                name: name.to_string(),
                version: manifest.project.version,
                path: path.clone(),
                rev,
                git,
                source_path,
                dependencies: dep_names.clone(),
            },
        );

        self.in_progress.remove(name);

        // Resolve transitive dependencies
        for (dep_name, dep_spec) in trans_deps {
            self.resolve(&dep_name, &dep_spec, name)?;
        }

        Ok(())
    }

    fn read_manifest(&self, path: &Path, name: &str) -> Result<PackageManifest, PackageError> {
        let manifest_path = path.join("grove.toml");
        let contents =
            std::fs::read_to_string(&manifest_path).map_err(|e| PackageError::IoError {
                message: format!("failed to read manifest for '{name}'"),
                source: e,
            })?;

        toml::from_str(&contents).map_err(|e| PackageError::InvalidManifest {
            package: name.to_string(),
            source: e,
        })
    }
}

/// Check if a package has a `run` statement (making it an executable, not a library).
pub fn check_is_library(path: &Path) -> Result<bool, PackageError> {
    // Read the entry file and check for `run` statement
    let manifest_path = path.join("sage.toml");
    let manifest_contents = std::fs::read_to_string(&manifest_path)?;
    let _manifest: PackageManifest =
        toml::from_str(&manifest_contents).map_err(|e| PackageError::InvalidManifest {
            package: path.display().to_string(),
            source: e,
        })?;

    // Get entry path
    #[derive(Deserialize)]
    struct FullManifest {
        project: FullProjectInfo,
    }
    #[derive(Deserialize)]
    struct FullProjectInfo {
        #[serde(default = "default_entry")]
        entry: String,
    }
    fn default_entry() -> String {
        "src/main.sg".to_string()
    }

    let full: FullManifest =
        toml::from_str(&manifest_contents).map_err(|e| PackageError::InvalidManifest {
            package: path.display().to_string(),
            source: e,
        })?;

    let entry_path = path.join(&full.project.entry);
    if !entry_path.exists() {
        // No entry file means it's a library
        return Ok(true);
    }

    let entry_contents = std::fs::read_to_string(&entry_path)?;

    // Simple check: look for `run` followed by identifier and semicolon
    // This is a rough check - a proper check would use the parser
    let has_run = entry_contents
        .lines()
        .any(|line| line.trim().starts_with("run ") && line.trim().ends_with(';'));

    Ok(!has_run)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_lock_freshness_matches_git() {
        let mut deps = HashMap::new();
        deps.insert(
            "foo".to_string(),
            DependencySpec::with_tag("https://github.com/example/foo", "v1.0.0"),
        );

        let lock = LockFile {
            version: 1,
            packages: vec![LockedPackage::git(
                "foo".to_string(),
                "1.0.0".to_string(),
                "https://github.com/example/foo".to_string(),
                "abc123".to_string(),
                vec![],
            )],
        };

        assert!(check_lock_freshness(&deps, &lock));
    }

    #[test]
    fn check_lock_freshness_matches_path() {
        let mut deps = HashMap::new();
        deps.insert("local".to_string(), DependencySpec::with_path("../lib"));

        let lock = LockFile {
            version: 1,
            packages: vec![LockedPackage::path(
                "local".to_string(),
                "0.1.0".to_string(),
                "../lib".to_string(),
                vec![],
            )],
        };

        assert!(check_lock_freshness(&deps, &lock));
    }

    #[test]
    fn check_lock_freshness_missing_dep() {
        let mut deps = HashMap::new();
        deps.insert(
            "foo".to_string(),
            DependencySpec::with_tag("https://github.com/example/foo", "v1.0.0"),
        );
        deps.insert(
            "bar".to_string(),
            DependencySpec::with_tag("https://github.com/example/bar", "v2.0.0"),
        );

        let lock = LockFile {
            version: 1,
            packages: vec![LockedPackage::git(
                "foo".to_string(),
                "1.0.0".to_string(),
                "https://github.com/example/foo".to_string(),
                "abc123".to_string(),
                vec![],
            )],
        };

        assert!(!check_lock_freshness(&deps, &lock));
    }

    #[test]
    fn check_lock_freshness_path_mismatch() {
        let mut deps = HashMap::new();
        deps.insert(
            "local".to_string(),
            DependencySpec::with_path("../different-path"),
        );

        let lock = LockFile {
            version: 1,
            packages: vec![LockedPackage::path(
                "local".to_string(),
                "0.1.0".to_string(),
                "../original-path".to_string(),
                vec![],
            )],
        };

        assert!(!check_lock_freshness(&deps, &lock));
    }
}
