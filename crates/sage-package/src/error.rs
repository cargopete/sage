//! Error types for the package manager.

use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during package operations.
#[derive(Debug, Error, Diagnostic)]
pub enum PackageError {
    /// E030: Two packages require incompatible versions of the same dependency.
    #[error("incompatible versions of '{package}'")]
    #[diagnostic(
        code(E030),
        help("Oswyn explains: '{package}' is required at {version_a} by {requirer_a} and {version_b} by {requirer_b}")
    )]
    IncompatibleVersions {
        package: String,
        version_a: String,
        requirer_a: String,
        version_b: String,
        requirer_b: String,
    },

    /// E031: Package name in sage add doesn't match package's sage.toml.
    #[error("package name mismatch: expected '{expected}', found '{found}'")]
    #[diagnostic(
        code(E031),
        help("Oswyn explains: the package declares its name as '{found}' in sage.toml")
    )]
    PackageNameMismatch { expected: String, found: String },

    /// E032: Attempted to use an executable package as a library dependency.
    #[error("'{package}' is an executable, not a library")]
    #[diagnostic(
        code(E032),
        help("Oswyn explains: packages with a `run` statement cannot be used as dependencies")
    )]
    DependencyIsExecutable { package: String },

    /// E033: sage run --offline with no lock file.
    #[error("no lock file found")]
    #[diagnostic(
        code(E033),
        help("Oswyn suggests: run `sage install` to create a lock file")
    )]
    LockFileMissing { path: PathBuf },

    /// E034: use references a package not declared in dependencies.
    #[error("package '{package}' not found in dependencies")]
    #[diagnostic(code(E034), help("Oswyn suggests: add it with `sage add {package} --git <url>`"))]
    PackageNotFound { package: String },

    /// E035: Git clone/fetch operation failed.
    #[error("failed to fetch '{url}'")]
    #[diagnostic(code(E035), help("Oswyn explains: {reason}"))]
    GitFetchFailed { url: String, reason: String },

    /// Invalid dependency specification in sage.toml.
    #[error("invalid dependency specification for '{package}'")]
    #[diagnostic(
        code(sage::package::invalid_dep),
        help("Oswyn explains: dependencies must specify exactly one of: tag, branch, or rev")
    )]
    InvalidDependencySpec { package: String },

    /// Missing git URL for a dependency.
    #[error("missing 'git' URL for dependency '{package}'")]
    #[diagnostic(code(sage::package::missing_git))]
    MissingGitUrl { package: String },

    /// IO error during package operations.
    #[error("IO error: {message}")]
    #[diagnostic(code(sage::package::io_error))]
    IoError {
        message: String,
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse package manifest.
    #[error("invalid sage.toml in '{package}'")]
    #[diagnostic(code(sage::package::invalid_manifest))]
    InvalidManifest {
        package: String,
        #[source]
        source: toml::de::Error,
    },

    /// Lock file is stale (sage.toml changed).
    #[error("sage.lock is out of date")]
    #[diagnostic(code(sage::package::stale_lock), help("Oswyn suggests: run `sage install` to update"))]
    StaleLockFile,

    /// Failed to parse lock file.
    #[error("invalid sage.lock")]
    #[diagnostic(code(sage::package::invalid_lock))]
    InvalidLockFile {
        #[source]
        source: toml::de::Error,
    },
}

impl From<std::io::Error> for PackageError {
    fn from(err: std::io::Error) -> Self {
        PackageError::IoError {
            message: err.to_string(),
            source: err,
        }
    }
}
