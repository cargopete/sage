//! Package cache management.
//!
//! Packages are cached at `~/.grove/packages/<name>/<rev>/`

use crate::dependency::GitDependency;
use crate::error::PackageError;
use git2::{FetchOptions, RemoteCallbacks, Repository};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Manages the local package cache.
pub struct PackageCache {
    /// Root directory for the cache (~/.grove/packages/).
    root: PathBuf,
}

/// Metadata file stored with each cached package.
#[derive(Debug, Serialize, Deserialize)]
struct PackageMeta {
    /// Package name.
    name: String,
    /// Git URL.
    git: String,
    /// Full SHA.
    rev: String,
    /// When this was cached (Unix timestamp).
    cached_at: u64,
}

impl PackageCache {
    /// Create a new package cache, using XDG-compliant paths.
    pub fn new() -> Result<Self, PackageError> {
        let root = Self::cache_dir()?;
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    /// Get the cache directory path.
    pub fn cache_dir() -> Result<PathBuf, PackageError> {
        // Use XDG_CACHE_HOME or ~/.grove/packages
        if let Some(cache) = dirs::cache_dir() {
            Ok(cache.join("grove").join("packages"))
        } else if let Some(home) = dirs::home_dir() {
            Ok(home.join(".grove").join("packages"))
        } else {
            Err(PackageError::IoError {
                message: "could not determine home directory".to_string(),
                source: std::io::Error::new(std::io::ErrorKind::NotFound, "no home directory"),
            })
        }
    }

    /// Get the root path of the cache.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the path for a cached package.
    pub fn package_path(&self, name: &str, rev: &str) -> PathBuf {
        // Use first 12 chars of rev for shorter paths
        let short_rev = if rev.len() > 12 { &rev[..12] } else { rev };
        self.root.join(name).join(short_rev)
    }

    /// Check if a package is cached.
    pub fn is_cached(&self, name: &str, rev: &str) -> bool {
        let path = self.package_path(name, rev);
        let meta_path = path.join(".grove-meta.toml");
        meta_path.exists()
    }

    /// Get the path of a cached package, or None if not cached.
    pub fn get(&self, name: &str, rev: &str) -> Option<PathBuf> {
        let path = self.package_path(name, rev);
        if self.is_cached(name, rev) {
            Some(path)
        } else {
            None
        }
    }

    /// Fetch a git package to the cache, returning its path.
    pub fn fetch(
        &self,
        name: &str,
        spec: &GitDependency,
    ) -> Result<(PathBuf, String), PackageError> {
        // First resolve the ref to a SHA
        let sha = self.resolve_ref(&spec.git, spec.ref_string())?;

        // Check if already cached
        if let Some(path) = self.get(name, &sha) {
            return Ok((path, sha));
        }

        // Clone/checkout to cache
        let path = self.package_path(name, &sha);
        std::fs::create_dir_all(&path)?;

        self.clone_at_rev(&spec.git, &sha, &path)?;

        // Write metadata
        let meta = PackageMeta {
            name: name.to_string(),
            git: spec.git.clone(),
            rev: sha.clone(),
            cached_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        let meta_path = path.join(".grove-meta.toml");
        let meta_toml = toml::to_string_pretty(&meta).map_err(|e| PackageError::IoError {
            message: format!("failed to serialize meta: {e}"),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, e),
        })?;
        std::fs::write(&meta_path, meta_toml)?;

        Ok((path, sha))
    }

    /// Resolve a git ref (tag/branch/rev) to a full SHA.
    pub fn resolve_ref(&self, url: &str, ref_str: &str) -> Result<String, PackageError> {
        // Try to open existing bare repo for this URL, or create temp one
        let temp_dir = self.root.join(".git-cache");
        std::fs::create_dir_all(&temp_dir)?;

        // Hash URL for stable temp path
        let url_hash = format!("{:x}", md5_hash(url));
        let repo_path = temp_dir.join(&url_hash);

        let repo = if repo_path.exists() {
            Repository::open_bare(&repo_path).map_err(|e| PackageError::GitFetchFailed {
                url: url.to_string(),
                reason: format!("failed to open cached repo: {e}"),
            })?
        } else {
            // Clone bare for fetching refs
            let mut callbacks = RemoteCallbacks::new();
            callbacks.transfer_progress(|_| true);

            let mut fetch_opts = FetchOptions::new();
            fetch_opts.remote_callbacks(callbacks);

            let mut builder = git2::build::RepoBuilder::new();
            builder.bare(true);
            builder.fetch_options(fetch_opts);

            builder
                .clone(url, &repo_path)
                .map_err(|e| PackageError::GitFetchFailed {
                    url: url.to_string(),
                    reason: e.message().to_string(),
                })?
        };

        // Fetch latest refs
        let mut remote = repo
            .find_remote("origin")
            .or_else(|_| repo.remote_anonymous(url))
            .map_err(|e| PackageError::GitFetchFailed {
                url: url.to_string(),
                reason: format!("failed to find remote: {e}"),
            })?;

        let mut callbacks = RemoteCallbacks::new();
        callbacks.transfer_progress(|_| true);

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        remote
            .fetch(&[ref_str], Some(&mut fetch_opts), None)
            .map_err(|e| PackageError::GitFetchFailed {
                url: url.to_string(),
                reason: format!("fetch failed: {e}"),
            })?;

        // Try to resolve the ref
        // First check if it's already a full SHA
        if ref_str.len() == 40 && ref_str.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(ref_str.to_string());
        }

        // Try as tag
        if let Ok(reference) = repo.find_reference(&format!("refs/tags/{ref_str}")) {
            if let Some(target) = reference.target() {
                return Ok(target.to_string());
            }
            // Annotated tag - peel to commit
            if let Ok(obj) = reference.peel(git2::ObjectType::Commit) {
                return Ok(obj.id().to_string());
            }
        }

        // Try as branch
        if let Ok(reference) = repo.find_reference(&format!("refs/remotes/origin/{ref_str}")) {
            if let Some(target) = reference.target() {
                return Ok(target.to_string());
            }
        }

        // Try FETCH_HEAD
        if let Ok(reference) = repo.find_reference("FETCH_HEAD") {
            if let Some(target) = reference.target() {
                return Ok(target.to_string());
            }
        }

        // Try as short SHA
        if let Ok(obj) = repo.revparse_single(ref_str) {
            return Ok(obj.id().to_string());
        }

        Err(PackageError::GitFetchFailed {
            url: url.to_string(),
            reason: format!("could not resolve ref '{ref_str}'"),
        })
    }

