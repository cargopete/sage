//! Error types for the module loader.

use miette::{Diagnostic, SourceSpan};
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during module loading.
#[derive(Debug, Error, Diagnostic)]
pub enum LoadError {
    /// File not found for a mod declaration.
    #[error("module '{mod_name}' not found")]
    #[diagnostic(
        code(sage::loader::file_not_found),
        help("expected one of: {}", searched.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "))
    )]
    FileNotFound {
        mod_name: String,
        searched: Vec<PathBuf>,
        #[label("declared here")]
        span: SourceSpan,
        #[source_code]
        source_code: String,
    },

    /// IO error reading a file.
    #[error("failed to read '{path}'")]
    #[diagnostic(code(sage::loader::io_error))]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Parse errors in a module.
    #[error("parse error in '{file}'")]
    #[diagnostic(code(sage::loader::parse_error))]
    ParseError { file: PathBuf, errors: Vec<String> },

    /// Circular dependency between modules.
    #[error("circular dependency detected")]
    #[diagnostic(
        code(sage::loader::circular_dependency),
        help("cycle: {}", cycle.join(" -> "))
    )]
    CircularDependency { cycle: Vec<String> },

    /// Ambiguous module - both foo.sg and foo/mod.sg exist.
    #[error("ambiguous module '{mod_name}'")]
    #[diagnostic(
        code(sage::loader::ambiguous_module),
        help("both {} exist", candidates.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" and "))
    )]
    AmbiguousModule {
        mod_name: String,
        candidates: Vec<PathBuf>,
    },

    /// Invalid manifest file.
    #[error("invalid sage.toml")]
    #[diagnostic(code(sage::loader::invalid_manifest))]
    InvalidManifest {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    /// Missing entry point in manifest.
    #[error("entry point not found: '{path}'")]
    #[diagnostic(code(sage::loader::missing_entry))]
    MissingEntry { path: PathBuf },

    /// No sage.toml found when expected.
    #[error("no sage.toml found in '{dir}' or parent directories")]
    #[diagnostic(code(sage::loader::no_manifest))]
    NoManifest { dir: PathBuf },
}
