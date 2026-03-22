//! Analysis pipeline for the LSP server.

use crate::convert::{
    check_error_to_diagnostic, lex_error_to_diagnostic, parse_error_to_diagnostic,
};
use std::sync::Arc;
use tower_lsp::lsp_types::Diagnostic;

/// Run the full compiler pipeline on source and return LSP diagnostics.
/// Never panics - all errors are caught and converted.
///
/// If `filename` is provided and ends with `_test.sg`, test constructs are allowed.
pub fn analyse(source: &str, filename: Option<&str>) -> Vec<Diagnostic> {
    let is_test_file = filename.map(|f| f.ends_with("_test.sg")).unwrap_or(false);
    let mut diagnostics = Vec::new();

    // Step 1: Lex (with error recovery)
    let (tokens, lex_errors) = sage_parser::lex_partial(source);
    for error in &lex_errors {
        diagnostics.push(lex_error_to_diagnostic(error, source));
    }

    // Bail early if lex produced nothing usable
    if tokens.is_empty() && !lex_errors.is_empty() {
        return diagnostics;
    }

    // Step 2: Parse (with error recovery - always returns partial AST)
    let source_arc: Arc<str> = Arc::from(source);
    let (program_opt, parse_errors) = sage_parser::parse(&tokens, source_arc);
    for error in &parse_errors {
        if let Some(d) = parse_error_to_diagnostic(error, source) {
            diagnostics.push(d);
        }
    }

    // Step 3: Type check (only if we have a usable AST)
    if let Some(program) = program_opt {
        let check_result = if is_test_file {
            sage_checker::check_test_file(&program)
        } else {
            sage_checker::check(&program)
        };
        for error in &check_result.errors {
            if let Some(d) = check_error_to_diagnostic(error, source) {
                diagnostics.push(d);
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyse_valid_program() {
        let source = r#"
agent Main {
    on start {
        yield(42);
    }
}
run Main;
"#;
        let diagnostics = analyse(source, None);
        assert!(
            diagnostics.is_empty(),
            "Expected no errors: {:?}",
            diagnostics
        );
    }

    #[test]
    fn analyse_undefined_variable() {
        let source = r#"
agent Main {
    on start {
        yield(x);
    }
}
run Main;
"#;
        let diagnostics = analyse(source, None);
        assert!(!diagnostics.is_empty());
        assert!(diagnostics[0].message.contains("undefined"));
    }

    #[test]
    fn analyse_lex_error() {
        let source = "let # = 42";
        let diagnostics = analyse(source, None);
        assert!(!diagnostics.is_empty());
        assert!(diagnostics[0].message.contains("invalid token"));
    }

    #[test]
    fn analyse_parse_error() {
        let source = "agent Main { on start { yield( } } run Main;";
        let diagnostics = analyse(source, None);
        assert!(!diagnostics.is_empty());
    }
}
