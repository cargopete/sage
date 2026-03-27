//! Contextual hints for common parse errors.
//!
//! Chumsky's `Simple` errors produce bare "found X expected Y" messages.
//! This module pattern-matches on frequent mistakes and returns an
//! actionable "Oswyn suggests:" hint that the compiler, playground,
//! and LSP can append to the raw error.

use crate::parser::ParseError;
use crate::token::Token;

/// Return an optional human-friendly hint for a parse error.
///
/// The hint is phrased as an "Oswyn suggests:" recommendation so it
/// slots naturally into the existing diagnostic style.
#[must_use]
pub fn error_hint(error: &ParseError) -> Option<String> {
    let found = error.found()?;
    let expected: Vec<_> = error.expected().cloned().collect();

    match found {
        // ── Index syntax: `x[0]` ────────────────────────────────────
        // After an expression the parser expects binary operators or `;`,
        // but the user wrote `[` (array indexing from other languages).
        Token::LBracket if looks_like_postfix_position(&expected) => Some(
            "Oswyn suggests: Sage does not support index syntax (`x[i]`). \
             Use `get(list, index)` instead.\n  \
             Example: `let item = get(my_list, 0);`"
                .to_string(),
        ),

        // ── `fn` inside an agent body ───────────────────────────────
        // The agent parser expects beliefs or `on` handlers.
        Token::KwFn if expected_contains(&expected, &Token::KwOn) => Some(
            "Oswyn suggests: function declarations are not allowed inside agents. \
             Define functions at the top level and call them from `on` handlers.\n  \
             Example:\n    \
             fn helper(x: Int) -> Int { return x + 1; }\n    \
             agent MyAgent {\n      \
               on start { let y = helper(5); yield(y); }\n    \
             }"
            .to_string(),
        ),

        // ── Compound assignment: `-=`, `+=` ─────────────────────────
        // Sage has no compound assignment. The parser sees `x -` as a
        // binary minus, then chokes on `=` where it expects an operand.
        // In that position the expected set contains expression starters
        // (idents, literals, keywords like `try`, `match`, etc.).
        Token::Eq if looks_like_expr_start_position(&expected) => Some(
            "Oswyn suggests: Sage does not support compound assignment \
             (`-=`, `+=`, `*=`). Use full reassignment instead.\n  \
             Example: `count = count - 1;`"
                .to_string(),
        ),

        // ── Assignment to expressions: `self.field = value` ─────────
        // Only plain variable names can appear on the LHS of `=`.
        // Agent beliefs are set at summon time and read via `self.field`,
        // but cannot be reassigned inside handlers.
        Token::Eq if looks_like_postfix_position(&expected) => Some(
            "Oswyn suggests: only local variables can be reassigned with `=`. \
             Agent beliefs are set when summoned and are read-only inside handlers.\n  \
             Example: `let count = self.count + 1;`\n  \
             To pass initial state: `summon Counter { count: 0 }`"
                .to_string(),
        ),

        // ── `let` inside an agent body (outside a handler) ──────────
        // Users sometimes write `let` at the agent level instead of
        // declaring beliefs or using `on` handlers.
        Token::KwLet if expected_contains(&expected, &Token::KwOn) => {
            Some(
                "Oswyn suggests: `let` bindings are only allowed inside handlers. \
                 Agent state is declared as beliefs (fields) at the agent level.\n  \
                 Example:\n    \
                 agent MyAgent {\n      \
                   count: Int          // belief (state)\n      \
                   on start {\n        \
                     let x = self.count; // local variable\n      \
                   }\n    \
                 }"
                .to_string(),
            )
        }

        // ── `while` inside an agent body (outside a handler) ────────
        // Same pattern — control flow belongs inside handlers.
        Token::KwWhile if expected_contains(&expected, &Token::KwOn) =>
        {
            Some(
                "Oswyn suggests: control flow (`while`, `for`, `if`) belongs \
                 inside `on` handlers, not at the agent body level.\n  \
                 Example:\n    \
                 agent MyAgent {\n      \
                   on start {\n        \
                     while condition { ... }\n      \
                   }\n    \
                 }"
                .to_string(),
            )
        }

        // ── `for` inside an agent body (outside a handler) ──────────
        Token::KwFor if expected_contains(&expected, &Token::KwOn) =>
        {
            Some(
                "Oswyn suggests: control flow (`for`, `while`, `if`) belongs \
                 inside `on` handlers, not at the agent body level.\n  \
                 Example:\n    \
                 agent MyAgent {\n      \
                   on start {\n        \
                     for item in my_list { print(item); }\n      \
                   }\n    \
                 }"
                .to_string(),
            )
        }

        // ── `if` inside an agent body (outside a handler) ───────────
        Token::KwIf if expected_contains(&expected, &Token::KwOn) => Some(
            "Oswyn suggests: control flow belongs inside `on` handlers, \
             not at the agent body level."
                .to_string(),
        ),

        // ── `not x` instead of `!x` ────────────────────────────────
        // `not` is not a keyword — it's parsed as an identifier.
        // Two identifiers in a row (e.g., `not contains(...)`) produces
        // "found <ident> expected operators" because the parser already
        // consumed `not` as a variable expression.
        Token::Ident if looks_like_postfix_position(&expected) => Some(
            "Oswyn suggests: if you meant logical negation, Sage uses `!` \
             not `not`.\n  \
             Example: `if !contains(list, item) { ... }`"
                .to_string(),
        ),

        // ── `catch expr { }` instead of `expr catch { }` ───────────
        // `catch` is a postfix operator, not a prefix keyword.
        Token::KwCatch if looks_like_expr_start_position(&expected) => Some(
            "Oswyn suggests: `catch` is a postfix operator, not a prefix. \
             Write the fallible expression first, then `catch`.\n  \
             Example: `let x = divine(\"prompt\") catch { \"fallback\" };`"
                .to_string(),
        ),

        _ => None,
    }
}

