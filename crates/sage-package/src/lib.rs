//! Package manager for Sage.
//!
//! This crate handles:
//! - Parsing `[dependencies]` from `grove.toml`
//! - Managing `grove.lock` for reproducible builds
//! - Package cache at `~/.sage/packages/<name>/<version>/`
//! - Git-based dependency resolution

#![forbid(unsafe_code)]

mod cache;
mod dependency;
mod error;
mod lock;
mod resolver;

pub use cache::PackageCache;
pub use dependency::{
    parse_dependencies, resolve_path, DependencySpec, GitDependency, PathDependency,
};
pub use error::PackageError;
pub use lock::{LockFile, LockedPackage};
pub use resolver::{
    check_is_library, check_lock_freshness, install_from_lock, resolve_dependencies,
    ResolvedPackages,
};
