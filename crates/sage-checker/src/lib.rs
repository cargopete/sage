//! Name resolution and type checker for the Sage language.
//!
//! This crate provides semantic analysis for Sage programs, including:
//! - Name resolution (checking that all identifiers are defined)
//! - Type checking (verifying type correctness of expressions and statements)
//! - Entry agent validation (ensuring the run target is valid)
//!
//! # Example
//!
//! ```
//! use sage_parser::{lex, parse};
//! use sage_checker::check;
//! use std::sync::Arc;
//!
//! let source = r#"
//!     agent Main {
//!         on start {
//!             yield(42);
//!         }
//!     }
//!     run Main;
//! "#;
//!
//! let lex_result = lex(source).expect("lexing failed");
//! let source_arc: Arc<str> = Arc::from(source);
//! let (program, parse_errors) = parse(lex_result.tokens(), source_arc);
//!
//! if let Some(program) = program {
//!     let result = check(&program);
//!     if result.errors.is_empty() {
//!         println!("Type checking passed!");
//!     }
//! }
//! ```

#![forbid(unsafe_code)]

mod checker;
mod error;
mod scope;
mod types;

pub use checker::{
    check, check_module_tree, check_test_file, CheckResult, Checker, ModuleCheckResult, ModulePath,
};
pub use error::CheckError;
pub use scope::{AgentInfo, FunctionInfo, Scope, SymbolTable};
pub use types::Type;

#[cfg(test)]
mod tests {
    use super::*;
    use sage_parser::{lex, parse};
    use std::sync::Arc;

    fn check_source(source: &str) -> (Option<sage_parser::Program>, CheckResult) {
        let lex_result = lex(source).expect("lexing should succeed");
        let source_arc: Arc<str> = Arc::from(source);
        let (program, parse_errors) = parse(lex_result.tokens(), source_arc);
        assert!(parse_errors.is_empty(), "parse errors: {parse_errors:?}");
        let program = program.expect("should parse");
        let result = check(&program);
        (Some(program), result)
    }

    fn check_test_source(source: &str) -> (Option<sage_parser::Program>, CheckResult) {
        let lex_result = lex(source).expect("lexing should succeed");
        let source_arc: Arc<str> = Arc::from(source);
        let (program, parse_errors) = parse(lex_result.tokens(), source_arc);
        assert!(parse_errors.is_empty(), "parse errors: {parse_errors:?}");
        let program = program.expect("should parse");
        let result = check_test_file(&program);
        (Some(program), result)
    }

