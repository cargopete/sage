//! Lexer and parser for the Sage language.
//!
//! This crate provides tokenization and parsing for Sage source code,
//! transforming source text into a typed Abstract Syntax Tree (AST).
//!
//! # Example
//!
//! ```
//! use sage_parser::{lex, parse};
//! use std::sync::Arc;
//!
//! let source = r#"
//!     agent Main {
//!         on start {
//!             emit(42);
//!         }
//!     }
//!     run Main;
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

// Lexer modules
mod lexer;
mod token;

// Parser modules
pub mod ast;
pub mod formatter;
mod parser;
pub mod span;
mod ty;

// Lexer exports
pub use lexer::{lex, lex_partial, LexError, LexErrorLocation, LexResult, Spanned};
pub use token::Token;

// Re-export logos for downstream use
pub use logos::Logos;

// Parser exports
pub use ast::*;
pub use formatter::format;
pub use parser::{parse, ParseError};
pub use span::{Ident, Span};
pub use ty::TypeExpr;
