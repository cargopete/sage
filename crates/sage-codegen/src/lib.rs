//! Code generator for compiling Sage to Rust.
//!
//! This crate transforms a type-checked Sage AST into Rust source code
//! that can be compiled with `rustc` via Cargo.
//!
//! # Example
//!
//! ```ignore
//! use sage_codegen::Codegen;
//! use sage_parser::Program;
//!
//! let program: Program = /* parse and check */;
//! let rust_code = Codegen::generate(&program);
//! ```

#![forbid(unsafe_code)]

mod emit;
mod generator;

pub use generator::{
    generate, generate_module_tree, generate_module_tree_with_config,
    generate_module_tree_with_full_config, generate_test_program,
    generate_test_program_with_config, generate_with_config, generate_with_full_config,
    CodegenConfig, GeneratedProject, GeneratedTestProject, ObservabilityConfig, PersistenceBackend,
    RuntimeDep,
};
