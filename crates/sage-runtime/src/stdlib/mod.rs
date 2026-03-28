//! Standard library functions for Sage.
//!
//! This module provides runtime helper functions for the Sage standard library.
//! Most stdlib functions are inlined during codegen, but some require runtime
//! helpers for correct Unicode handling or complex logic.

#[cfg(not(target_arch = "wasm32"))]
mod env;
#[cfg(not(target_arch = "wasm32"))]
mod io;
mod json;
#[cfg(feature = "mcp")]
pub mod mcp;
mod parsing;
mod string;
mod time;

#[cfg(not(target_arch = "wasm32"))]
pub use env::*;
#[cfg(not(target_arch = "wasm32"))]
pub use io::*;
pub use json::*;
pub use parsing::*;
pub use string::*;
pub use time::*;

// WASM stubs for env functions that require OS access
#[cfg(target_arch = "wasm32")]
mod wasm_env {
    #[must_use]
    pub fn env_var(_key: &str) -> Option<String> {
        None
    }

    #[must_use]
    pub fn env_or(_key: &str, default: &str) -> String {
        default.to_string()
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_env::*;

// WASM stubs for I/O functions that require filesystem/stdin access
#[cfg(target_arch = "wasm32")]
mod wasm_io {
    pub fn read_file(_path: &str) -> Result<String, String> {
        Err("read_file is not available in the WASM target".to_string())
    }

    pub fn write_file(_path: &str, _contents: &str) -> Result<(), String> {
        Err("write_file is not available in the WASM target".to_string())
    }

    pub fn append_file(_path: &str, _contents: &str) -> Result<(), String> {
        Err("append_file is not available in the WASM target".to_string())
    }

    #[must_use]
    pub fn file_exists(_path: &str) -> bool {
        false
    }

    pub fn delete_file(_path: &str) -> Result<(), String> {
        Err("delete_file is not available in the WASM target".to_string())
    }

    pub fn list_dir(_path: &str) -> Result<Vec<String>, String> {
        Err("list_dir is not available in the WASM target".to_string())
    }

    pub fn make_dir(_path: &str) -> Result<(), String> {
        Err("make_dir is not available in the WASM target".to_string())
    }

    pub fn read_line() -> Result<String, String> {
        Err("read_line is not available in the WASM target".to_string())
    }

    pub fn read_all() -> Result<String, String> {
        Err("read_all is not available in the WASM target".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_io::*;
