//! Module loader for Sage projects.
//!
//! This crate handles:
//! - Loading single-file programs
//! - Loading multi-file projects with `grove.toml`
//! - Module tree construction from `mod` declarations
//! - Cycle detection in imports

#![forbid(unsafe_code)]

mod error;
mod manifest;
mod tree;

pub use error::LoadError;
pub use manifest::{
    ObservabilityConfig, PersistenceConfig, ProjectManifest, SupervisionConfig, TestConfig,
};
pub use tree::{
    discover_test_files, load_project, load_project_with_packages, load_single_file,
    load_test_files, ModulePath, ModuleTree, ParsedModule, TestFile,
};