    /// Clone a repo at a specific revision.
    fn clone_at_rev(&self, url: &str, rev: &str, dest: &Path) -> Result<(), PackageError> {
        // Clone the repository
        let mut callbacks = RemoteCallbacks::new();
        callbacks.transfer_progress(|_| true);

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(callbacks);

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fetch_opts);

        let repo = builder
            .clone(url, dest)
            .map_err(|e| PackageError::GitFetchFailed {
                url: url.to_string(),
                reason: e.message().to_string(),
            })?;

        // Checkout the specific revision
        let oid = git2::Oid::from_str(rev).map_err(|e| PackageError::GitFetchFailed {
            url: url.to_string(),
            reason: format!("invalid SHA: {e}"),
        })?;

        let commit = repo
            .find_commit(oid)
            .map_err(|e| PackageError::GitFetchFailed {
                url: url.to_string(),
                reason: format!("commit not found: {e}"),
            })?;

        repo.checkout_tree(commit.as_object(), None)
            .map_err(|e| PackageError::GitFetchFailed {
                url: url.to_string(),
                reason: format!("checkout failed: {e}"),
            })?;

        repo.set_head_detached(oid)
            .map_err(|e| PackageError::GitFetchFailed {
                url: url.to_string(),
                reason: format!("set head failed: {e}"),
            })?;

        // Remove .git directory to save space
        let git_dir = dest.join(".git");
        if git_dir.exists() {
            std::fs::remove_dir_all(&git_dir)?;
        }

        Ok(())
    }

    /// List all cached packages.
    pub fn list(&self) -> Result<Vec<(String, String, PathBuf)>, PackageError> {
        let mut packages = Vec::new();

        if !self.root.exists() {
            return Ok(packages);
        }

        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            let pkg_name = entry.file_name().to_string_lossy().to_string();

            // Skip .git-cache
            if pkg_name.starts_with('.') {
                continue;
            }

            if entry.path().is_dir() {
                for version_entry in std::fs::read_dir(entry.path())? {
                    let version_entry = version_entry?;
                    let rev = version_entry.file_name().to_string_lossy().to_string();
                    let path = version_entry.path();

                    if path.join(".grove-meta.toml").exists() {
                        packages.push((pkg_name.clone(), rev, path));
                    }
                }
            }
        }

        Ok(packages)
    }

    /// Remove a package from the cache.
    pub fn remove(&self, name: &str) -> Result<(), PackageError> {
        let pkg_dir = self.root.join(name);
        if pkg_dir.exists() {
            std::fs::remove_dir_all(&pkg_dir)?;
        }
        Ok(())
    }

    /// Remove a specific version from the cache.
    pub fn remove_version(&self, name: &str, rev: &str) -> Result<(), PackageError> {
        let path = self.package_path(name, rev);
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }
        Ok(())
    }

    /// Clear the entire cache.
    pub fn clean(&self) -> Result<(), PackageError> {
        if self.root.exists() {
            std::fs::remove_dir_all(&self.root)?;
            std::fs::create_dir_all(&self.root)?;
        }
        Ok(())
    }

    /// Get cache size in bytes.
    pub fn size(&self) -> Result<u64, PackageError> {
        fn dir_size(path: &Path) -> std::io::Result<u64> {
            let mut size = 0;
            if path.is_dir() {
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        size += dir_size(&path)?;
                    } else {
                        size += entry.metadata()?.len();
                    }
                }
            }
            Ok(size)
        }

        dir_size(&self.root).map_err(|e| PackageError::IoError {
            message: "failed to calculate cache size".to_string(),
            source: e,
        })
    }
}

impl Default for PackageCache {
    fn default() -> Self {
        Self::new().expect("failed to create package cache")
    }
}

/// Simple hash for URL -> directory name.
fn md5_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// Holds information about all resolved packages.
#[derive(Debug, Default)]
pub struct ResolvedPackage {
    /// Package name.
    pub name: String,
    /// Package version.
    pub version: String,
    /// Path to the package (cached for git, local for path deps).
    pub path: PathBuf,
    /// Full SHA (None for path dependencies).
    pub rev: Option<String>,
    /// Git URL (None for path dependencies).
    pub git: Option<String>,
    /// Original path string (for path dependencies).
    pub source_path: Option<String>,
    /// Dependencies of this package.
    pub dependencies: Vec<String>,
}

/// Map of package name to resolved package info.
pub type ResolvedPackagesMap = HashMap<String, ResolvedPackage>;
