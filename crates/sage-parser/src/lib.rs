//! Parser for the Sage language.
//!
//! This crate provides parsing for Sage source code, transforming a token
//! stream into a typed Abstract Syntax Tree (AST).
//!
//! # Example
//!
//! ```
//! use sage_lexer::lex;
//! use sage_parser::parse;
//! use std::sync::Arc;
//!
//! let source = r#"
//!     agent Main {
//!         on start {
//!             emit(42)
//!         }
//!     }
//!     run Main
//! "#;
//!
//! let lex_result = lex(source).expect("lexing failed");
//! let source_arc: Arc<str> = Arc::from(source);
//! let (program, errors) = parse(lex_result.tokens(), source_arc);
//!
//! assert!(errors.is_empty());
//! assert!(program.is_some());
//! ```

#![forbid(unsafe_code)]

pub mod ast;
mod parser;

pub use ast::*;
pub use parser::{parse, ParseError};