    #[test]
    fn check_minimal_valid_program() {
        let source = r#"
            agent Main {
                on start {
                    yield(42);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_undefined_variable() {
        let source = r#"
            agent Main {
                on start {
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::UndefinedVariable { .. }
        ));
    }

    #[test]
    fn check_type_mismatch_in_binary_op() {
        let source = r#"
            agent Main {
                on start {
                    let x = 1 + "hello";
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::InvalidBinaryOp { .. }
        ));
    }

    #[test]
    fn check_let_with_type_annotation() {
        let source = r#"
            agent Main {
                on start {
                    let x: Int = 42;
                    let y: String = "hello";
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_type_mismatch_in_let() {
        let source = r#"
            agent Main {
                on start {
                    let x: String = 42;
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::TypeMismatch { .. }));
    }

    #[test]
    fn check_function_call() {
        let source = r#"
            fn add(a: Int, b: Int) -> Int {
                return a + b;
            }

            agent Main {
                on start {
                    let sum = add(1, 2);
                    yield(sum);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_wrong_arg_count() {
        let source = r#"
            fn greet(name: String) -> String {
                return name;
            }

            agent Main {
                on start {
                    let msg = greet("a", "b");
                    yield(msg);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::WrongArgCount { .. }));
    }

    #[test]
    fn check_spawn_with_beliefs() {
        let source = r#"
            agent Worker {
                name: String

                on start {
                    yield(self.name);
                }
            }

            agent Main {
                on start {
                    let w = summon Worker { name: "test" };
                    let result = try await w;
                    yield(result);
                }

                on error(e) {
                    yield("error");
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_missing_belief_init() {
        let source = r#"
            agent Worker {
                name: String

                on start {
                    yield(self.name);
                }
            }

            agent Main {
                on start {
                    let w = summon Worker { };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::MissingBeliefInit { .. }
        ));
    }

    #[test]
    fn check_entry_agent_with_beliefs() {
        let source = r#"
            agent Main {
                x: Int

                on start {
                    yield(self.x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::EntryAgentHasBeliefs { .. }
        ));
    }

    #[test]
    fn check_for_loop() {
        let source = r#"
            agent Main {
                on start {
                    for x in [1, 2, 3] {
                        print(int_to_str(x));
                    }
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_if_non_bool_condition() {
        let source = r#"
            agent Main {
                on start {
                    if 42 {
                        yield(1);
                    }
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::NonBoolCondition { .. }
        ));
    }

    #[test]
    fn check_while_loop() {
        let source = r#"
            agent Main {
                on start {
                    let n = 5;
                    while n > 0 {
                        n = n - 1;
                    }
                    yield(n);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_while_non_bool_condition() {
        let source = r#"
            agent Main {
                on start {
                    while 42 {
                        yield(1);
                    }
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::NonBoolCondition { .. }
        ));
    }

    #[test]
    fn check_self_outside_agent() {
        let source = r#"
            fn broken() -> Int {
                return self.x;
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::SelfOutsideAgent { .. }
        ));
    }

    #[test]
    fn check_divine_returns_oracle_type() {
        let source = r#"
            agent Main {
                on start {
                    let x: Oracle<String> = try divine("Hello");
                    yield(x);
                }

                on error(e) {
                    yield("error");
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_string_concat() {
        let source = r#"
            agent Main {
                on start {
                    let msg = "Hello, " ++ "World";
                    yield(msg);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_builtin_print() {
        let source = r#"
            agent Main {
                on start {
                    print("Hello");
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_unused_belief_warning() {
        let source = r#"
            agent Worker {
                unused: Int

                on start {
                    yield(42);
                }
            }

            agent Main {
                on start {
                    let w = summon Worker { unused: 1 };
                    let result = try await w;
                    yield(result);
                }

                on error(e) {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        // Should have exactly one warning for unused belief
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(result.errors[0], CheckError::UnusedBelief { .. }));
    }

    #[test]
    fn check_used_belief_no_warning() {
        let source = r#"
            agent Worker {
                value: Int

                on start {
                    yield(self.value * 2);
                }
            }

            agent Main {
                on start {
                    let w = summon Worker { value: 21 };
                    let result = try await w;
                    yield(result);
                }

                on error(e) {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_str_builtin() {
        let source = r#"
            agent Main {
                on start {
                    let a = str(42);
                    let b = str(3.14);
                    let c = str(true);
                    let d = str("hello");
                    print(a ++ b ++ c ++ d);
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_string_interpolation() {
        let source = r#"
            agent Main {
                on start {
                    let name = "World";
                    let count = 42;
                    let msg = "Hello, {name}! Count is {count}.";
                    print(msg);
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_string_interpolation_undefined_var() {
        let source = r#"
            agent Main {
                on start {
                    let msg = "Hello, {undefined}!";
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::UndefinedVariable { .. }
        ));
    }

    #[test]
    fn check_module_tree_simple() {
        use sage_loader::load_single_file;
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.sg");
        fs::write(
            &file,
            r#"
agent Main {
    on start {
        yield(42);
    }
}
run Main;
"#,
        )
        .unwrap();

        let tree = load_single_file(&file).unwrap();
        let result = check_module_tree(&tree);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_record_construction() {
        let source = r#"
            record Point {
                x: Int,
                y: Int,
            }

            agent Main {
                on start {
                    let p = Point { x: 10, y: 20 };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_record_field_access() {
        let source = r#"
            record Point {
                x: Int,
                y: Int,
            }

            agent Main {
                on start {
                    let p = Point { x: 10, y: 20 };
                    let sum = p.x + p.y;
                    yield(sum);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_record_missing_field() {
        let source = r#"
            record Point {
                x: Int,
                y: Int,
            }

            agent Main {
                on start {
                    let p = Point { x: 10 };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::MissingField { .. }));
    }

    #[test]
    fn check_record_unknown_field() {
        let source = r#"
            record Point {
                x: Int,
                y: Int,
            }

            agent Main {
                on start {
                    let p = Point { x: 10, y: 20, z: 30 };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::UnknownField { .. }));
    }

    #[test]
    fn check_undefined_record_type() {
        let source = r#"
            agent Main {
                on start {
                    let p = Unknown { x: 10 };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::UndefinedType { .. }));
    }

    #[test]
    fn check_field_access_on_non_record() {
        let source = r#"
            agent Main {
                on start {
                    let x = 42;
                    let y = x.field;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::FieldAccessOnNonRecord { .. }
        ));
    }

    #[test]
    fn check_record_field_type_mismatch() {
        let source = r#"
            record Point {
                x: Int,
                y: Int,
            }

            agent Main {
                on start {
                    let p = Point { x: "not an int", y: 20 };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::TypeMismatch { .. }));
    }

    #[test]
    fn check_match_exhaustive_enum() {
        let source = r#"
            enum Status {
                Active,
                Inactive,
            }

            fn check_status(s: Status) -> Int {
                return match s {
                    Active => 1,
                    Inactive => 0,
                };
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_match_non_exhaustive_enum() {
        let source = r#"
            enum Status {
                Active,
                Inactive,
                Pending,
            }

            fn check_status(s: Status) -> Int {
                return match s {
                    Active => 1,
                    Inactive => 0,
                };
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::NonExhaustiveMatch { .. }
        ));
    }

    #[test]
    fn check_match_with_wildcard() {
        let source = r#"
            enum Status {
                Active,
                Inactive,
                Pending,
            }

            fn check_status(s: Status) -> Int {
                return match s {
                    Active => 1,
                    _ => 0,
                };
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_match_bool_exhaustive() {
        let source = r#"
            fn bool_to_int(b: Bool) -> Int {
                return match b {
                    true => 1,
                    false => 0,
                };
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_match_bool_non_exhaustive() {
        let source = r#"
            fn bool_to_int(b: Bool) -> Int {
                return match b {
                    true => 1,
                };
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::NonExhaustiveMatch { .. }
        ));
    }

    #[test]
    fn check_match_int_needs_wildcard() {
        let source = r#"
            fn check_int(n: Int) -> String {
                return match n {
                    1 => "one",
                    2 => "two",
                };
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::NonExhaustiveMatch { .. }
        ));
    }

    #[test]
    fn check_match_binding_pattern() {
        let source = r#"
            fn describe(n: Int) -> String {
                return match n {
                    1 => "one",
                    x => "other",
                };
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_const_declaration() {
        let source = r#"
            const MAX_SIZE: Int = 100;
            const GREETING: String = "Hello";

            agent Main {
                on start {
                    let x = MAX_SIZE;
                    print(GREETING);
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_const_type_mismatch() {
        let source = r#"
            const VALUE: Int = "not an int";

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::TypeMismatch { .. }));
    }

    #[test]
    fn check_const_used_as_variable() {
        let source = r#"
            const PI: Float = 3.14;

            fn area(r: Float) -> Float {
                return PI * r * r;
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    // =========================================================================
    // RFC-0006: Message passing tests
    // =========================================================================

    #[test]
    fn check_loop_break() {
        let source = r#"
            agent Main {
                on start {
                    let count = 0;
                    loop {
                        count = count + 1;
                        if count > 5 {
                            break;
                        }
                    }
                    yield(count);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_break_outside_loop() {
        let source = r#"
            agent Main {
                on start {
                    break;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::BreakOutsideLoop { .. }
        ));
    }

    #[test]
    fn check_receive_with_receives() {
        let source = r#"
            enum WorkerMsg {
                Task,
                Shutdown,
            }

            agent Worker receives WorkerMsg {
                on start {
                    let msg = receive();
                    yield(0);
                }
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_receive_without_receives() {
        let source = r#"
            agent Main {
                on start {
                    let msg = receive();
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::ReceiveWithoutReceives { .. }
        ));
    }

    // =========================================================================
    // RFC-0009: Closures and function types
    // =========================================================================

    #[test]
    fn check_closure_with_typed_params() {
        let source = r#"
            agent Main {
                on start {
                    let f = |x: Int| x + 1;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_closure_param_needs_type() {
        let source = r#"
            agent Main {
                on start {
                    let f = |x| x + 1;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::ClosureParamNeedsType { .. }
        ));
    }

    #[test]
    fn check_closure_body_type_error() {
        let source = r#"
            agent Main {
                on start {
                    let f = |x: Int| x + "invalid";
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::InvalidBinaryOp { .. }
        ));
    }

    #[test]
    fn check_empty_closure() {
        let source = r#"
            agent Main {
                on start {
                    let f = || 42;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    // =========================================================================
    // RFC-0007: Error handling tests
    // =========================================================================

    #[test]
    fn check_e013_unhandled_divine() {
        // E013: infer without try or catch should produce an error
        let source = r#"
            agent Main {
                on start {
                    let x = divine("Hello");
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::UnhandledError { .. }
        ));
    }

    #[test]
    fn check_e013_unhandled_await() {
        // E013: await without try or catch should produce an error
        let source = r#"
            agent Worker {
                on start {
                    yield(42);
                }
            }

            agent Main {
                on start {
                    let w = summon Worker { };
                    let result = await w;
                    yield(result);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::UnhandledError { .. }
        ));
    }

    #[test]
    fn check_e013_handled_with_try() {
        // Using try should NOT produce E013 (when agent has on error handler)
        let source = r#"
            agent Main {
                on start {
                    let x = try divine("Hello");
                    yield(x);
                }

                on error(e) {
                    yield("error");
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_e013_handled_with_catch() {
        // Using catch should NOT produce E013
        let source = r#"
            agent Main {
                on start {
                    let x = divine("Hello") catch { "fallback" };
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_e013_fallible_function_unhandled() {
        // E013: calling a fails function without try or catch
        let source = r#"
            fn risky() -> Int fails { return 42; }

            agent Main {
                on start {
                    let x = risky();
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(
            result.errors[0],
            CheckError::UnhandledError { .. }
        ));
    }

    #[test]
    fn check_e013_fallible_function_handled() {
        // fails function with try should NOT produce E013
        let source = r#"
            fn risky() -> Int fails { return 42; }

            agent Main {
                on start {
                    let x = try risky();
                    yield(x);
                }

                on error(e) {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_persistent_field_not_serializable() {
        // @persistent field with Agent type should produce E052
        let source = r#"
            agent Worker {
                on start {
                    yield(0);
                }
            }

            agent Manager {
                @persistent helper: Agent<Worker>

                on start {
                    yield(0);
                }
            }
            run Manager with { helper: summon Worker };
        "#;

        let (_, result) = check_source(source);
        let persistence_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| matches!(e, CheckError::PersistentFieldNotSerializable { .. }))
            .collect();
        assert!(
            !persistence_errors.is_empty(),
            "expected E052 error, got: {:?}",
            result.errors
        );
        assert!(
            matches!(
                persistence_errors[0],
                CheckError::PersistentFieldNotSerializable { name, .. } if name == "helper"
            ),
            "expected PersistentFieldNotSerializable for 'helper', got {:?}",
            persistence_errors[0]
        );
    }

    #[test]
    fn check_persistent_field_serializable_ok() {
        // @persistent field with serializable type should be fine
        let source = r#"
            agent Counter {
                @persistent count: Int

                on start {
                    yield(self.count);
                }
            }
            run Counter with { count: 0 };
        "#;

        let (_, result) = check_source(source);
        // Filter out W005 (unused belief) if present - we only care about no E052
        let errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| matches!(e, CheckError::PersistentFieldNotSerializable { .. }))
            .collect();
        assert!(
            errors.is_empty(),
            "unexpected persistence error: {:?}",
            errors
        );
    }

    #[test]
    fn check_waking_without_persistent_fields() {
        // on waking without any @persistent fields should produce W006
        let source = r#"
            agent Sleeper {
                count: Int

                on waking {
                    print("woke up");
                }

                on start {
                    yield(self.count);
                }
            }
            run Sleeper with { count: 0 };
        "#;

        let (_, result) = check_source(source);
        let warnings: Vec<_> = result
            .errors
            .iter()
            .filter(|e| matches!(e, CheckError::WakingWithoutPersistentFields { .. }))
            .collect();
        assert!(
            !warnings.is_empty(),
            "expected W006 warning, got {:?}",
            result.errors
        );
    }

    #[test]
    fn check_waking_with_persistent_fields_ok() {
        // on waking WITH @persistent fields should be fine
        let source = r#"
            agent Sleeper {
                @persistent count: Int

                on waking {
                    print("woke up with count: " ++ str(self.count));
                }

                on start {
                    yield(self.count);
                }
            }
            run Sleeper with { count: 0 };
        "#;

        let (_, result) = check_source(source);
        let warnings: Vec<_> = result
            .errors
            .iter()
            .filter(|e| matches!(e, CheckError::WakingWithoutPersistentFields { .. }))
            .collect();
        assert!(
            warnings.is_empty(),
            "unexpected W006 warning: {:?}",
            warnings
        );
    }

    // =========================================================================
    // RFC-0015: Generics tests
    // =========================================================================

    #[test]
    fn check_generic_function_identity() {
        let source = r#"
            fn identity<T>(x: T) -> T {
                return x;
            }

            agent Main {
                on start {
                    let n = identity::<Int>(42);
                    let s = identity::<String>("hello");
                    yield(n);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_generic_record_pair() {
        let source = r#"
            record Pair<A, B> {
                first: A,
                second: B,
            }

            agent Main {
                on start {
                    let p = Pair::<Int, String> { first: 42, second: "hello" };
                    let n = p.first;
                    let s = p.second;
                    yield(n);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_turbofish_type_substitution() {
        let source = r#"
            fn make_pair<A, B>(a: A, b: B) -> (A, B) {
                return (a, b);
            }

            agent Main {
                on start {
                    let pair = make_pair::<Int, String>(42, "hello");
                    let (n, s) = pair;
                    yield(n);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_generic_inference_from_args() {
        let source = r#"
            fn identity<T>(x: T) -> T {
                return x;
            }

            agent Main {
                on start {
                    let n = identity(42);
                    let s = identity("hello");
                    yield(n);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_generic_wrong_type_args_count() {
        let source = r#"
            fn identity<T>(x: T) -> T {
                return x;
            }

            agent Main {
                on start {
                    let n = identity::<Int, String>(42);
                    yield(n);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::GenericError { .. }));
    }

    #[test]
    fn check_generic_type_mismatch() {
        let source = r#"
            fn identity<T>(x: T) -> T {
                return x;
            }

            agent Main {
                on start {
                    let n: String = identity::<Int>(42);
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::TypeMismatch { .. }));
    }

    #[test]
    fn check_generic_record_field_type_mismatch() {
        let source = r#"
            record Box<T> {
                value: T,
            }

            agent Main {
                on start {
                    let b = Box::<Int> { value: "not an int" };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::TypeMismatch { .. }));
    }

    #[test]
    fn check_non_generic_function_with_type_args() {
        let source = r#"
            fn add(a: Int, b: Int) -> Int {
                return a + b;
            }

            agent Main {
                on start {
                    let n = add::<Int>(1, 2);
                    yield(n);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(!result.errors.is_empty());
        assert!(matches!(result.errors[0], CheckError::GenericError { .. }));
    }

    #[test]
    fn check_generic_enum_variant() {
        let source = r#"
            enum Either<L, R> {
                Left(L),
                Right(R),
            }

            agent Main {
                on start {
                    let e = Either::<String, Int>::Left("hello");
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_generic_list_param() {
        let source = r#"
            fn len_of<T>(list: List<T>) -> Int {
                return len(list);
            }

            agent Main {
                on start {
                    let x = len_of::<Int>([1, 2, 3]);
                    yield(x);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_generic_nested_types() {
        let source = r#"
            record Box<T> {
                value: T,
            }

            fn wrap<T>(x: T) -> Box<T> {
                return Box { value: x };
            }

            agent Main {
                on start {
                    let b = wrap::<Int>(42);
                    let n = b.value;
                    yield(n);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    // =========================================================================
    // Phase 3: Session Types & Algebraic Effects tests
    // =========================================================================

    #[test]
    fn check_protocol_declaration() {
        let source = r#"
            protocol SchemaSync {
                A -> B: String
                B -> A: Int
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_agent_follows_valid_protocol() {
        let source = r#"
            protocol PingPong {
                Pinger -> Ponger: String
                Ponger -> Pinger: Int
            }

            agent Worker follows PingPong as Ponger {
                on start {
                    yield(0);
                }
            }
            run Worker;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_agent_follows_unknown_protocol() {
        let source = r#"
            agent Worker follows UnknownProto as SomeRole {
                on start {
                    yield(0);
                }
            }
            run Worker;
        "#;

        let (_, result) = check_source(source);
        assert_eq!(
            result.errors.len(),
            1,
            "expected 1 error, got {:?}",
            result.errors
        );
        assert!(matches!(
            result.errors[0],
            CheckError::UnknownProtocol { .. }
        ));
    }

    #[test]
    fn check_agent_follows_invalid_role() {
        let source = r#"
            protocol PingPong {
                Pinger -> Ponger: String
            }

            agent Worker follows PingPong as InvalidRole {
                on start {
                    yield(0);
                }
            }
            run Worker;
        "#;

        let (_, result) = check_source(source);
        assert_eq!(
            result.errors.len(),
            1,
            "expected 1 error, got {:?}",
            result.errors
        );
        assert!(matches!(
            result.errors[0],
            CheckError::UnknownProtocolRole { .. }
        ));
    }

    #[test]
    fn check_reply_in_message_handler() {
        let source = r#"
            agent Worker receives String {
                on start {
                    yield(0);
                }
                on message(msg: String) {
                    reply(42);
                }
            }
            run Worker;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_reply_outside_message_handler() {
        let source = r#"
            agent Worker {
                on start {
                    reply(42);
                }
            }
            run Worker;
        "#;

        let (_, result) = check_source(source);
        assert_eq!(
            result.errors.len(),
            1,
            "expected 1 error, got {:?}",
            result.errors
        );
        assert!(matches!(
            result.errors[0],
            CheckError::ReplyOutsideMessageHandler { .. }
        ));
    }

    #[test]
    fn check_effect_handler_declaration() {
        let source = r#"
            handler DefaultLLM handles Infer {
                model: "gpt-4o"
                temperature: 0.7
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_supervisor_with_valid_handler_assignment() {
        let source = r#"
            handler DefaultLLM handles Infer {
                model: "gpt-4o"
            }

            agent Worker {
                on start {
                    yield(0);
                }
            }

            supervisor AppSupervisor {
                strategy: OneForOne
                children {
                    Worker {
                        restart: Permanent
                        handler Infer: DefaultLLM
                    }
                }
            }

            run AppSupervisor;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_supervisor_with_unknown_handler() {
        let source = r#"
            agent Worker {
                on start {
                    yield(0);
                }
            }

            supervisor AppSupervisor {
                strategy: OneForOne
                children {
                    Worker {
                        restart: Permanent
                        handler Infer: NonExistentHandler
                    }
                }
            }

            run AppSupervisor;
        "#;

        let (_, result) = check_source(source);
        assert_eq!(
            result.errors.len(),
            1,
            "expected 1 error, got {:?}",
            result.errors
        );
        assert!(matches!(
            result.errors[0],
            CheckError::UnknownEffectHandler { .. }
        ));
    }

    // =========================================================================
    // Phase 3: Session Types - Protocol Verification Tests
    // =========================================================================

    #[test]
    fn check_protocol_valid_send() {
        let source = r#"
            record Ping {}
            record Pong {}

            protocol PingPong {
                Pinger -> Ponger: Ping
                Ponger -> Pinger: Pong
            }

            agent Pinger follows PingPong as Pinger {
                on start {
                    let p = summon Ponger {};
                    try send(p, Ping {});
                }
                on error(e) {
                    print("error");
                }
            }

            agent Ponger follows PingPong as Ponger {
                on message(m: Ping) {
                    reply(Pong {});
                }
            }

            run Pinger;
        "#;

        let (_, result) = check_source(source);
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
    }

    #[test]
    fn check_protocol_wrong_message_type() {
        // Sending WrongMsg when protocol only allows Ping
        let source = r#"
            record Ping {}
            record Pong {}
            record WrongMsg {}

            protocol PingPong {
                Pinger -> Ponger: Ping
                Ponger -> Pinger: Pong
            }

            agent Pinger follows PingPong as Pinger {
                on start {
                    let p = summon Ponger {};
                    try send(p, WrongMsg {});
                }
                on error(e) {
                    print("error");
                }
            }

            agent Ponger follows PingPong as Ponger {
                on message(m: Ping) {
                    reply(Pong {});
                }
            }

            run Pinger;
        "#;

        let (_, result) = check_source(source);
        // Expect E074: ProtocolMessageMismatch (and possibly a type mismatch for the handler)
        let has_protocol_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::ProtocolMessageMismatch { .. }));
        assert!(
            has_protocol_error,
            "expected E074 ProtocolMessageMismatch, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_protocol_missing_reply() {
        // Ponger doesn't call reply() but protocol expects it
        let source = r#"
            record Ping {}
            record Pong {}

            protocol PingPong {
                Pinger -> Ponger: Ping
                Ponger -> Pinger: Pong
            }

            agent Pinger follows PingPong as Pinger {
                on start {
                    let p = summon Ponger {};
                    try send(p, Ping {});
                }
                on error(e) {
                    print("error");
                }
            }

            agent Ponger follows PingPong as Ponger {
                on message(m: Ping) {
                    // Missing reply(Pong {})!
                    print("got ping");
                }
            }

            run Pinger;
        "#;

        let (_, result) = check_source(source);
        let has_missing_reply = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::ProtocolMissingReply { .. }));
        assert!(
            has_missing_reply,
            "expected E076 ProtocolMissingReply, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_protocol_no_shared_protocol() {
        // Two agents follow different protocols - no common ground
        let source = r#"
            record Ping {}
            record Pong {}
            record Request {}
            record Response {}

            protocol PingPong {
                Pinger -> Ponger: Ping
                Ponger -> Pinger: Pong
            }

            protocol ReqRes {
                Client -> Server: Request
                Server -> Client: Response
            }

            agent Pinger follows PingPong as Pinger {
                on start {
                    let s = summon Server {};
                    try send(s, Ping {});
                }
                on error(e) {
                    print("error");
                }
            }

            agent Server follows ReqRes as Server {
                on message(m: Request) {
                    reply(Response {});
                }
            }

            run Pinger;
        "#;

        let (_, result) = check_source(source);
        let has_no_shared = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::NoSharedProtocol { .. }));
        assert!(
            has_no_shared,
            "expected E078 NoSharedProtocol, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_protocol_agent_without_protocol_can_send() {
        // Agent not following any protocol should be allowed to send
        let source = r#"
            record Msg {}

            agent Sender {
                on start {
                    let r = summon Receiver {};
                    try send(r, Msg {});
                }
                on error(e) {
                    print("error");
                }
            }

            agent Receiver {
                on message(m: Msg) {
                    print("got it");
                }
            }

            run Sender;
        "#;

        let (_, result) = check_source(source);
        // No protocol errors expected - both agents don't follow protocols
        let has_protocol_error = result.errors.iter().any(|e| {
            matches!(
                e,
                CheckError::ProtocolMessageMismatch { .. }
                    | CheckError::NoSharedProtocol { .. }
                    | CheckError::ProtocolMissingReply { .. }
            )
        });
        assert!(
            !has_protocol_error,
            "unexpected protocol error: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_protocol_reply_outside_handler() {
        // Reply called outside message handler should error
        let source = r#"
            record Msg {}

            agent Broken {
                on start {
                    reply(Msg {});
                }
            }

            run Broken;
        "#;

        let (_, result) = check_source(source);
        let has_reply_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::ReplyOutsideMessageHandler { .. }));
        assert!(
            has_reply_error,
            "expected E073 ReplyOutsideMessageHandler, got: {:?}",
            result.errors
        );
    }

    // =========================================================================
    // v2.0: Explicit Checkpoint Tests (E053)
    // =========================================================================

    #[test]
    fn check_checkpoint_outside_agent() {
        // checkpoint() called outside agent should produce E053
        let source = r#"
            fn helper() -> Int {
                checkpoint();
                return 42;
            }

            agent Worker {
                on start {
                    yield(0);
                }
            }

            run Worker;
        "#;

        let (_, result) = check_source(source);
        let has_checkpoint_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::CheckpointOutsideAgent { .. }));
        assert!(
            has_checkpoint_error,
            "expected E053 CheckpointOutsideAgent, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_checkpoint_inside_agent_ok() {
        // checkpoint() inside agent handler should be fine
        let source = r#"
            agent Worker {
                @persistent count: Int

                on start {
                    checkpoint();
                    yield(0);
                }
            }

            run Worker;
        "#;

        let (_, result) = check_source(source);
        let has_checkpoint_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::CheckpointOutsideAgent { .. }));
        assert!(
            !has_checkpoint_error,
            "unexpected E053 error: {:?}",
            result.errors
        );
    }

    // =========================================================================
    // v2.0: Supervisor Validation Tests (E061, E062)
    // =========================================================================
    //
    // Note: E060 (SupervisorNoChildren) is enforced at the parser level - the
    // children block requires at least one child. W004 (PermanentWithoutPersistence)
    // is not yet implemented - it would require extending AgentInfo to track
    // @persistent fields.

    #[test]
    fn check_supervisor_child_not_found() {
        // Supervisor referencing nonexistent agent should produce E061
        let source = r#"
            supervisor BadSupervisor {
                strategy: OneForOne
                children {
                    NonexistentAgent {}
                }
            }

            run BadSupervisor;
        "#;

        let (_, result) = check_source(source);
        let has_not_found_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::SupervisorChildNotFound { .. }));
        assert!(
            has_not_found_error,
            "expected E061 SupervisorChildNotFound, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_supervisor_child_missing_belief() {
        // Supervisor child missing required belief should produce E062
        let source = r#"
            agent Worker {
                count: Int
                name: String

                on start {
                    yield(self.count);
                }
            }

            supervisor BadSupervisor {
                strategy: OneForOne
                children {
                    Worker { count: 0 }
                }
            }

            run BadSupervisor;
        "#;

        let (_, result) = check_source(source);
        let has_missing_belief_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::SupervisorChildMissingBelief { .. }));
        assert!(
            has_missing_belief_error,
            "expected E062 SupervisorChildMissingBelief, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_supervisor_valid() {
        // Valid supervisor should pass
        let source = r#"
            agent Worker {
                count: Int

                on start {
                    yield(self.count);
                }
            }

            supervisor AppSupervisor {
                strategy: OneForOne
                children {
                    Worker { count: 0 }
                }
            }

            run AppSupervisor;
        "#;

        let (_, result) = check_source(source);
        let has_supervisor_error = result.errors.iter().any(|e| {
            matches!(
                e,
                CheckError::SupervisorNoChildren { .. }
                    | CheckError::SupervisorChildNotFound { .. }
                    | CheckError::SupervisorChildMissingBelief { .. }
            )
        });
        assert!(
            !has_supervisor_error,
            "unexpected supervisor error: {:?}",
            result.errors
        );
    }

    // =========================================================================
    // v2.0: Additional Error Code Tests
    // =========================================================================

    #[test]
    fn check_duplicate_test_name() {
        // Duplicate test names should produce E055
        let source = r#"
            test "my test" {
                assert_true(true);
            }

            test "my test" {
                assert_true(true);
            }
        "#;

        let (_, result) = check_test_source(source);
        let has_duplicate_test = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::DuplicateTestName { .. }));
        assert!(
            has_duplicate_test,
            "expected E055 DuplicateTestName, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_mock_divine_outside_test() {
        // mock divine outside test block should produce E056
        let source = r#"
            agent Worker {
                on start {
                    mock divine -> "test";
                    yield(0);
                }
            }
            run Worker;
        "#;

        let (_, result) = check_source(source);
        let has_mock_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::MockInferOutsideTest { .. }));
        assert!(
            has_mock_error,
            "expected E056 MockInferOutsideTest, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_mock_tool_outside_test() {
        // mock tool outside test block should produce E057
        let source = r#"
            tool Http {
                fn get(url: String) -> Result<String, String>
            }

            agent Worker {
                use Http
                on start {
                    mock tool Http.get -> "response";
                    yield(0);
                }
            }
            run Worker;
        "#;

        let (_, result) = check_source(source);
        let has_mock_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::MockToolOutsideTest { .. }));
        assert!(
            has_mock_error,
            "expected E057 MockToolOutsideTest, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_mock_fail_not_string() {
        // mock divine -> fail(non-string) should produce E058
        let source = r#"
            test "bad mock fail" {
                mock divine -> fail(42);
                assert_true(true);
            }
        "#;

        let (_, result) = check_test_source(source);
        let has_fail_error = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::MockFailNotString { .. }));
        assert!(
            has_fail_error,
            "expected E058 MockFailNotString, got: {:?}",
            result.errors
        );
    }

    // Note: W004 PermanentWithoutPersistence is not yet implemented per checker.rs:721
    // The warning requires extending AgentInfo to track @persistent fields.

    #[test]
    fn check_unknown_protocol() {
        // Following an unknown protocol should produce E070
        let source = r#"
            record Msg {}

            agent Worker follows NonExistent as Role {
                on start {
                    yield(0);
                }
            }

            run Worker;
        "#;

        let (_, result) = check_source(source);
        let has_unknown_protocol = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::UnknownProtocol { .. }));
        assert!(
            has_unknown_protocol,
            "expected E070 UnknownProtocol, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_unknown_protocol_role() {
        // Following a protocol with unknown role should produce E071
        let source = r#"
            record Msg {}

            protocol MyProtocol {
                Sender -> Receiver: Msg
            }

            agent Worker follows MyProtocol as UnknownRole {
                on start {
                    yield(0);
                }
            }

            run Worker;
        "#;

        let (_, result) = check_source(source);
        let has_unknown_role = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::UnknownProtocolRole { .. }));
        assert!(
            has_unknown_role,
            "expected E071 UnknownProtocolRole, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn check_unknown_effect_handler() {
        // Using an unknown effect handler in supervisor should produce E072
        let source = r#"
            agent Worker {
                on start {
                    yield(0);
                }
            }

            supervisor Supervisor {
                strategy: OneForOne
                children {
                    Worker { handler Infer: NonExistentHandler }
                }
            }

            run Supervisor;
        "#;

        let (_, result) = check_source(source);
        let has_unknown_handler = result
            .errors
            .iter()
            .any(|e| matches!(e, CheckError::UnknownEffectHandler { .. }));
        assert!(
            has_unknown_handler,
            "expected E072 UnknownEffectHandler, got: {:?}",
            result.errors
        );
    }
}
