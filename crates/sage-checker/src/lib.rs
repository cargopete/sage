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
//! use sage_lexer::lex;
//! use sage_parser::parse;
//! use sage_checker::check;
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

pub use checker::{check, check_module_tree, CheckResult, Checker, ModuleCheckResult, ModulePath};
pub use error::CheckError;
pub use scope::{AgentInfo, FunctionInfo, Scope, SymbolTable};
pub use types::Type;

#[cfg(test)]
mod tests {
    use super::*;
    use sage_lexer::lex;
    use sage_parser::parse;
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

    #[test]
    fn check_minimal_valid_program() {
        let source = r#"
            agent Main {
                on start {
                    emit(42);
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
                    emit(x);
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
                    emit(x);
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
                    emit(x);
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
                    emit(x);
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
                    emit(sum);
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
                    emit(msg);
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
                    emit(self.name);
                }
            }

            agent Main {
                on start {
                    let w = spawn Worker { name: "test" };
                    let result = await w;
                    emit(result);
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
                    emit(self.name);
                }
            }

            agent Main {
                on start {
                    let w = spawn Worker { };
                    emit(0);
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
                    emit(self.x);
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
                    emit(0);
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
                        emit(1);
                    }
                    emit(0);
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
                    emit(n);
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
                        emit(1);
                    }
                    emit(0);
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
                    emit(0);
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
    fn check_infer_returns_inferred_type() {
        let source = r#"
            agent Main {
                on start {
                    let x: Inferred<String> = infer("Hello");
                    emit(x);
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
                    emit(msg);
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
                    emit(0);
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
                    emit(42);
                }
            }

            agent Main {
                on start {
                    let w = spawn Worker { unused: 1 };
                    emit(await w);
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
                    emit(self.value * 2);
                }
            }

            agent Main {
                on start {
                    let w = spawn Worker { value: 21 };
                    emit(await w);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
        emit(42);
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
                    emit(0);
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
                    emit(sum);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(0);
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
                    emit(x);
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
                    emit(0);
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
                    emit(0);
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
                    emit(count);
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
                    emit(0);
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
                    emit(0);
                }
            }

            agent Main {
                on start { emit(0); }
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
                    emit(0);
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
}