/// Format a parse error with an optional hint appended.
///
/// This is the recommended replacement for bare `format!("{e}")` on
/// parse errors throughout the codebase.
#[must_use]
pub fn format_error(error: &ParseError) -> String {
    let base = format!("{error}");
    match error_hint(error) {
        Some(hint) => format!("{base}\n  {hint}"),
        None => base,
    }
}

// ── helpers ─────────────────────────────────────────────────────────

/// Does the expected-token set look like "things that follow an expression"?
///
/// When the parser has just consumed a complete expression it expects
/// binary operators, semicolons, or closing delimiters — NOT the start
/// of a new expression.  If the expected set contains operators like
/// `+`, `*`, `==`, etc., we are in postfix position.
fn looks_like_postfix_position(expected: &[Option<Token>]) -> bool {
    let postfix_tokens = [
        Token::Plus,
        Token::Star,
        Token::Semicolon,
        Token::PlusPlus,
    ];
    postfix_tokens
        .iter()
        .any(|t| expected.contains(&Some(t.clone())))
}

/// Does the expected-token set look like "things that start an expression"?
///
/// After a binary operator the parser expects the RHS, so the expected set
/// contains identifiers, literals, unary operators, and keywords that begin
/// an expression (`try`, `match`, `self`, etc.).
fn looks_like_expr_start_position(expected: &[Option<Token>]) -> bool {
    let expr_start_tokens = [
        Token::Ident,
        Token::IntLit,
        Token::KwSelf,
        Token::KwTry,
        Token::KwMatch,
    ];
    expr_start_tokens
        .iter()
        .any(|t| expected.contains(&Some(t.clone())))
}

/// Check whether a specific token appears in the expected set.
fn expected_contains(expected: &[Option<Token>], token: &Token) -> bool {
    expected.contains(&Some(token.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chumsky::prelude::*;

    /// Build a synthetic parse error: `found X, expected [Y...]`.
    fn make_error(found: Token, expected: Vec<Token>) -> ParseError {
        Simple::expected_input_found(0..1, expected.into_iter().map(Some), Some(found))
    }

    #[test]
    fn hint_index_syntax() {
        let err = make_error(
            Token::LBracket,
            vec![Token::Plus, Token::Star, Token::Semicolon],
        );
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("get(list, index)"), "hint: {hint}");
        assert!(hint.contains("Example:"), "should include example code");
    }

    #[test]
    fn hint_fn_inside_agent() {
        let err = make_error(Token::KwFn, vec![Token::RBrace, Token::KwOn]);
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("top level"), "hint: {hint}");
        assert!(hint.contains("fn helper"), "should include example");
    }

    #[test]
    fn hint_compound_assignment() {
        // After `x -`, the parser expects expression starters for the RHS
        let err = make_error(
            Token::Eq,
            vec![Token::Ident, Token::IntLit, Token::KwSelf, Token::KwTry],
        );
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("count = count - 1"), "hint: {hint}");
    }

    #[test]
    fn hint_self_field_assignment() {
        // `self.field =` — found `=` in postfix position
        let err = make_error(
            Token::Eq,
            vec![
                Token::Plus,
                Token::Star,
                Token::Semicolon,
                Token::PlusPlus,
            ],
        );
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("read-only"), "hint: {hint}");
        assert!(hint.contains("summon"), "should show summon pattern");
    }

    #[test]
    fn hint_let_in_agent_body() {
        // Inside agent body, parser expects `on` or `}`
        let err = make_error(Token::KwLet, vec![Token::RBrace, Token::KwOn]);
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("inside handlers"), "hint: {hint}");
    }

    #[test]
    fn hint_while_in_agent_body() {
        let err = make_error(Token::KwWhile, vec![Token::RBrace, Token::KwOn]);
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("on"), "hint: {hint}");
    }

    #[test]
    fn hint_for_in_agent_body() {
        let err = make_error(Token::KwFor, vec![Token::RBrace, Token::KwOn]);
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("on"), "hint: {hint}");
    }

    #[test]
    fn hint_if_in_agent_body() {
        let err = make_error(Token::KwIf, vec![Token::RBrace, Token::KwOn]);
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("on"), "hint: {hint}");
    }

    #[test]
    fn hint_not_keyword() {
        // `not contains(...)` → `not` parsed as ident, then `contains` is another ident
        let err = make_error(
            Token::Ident,
            vec![Token::Plus, Token::Star, Token::Semicolon, Token::PlusPlus],
        );
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("!"), "hint: {hint}");
    }

    #[test]
    fn hint_catch_prefix() {
        // `catch divine(...)` — catch used as prefix instead of postfix
        let err = make_error(
            Token::KwCatch,
            vec![Token::Ident, Token::IntLit, Token::KwSelf, Token::KwTry],
        );
        let hint = error_hint(&err).expect("should produce a hint");
        assert!(hint.contains("postfix"), "hint: {hint}");
    }

    #[test]
    fn no_hint_for_generic_error() {
        // `let` with only `}` expected (not in agent context) should NOT hint
        let err = make_error(Token::KwLet, vec![Token::RBrace]);
        assert!(error_hint(&err).is_none());
    }
}
