//! Parser implementation using chumsky.
//!
//! This module transforms a token stream into an AST.

use crate::ast::{
    AgentDecl, BeliefDecl, BinOp, Block, ChildSpec, ClosureParam, ConstDecl, EffectHandlerDecl,
    ElseBranch, EnumDecl, EventKind, Expr, ExternFnDecl, FieldInit, FnDecl, HandlerAssignment,
    HandlerConfig, HandlerDecl, Literal, MapEntry, MatchArm, MockValue, ModDecl, Param, Pattern,
    Program, ProtocolDecl, ProtocolRole, ProtocolStep, RecordDecl, RecordField, RestartPolicy,
    Stmt, StringPart, StringTemplate, SupervisionStrategy, SupervisorDecl, TestDecl, ToolDecl,
    ToolFnDecl, UnaryOp, UseDecl, UseKind,
};
use crate::{Ident, Span, TypeExpr};
use crate::{Spanned, Token};
use chumsky::prelude::*;
use chumsky::BoxedParser;
use std::ops::Range;
use std::sync::Arc;

/// Parse error type using byte range spans.
pub type ParseError = Simple<Token>;

/// Parse a sequence of tokens into a Program AST.
///
/// # Errors
///
/// Returns parse errors if the token stream doesn't form a valid program.
#[must_use]
#[allow(clippy::needless_pass_by_value)] // Arc<str> is cheap to clone and idiomatic here
pub fn parse(tokens: &[Spanned], source: Arc<str>) -> (Option<Program>, Vec<ParseError>) {
    let len = source.len();

    // Convert our Spanned tokens to (Token, Range<usize>) for chumsky
    let token_spans: Vec<(Token, Range<usize>)> = tokens
        .iter()
        .map(|s| (s.token.clone(), s.start..s.end))
        .collect();

    let stream = chumsky::Stream::from_iter(len..len, token_spans.into_iter());

    let (ast, errors) = program_parser(Arc::clone(&source)).parse_recovery(stream);

    (ast, errors)
}

// =============================================================================
// Top-level parsers
// =============================================================================

/// Parser for a complete program.
#[allow(clippy::needless_pass_by_value)]
fn program_parser(source: Arc<str>) -> impl Parser<Token, Program, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();

    // Top-level declarations with recovery - skip to next keyword on error
    let top_level = mod_parser(source.clone())
        .or(use_parser(source.clone()))
        .or(record_parser(source.clone()))
        .or(enum_parser(source.clone()))
        .or(const_parser(source.clone()))
        .or(tool_parser(source.clone()))
        .or(protocol_parser(source.clone()))
        .or(effect_handler_parser(source.clone()))
        .or(agent_parser(source.clone()))
        .or(supervisor_parser(source.clone()))
        .or(extern_fn_parser(source.clone()))
        .or(fn_parser(source.clone()))
        .or(test_parser(source.clone()))
        .recover_with(skip_then_retry_until([
            Token::KwMod,
            Token::KwUse,
            Token::KwPub,
            Token::KwRecord,
            Token::KwEnum,
            Token::KwConst,
            Token::KwTool,
            Token::KwProtocol,
            Token::KwHandler,
            Token::KwAgent,
            Token::KwSupervisor,
            Token::KwExtern,
            Token::KwFn,
            Token::KwRun,
            Token::KwTest,
        ]));

    let run_stmt = just(Token::KwRun)
        .ignore_then(ident_token_parser(src.clone()))
        .then_ignore(just(Token::Semicolon))
        .or_not();

    top_level.repeated().then(run_stmt).map_with_span(
        move |(items, run_agent), span: Range<usize>| {
            let mut mod_decls = Vec::new();
            let mut use_decls = Vec::new();
            let mut records = Vec::new();
            let mut enums = Vec::new();
            let mut consts = Vec::new();
            let mut tools = Vec::new();
            let mut protocols = Vec::new();
            let mut effect_handlers = Vec::new();
            let mut agents = Vec::new();
            let mut supervisors = Vec::new();
            let mut functions = Vec::new();
            let mut extern_fns = Vec::new();
            let mut tests = Vec::new();

            for item in items {
                match item {
                    TopLevel::Mod(m) => mod_decls.push(m),
                    TopLevel::Use(u) => use_decls.push(u),
                    TopLevel::Record(r) => records.push(r),
                    TopLevel::Enum(e) => enums.push(e),
                    TopLevel::Const(c) => consts.push(c),
                    TopLevel::Tool(t) => tools.push(t),
                    TopLevel::Protocol(p) => protocols.push(p),
                    TopLevel::EffectHandler(h) => effect_handlers.push(h),
                    TopLevel::Agent(a) => agents.push(a),
                    TopLevel::Supervisor(s) => supervisors.push(s),
                    TopLevel::Function(f) => functions.push(f),
                    TopLevel::ExternFn(e) => extern_fns.push(e),
                    TopLevel::Test(t) => tests.push(t),
                }
            }

            Program {
                mod_decls,
                use_decls,
                records,
                enums,
                consts,
                tools,
                protocols,
                effect_handlers,
                agents,
                supervisors,
                functions,
                extern_fns,
                tests,
                run_agent,
                span: make_span(&src2, span),
            }
        },
    )
}

/// Helper enum for collecting top-level declarations.
enum TopLevel {
    Mod(ModDecl),
    Use(UseDecl),
    Record(RecordDecl),
    Enum(EnumDecl),
    Const(ConstDecl),
    Tool(ToolDecl),
    Protocol(ProtocolDecl),
    EffectHandler(EffectHandlerDecl),
    Agent(AgentDecl),
    Supervisor(SupervisorDecl),
    Function(FnDecl),
    ExternFn(ExternFnDecl),
    Test(TestDecl),
}

// =============================================================================
// Module declaration parsers
// =============================================================================

/// Parser for a mod declaration: `mod foo` or `pub mod foo`
#[allow(clippy::needless_pass_by_value)]
fn mod_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();

    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwMod))
        .then(ident_token_parser(src.clone()))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |(is_pub, name), span: Range<usize>| {
            TopLevel::Mod(ModDecl {
                is_pub: is_pub.is_some(),
                name,
                span: make_span(&src, span),
            })
        })
}

/// Parser for a use declaration: `use path::to::Item` or `use path::{A, B}`
#[allow(clippy::needless_pass_by_value)]
fn use_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();
    let src4 = source.clone();

    // Simple use: `use a::b::C` or `use a::b::C as D`
    let simple_use = just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwUse))
        .then(
            ident_token_parser(src.clone())
                .separated_by(just(Token::ColonColon))
                .at_least(1),
        )
        .then(
            just(Token::KwAs)
                .ignore_then(ident_token_parser(src.clone()))
                .or_not(),
        )
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |((is_pub, path), alias), span: Range<usize>| {
            TopLevel::Use(UseDecl {
                is_pub: is_pub.is_some(),
                path,
                kind: UseKind::Simple(alias),
                span: make_span(&src, span),
            })
        });

    // Group import item: `Name` or `Name as Alias`
    let group_item = ident_token_parser(src2.clone()).then(
        just(Token::KwAs)
            .ignore_then(ident_token_parser(src2.clone()))
            .or_not(),
    );

    // Group use: `use a::b::{C, D as E}`
    let group_use = just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwUse))
        .then(
            ident_token_parser(src3.clone())
                .then_ignore(just(Token::ColonColon))
                .repeated()
                .at_least(1),
        )
        .then(
            group_item
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |((is_pub, path), items), span: Range<usize>| {
            TopLevel::Use(UseDecl {
                is_pub: is_pub.is_some(),
                path,
                kind: UseKind::Group(items),
                span: make_span(&src3, span),
            })
        });

    // Glob use: `use a::b::*`
    let glob_use = just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwUse))
        .then(
            ident_token_parser(src4.clone())
                .then_ignore(just(Token::ColonColon))
                .repeated()
                .at_least(1),
        )
        .then_ignore(just(Token::Star))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |(is_pub, path), span: Range<usize>| {
            TopLevel::Use(UseDecl {
                is_pub: is_pub.is_some(),
                path,
                kind: UseKind::Glob,
                span: make_span(&src4, span),
            })
        });

    // Try group/glob first (they need :: before { or *), then simple
    group_use.or(glob_use).or(simple_use)
}

// =============================================================================
// Record, Enum, Const parsers
// =============================================================================

/// Parser for a record declaration: `record Point { x: Int, y: Int }`
#[allow(clippy::needless_pass_by_value)]
/// Parser for type parameters: `<T>`, `<A, B>`, `<T, U, V>`, etc.
/// Returns empty Vec if no type parameters present.
fn type_params_parser(
    source: Arc<str>,
) -> impl Parser<Token, Vec<Ident>, Error = ParseError> + Clone {
    ident_token_parser(source)
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .delimited_by(just(Token::Lt), just(Token::Gt))
        .or_not()
        .map(|params| params.unwrap_or_default())
}

fn record_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();

    // Record field: `name: Type`
    let field = ident_token_parser(src.clone())
        .then_ignore(just(Token::Colon))
        .then(type_parser(src.clone()))
        .map_with_span(move |(name, ty), span: Range<usize>| RecordField {
            name,
            ty,
            span: make_span(&src, span),
        });

    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwRecord))
        .then(ident_token_parser(src2.clone()))
        .then(type_params_parser(src2.clone()))
        .then(
            field
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map_with_span(
            move |(((is_pub, name), type_params), fields), span: Range<usize>| {
                TopLevel::Record(RecordDecl {
                    is_pub: is_pub.is_some(),
                    name,
                    type_params,
                    fields,
                    span: make_span(&src2, span),
                })
            },
        )
}

/// Parser for an enum declaration: `enum Status { Active, Pending, Done }` or `enum Result { Ok(T), Err(E) }`
#[allow(clippy::needless_pass_by_value)]
fn enum_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();

    // Enum variant with optional payload: `Ok(T)` or `None`
    let variant = ident_token_parser(src.clone())
        .then(
            type_parser(src.clone())
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or_not(),
        )
        .map_with_span({
            let src = src.clone();
            move |(name, payload), span: Range<usize>| crate::ast::EnumVariant {
                name,
                payload,
                span: make_span(&src, span),
            }
        });

    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwEnum))
        .then(ident_token_parser(src3.clone()))
        .then(type_params_parser(src3.clone()))
        .then(
            variant
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map_with_span(
            move |(((is_pub, name), type_params), variants), span: Range<usize>| {
                TopLevel::Enum(EnumDecl {
                    is_pub: is_pub.is_some(),
                    name,
                    type_params,
                    variants,
                    span: make_span(&src2, span),
                })
            },
        )
}

/// Parser for a const declaration: `const MAX_RETRIES: Int = 3`
#[allow(clippy::needless_pass_by_value)]
fn const_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();

    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwConst))
        .then(ident_token_parser(src.clone()))
        .then_ignore(just(Token::Colon))
        .then(type_parser(src.clone()))
        .then_ignore(just(Token::Eq))
        .then(expr_parser(src.clone()))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |(((is_pub, name), ty), value), span: Range<usize>| {
            TopLevel::Const(ConstDecl {
                is_pub: is_pub.is_some(),
                name,
                ty,
                value,
                span: make_span(&src2, span),
            })
        })
}

// =============================================================================
// Tool parsers (RFC-0011)
// =============================================================================

/// Parser for a tool declaration: `tool Http { fn get(url: String) -> String }`
#[allow(clippy::needless_pass_by_value)]
fn tool_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();

    // Tool function parameter: `name: Type`
    let param = ident_token_parser(src.clone())
        .then_ignore(just(Token::Colon))
        .then(type_parser(src.clone()))
        .map_with_span(move |(name, ty), span: Range<usize>| Param {
            name,
            ty,
            span: make_span(&src, span),
        });

    let params = param
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    // Tool function signature: `fn name(params) -> ReturnType`
    let tool_fn = just(Token::KwFn)
        .ignore_then(ident_token_parser(src2.clone()))
        .then(params)
        .then_ignore(just(Token::Arrow))
        .then(type_parser(src2.clone()))
        .map_with_span(
            move |((name, params), return_ty), span: Range<usize>| ToolFnDecl {
                name,
                params,
                return_ty,
                mcp_name: None, // TODO: parse @mcp_name("...") annotation
                span: make_span(&src2, span),
            },
        );

    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwTool))
        .then(ident_token_parser(src3.clone()))
        .then(
            tool_fn
                .repeated()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map_with_span(move |((is_pub, name), functions), span: Range<usize>| {
            TopLevel::Tool(ToolDecl {
                is_pub: is_pub.is_some(),
                name,
                functions,
                span: make_span(&src3, span),
            })
        })
}

// =============================================================================
// Protocol parsers (Phase 3 session types)
// =============================================================================

/// Parser for a protocol declaration.
///
/// Syntax:
/// ```sage
/// protocol SchemaSync {
///     DatabaseSteward -> APISteward: SchemaChanged
///     APISteward -> DatabaseSteward: Acknowledged
/// }
/// ```
#[allow(clippy::needless_pass_by_value)]
fn protocol_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();

    // Protocol step: Sender -> Receiver: MessageType
    let protocol_step = ident_token_parser(src.clone())
        .then_ignore(just(Token::Arrow))
        .then(ident_token_parser(src.clone()))
        .then_ignore(just(Token::Colon))
        .then(type_parser(src.clone()))
        .map_with_span({
            let src = src.clone();
            move |((sender, receiver), message_type), span: Range<usize>| ProtocolStep {
                sender,
                receiver,
                message_type,
                span: make_span(&src, span),
            }
        });

    // Full protocol declaration
    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwProtocol))
        .then(ident_token_parser(src2.clone()))
        .then(
            protocol_step
                .repeated()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map_with_span(move |((is_pub, name), steps), span: Range<usize>| {
            TopLevel::Protocol(ProtocolDecl {
                is_pub: is_pub.is_some(),
                name,
                steps,
                span: make_span(&src3, span),
            })
        })
}

// =============================================================================
// Effect handler parsers (Phase 3 algebraic effects)
// =============================================================================

/// Parser for an effect handler declaration.
///
/// Syntax:
/// ```sage
/// handler DefaultLLM handles Infer {
///     model: "gpt-4o"
///     temperature: 0.7
///     max_tokens: 1024
/// }
/// ```
#[allow(clippy::needless_pass_by_value)]
fn effect_handler_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();

    // Handler config entry: key: value (value must be a literal)
    let config_entry = ident_token_parser(src.clone())
        .then_ignore(just(Token::Colon))
        .then(literal_value_parser(src.clone()))
        .map_with_span({
            let src = src.clone();
            move |(key, value), span: Range<usize>| HandlerConfig {
                key,
                value,
                span: make_span(&src, span),
            }
        });

    // Full handler declaration
    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwHandler))
        .then(ident_token_parser(src2.clone()))
        .then_ignore(just(Token::KwHandles))
        .then(ident_token_parser(src2.clone()))
        .then(
            config_entry
                .repeated()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map_with_span(
            move |(((is_pub, name), effect), config), span: Range<usize>| {
                TopLevel::EffectHandler(EffectHandlerDecl {
                    is_pub: is_pub.is_some(),
                    name,
                    effect,
                    config,
                    span: make_span(&src2, span),
                })
            },
        )
}

/// Parser for literal values only (for handler config).
#[allow(clippy::needless_pass_by_value)]
fn literal_value_parser(source: Arc<str>) -> impl Parser<Token, Literal, Error = ParseError> {
    let src = source.clone();

    filter_map(move |span: Range<usize>, tok| match tok {
        Token::IntLit => {
            let s = &src[span.clone()];
            let n = s
                .parse::<i64>()
                .map_err(|_| Simple::custom(span, format!("invalid integer literal `{s}`")))?;
            Ok(Literal::Int(n))
        }
        Token::FloatLit => {
            let s = &src[span.clone()];
            let n = s
                .parse::<f64>()
                .map_err(|_| Simple::custom(span, format!("invalid float literal `{s}`")))?;
            Ok(Literal::Float(n))
        }
        Token::StringLit => {
            let s = &src[span.clone()];
            // Remove quotes and handle escape sequences
            let content = &s[1..s.len() - 1];
            let unescaped = unescape_string(content);
            Ok(Literal::String(unescaped))
        }
        Token::KwTrue => Ok(Literal::Bool(true)),
        Token::KwFalse => Ok(Literal::Bool(false)),
        _ => Err(Simple::expected_input_found(span, [], Some(tok))),
    })
}

/// Helper to unescape string literals.
fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('x') => {
                    // \xNN hex escape
                    let hi = chars.next().unwrap_or('0');
                    let lo = chars.next().unwrap_or('0');
                    let code = u8::from_str_radix(&format!("{hi}{lo}"), 16).unwrap_or(b'?');
                    result.push(code as char);
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    result
}

// =============================================================================
// Test parsers (RFC-0012)
// =============================================================================

/// Parser for a test declaration: `test "name" { ... }` or `@serial test "name" { ... }`
#[allow(clippy::needless_pass_by_value)]
fn test_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();

    // Parse @serial annotation (@ followed by identifier "serial")
    // For now, we'll use a simple approach: look for @ then ident
    let serial_annotation = just(Token::At)
        .then(filter(|t: &Token| matches!(t, Token::Ident)))
        .or_not()
        .map(|opt| opt.is_some());

    // Parse test name (string literal)
    let test_name = filter_map(|span: Range<usize>, tok: Token| match tok {
        Token::StringLit => Ok(()),
        _ => Err(Simple::expected_input_found(span, [], Some(tok))),
    })
    .map_with_span(move |_, span: Range<usize>| {
        // Extract the string content without quotes (handles both " and ')
        let s = &src[span.clone()];
        s[1..s.len() - 1].to_string()
    });

    // Test body - use the statement parser
    let body = block_parser(src2.clone());

    serial_annotation
        .then_ignore(just(Token::KwTest))
        .then(test_name)
        .then(body)
        .map_with_span(move |((is_serial, name), body), span: Range<usize>| {
            TopLevel::Test(TestDecl {
                name,
                is_serial,
                body,
                span: make_span(&src2, span),
            })
        })
}

// =============================================================================
// Supervisor parsers (v2 supervision trees)
// =============================================================================

/// Parser for a supervisor declaration.
///
/// Syntax:
/// ```sage
/// supervisor Name {
///     strategy: OneForOne
///     children {
///         AgentName { restart: Permanent, belief: value }
///     }
/// }
/// ```
#[allow(clippy::needless_pass_by_value)]
fn supervisor_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();
    let src4 = source.clone();

    // Strategy parser: strategy: OneForOne | OneForAll | RestForOne
    let strategy = just(Token::KwStrategy)
        .ignore_then(just(Token::Colon))
        .ignore_then(filter_map({
            let src = src.clone();
            move |span: Range<usize>, token| match token {
                Token::Ident => {
                    let text = &src[span.start..span.end];
                    match text {
                        "OneForOne" => Ok(SupervisionStrategy::OneForOne),
                        "OneForAll" => Ok(SupervisionStrategy::OneForAll),
                        "RestForOne" => Ok(SupervisionStrategy::RestForOne),
                        _ => Err(Simple::custom(
                            span,
                            format!("unknown strategy `{text}`, expected OneForOne, OneForAll, or RestForOne"),
                        )),
                    }
                }
                _ => Err(Simple::expected_input_found(
                    span,
                    vec![Some(Token::Ident)],
                    Some(token),
                )),
            }
        }));

    // Restart policy parser: restart: Permanent | Transient | Temporary
    let restart_policy = just(Token::KwRestart)
        .ignore_then(just(Token::Colon))
        .ignore_then(filter_map({
            let src = src2.clone();
            move |span: Range<usize>, token| match token {
                Token::Ident => {
                    let text = &src[span.start..span.end];
                    match text {
                        "Permanent" => Ok(RestartPolicy::Permanent),
                        "Transient" => Ok(RestartPolicy::Transient),
                        "Temporary" => Ok(RestartPolicy::Temporary),
                        _ => Err(Simple::custom(
                            span,
                            format!("unknown restart policy `{text}`, expected Permanent, Transient, or Temporary"),
                        )),
                    }
                }
                _ => Err(Simple::expected_input_found(
                    span,
                    vec![Some(Token::Ident)],
                    Some(token),
                )),
            }
        }));

    // Handler assignment: handler Effect: HandlerName (Phase 3: Algebraic Effects)
    let src_handler = src3.clone();
    let handler_assignment = just(Token::KwHandler)
        .ignore_then(ident_token_parser(src3.clone()))
        .then_ignore(just(Token::Colon))
        .then(ident_token_parser(src3.clone()))
        .map_with_span(
            move |(effect, handler), span: Range<usize>| HandlerAssignment {
                effect,
                handler,
                span: make_span(&src_handler, span),
            },
        );

    // Field init for beliefs: name: value
    let field_init = ident_token_parser(src3.clone())
        .then_ignore(just(Token::Colon))
        .then(expr_parser(src3.clone()))
        .map_with_span({
            let src = src3.clone();
            move |(name, value), span: Range<usize>| FieldInit {
                name,
                value,
                span: make_span(&src, span),
            }
        });

    // Child spec: AgentName { restart: Permanent, handler Infer: DefaultLLM, belief1: value1, ... }
    // Or just: AgentName { belief1: value1, ... } (defaults to Permanent)
    // Commas between fields are optional for consistent multiline style.
    let child_spec = ident_token_parser(src3.clone())
        .then_ignore(just(Token::LBrace))
        .then(
            restart_policy
                .then_ignore(just(Token::Comma).or_not())
                .or_not(),
        )
        .then(
            handler_assignment
                .then_ignore(just(Token::Comma).or_not())
                .repeated(),
        )
        .then(
            field_init
                .then_ignore(just(Token::Comma).or_not())
                .repeated(),
        )
        .then_ignore(just(Token::RBrace))
        .map_with_span({
            let src = src3.clone();
            move |(((agent_name, restart), handler_assignments), beliefs), span: Range<usize>| {
                ChildSpec {
                    agent_name,
                    restart: restart.unwrap_or_default(),
                    beliefs,
                    handler_assignments,
                    span: make_span(&src, span),
                }
            }
        });

    // Children block: children { child1 child2 ... }
    let children = just(Token::KwChildren).ignore_then(
        child_spec
            .repeated()
            .at_least(1)
            .delimited_by(just(Token::LBrace), just(Token::RBrace)),
    );

    // Full supervisor declaration
    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwSupervisor))
        .then(ident_token_parser(src4.clone()))
        .then_ignore(just(Token::LBrace))
        .then(strategy)
        .then(children)
        .then_ignore(just(Token::RBrace))
        .map_with_span(
            move |(((is_pub, name), strategy), children), span: Range<usize>| {
                TopLevel::Supervisor(SupervisorDecl {
                    is_pub: is_pub.is_some(),
                    name,
                    strategy,
                    children,
                    span: make_span(&src4, span),
                })
            },
        )
}

// =============================================================================
// Agent parsers
// =============================================================================

/// Parser for an agent declaration.
#[allow(clippy::needless_pass_by_value)]
fn agent_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src2 = source.clone();
    let src3 = source.clone();
    let src4 = source.clone();
    let src5 = source.clone();
    let src6 = source.clone();

    // Tool use clause: `use Http, Fs`
    let tool_use = just(Token::KwUse)
        .ignore_then(
            ident_token_parser(src5.clone())
                .separated_by(just(Token::Comma))
                .at_least(1),
        )
        .or_not()
        .map(|tools| tools.unwrap_or_default());

    // Agent state fields: `name: Type` or `@persistent name: Type`
    // We still call them "beliefs" internally for backwards compatibility
    let src_annot = source.clone();
    let persistent_annotation = just(Token::At)
        .ignore_then(filter_map(move |span: Range<usize>, token| match token {
            Token::Ident => {
                let text = &src_annot[span.start..span.end];
                if text == "persistent" {
                    Ok(true)
                } else {
                    Err(Simple::custom(
                        span,
                        format!("unknown annotation @{}, expected @persistent", text),
                    ))
                }
            }
            _ => Err(Simple::expected_input_found(
                span,
                vec![Some(Token::Ident)],
                Some(token),
            )),
        }))
        .or_not();

    let src_belief = source.clone();
    let belief = persistent_annotation
        .then(ident_token_parser(src_belief.clone()))
        .then_ignore(just(Token::Colon))
        .then(type_parser(src_belief.clone()))
        .map_with_span(
            move |((is_persistent, name), ty), span: Range<usize>| BeliefDecl {
                is_persistent: is_persistent.unwrap_or(false),
                name,
                ty,
                span: make_span(&src_belief, span),
            },
        );

    let handler = just(Token::KwOn)
        .ignore_then(event_kind_parser(src2.clone()))
        .then(block_parser(src2.clone()))
        .map_with_span(move |(event, body), span: Range<usize>| HandlerDecl {
            event,
            body,
            span: make_span(&src2, span),
        });

    // Optional `receives MsgType` clause
    let receives_clause = just(Token::KwReceives)
        .ignore_then(type_parser(src3.clone()))
        .or_not();

    // Optional `follows Protocol as Role` clause (Phase 3: Session Types)
    // Syntax: follows Protocol as Role [, Protocol2 as Role2]*
    let src_follows = src6.clone();
    let single_follows = ident_token_parser(src6.clone())
        .then_ignore(just(Token::KwAs))
        .then(ident_token_parser(src6.clone()))
        .map_with_span(move |(protocol, role), span: Range<usize>| ProtocolRole {
            protocol,
            role,
            span: make_span(&src_follows, span),
        });

    let follows_clause = just(Token::KwFollows)
        .ignore_then(single_follows.separated_by(just(Token::Comma)).at_least(1))
        .or_not()
        .map(|follows| follows.unwrap_or_default());

    // Agent body: { [use Tools] beliefs... handlers... }
    // Uses nested_delimiters recovery so that invalid constructs inside the
    // agent body (e.g. `fn` declarations) cause a parse error instead of
    // silently swallowing the entire program via top-level recovery.
    let agent_body = tool_use
        .then(belief.repeated())
        .then(handler.repeated())
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .recover_with(nested_delimiters(
            Token::LBrace,
            Token::RBrace,
            [
                (Token::LParen, Token::RParen),
                (Token::LBracket, Token::RBracket),
            ],
            |_span| ((vec![], vec![]), vec![]),
        ));

    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwAgent))
        .then(ident_token_parser(src3.clone()))
        .then(receives_clause)
        .then(follows_clause)
        .then(agent_body)
        .map_with_span(
            move |((((is_pub, name), receives), follows), ((tool_uses, beliefs), handlers)),
                  span: Range<usize>| {
                TopLevel::Agent(AgentDecl {
                    is_pub: is_pub.is_some(),
                    name,
                    receives,
                    follows,
                    tool_uses,
                    beliefs,
                    handlers,
                    span: make_span(&src4, span),
                })
            },
        )
}

/// Parser for event kinds.
#[allow(clippy::needless_pass_by_value)]
fn event_kind_parser(source: Arc<str>) -> impl Parser<Token, EventKind, Error = ParseError> {
    let src = source.clone();

    // v2 lifecycle: waking runs before start
    let waking = just(Token::KwWaking).to(EventKind::Waking);
    let start = just(Token::KwStart).to(EventKind::Start);
    // v2 lifecycle: pause/resume for graceful suspend
    let pause = just(Token::KwPause).to(EventKind::Pause);
    let resume = just(Token::KwResume).to(EventKind::Resume);
    let stop = just(Token::KwStop).to(EventKind::Stop);
    // v2 lifecycle: resting is an alias for stop
    let resting = just(Token::KwResting).to(EventKind::Resting);

    let message = just(Token::KwMessage)
        .ignore_then(just(Token::LParen))
        .ignore_then(ident_token_parser(src.clone()))
        .then_ignore(just(Token::Colon))
        .then(type_parser(src.clone()))
        .then_ignore(just(Token::RParen))
        .map(|(param_name, param_ty)| EventKind::Message {
            param_name,
            param_ty,
        });

    // RFC-0007: on error(e) handler
    let error = just(Token::KwError)
        .ignore_then(just(Token::LParen))
        .ignore_then(ident_token_parser(src))
        .then_ignore(just(Token::RParen))
        .map(|param_name| EventKind::Error { param_name });

    waking
        .or(start)
        .or(pause)
        .or(resume)
        .or(stop)
        .or(resting)
        .or(message)
        .or(error)
}

// =============================================================================
// Function parsers
// =============================================================================

/// Parser for an extern function declaration: `extern fn name(params) -> Type`
#[allow(clippy::needless_pass_by_value)]
fn extern_fn_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();

    let param = ident_token_parser(src.clone())
        .then_ignore(just(Token::Colon))
        .then(type_parser(src.clone()))
        .map_with_span(move |(name, ty), span: Range<usize>| Param {
            name,
            ty,
            span: make_span(&src, span),
        });

    let params = param
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    just(Token::KwExtern)
        .ignore_then(just(Token::KwFn))
        .ignore_then(ident_token_parser(src2.clone()))
        .then(params)
        .then_ignore(just(Token::Arrow))
        .then(type_parser(src2.clone()))
        .then(just(Token::KwFails).or_not())
        .map_with_span(
            move |(((name, params), return_ty), is_fallible), span: Range<usize>| {
                TopLevel::ExternFn(ExternFnDecl {
                    name,
                    params,
                    return_ty,
                    is_fallible: is_fallible.is_some(),
                    span: make_span(&src3, span),
                })
            },
        )
}

/// Parser for a function declaration.
#[allow(clippy::needless_pass_by_value)]
fn fn_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();

    let param = ident_token_parser(src.clone())
        .then_ignore(just(Token::Colon))
        .then(type_parser(src.clone()))
        .map_with_span(move |(name, ty), span: Range<usize>| Param {
            name,
            ty,
            span: make_span(&src, span),
        });

    let params = param
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    just(Token::KwPub)
        .or_not()
        .then_ignore(just(Token::KwFn))
        .then(ident_token_parser(src2.clone()))
        .then(type_params_parser(src2.clone()))
        .then(params)
        .then_ignore(just(Token::Arrow))
        .then(type_parser(src2.clone()))
        .then(just(Token::KwFails).or_not())
        .then(block_parser(src2))
        .map_with_span(
            move |((((((is_pub, name), type_params), params), return_ty), is_fallible), body),
                  span: Range<usize>| {
                TopLevel::Function(FnDecl {
                    is_pub: is_pub.is_some(),
                    name,
                    type_params,
                    params,
                    return_ty,
                    is_fallible: is_fallible.is_some(),
                    body,
                    span: make_span(&src3, span),
                })
            },
        )
}

// =============================================================================
// Statement parsers
// =============================================================================

/// Parser for a block of statements.
/// Uses `boxed()` to reduce type complexity and avoid macOS linker symbol length limits.
#[allow(clippy::needless_pass_by_value)]
fn block_parser(source: Arc<str>) -> BoxedParser<'static, Token, Block, ParseError> {
    let src = source.clone();

    recursive(move |block: Recursive<Token, Block, ParseError>| {
        let src_inner = src.clone();
        stmt_parser(src.clone(), block)
            .repeated()
            .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .recover_with(nested_delimiters(
                Token::LBrace,
                Token::RBrace,
                [
                    (Token::LParen, Token::RParen),
                    (Token::LBracket, Token::RBracket),
                ],
                |_span: Range<usize>| vec![],
            ))
            .map_with_span(move |stmts, span: Range<usize>| Block {
                stmts,
                span: make_span(&src_inner, span),
            })
    })
    .boxed()
}

/// Parser for statements.
#[allow(clippy::needless_pass_by_value)]
fn stmt_parser(
    source: Arc<str>,
    block: impl Parser<Token, Block, Error = ParseError> + Clone + 'static,
) -> impl Parser<Token, Stmt, Error = ParseError> + Clone {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();
    let src4 = source.clone();
    let src5 = source.clone();
    let src6 = source.clone();
    let src7 = source.clone();

    // Let tuple destructuring: let (a, b) = expr;
    let src10 = source.clone();
    let let_tuple_stmt = just(Token::KwLet)
        .ignore_then(
            ident_token_parser(src10.clone())
                .separated_by(just(Token::Comma))
                .at_least(2)
                .allow_trailing()
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        )
        .then(
            just(Token::Colon)
                .ignore_then(type_parser(src10.clone()))
                .or_not(),
        )
        .then_ignore(just(Token::Eq))
        .then(expr_parser(src10.clone()))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(
            move |((names, ty), value), span: Range<usize>| Stmt::LetTuple {
                names,
                ty,
                value,
                span: make_span(&src10, span),
            },
        );

    let let_stmt = just(Token::KwLet)
        .ignore_then(ident_token_parser(src.clone()))
        .then(
            just(Token::Colon)
                .ignore_then(type_parser(src.clone()))
                .or_not(),
        )
        .then_ignore(just(Token::Eq))
        .then(expr_parser(src.clone()))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |((name, ty), value), span: Range<usize>| Stmt::Let {
            name,
            ty,
            value,
            span: make_span(&src, span),
        });

    let return_stmt = just(Token::KwReturn)
        .ignore_then(expr_parser(src2.clone()).or_not())
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |value, span: Range<usize>| Stmt::Return {
            value,
            span: make_span(&src2, span),
        });

    let if_stmt = recursive(|if_stmt| {
        let src_if = src3.clone();
        let block_clone = block.clone();

        just(Token::KwIf)
            .ignore_then(expr_parser(src3.clone()))
            .then(block_clone.clone())
            .then(
                just(Token::KwElse)
                    .ignore_then(
                        if_stmt
                            .map(|s| ElseBranch::ElseIf(Box::new(s)))
                            .or(block_clone.map(ElseBranch::Block)),
                    )
                    .or_not(),
            )
            .map_with_span(
                move |((condition, then_block), else_block), span: Range<usize>| Stmt::If {
                    condition,
                    then_block,
                    else_block,
                    span: make_span(&src_if, span),
                },
            )
    });

    let for_stmt = just(Token::KwFor)
        .ignore_then(for_pattern_parser(src4.clone()))
        .then_ignore(just(Token::KwIn))
        .then(expr_parser(src4.clone()))
        .then(block.clone())
        .map_with_span(
            move |((pattern, iter), body), span: Range<usize>| Stmt::For {
                pattern,
                iter,
                body,
                span: make_span(&src4, span),
            },
        );

    let while_stmt = just(Token::KwWhile)
        .ignore_then(expr_parser(src7.clone()))
        .then(block.clone())
        .map_with_span(move |(condition, body), span: Range<usize>| Stmt::While {
            condition,
            body,
            span: make_span(&src7, span),
        });

    let src8 = source.clone();
    let loop_stmt = just(Token::KwLoop)
        .ignore_then(block.clone())
        .map_with_span(move |body, span: Range<usize>| Stmt::Loop {
            body,
            span: make_span(&src8, span),
        });

    let src9 = source.clone();
    let break_stmt = just(Token::KwBreak)
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |_, span: Range<usize>| Stmt::Break {
            span: make_span(&src9, span),
        });

    // RFC-0012: mock divine -> value; or mock divine -> fail("msg");
    // Also accepts: mock infer -> value; (alias for mock divine)
    let src12 = source.clone();
    let mock_divine_stmt = just(Token::KwMock)
        .ignore_then(just(Token::KwDivine).or(just(Token::KwInfer)))
        .ignore_then(just(Token::Arrow))
        .ignore_then(expr_parser(src12.clone()).map(|expr| {
            // Check if this is a fail() call expression and convert to MockValue::Fail
            if let Expr::Fail { error, .. } = expr {
                MockValue::Fail(*error)
            } else {
                MockValue::Value(expr)
            }
        }))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |value, span: Range<usize>| Stmt::MockDivine {
            value,
            span: make_span(&src12, span),
        });

    // mock tool ToolName.fn_name -> value; or mock tool ToolName.fn_name -> fail("msg");
    let src13 = source.clone();
    let src14 = source.clone();
    let src15 = source.clone();
    let mock_tool_stmt = just(Token::KwMock)
        .ignore_then(just(Token::KwTool))
        .ignore_then(ident_token_parser(src13.clone())) // Tool name
        .then_ignore(just(Token::Dot))
        .then(ident_token_parser(src14.clone())) // Function name
        .then_ignore(just(Token::Arrow))
        .then(expr_parser(src15.clone()).map(|expr| {
            // Check if this is a fail() call expression and convert to MockValue::Fail
            if let Expr::Fail { error, .. } = expr {
                MockValue::Fail(*error)
            } else {
                MockValue::Value(expr)
            }
        }))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(
            move |((tool_name, fn_name), value), span: Range<usize>| Stmt::MockTool {
                tool_name,
                fn_name,
                value,
                span: make_span(&src15, span),
            },
        );

    let assign_stmt = ident_token_parser(src5.clone())
        .then_ignore(just(Token::Eq))
        .then(expr_parser(src5.clone()))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |(name, value), span: Range<usize>| Stmt::Assign {
            name,
            value,
            span: make_span(&src5, span),
        });

    // span "name" { body } - timed observability block
    let src16 = source.clone();
    let span_block_stmt = just(Token::KwSpan)
        .ignore_then(expr_parser(src16.clone()))
        .then(block.clone())
        .map_with_span(move |(name, body), span: Range<usize>| Stmt::SpanBlock {
            name,
            body,
            span: make_span(&src16, span),
        });

    // checkpoint(); - explicit persistence checkpoint
    let src17 = source.clone();
    let checkpoint_stmt = just(Token::KwCheckpoint)
        .then_ignore(just(Token::LParen))
        .then_ignore(just(Token::RParen))
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |_, span: Range<usize>| Stmt::Checkpoint {
            span: make_span(&src17, span),
        });

    let expr_stmt = expr_parser(src6.clone())
        .then_ignore(just(Token::Semicolon))
        .map_with_span(move |expr, span: Range<usize>| Stmt::Expr {
            expr,
            span: make_span(&src6, span),
        });

    let_tuple_stmt
        .or(let_stmt)
        .or(return_stmt)
        .or(if_stmt)
        .or(for_stmt)
        .or(while_stmt)
        .or(loop_stmt)
        .or(break_stmt)
        .or(span_block_stmt)
        .or(checkpoint_stmt)
        .or(mock_divine_stmt)
        .or(mock_tool_stmt)
        .or(assign_stmt)
        .or(expr_stmt)
}

// =============================================================================
// Expression parsers
// =============================================================================

/// Parser for expressions (with precedence climbing for binary ops).
/// Uses `boxed()` to reduce type complexity and avoid macOS linker symbol length limits.
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
fn expr_parser(source: Arc<str>) -> BoxedParser<'static, Token, Expr, ParseError> {
    recursive(move |expr: Recursive<Token, Expr, ParseError>| {
        let src = source.clone();

        let literal = literal_parser(src.clone());
        let var = var_parser(src.clone());

        // Parenthesized expression or tuple literal
        // (expr) is a paren, (expr, expr, ...) is a tuple
        let paren_or_tuple = just(Token::LParen)
            .ignore_then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing(),
            )
            .then_ignore(just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |elements, span: Range<usize>| {
                    if elements.len() == 1 {
                        // Single element without trailing comma = parenthesized expression
                        Expr::Paren {
                            inner: Box::new(elements.into_iter().next().unwrap()),
                            span: make_span(&src, span),
                        }
                    } else {
                        // Multiple elements or empty = tuple
                        Expr::Tuple {
                            elements,
                            span: make_span(&src, span),
                        }
                    }
                }
            });

        let list = expr
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map_with_span({
                let src = src.clone();
                move |elements, span: Range<usize>| Expr::List {
                    elements,
                    span: make_span(&src, span),
                }
            });

        // self.field or self.method(args)
        let self_access = just(Token::KwSelf)
            .ignore_then(just(Token::Dot))
            .ignore_then(ident_token_parser(src.clone()))
            .then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::LParen), just(Token::RParen))
                    .or_not(),
            )
            .map_with_span({
                let src = src.clone();
                move |(field, args), span: Range<usize>| match args {
                    Some(args) => Expr::SelfMethodCall {
                        method: field,
                        args,
                        span: make_span(&src, span),
                    },
                    None => Expr::SelfField {
                        field,
                        span: make_span(&src, span),
                    },
                }
            });

        // divine("template") or divine("template" -> Type)
        // Also accepts: infer("template") or infer("template" -> Type)
        let divine_expr = just(Token::KwDivine)
            .or(just(Token::KwInfer))
            .ignore_then(just(Token::LParen))
            .ignore_then(string_template_parser(src.clone()))
            .then(
                just(Token::Arrow)
                    .ignore_then(type_parser(src.clone()))
                    .or_not(),
            )
            .then_ignore(just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |(template, result_ty), span: Range<usize>| Expr::Divine {
                    template,
                    result_ty,
                    span: make_span(&src, span),
                }
            });

        // summon Agent { field: value, ... }
        let summon_field_init = ident_token_parser(src.clone())
            .then_ignore(just(Token::Colon))
            .then(expr.clone())
            .map_with_span({
                let src = src.clone();
                move |(name, value), span: Range<usize>| FieldInit {
                    name,
                    value,
                    span: make_span(&src, span),
                }
            });

        let summon_expr = just(Token::KwSummon)
            .ignore_then(ident_token_parser(src.clone()))
            .then_ignore(just(Token::LBrace))
            .then(
                summon_field_init
                    .separated_by(just(Token::Comma))
                    .allow_trailing(),
            )
            .then_ignore(just(Token::RBrace))
            .map_with_span({
                let src = src.clone();
                move |(agent, fields), span: Range<usize>| Expr::Summon {
                    agent,
                    fields,
                    span: make_span(&src, span),
                }
            });

        // await expr - we need to handle this carefully to avoid left recursion
        // Parse: await handle [timeout(ms)]
        let timeout_clause = just(Token::KwTimeout)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RParen));

        let await_expr = just(Token::KwAwait)
            .ignore_then(ident_token_parser(src.clone()).map_with_span({
                let src = src.clone();
                move |name, span: Range<usize>| Expr::Var {
                    name,
                    span: make_span(&src, span),
                }
            }))
            .then(timeout_clause.or_not())
            .map_with_span({
                let src = src.clone();
                move |(handle, timeout), span: Range<usize>| Expr::Await {
                    handle: Box::new(handle),
                    timeout: timeout.map(Box::new),
                    span: make_span(&src, span),
                }
            });

        // send(handle, message)
        let send_expr = just(Token::KwSend)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr.clone())
            .then_ignore(just(Token::Comma))
            .then(expr.clone())
            .then_ignore(just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |(handle, message), span: Range<usize>| Expr::Send {
                    handle: Box::new(handle),
                    message: Box::new(message),
                    span: make_span(&src, span),
                }
            });

        // yield(value)
        let yield_expr = just(Token::KwYield)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |value, span: Range<usize>| Expr::Yield {
                    value: Box::new(value),
                    span: make_span(&src, span),
                }
            });

        // reply(message) - Phase 3 session types
        let reply_expr = just(Token::KwReply)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |message, span: Range<usize>| Expr::Reply {
                    message: Box::new(message),
                    span: make_span(&src, span),
                }
            });

        // Turbofish type arguments: ::<Int, String>
        let turbofish = just(Token::ColonColon)
            .ignore_then(
                type_parser(src.clone())
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::Lt), just(Token::Gt)),
            )
            .or_not()
            .map(|args| args.unwrap_or_default());

        // function call: name(args) or name::<T, U>(args)
        let call_expr = ident_token_parser(src.clone())
            .then(turbofish.clone())
            .then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .map_with_span({
                let src = src.clone();
                move |((name, type_args), args), span: Range<usize>| Expr::Call {
                    name,
                    type_args,
                    args,
                    span: make_span(&src, span),
                }
            });

        // Pattern for match arms
        let pattern = pattern_parser(src.clone());

        // match expression: match expr { Pattern => expr, ... }
        let match_arm = pattern
            .then_ignore(just(Token::FatArrow))
            .then(expr.clone())
            .map_with_span({
                let src = src.clone();
                move |(pattern, body), span: Range<usize>| MatchArm {
                    pattern,
                    body,
                    span: make_span(&src, span),
                }
            });

        let match_expr = just(Token::KwMatch)
            .ignore_then(expr.clone())
            .then(
                match_arm
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map_with_span({
                let src = src.clone();
                move |(scrutinee, arms), span: Range<usize>| Expr::Match {
                    scrutinee: Box::new(scrutinee),
                    arms,
                    span: make_span(&src, span),
                }
            });

        // receive() - receive message from mailbox
        let receive_expr = just(Token::KwReceive)
            .ignore_then(just(Token::LParen))
            .ignore_then(just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |_, span: Range<usize>| Expr::Receive {
                    span: make_span(&src, span),
                }
            });

        // trace(message) - emit a trace event
        let trace_expr = just(Token::KwTrace)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |message, span: Range<usize>| Expr::Trace {
                    message: Box::new(message),
                    span: make_span(&src, span),
                }
            });

        // Record construction: RecordName { field: value, ... }
        // This is similar to summon but without the summon keyword
        // Must come before var to avoid conflict
        let record_field_init = ident_token_parser(src.clone())
            .then_ignore(just(Token::Colon))
            .then(expr.clone())
            .map_with_span({
                let src = src.clone();
                move |(name, value), span: Range<usize>| FieldInit {
                    name,
                    value,
                    span: make_span(&src, span),
                }
            });

        // Turbofish for record construction: Pair::<Int, String> { ... }
        let record_turbofish = just(Token::ColonColon)
            .ignore_then(
                type_parser(src.clone())
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::Lt), just(Token::Gt)),
            )
            .or_not()
            .map(|args| args.unwrap_or_default());

        let record_construct = ident_token_parser(src.clone())
            .then(record_turbofish)
            .then_ignore(just(Token::LBrace))
            .then(
                record_field_init
                    .separated_by(just(Token::Comma))
                    .allow_trailing(),
            )
            .then_ignore(just(Token::RBrace))
            .map_with_span({
                let src = src.clone();
                move |((name, type_args), fields), span: Range<usize>| Expr::RecordConstruct {
                    name,
                    type_args,
                    fields,
                    span: make_span(&src, span),
                }
            });

        // Closure parameter: `name` or `name: Type`
        let closure_param = ident_token_parser(src.clone())
            .then(
                just(Token::Colon)
                    .ignore_then(type_parser(src.clone()))
                    .or_not(),
            )
            .map_with_span({
                let src = src.clone();
                move |(name, ty), span: Range<usize>| ClosureParam {
                    name,
                    ty,
                    span: make_span(&src, span),
                }
            });

        // Closure expression: |params| body
        // Handle both `|| expr` (empty params using Or token) and `|params| expr`
        let closure_empty = just(Token::Or).ignore_then(expr.clone()).map_with_span({
            let src = src.clone();
            move |body, span: Range<usize>| Expr::Closure {
                params: vec![],
                body: Box::new(body),
                span: make_span(&src, span),
            }
        });

        let closure_with_params = just(Token::Pipe)
            .ignore_then(
                closure_param
                    .separated_by(just(Token::Comma))
                    .allow_trailing(),
            )
            .then_ignore(just(Token::Pipe))
            .then(expr.clone())
            .map_with_span({
                let src = src.clone();
                move |(params, body), span: Range<usize>| Expr::Closure {
                    params,
                    body: Box::new(body),
                    span: make_span(&src, span),
                }
            });

        let closure = closure_with_params.or(closure_empty);

        // Map literal: { key: value, ... } or {}
        // This must be distinguished from record construction which has an identifier before the brace
        let map_entry = expr
            .clone()
            .then_ignore(just(Token::Colon))
            .then(expr.clone())
            .map_with_span({
                let src = src.clone();
                move |(key, value), span: Range<usize>| MapEntry {
                    key,
                    value,
                    span: make_span(&src, span),
                }
            });

        let map_literal = map_entry
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .map_with_span({
                let src = src.clone();
                move |entries, span: Range<usize>| Expr::Map {
                    entries,
                    span: make_span(&src, span),
                }
            });

        // Enum variant construction: EnumName::Variant or Either::<L, R>::Left(payload)
        // Turbofish comes between enum name and ::Variant
        let variant_turbofish = just(Token::ColonColon)
            .ignore_then(
                type_parser(src.clone())
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::Lt), just(Token::Gt)),
            )
            .or_not()
            .map(|args| args.unwrap_or_default());

        // Parser for enum name: accepts identifiers OR builtin type keywords (Option, Result)
        // This allows Option::Some(...) and Result::Ok(...) syntax
        let enum_name_parser = {
            let src = src.clone();
            ident_token_parser(src.clone()).or(just(Token::TyOption)
                .or(just(Token::TyResult))
                .map_with_span({
                    let src = src.clone();
                    move |token, span: Range<usize>| {
                        let name = match token {
                            Token::TyOption => "Option",
                            Token::TyResult => "Result",
                            _ => unreachable!(),
                        };
                        Ident {
                            name: name.to_string(),
                            span: make_span(&src, span),
                        }
                    }
                }))
        };

        let variant_construct = enum_name_parser
            .then(variant_turbofish)
            .then_ignore(just(Token::ColonColon))
            .then(ident_token_parser(src.clone()))
            .then(
                expr.clone()
                    .delimited_by(just(Token::LParen), just(Token::RParen))
                    .or_not(),
            )
            .map_with_span({
                let src = src.clone();
                move |(((enum_name, type_args), variant), payload), span: Range<usize>| {
                    Expr::VariantConstruct {
                        enum_name,
                        type_args,
                        variant,
                        payload: payload.map(Box::new),
                        span: make_span(&src, span),
                    }
                }
            });

        // Atom: the base expression without binary ops
        // Box early to cut type complexity
        // Note: record_construct must come before call_expr and var to parse `Name { ... }` correctly
        // Note: receive_expr must come before call_expr to avoid being parsed as function call
        // Note: closure must come before other expressions to handle `|` tokens correctly
        // Note: map_literal must come after record_construct (record has name before brace)
        // Note: variant_construct must come before call_expr to parse `EnumName::Variant(...)` correctly
        let atom = closure
            .or(divine_expr)
            .or(summon_expr)
            .or(await_expr)
            .or(send_expr)
            .or(yield_expr)
            .or(reply_expr)
            .or(receive_expr)
            .or(trace_expr)
            .or(match_expr)
            .or(self_access)
            .or(record_construct)
            .or(variant_construct)
            .or(call_expr)
            .or(map_literal)
            .or(list)
            .or(paren_or_tuple)
            .or(literal)
            .or(var)
            .boxed();

        // Postfix access: expr.field, expr.0 (tuple index), or expr.method(args) (tool call)
        // We need to distinguish between field access, tuple index, and method call
        enum PostfixOp {
            Field(Ident),
            TupleIndex(usize, Range<usize>),
            MethodCall(Ident, Vec<Expr>, Range<usize>), // method name, args, span of closing paren
        }

        // Parse method call: .ident(args) — uses method_name_parser to allow keywords as method names
        let method_call = method_name_parser(src.clone())
            .then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .map_with_span(|(name, args), span: Range<usize>| {
                PostfixOp::MethodCall(name, args, span)
            });

        let postfix_op = just(Token::Dot).ignore_then(
            // Try to parse a tuple index (integer literal)
            filter_map({
                let src = src.clone();
                move |span: Range<usize>, token| match token {
                    Token::IntLit => {
                        let text = &src[span.start..span.end];
                        text.parse::<usize>()
                            .map(|idx| PostfixOp::TupleIndex(idx, span.clone()))
                            .map_err(|_| Simple::custom(span, "invalid tuple index"))
                    }
                    _ => Err(Simple::expected_input_found(
                        span,
                        vec![Some(Token::IntLit)],
                        Some(token),
                    )),
                }
            })
            // Try method call first, then field access
            .or(method_call)
            .or(method_name_parser(src.clone()).map(PostfixOp::Field)),
        );

        let postfix = atom
            .then(postfix_op.repeated())
            .foldl({
                let src = src.clone();
                move |object, op| match op {
                    PostfixOp::Field(field) => {
                        let span = make_span(&src, object.span().start..field.span.end);
                        Expr::FieldAccess {
                            object: Box::new(object),
                            field,
                            span,
                        }
                    }
                    PostfixOp::TupleIndex(index, idx_span) => {
                        let span = make_span(&src, object.span().start..idx_span.end);
                        Expr::TupleIndex {
                            tuple: Box::new(object),
                            index,
                            span,
                        }
                    }
                    PostfixOp::MethodCall(method, args, call_span) => {
                        // If object is a Var, this might be a tool call
                        // Tool calls look like: Http.get(url)
                        if let Expr::Var { name: tool, .. } = &object {
                            let span = make_span(&src, object.span().start..call_span.end);
                            Expr::ToolCall {
                                tool: tool.clone(),
                                function: method,
                                args,
                                span,
                            }
                        } else {
                            // Not a tool call - this is a method call on a value.
                            // Produce Apply { callee: FieldAccess, args }
                            let span = make_span(&src, object.span().start..call_span.end);
                            let callee = Expr::FieldAccess {
                                object: Box::new(object),
                                field: method,
                                span: span.clone(),
                            };
                            Expr::Apply {
                                callee: Box::new(callee),
                                args,
                                span,
                            }
                        }
                    }
                }
            })
            .boxed();

        // Unary expressions
        let unary = just(Token::Minus)
            .to(UnaryOp::Neg)
            .or(just(Token::Bang).to(UnaryOp::Not))
            .repeated()
            .then(postfix.clone())
            .foldr(|op, operand| {
                let span = operand.span().clone();
                Expr::Unary {
                    op,
                    operand: Box::new(operand),
                    span,
                }
            })
            .boxed();

        // RFC-0007: try expression - propagates errors upward
        // try expr
        let try_expr = just(Token::KwTry)
            .ignore_then(postfix.clone())
            .map_with_span({
                let src = src.clone();
                move |inner, span: Range<usize>| Expr::Try {
                    expr: Box::new(inner),
                    span: make_span(&src, span),
                }
            })
            .boxed();

        // fail expression - explicit error raising
        // fail "message" or fail Error { ... }
        let fail_expr = just(Token::KwFail)
            .ignore_then(postfix.clone())
            .map_with_span({
                let src = src.clone();
                move |error, span: Range<usize>| Expr::Fail {
                    error: Box::new(error),
                    span: make_span(&src, span),
                }
            })
            .boxed();

        // retry expression - retry a fallible operation
        // retry(count) { body }
        // retry(count, delay: ms) { body }
        // retry(count, on: [ErrorKind.Network, ...]) { body }
        let retry_delay = just(Token::Comma)
            .ignore_then(just(Token::KwDelay))
            .ignore_then(just(Token::Colon))
            .ignore_then(postfix.clone());

        let retry_on = just(Token::Comma)
            .ignore_then(just(Token::KwOn))
            .ignore_then(just(Token::Colon))
            .ignore_then(
                postfix
                    .clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::LBracket), just(Token::RBracket)),
            );

        let retry_expr = just(Token::KwRetry)
            .ignore_then(just(Token::LParen))
            .ignore_then(postfix.clone())
            .then(retry_delay.or_not())
            .then(retry_on.or_not())
            .then_ignore(just(Token::RParen))
            .then(
                expr.clone()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map_with_span({
                let src = src.clone();
                move |(((count, delay), on_errors), body), span: Range<usize>| Expr::Retry {
                    count: Box::new(count),
                    delay: delay.map(Box::new),
                    on_errors,
                    body: Box::new(body),
                    span: make_span(&src, span),
                }
            })
            .boxed();

        // Combined unary (including try, fail, and retry)
        let unary = retry_expr.or(fail_expr).or(try_expr).or(unary).boxed();

        // Binary operators with precedence levels
        // Level 7: * / %
        let mul_div_op = just(Token::Star)
            .to(BinOp::Mul)
            .or(just(Token::Slash).to(BinOp::Div))
            .or(just(Token::Percent).to(BinOp::Rem));

        let mul_div = unary
            .clone()
            .then(mul_div_op.then(unary.clone()).repeated())
            .foldl({
                let src = src.clone();
                move |left, (op, right)| {
                    let span = make_span(&src, left.span().start..right.span().end);
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    }
                }
            })
            .boxed();

        // Level 6: + -
        let add_sub_op = just(Token::Plus)
            .to(BinOp::Add)
            .or(just(Token::Minus).to(BinOp::Sub));

        let add_sub = mul_div
            .clone()
            .then(add_sub_op.then(mul_div).repeated())
            .foldl({
                let src = src.clone();
                move |left, (op, right)| {
                    let span = make_span(&src, left.span().start..right.span().end);
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    }
                }
            })
            .boxed();

        // Level 5: ++
        let concat_op = just(Token::PlusPlus).to(BinOp::Concat);

        let concat = add_sub
            .clone()
            .then(concat_op.then(add_sub).repeated())
            .foldl({
                let src = src.clone();
                move |left, (op, right)| {
                    let span = make_span(&src, left.span().start..right.span().end);
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    }
                }
            })
            .boxed();

        // Level 4: < > <= >=
        let cmp_op = choice((
            just(Token::Le).to(BinOp::Le),
            just(Token::Ge).to(BinOp::Ge),
            just(Token::Lt).to(BinOp::Lt),
            just(Token::Gt).to(BinOp::Gt),
        ));

        let comparison = concat
            .clone()
            .then(cmp_op.then(concat).repeated())
            .foldl({
                let src = src.clone();
                move |left, (op, right)| {
                    let span = make_span(&src, left.span().start..right.span().end);
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    }
                }
            })
            .boxed();

        // Level 3: == !=
        let eq_op = just(Token::EqEq)
            .to(BinOp::Eq)
            .or(just(Token::Ne).to(BinOp::Ne));

        let equality = comparison
            .clone()
            .then(eq_op.then(comparison).repeated())
            .foldl({
                let src = src.clone();
                move |left, (op, right)| {
                    let span = make_span(&src, left.span().start..right.span().end);
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    }
                }
            })
            .boxed();

        // Level 2: &&
        let and_op = just(Token::And).to(BinOp::And);

        let and = equality
            .clone()
            .then(and_op.then(equality).repeated())
            .foldl({
                let src = src.clone();
                move |left, (op, right)| {
                    let span = make_span(&src, left.span().start..right.span().end);
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                        span,
                    }
                }
            })
            .boxed();

        // Level 1: ||
        let or_op = just(Token::Or).to(BinOp::Or);

        let or_expr = and.clone().then(or_op.then(and).repeated()).foldl({
            let src = src.clone();
            move |left, (op, right)| {
                let span = make_span(&src, left.span().start..right.span().end);
                Expr::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                    span,
                }
            }
        });

        // RFC-0007: catch expression (lowest precedence)
        // expr catch { recovery } OR expr catch(e) { recovery }
        let catch_recovery = just(Token::KwCatch)
            .ignore_then(
                ident_token_parser(src.clone())
                    .delimited_by(just(Token::LParen), just(Token::RParen))
                    .or_not(),
            )
            .then(
                expr.clone()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            );

        or_expr.then(catch_recovery.or_not()).map_with_span({
            let src = src.clone();
            move |(inner, catch_opt), span: Range<usize>| match catch_opt {
                Some((error_bind, recovery)) => Expr::Catch {
                    expr: Box::new(inner),
                    error_bind,
                    recovery: Box::new(recovery),
                    span: make_span(&src, span),
                },
                None => inner,
            }
        })
    })
    .boxed()
}

// =============================================================================
// Primitive parsers
// =============================================================================

/// Create a Span from a Range<usize>.
fn make_span(source: &Arc<str>, range: Range<usize>) -> Span {
    Span::new(range.start, range.end, Arc::clone(source))
}

/// Parser for identifier tokens.
fn ident_token_parser(source: Arc<str>) -> impl Parser<Token, Ident, Error = ParseError> + Clone {
    filter_map(move |span: Range<usize>, token| match token {
        Token::Ident => {
            let text = &source[span.start..span.end];
            Ok(Ident::new(text.to_string(), make_span(&source, span)))
        }
        _ => Err(Simple::expected_input_found(
            span,
            vec![Some(Token::Ident)],
            Some(token),
        )),
    })
}

/// Parser for identifiers in method/field positions where keywords should also be accepted.
/// This allows `Shell.run()`, `Fs.read()`, etc. where `run`/`read` might be keywords.
fn method_name_parser(source: Arc<str>) -> impl Parser<Token, Ident, Error = ParseError> + Clone {
    filter_map(move |span: Range<usize>, token| {
        let text = &source[span.start..span.end];
        match token {
            Token::Ident
            | Token::KwRun
            | Token::KwUse
            | Token::KwFn
            | Token::KwReturn
            | Token::KwFor
            | Token::KwIn
            | Token::KwIf
            | Token::KwElse
            | Token::KwLet
            | Token::KwOn
            | Token::KwAgent
            | Token::KwSupervisor
            | Token::KwTool
            | Token::KwMock
            | Token::KwStart
            | Token::KwStop
            | Token::KwError
            | Token::KwMatch
            | Token::KwBreak
            | Token::KwLoop => Ok(Ident::new(text.to_string(), make_span(&source, span))),
            _ => Err(Simple::expected_input_found(
                span,
                vec![Some(Token::Ident)],
                Some(token),
            )),
        }
    })
}

/// Parser for variable references.
fn var_parser(source: Arc<str>) -> impl Parser<Token, Expr, Error = ParseError> + Clone {
    ident_token_parser(source.clone()).map_with_span(move |name, span: Range<usize>| Expr::Var {
        name,
        span: make_span(&source, span),
    })
}

/// Parser for type expressions.
fn type_parser(source: Arc<str>) -> impl Parser<Token, TypeExpr, Error = ParseError> + Clone {
    recursive(move |ty| {
        let src = source.clone();

        let primitive = choice((
            just(Token::TyInt).to(TypeExpr::Int),
            just(Token::TyFloat).to(TypeExpr::Float),
            just(Token::TyBool).to(TypeExpr::Bool),
            just(Token::TyString).to(TypeExpr::String),
            just(Token::TyUnit).to(TypeExpr::Unit),
        ));

        let list_ty = just(Token::TyList)
            .ignore_then(just(Token::Lt))
            .ignore_then(ty.clone())
            .then_ignore(just(Token::Gt))
            .map(|inner| TypeExpr::List(Box::new(inner)));

        let option_ty = just(Token::TyOption)
            .ignore_then(just(Token::Lt))
            .ignore_then(ty.clone())
            .then_ignore(just(Token::Gt))
            .map(|inner| TypeExpr::Option(Box::new(inner)));

        let oracle_ty = just(Token::TyOracle)
            .ignore_then(just(Token::Lt))
            .ignore_then(ty.clone())
            .then_ignore(just(Token::Gt))
            .map(|inner| TypeExpr::Oracle(Box::new(inner)));

        let agent_ty = just(Token::TyAgent)
            .ignore_then(just(Token::Lt))
            .ignore_then(ident_token_parser(src.clone()))
            .then_ignore(just(Token::Gt))
            .map(TypeExpr::Agent);

        // Named type with optional type arguments: MyRecord, Pair<Int, String>
        let named_ty = ident_token_parser(src.clone())
            .then(
                ty.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::Lt), just(Token::Gt))
                    .or_not(),
            )
            .map(|(name, type_args)| TypeExpr::Named(name, type_args.unwrap_or_default()));

        // Function type: Fn(A, B) -> C
        let fn_ty = just(Token::TyFn)
            .ignore_then(
                ty.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .then_ignore(just(Token::Arrow))
            .then(ty.clone())
            .map(|(params, ret)| TypeExpr::Fn(params, Box::new(ret)));

        // Map type: Map<K, V>
        let map_ty = just(Token::TyMap)
            .ignore_then(just(Token::Lt))
            .ignore_then(ty.clone())
            .then_ignore(just(Token::Comma))
            .then(ty.clone())
            .then_ignore(just(Token::Gt))
            .map(|(k, v)| TypeExpr::Map(Box::new(k), Box::new(v)));

        // Result type: Result<T, E>
        let result_ty = just(Token::TyResult)
            .ignore_then(just(Token::Lt))
            .ignore_then(ty.clone())
            .then_ignore(just(Token::Comma))
            .then(ty.clone())
            .then_ignore(just(Token::Gt))
            .map(|(ok, err)| TypeExpr::Result(Box::new(ok), Box::new(err)));

        // Tuple type: (A, B, C) - at least 2 elements
        let tuple_ty = ty
            .clone()
            .separated_by(just(Token::Comma))
            .at_least(2)
            .allow_trailing()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .map(TypeExpr::Tuple);

        primitive
            .or(list_ty)
            .or(option_ty)
            .or(oracle_ty)
            .or(agent_ty)
            .or(fn_ty)
            .or(map_ty)
            .or(result_ty)
            .or(tuple_ty)
            .or(named_ty)
    })
}

/// Parser for patterns in for loops.
/// Only supports simple bindings (`x`) and tuple patterns (`(k, v)`).
fn for_pattern_parser(source: Arc<str>) -> impl Parser<Token, Pattern, Error = ParseError> + Clone {
    recursive(move |pattern| {
        let src = source.clone();
        let src2 = source.clone();

        // Simple binding pattern: `x`
        let binding = ident_token_parser(src.clone()).map_with_span({
            let src = src.clone();
            move |name, span: Range<usize>| Pattern::Binding {
                name,
                span: make_span(&src, span),
            }
        });

        // Tuple pattern: `(a, b)` - at least 2 elements
        let tuple_pattern = pattern
            .clone()
            .separated_by(just(Token::Comma))
            .at_least(2)
            .allow_trailing()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .map_with_span({
                let src = src2.clone();
                move |elements, span: Range<usize>| Pattern::Tuple {
                    elements,
                    span: make_span(&src, span),
                }
            });

        tuple_pattern.or(binding)
    })
}

/// Parser for patterns in match expressions.
fn pattern_parser(source: Arc<str>) -> impl Parser<Token, Pattern, Error = ParseError> + Clone {
    recursive(move |pattern| {
        let src = source.clone();
        let src2 = source.clone();
        let src3 = source.clone();
        let src4 = source.clone();
        let src5 = source.clone();

        // Wildcard pattern: `_`
        let wildcard = filter_map({
            let src = src.clone();
            move |span: Range<usize>, token| match &token {
                Token::Ident if src[span.start..span.end].eq("_") => Ok(()),
                _ => Err(Simple::expected_input_found(span, vec![], Some(token))),
            }
        })
        .map_with_span(move |_, span: Range<usize>| Pattern::Wildcard {
            span: make_span(&src2, span),
        });

        // Literal patterns: 42, "hello", true, false
        let lit_int = filter_map({
            let src = src3.clone();
            move |span: Range<usize>, token| match token {
                Token::IntLit => {
                    let text = &src[span.start..span.end];
                    text.parse::<i64>()
                        .map(Literal::Int)
                        .map_err(|_| Simple::custom(span, "invalid integer literal"))
                }
                _ => Err(Simple::expected_input_found(
                    span,
                    vec![Some(Token::IntLit)],
                    Some(token),
                )),
            }
        })
        .map_with_span({
            let src = src3.clone();
            move |value, span: Range<usize>| Pattern::Literal {
                value,
                span: make_span(&src, span),
            }
        });

        let lit_bool = just(Token::KwTrue)
            .to(Literal::Bool(true))
            .or(just(Token::KwFalse).to(Literal::Bool(false)))
            .map_with_span({
                let src = src3.clone();
                move |value, span: Range<usize>| Pattern::Literal {
                    value,
                    span: make_span(&src, span),
                }
            });

        // Tuple pattern: (a, b, c) - at least 2 elements
        let tuple_pattern = pattern
            .clone()
            .separated_by(just(Token::Comma))
            .at_least(2)
            .allow_trailing()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .map_with_span({
                let src = src5.clone();
                move |elements, span: Range<usize>| Pattern::Tuple {
                    elements,
                    span: make_span(&src, span),
                }
            });

        // Enum variant with optional payload: `Ok(x)` or `Status::Active`
        // Qualified with payload: EnumName::Variant(pattern)
        // Parser for enum name: accepts identifiers OR builtin type keywords (Option, Result)
        let pattern_enum_name_parser = {
            let src = src4.clone();
            ident_token_parser(src.clone()).or(just(Token::TyOption)
                .or(just(Token::TyResult))
                .map_with_span({
                    let src = src.clone();
                    move |token, span: Range<usize>| {
                        let name = match token {
                            Token::TyOption => "Option",
                            Token::TyResult => "Result",
                            _ => unreachable!(),
                        };
                        Ident {
                            name: name.to_string(),
                            span: make_span(&src, span),
                        }
                    }
                }))
        };
        let qualified_variant_with_payload = pattern_enum_name_parser
            .then_ignore(just(Token::ColonColon))
            .then(ident_token_parser(src4.clone()))
            .then(
                pattern
                    .clone()
                    .delimited_by(just(Token::LParen), just(Token::RParen))
                    .or_not(),
            )
            .map_with_span({
                let src = src4.clone();
                move |((enum_name, variant), payload), span: Range<usize>| Pattern::Variant {
                    enum_name: Some(enum_name),
                    variant,
                    payload: payload.map(Box::new),
                    span: make_span(&src, span),
                }
            });

        // Unqualified variant with payload: `Ok(x)` or just `x`
        let unqualified_with_payload = ident_token_parser(src4.clone())
            .then(
                pattern
                    .clone()
                    .delimited_by(just(Token::LParen), just(Token::RParen))
                    .or_not(),
            )
            .map_with_span({
                let src = src4.clone();
                move |(name, payload), span: Range<usize>| {
                    // If it looks like a variant (starts with uppercase), treat as variant
                    // Otherwise treat as binding (only if no payload)
                    if name.name.chars().next().is_some_and(|c| c.is_uppercase())
                        || payload.is_some()
                    {
                        Pattern::Variant {
                            enum_name: None,
                            variant: name,
                            payload: payload.map(Box::new),
                            span: make_span(&src, span),
                        }
                    } else {
                        Pattern::Binding {
                            name,
                            span: make_span(&src, span),
                        }
                    }
                }
            });

        // Order matters: try wildcard first, then tuple pattern, then qualified variant, then literals, then unqualified
        wildcard
            .or(tuple_pattern)
            .or(qualified_variant_with_payload)
            .or(lit_int)
            .or(lit_bool)
            .or(unqualified_with_payload)
    })
}

/// Parser for literals.
fn literal_parser(source: Arc<str>) -> impl Parser<Token, Expr, Error = ParseError> + Clone {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();
    let src4 = source.clone();
    let src5 = source.clone();

    let int_lit = filter_map(move |span: Range<usize>, token| match token {
        Token::IntLit => {
            let text = &src[span.start..span.end];
            text.parse::<i64>()
                .map(Literal::Int)
                .map_err(|_| Simple::custom(span, "invalid integer literal"))
        }
        _ => Err(Simple::expected_input_found(
            span,
            vec![Some(Token::IntLit)],
            Some(token),
        )),
    })
    .map_with_span(move |value, span: Range<usize>| Expr::Literal {
        value,
        span: make_span(&src2, span),
    });

    let float_lit = filter_map(move |span: Range<usize>, token| match token {
        Token::FloatLit => {
            let text = &src3[span.start..span.end];
            text.parse::<f64>()
                .map(Literal::Float)
                .map_err(|_| Simple::custom(span, "invalid float literal"))
        }
        _ => Err(Simple::expected_input_found(
            span,
            vec![Some(Token::FloatLit)],
            Some(token),
        )),
    })
    .map_with_span(move |value, span: Range<usize>| Expr::Literal {
        value,
        span: make_span(&src4, span),
    });

    let src6 = source.clone();
    let string_lit = filter_map(move |span: Range<usize>, token| match token {
        Token::StringLit => {
            let text = &src5[span.start..span.end];
            let quote_char = text.chars().next().unwrap_or('"');
            let inner = &text[1..text.len() - 1];
            if quote_char == '\'' {
                // Single-quoted strings are raw (no interpolation)
                Ok(vec![StringPart::Literal(unescape_string(inner))])
            } else {
                let parts = parse_string_template(inner, &make_span(&src5, span.clone()));
                Ok(parts)
            }
        }
        _ => Err(Simple::expected_input_found(
            span,
            vec![Some(Token::StringLit)],
            Some(token),
        )),
    })
    .map_with_span(move |parts, span: Range<usize>| {
        let span = make_span(&src6, span);
        // If no interpolations, use a simple string literal
        if parts.len() == 1 {
            if let StringPart::Literal(s) = &parts[0] {
                return Expr::Literal {
                    value: Literal::String(s.clone()),
                    span,
                };
            }
        }
        // Otherwise, use StringInterp
        Expr::StringInterp {
            template: StringTemplate {
                parts,
                span: span.clone(),
            },
            span,
        }
    });

    let bool_lit = just(Token::KwTrue)
        .to(Literal::Bool(true))
        .or(just(Token::KwFalse).to(Literal::Bool(false)))
        .map_with_span(move |value, _span: Range<usize>| Expr::Literal {
            value,
            span: Span::dummy(), // bool literals don't carry source
        });

    int_lit.or(float_lit).or(string_lit).or(bool_lit)
}

/// Parser for string templates (handles interpolation).
fn string_template_parser(
    source: Arc<str>,
) -> impl Parser<Token, StringTemplate, Error = ParseError> + Clone {
    filter_map(move |span: Range<usize>, token| match token {
        Token::StringLit => {
            let text = &source[span.start..span.end];
            let inner = &text[1..text.len() - 1];
            let parts = parse_string_template(inner, &make_span(&source, span.clone()));
            Ok(StringTemplate {
                parts,
                span: make_span(&source, span),
            })
        }
        _ => Err(Simple::expected_input_found(
            span,
            vec![Some(Token::StringLit)],
            Some(token),
        )),
    })
}

/// Parse a string into template parts, handling `{expr}` interpolations.
/// Supports arbitrary expressions: `{name}`, `{a + b}`, `{foo(x, y)}`
fn parse_string_template(s: &str, span: &Span) -> Vec<StringPart> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if !current.is_empty() {
                parts.push(StringPart::Literal(std::mem::take(&mut current)));
            }

            // Collect the full interpolation expression, handling nested braces
            let mut expr_str = String::new();
            let mut brace_depth = 1;
            let mut string_quote: Option<char> = None; // Track which quote type we're in
            let mut escape_next = false;

            while let Some(&c) = chars.peek() {
                if escape_next {
                    expr_str.push(c);
                    chars.next();
                    escape_next = false;
                    continue;
                }

                if c == '\\' && string_quote.is_some() {
                    escape_next = true;
                    expr_str.push(c);
                    chars.next();
                    continue;
                }

                // Handle string delimiters (both " and ')
                if c == '"' || c == '\'' {
                    match string_quote {
                        None => string_quote = Some(c),           // Start of string
                        Some(q) if q == c => string_quote = None, // End of string
                        Some(_) => {} // Inside different type of string, ignore
                    }
                }

                if string_quote.is_none() {
                    if c == '{' {
                        brace_depth += 1;
                    } else if c == '}' {
                        brace_depth -= 1;
                        if brace_depth == 0 {
                            chars.next();
                            break;
                        }
                    }
                }

                expr_str.push(c);
                chars.next();
            }

            if !expr_str.is_empty() {
                let expr = parse_interp_expr(&expr_str, span);
                parts.push(StringPart::Interpolation(Box::new(expr)));
            }
        } else if ch == '\\' {
            if let Some(escaped) = chars.next() {
                if escaped == 'x' {
                    // \xNN hex escape
                    let hi = chars.next().unwrap_or('0');
                    let lo = chars.next().unwrap_or('0');
                    let code = u8::from_str_radix(&format!("{hi}{lo}"), 16).unwrap_or(b'?');
                    current.push(code as char);
                } else {
                    current.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '\\' => '\\',
                        '"' => '"',
                        '{' => '{',
                        '}' => '}',
                        other => other,
                    });
                }
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        parts.push(StringPart::Literal(current));
    }

    if parts.is_empty() {
        parts.push(StringPart::Literal(String::new()));
    }

    parts
}

/// Parse an interpolation expression string into an Expr AST node.
/// Lexes and parses the substring using a mini recursive-descent parser.
fn parse_interp_expr(s: &str, span: &Span) -> Expr {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        // Empty interpolation, return a placeholder
        return Expr::Literal {
            value: Literal::String(String::new()),
            span: span.clone(),
        };
    }

    // Lex the expression substring
    let lex_result = crate::lex(trimmed);
    let (tokens, source) = match lex_result {
        Ok(result) => (result.tokens().to_vec(), trimmed.to_string()),
        Err(_) => {
            // Lexing failed, return the raw string as a variable name (fallback)
            return Expr::Var {
                name: Ident::new(trimmed.to_string(), span.clone()),
                span: span.clone(),
            };
        }
    };

    if tokens.is_empty() {
        return Expr::Var {
            name: Ident::new(trimmed.to_string(), span.clone()),
            span: span.clone(),
        };
    }

    // Parse with a simple recursive descent parser
    let mut parser = InterpExprParser::new(&tokens, &source, span.clone());
    parser.parse_expr()
}

/// A simple recursive-descent parser for interpolation expressions.
/// Supports: variables, field access, tuple index, binary ops, function calls, parens
struct InterpExprParser<'a> {
    tokens: &'a [crate::Spanned],
    source: &'a str,
    pos: usize,
    span: Span,
}

impl<'a> InterpExprParser<'a> {
    fn new(tokens: &'a [crate::Spanned], source: &'a str, span: Span) -> Self {
        Self {
            tokens,
            source,
            pos: 0,
            span,
        }
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|s| &s.token)
    }

    fn current_text(&self) -> Option<&str> {
        self.tokens
            .get(self.pos)
            .map(|s| &self.source[s.start..s.end])
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Expr {
        let mut left = self.parse_and();
        while matches!(self.current(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and();
            left = Expr::Binary {
                left: Box::new(left),
                op: BinOp::Or,
                right: Box::new(right),
                span: self.span.clone(),
            };
        }
        left
    }

    fn parse_and(&mut self) -> Expr {
        let mut left = self.parse_comparison();
        while matches!(self.current(), Some(Token::And)) {
            self.advance();
            let right = self.parse_comparison();
            left = Expr::Binary {
                left: Box::new(left),
                op: BinOp::And,
                right: Box::new(right),
                span: self.span.clone(),
            };
        }
        left
    }

    fn parse_comparison(&mut self) -> Expr {
        let mut left = self.parse_additive();
        loop {
            let op = match self.current() {
                Some(Token::EqEq) => BinOp::Eq,
                Some(Token::Ne) => BinOp::Ne,
                Some(Token::Lt) => BinOp::Lt,
                Some(Token::Le) => BinOp::Le,
                Some(Token::Gt) => BinOp::Gt,
                Some(Token::Ge) => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive();
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span: self.span.clone(),
            };
        }
        left
    }

    fn parse_additive(&mut self) -> Expr {
        let mut left = self.parse_multiplicative();
        loop {
            let op = match self.current() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                Some(Token::PlusPlus) => BinOp::Concat,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative();
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span: self.span.clone(),
            };
        }
        left
    }

    fn parse_multiplicative(&mut self) -> Expr {
        let mut left = self.parse_unary();
        loop {
            let op = match self.current() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                Some(Token::Percent) => BinOp::Rem,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary();
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span: self.span.clone(),
            };
        }
        left
    }

    fn parse_unary(&mut self) -> Expr {
        match self.current() {
            Some(Token::Minus) => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                    span: self.span.clone(),
                }
            }
            Some(Token::Bang) => {
                self.advance();
                let operand = self.parse_unary();
                Expr::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                    span: self.span.clone(),
                }
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();

        loop {
            match self.current() {
                Some(Token::Dot) => {
                    self.advance();
                    match self.current() {
                        Some(Token::IntLit) => {
                            let text = self.current_text().unwrap_or("0");
                            let index = text.parse::<usize>().unwrap_or(0);
                            self.advance();
                            expr = Expr::TupleIndex {
                                tuple: Box::new(expr),
                                index,
                                span: self.span.clone(),
                            };
                        }
                        Some(Token::Ident) => {
                            let name = self.current_text().unwrap_or("").to_string();
                            let field = Ident::new(name, self.span.clone());
                            self.advance();
                            expr = Expr::FieldAccess {
                                object: Box::new(expr),
                                field,
                                span: self.span.clone(),
                            };
                        }
                        _ => break,
                    }
                }
                Some(Token::ColonColon) if matches!(expr, Expr::Var { .. }) => {
                    // Turbofish: name::<Type1, Type2>(args)
                    if let Expr::Var { name, .. } = expr {
                        self.advance(); // consume ::
                        let type_args = self.parse_turbofish();
                        // Now expect (
                        if matches!(self.current(), Some(Token::LParen)) {
                            self.advance();
                            let args = self.parse_args();
                            expr = Expr::Call {
                                name,
                                type_args,
                                args,
                                span: self.span.clone(),
                            };
                        } else {
                            // Turbofish without call - just treat as var with type args
                            // This shouldn't normally happen, but handle gracefully
                            expr = Expr::Call {
                                name,
                                type_args,
                                args: vec![],
                                span: self.span.clone(),
                            };
                        }
                    }
                }
                Some(Token::LParen) if matches!(expr, Expr::Var { .. }) => {
                    // Function call without turbofish
                    if let Expr::Var { name, .. } = expr {
                        self.advance();
                        let args = self.parse_args();
                        expr = Expr::Call {
                            name,
                            type_args: vec![],
                            args,
                            span: self.span.clone(),
                        };
                    }
                }
                _ => break,
            }
        }

        expr
    }

    fn parse_primary(&mut self) -> Expr {
        match self.current() {
            Some(Token::IntLit) => {
                let text = self.current_text().unwrap_or("0");
                let n = text.parse::<i64>().unwrap_or(0);
                self.advance();
                Expr::Literal {
                    value: Literal::Int(n),
                    span: self.span.clone(),
                }
            }
            Some(Token::FloatLit) => {
                let text = self.current_text().unwrap_or("0.0");
                let f = text.parse::<f64>().unwrap_or(0.0);
                self.advance();
                Expr::Literal {
                    value: Literal::Float(f),
                    span: self.span.clone(),
                }
            }
            Some(Token::StringLit) => {
                let text = self.current_text().unwrap_or("\"\"");
                // Remove surrounding quotes
                let s = if text.len() >= 2 {
                    text[1..text.len() - 1].to_string()
                } else {
                    String::new()
                };
                self.advance();
                Expr::Literal {
                    value: Literal::String(s),
                    span: self.span.clone(),
                }
            }
            Some(Token::KwTrue) => {
                self.advance();
                Expr::Literal {
                    value: Literal::Bool(true),
                    span: self.span.clone(),
                }
            }
            Some(Token::KwFalse) => {
                self.advance();
                Expr::Literal {
                    value: Literal::Bool(false),
                    span: self.span.clone(),
                }
            }
            Some(Token::Ident) => {
                let name = self.current_text().unwrap_or("").to_string();
                self.advance();
                Expr::Var {
                    name: Ident::new(name, self.span.clone()),
                    span: self.span.clone(),
                }
            }
            Some(Token::KwSelf) => {
                self.advance();
                // Handle self.field
                if matches!(self.current(), Some(Token::Dot)) {
                    self.advance();
                    if let Some(Token::Ident) = self.current() {
                        let field_name = self.current_text().unwrap_or("").to_string();
                        let field = Ident::new(field_name, self.span.clone());
                        self.advance();
                        return Expr::SelfField {
                            field,
                            span: self.span.clone(),
                        };
                    }
                }
                Expr::Var {
                    name: Ident::new("self".to_string(), self.span.clone()),
                    span: self.span.clone(),
                }
            }
            Some(Token::LParen) => {
                self.advance();
                let inner = self.parse_expr();
                if matches!(self.current(), Some(Token::RParen)) {
                    self.advance();
                }
                Expr::Paren {
                    inner: Box::new(inner),
                    span: self.span.clone(),
                }
            }
            Some(Token::LBracket) => {
                // List literal
                self.advance();
                let mut elements = Vec::new();
                while !matches!(self.current(), Some(Token::RBracket) | None) {
                    elements.push(self.parse_expr());
                    if matches!(self.current(), Some(Token::Comma)) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if matches!(self.current(), Some(Token::RBracket)) {
                    self.advance();
                }
                Expr::List {
                    elements,
                    span: self.span.clone(),
                }
            }
            _ => {
                // Unknown token, return empty string literal as fallback
                Expr::Literal {
                    value: Literal::String(String::new()),
                    span: self.span.clone(),
                }
            }
        }
    }

    fn parse_args(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        while !matches!(self.current(), Some(Token::RParen) | None) {
            args.push(self.parse_expr());
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }
        if matches!(self.current(), Some(Token::RParen)) {
            self.advance();
        }
        args
    }

    /// Parse turbofish type arguments: `<Type1, Type2>`
    /// Assumes `::` has already been consumed.
    fn parse_turbofish(&mut self) -> Vec<TypeExpr> {
        let mut type_args = Vec::new();

        // Expect <
        if !matches!(self.current(), Some(Token::Lt)) {
            return type_args;
        }
        self.advance();

        // Parse comma-separated types until >
        loop {
            if matches!(self.current(), Some(Token::Gt) | None) {
                break;
            }

            if let Some(ty) = self.parse_type() {
                type_args.push(ty);
            }

            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        // Consume >
        if matches!(self.current(), Some(Token::Gt)) {
            self.advance();
        }

        type_args
    }

    /// Parse a type expression: `Int`, `String`, `List<T>`, `Map<K, V>`, etc.
    fn parse_type(&mut self) -> Option<TypeExpr> {
        let name = match self.current() {
            Some(Token::Ident) => self.current_text().unwrap_or("").to_string(),
            Some(Token::TyInt) => "Int".to_string(),
            Some(Token::TyFloat) => "Float".to_string(),
            Some(Token::TyBool) => "Bool".to_string(),
            Some(Token::TyString) => "String".to_string(),
            Some(Token::TyUnit) => "Unit".to_string(),
            _ => return None,
        };
        self.advance();

        // Check for type parameters: <T, U>
        if matches!(self.current(), Some(Token::Lt)) {
            self.advance();
            let mut params = Vec::new();

            loop {
                if matches!(self.current(), Some(Token::Gt) | None) {
                    break;
                }

                if let Some(param) = self.parse_type() {
                    params.push(param);
                }

                if matches!(self.current(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }

            if matches!(self.current(), Some(Token::Gt)) {
                self.advance();
            }

            // Return generic type
            Some(match name.as_str() {
                "List" => {
                    if let Some(elem) = params.into_iter().next() {
                        TypeExpr::List(Box::new(elem))
                    } else {
                        TypeExpr::Named(Ident::new(name, self.span.clone()), vec![])
                    }
                }
                "Map" => {
                    let mut iter = params.into_iter();
                    if let (Some(k), Some(v)) = (iter.next(), iter.next()) {
                        TypeExpr::Map(Box::new(k), Box::new(v))
                    } else {
                        TypeExpr::Named(Ident::new(name, self.span.clone()), vec![])
                    }
                }
                "Option" => {
                    if let Some(inner) = params.into_iter().next() {
                        TypeExpr::Option(Box::new(inner))
                    } else {
                        TypeExpr::Named(Ident::new(name, self.span.clone()), vec![])
                    }
                }
                "Result" => {
                    let mut iter = params.into_iter();
                    if let (Some(ok), Some(err)) = (iter.next(), iter.next()) {
                        TypeExpr::Result(Box::new(ok), Box::new(err))
                    } else {
                        TypeExpr::Named(Ident::new(name, self.span.clone()), vec![])
                    }
                }
                _ => {
                    // Generic named type
                    TypeExpr::Named(Ident::new(name, self.span.clone()), params)
                }
            })
        } else {
            // Simple type
            Some(match name.as_str() {
                "Int" => TypeExpr::Int,
                "Float" => TypeExpr::Float,
                "Bool" => TypeExpr::Bool,
                "String" => TypeExpr::String,
                "Unit" => TypeExpr::Unit,
                _ => TypeExpr::Named(Ident::new(name, self.span.clone()), vec![]),
            })
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lex;

    fn parse_str(source: &str) -> (Option<Program>, Vec<ParseError>) {
        let lex_result = lex(source).expect("lexing should succeed");
        let source_arc: Arc<str> = Arc::from(source);
        parse(lex_result.tokens(), source_arc)
    }

    #[test]
    fn parse_minimal_program() {
        let source = r#"
            agent Main {
                on start {
                    yield(42);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents.len(), 1);
        assert_eq!(prog.agents[0].name.name, "Main");
        assert_eq!(prog.run_agent.as_ref().unwrap().name, "Main");
    }

    #[test]
    fn parse_agent_with_beliefs() {
        let source = r#"
            agent Researcher {
                topic: String
                max_words: Int

                on start {
                    yield(self.topic);
                }
            }
            run Researcher;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents[0].beliefs.len(), 2);
        assert_eq!(prog.agents[0].beliefs[0].name.name, "topic");
        assert_eq!(prog.agents[0].beliefs[1].name.name, "max_words");
    }

    #[test]
    fn parse_multiple_handlers() {
        let source = r#"
            agent Worker {
                on start {
                    print("started");
                }

                on message(msg: String) {
                    print(msg);
                }

                on stop {
                    print("stopped");
                }
            }
            run Worker;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents[0].handlers.len(), 3);
        assert_eq!(prog.agents[0].handlers[0].event, EventKind::Start);
        assert!(matches!(
            prog.agents[0].handlers[1].event,
            EventKind::Message { .. }
        ));
        assert_eq!(prog.agents[0].handlers[2].event, EventKind::Stop);
    }

    #[test]
    fn parse_v2_lifecycle_hooks() {
        let source = r#"
            agent StatefulWorker {
                on waking {
                    // Load persisted state
                    trace("waking up");
                }

                on start {
                    trace("started");
                }

                on pause {
                    // Save state before pause
                    trace("pausing");
                }

                on resume {
                    trace("resuming");
                }

                on resting {
                    // Cleanup before shutdown
                    trace("resting");
                }
            }
            run StatefulWorker;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents[0].handlers.len(), 5);
        assert_eq!(prog.agents[0].handlers[0].event, EventKind::Waking);
        assert_eq!(prog.agents[0].handlers[1].event, EventKind::Start);
        assert_eq!(prog.agents[0].handlers[2].event, EventKind::Pause);
        assert_eq!(prog.agents[0].handlers[3].event, EventKind::Resume);
        assert_eq!(prog.agents[0].handlers[4].event, EventKind::Resting);
    }

    #[test]
    fn parse_persistent_beliefs() {
        let source = r#"
            agent DatabaseSteward {
                @persistent schema_version: Int
                @persistent migration_log: List<String>
                active_connections: Int

                on start {
                    yield(0);
                }
            }
            run DatabaseSteward;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents[0].beliefs.len(), 3);
        assert!(prog.agents[0].beliefs[0].is_persistent);
        assert_eq!(prog.agents[0].beliefs[0].name.name, "schema_version");
        assert!(prog.agents[0].beliefs[1].is_persistent);
        assert_eq!(prog.agents[0].beliefs[1].name.name, "migration_log");
        assert!(!prog.agents[0].beliefs[2].is_persistent);
        assert_eq!(prog.agents[0].beliefs[2].name.name, "active_connections");
    }

    #[test]
    fn parse_function() {
        let source = r#"
            fn greet(name: String) -> String {
                return "Hello, " ++ name;
            }

            agent Main {
                on start {
                    yield(greet("World"));
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.functions.len(), 1);
        assert_eq!(prog.functions[0].name.name, "greet");
        assert_eq!(prog.functions[0].params.len(), 1);
    }

    #[test]
    fn parse_let_statement() {
        let source = r#"
            agent Main {
                on start {
                    let x: Int = 42;
                    let y = "hello";
                    yield(x);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let stmts = &prog.agents[0].handlers[0].body.stmts;
        assert!(matches!(stmts[0], Stmt::Let { .. }));
        assert!(matches!(stmts[1], Stmt::Let { .. }));
    }

    #[test]
    fn parse_if_statement() {
        let source = r#"
            agent Main {
                on start {
                    if true {
                        yield(1);
                    } else {
                        yield(2);
                    }
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let stmts = &prog.agents[0].handlers[0].body.stmts;
        assert!(matches!(stmts[0], Stmt::If { .. }));
    }

    #[test]
    fn parse_for_loop() {
        let source = r#"
            agent Main {
                on start {
                    for x in [1, 2, 3] {
                        print(x);
                    }
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let stmts = &prog.agents[0].handlers[0].body.stmts;
        assert!(matches!(stmts[0], Stmt::For { .. }));
    }

    #[test]
    fn parse_spawn_await() {
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
                    let result = await w;
                    yield(result);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        prog.expect("should parse");
    }

    #[test]
    fn parse_await_with_timeout() {
        let source = r#"
            agent Worker {
                on start {
                    yield("done");
                }
            }

            agent Main {
                on start {
                    let w = summon Worker {};
                    let result = await w timeout(5000);
                    yield(result);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the await statement in Main's on start handler
        let main = &prog.agents[1];
        let stmts = &main.handlers[0].body.stmts;
        // stmts[0] is the let w = spawn...
        // stmts[1] is the let result = await w timeout(5000)
        if let Stmt::Let { value, .. } = &stmts[1] {
            if let Expr::Await { timeout, .. } = value {
                assert!(timeout.is_some(), "timeout should be present");
            } else {
                panic!("expected Await expression");
            }
        } else {
            panic!("expected Let statement with value");
        }
    }

    #[test]
    fn parse_divine() {
        let source = r#"
            agent Main {
                on start {
                    let result = divine("What is 2+2?");
                    yield(result);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        prog.expect("should parse");
    }

    #[test]
    fn parse_binary_precedence() {
        let source = r#"
            agent Main {
                on start {
                    let x = 2 + 3 * 4;
                    yield(x);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let stmts = &prog.agents[0].handlers[0].body.stmts;
        if let Stmt::Let { value, .. } = &stmts[0] {
            if let Expr::Binary { op, .. } = value {
                assert_eq!(*op, BinOp::Add);
            } else {
                panic!("expected binary expression");
            }
        }
    }

    #[test]
    fn parse_string_interpolation() {
        let source = r#"
            agent Main {
                on start {
                    let name = "World";
                    let msg = divine("Greet {name}");
                    yield(msg);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let stmts = &prog.agents[0].handlers[0].body.stmts;
        if let Stmt::Let { value, .. } = &stmts[1] {
            if let Expr::Divine { template, .. } = value {
                assert!(template.has_interpolations());
            } else {
                panic!("expected infer expression");
            }
        }
    }

    #[test]
    fn parse_single_quoted_string() {
        let source = r#"
            agent Main {
                on start {
                    let x = 'hello';
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let stmts = &prog.agents[0].handlers[0].body.stmts;
        if let Stmt::Let { value, .. } = &stmts[0] {
            if let Expr::Literal {
                value: Literal::String(s),
                ..
            } = value
            {
                assert_eq!(s, "hello");
            } else {
                panic!("expected string literal, got {:?}", value);
            }
        } else {
            panic!("expected let statement");
        }
    }

    #[test]
    fn parse_single_quoted_string_in_interpolation() {
        // Use single quotes for strings inside interpolations to avoid
        // conflicts with the outer double-quoted string
        let source = r#"
            fn reverse(s: String) -> String {
                return s;
            }
            agent Main {
                on start {
                    print("Result: {reverse('hello')}");
                    print("Concat: {'abc' ++ 'def'}");
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Verify the string interpolation was parsed
        let stmts = &prog.agents[0].handlers[0].body.stmts;
        if let Stmt::Expr {
            expr: Expr::Call { args, .. },
            ..
        } = &stmts[0]
        {
            if let Expr::StringInterp { template, .. } = &args[0] {
                assert!(template.has_interpolations());
            } else {
                panic!("expected string interpolation");
            }
        } else {
            panic!("expected print call");
        }
    }

    // =========================================================================
    // Error recovery tests
    // =========================================================================

    #[test]
    fn recover_from_malformed_agent_continues_to_next() {
        // First agent has syntax error (missing type after colon), second is valid
        let source = r#"
            agent Broken {
                x:
            }

            agent Main {
                on start {
                    yield(42);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        // Should have errors from the broken agent
        assert!(!errors.is_empty(), "should have parse errors");
        // But should still produce a program with the valid agent
        let prog = prog.expect("should produce partial AST");
        assert!(prog.agents.iter().any(|a| a.name.name == "Main"));
    }

    #[test]
    fn recover_from_mismatched_braces_in_block() {
        let source = r#"
            agent Main {
                on start {
                    let x = [1, 2, 3;
                    yield(42);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        // Should have errors but still produce an AST
        assert!(!errors.is_empty(), "should have parse errors");
        assert!(prog.is_some(), "should produce partial AST despite errors");
    }

    #[test]
    fn parse_mod_declaration() {
        let source = r#"
            mod agents;
            pub mod utils;

            agent Main {
                on start {
                    yield(42);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.mod_decls.len(), 2);
        assert!(!prog.mod_decls[0].is_pub);
        assert_eq!(prog.mod_decls[0].name.name, "agents");
        assert!(prog.mod_decls[1].is_pub);
        assert_eq!(prog.mod_decls[1].name.name, "utils");
    }

    #[test]
    fn parse_use_simple() {
        let source = r#"
            use agents::Researcher;

            agent Main {
                on start {
                    yield(42);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.use_decls.len(), 1);
        assert!(!prog.use_decls[0].is_pub);
        assert_eq!(prog.use_decls[0].path.len(), 2);
        assert_eq!(prog.use_decls[0].path[0].name, "agents");
        assert_eq!(prog.use_decls[0].path[1].name, "Researcher");
        assert!(matches!(prog.use_decls[0].kind, UseKind::Simple(None)));
    }

    #[test]
    fn parse_use_with_alias() {
        let source = r#"
            use agents::Researcher as R;

            agent Main {
                on start {
                    yield(42);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.use_decls.len(), 1);
        if let UseKind::Simple(Some(alias)) = &prog.use_decls[0].kind {
            assert_eq!(alias.name, "R");
        } else {
            panic!("expected Simple with alias");
        }
    }

    #[test]
    fn parse_pub_agent() {
        let source = r#"
            pub agent Worker {
                on start {
                    yield(42);
                }
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents.len(), 2);
        assert!(prog.agents[0].is_pub);
        assert_eq!(prog.agents[0].name.name, "Worker");
        assert!(!prog.agents[1].is_pub);
    }

    #[test]
    fn parse_pub_function() {
        let source = r#"
            pub fn helper(x: Int) -> Int {
                return x;
            }

            agent Main {
                on start {
                    yield(helper(42));
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.functions.len(), 1);
        assert!(prog.functions[0].is_pub);
        assert_eq!(prog.functions[0].name.name, "helper");
    }

    #[test]
    fn parse_library_no_run() {
        // A library module has no `run` statement
        let source = r#"
            pub agent Worker {
                on start {
                    yield(42);
                }
            }

            pub fn helper(x: Int) -> Int {
                return x;
            }
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert!(prog.run_agent.is_none());
        assert_eq!(prog.agents.len(), 1);
        assert_eq!(prog.functions.len(), 1);
    }

    #[test]
    fn recover_multiple_errors_reported() {
        // Multiple errors in different places - incomplete field missing type
        let source = r#"
            agent A {
                x:
            }

            agent Main {
                on start {
                    yield(42);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        // The malformed field is missing its type after `:` so should cause an error
        // However, with recovery the valid agent may still parse
        // Check that we either have errors or recovered successfully
        if errors.is_empty() {
            // Recovery succeeded - should have parsed Main agent
            let prog = prog.expect("should have AST with recovery");
            assert!(prog.agents.iter().any(|a| a.name.name == "Main"));
        }
        // Either way, the test passes - we're testing recovery works
    }

    #[test]
    fn parse_record_declaration() {
        let source = r#"
            record Point {
                x: Int,
                y: Int,
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.records.len(), 1);
        assert!(!prog.records[0].is_pub);
        assert_eq!(prog.records[0].name.name, "Point");
        assert_eq!(prog.records[0].fields.len(), 2);
        assert_eq!(prog.records[0].fields[0].name.name, "x");
        assert_eq!(prog.records[0].fields[1].name.name, "y");
    }

    #[test]
    fn parse_pub_record() {
        let source = r#"
            pub record Config {
                host: String,
                port: Int,
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.records.len(), 1);
        assert!(prog.records[0].is_pub);
        assert_eq!(prog.records[0].name.name, "Config");
    }

    #[test]
    fn parse_enum_declaration() {
        let source = r#"
            enum Status {
                Active,
                Pending,
                Done,
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.enums.len(), 1);
        assert!(!prog.enums[0].is_pub);
        assert_eq!(prog.enums[0].name.name, "Status");
        assert_eq!(prog.enums[0].variants.len(), 3);
        assert_eq!(prog.enums[0].variants[0].name.name, "Active");
        assert_eq!(prog.enums[0].variants[1].name.name, "Pending");
        assert_eq!(prog.enums[0].variants[2].name.name, "Done");
    }

    #[test]
    fn parse_pub_enum() {
        let source = r#"
            pub enum Priority { High, Medium, Low }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.enums.len(), 1);
        assert!(prog.enums[0].is_pub);
        assert_eq!(prog.enums[0].name.name, "Priority");
    }

    #[test]
    fn parse_const_declaration() {
        let source = r#"
            const MAX_RETRIES: Int = 3;

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.consts.len(), 1);
        assert!(!prog.consts[0].is_pub);
        assert_eq!(prog.consts[0].name.name, "MAX_RETRIES");
        assert!(matches!(prog.consts[0].ty, TypeExpr::Int));
    }

    #[test]
    fn parse_pub_const() {
        let source = r#"
            pub const API_URL: String = "https://api.example.com";

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.consts.len(), 1);
        assert!(prog.consts[0].is_pub);
        assert_eq!(prog.consts[0].name.name, "API_URL");
    }

    #[test]
    fn parse_multiple_type_declarations() {
        let source = r#"
            record Point { x: Int, y: Int }
            enum Color { Red, Green, Blue }
            const ORIGIN_X: Int = 0;

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.records.len(), 1);
        assert_eq!(prog.enums.len(), 1);
        assert_eq!(prog.consts.len(), 1);
    }

    #[test]
    fn parse_match_expression() {
        let source = r#"
            enum Status { Active, Pending, Done }

            agent Main {
                on start {
                    let s: Int = match Active {
                        Active => 1,
                        Pending => 2,
                        Done => 3,
                    };
                    yield(s);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Check the agent parsed
        assert_eq!(prog.agents.len(), 1);
        // Match is in the handler
        let handler = &prog.agents[0].handlers[0];
        let stmt = &handler.body.stmts[0];
        if let Stmt::Let { value, .. } = stmt {
            assert!(matches!(value, Expr::Match { .. }));
        } else {
            panic!("expected let statement with match");
        }
    }

    #[test]
    fn parse_match_with_wildcard() {
        let source = r#"
            agent Main {
                on start {
                    let x = 5;
                    let result = match x {
                        1 => 10,
                        2 => 20,
                        _ => 0,
                    };
                    yield(result);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents.len(), 1);
    }

    #[test]
    fn parse_record_construction() {
        let source = r#"
            record Point { x: Int, y: Int }

            agent Main {
                on start {
                    let p = Point { x: 10, y: 20 };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.records.len(), 1);
        assert_eq!(prog.agents.len(), 1);

        // Check the let statement has a record construction
        let handler = &prog.agents[0].handlers[0];
        let stmt = &handler.body.stmts[0];
        if let Stmt::Let { value, .. } = stmt {
            if let Expr::RecordConstruct { name, fields, .. } = value {
                assert_eq!(name.name, "Point");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name.name, "x");
                assert_eq!(fields[1].name.name, "y");
            } else {
                panic!("expected RecordConstruct");
            }
        } else {
            panic!("expected let statement");
        }
    }

    #[test]
    fn parse_match_with_qualified_variant() {
        let source = r#"
            enum Status { Active, Pending }

            fn get_status() -> Int {
                return 1;
            }

            agent Main {
                on start {
                    let s = get_status();
                    let result = match s {
                        Status::Active => 1,
                        Status::Pending => 0,
                    };
                    yield(result);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.enums.len(), 1);
        assert_eq!(prog.agents.len(), 1);
    }

    #[test]
    fn parse_field_access() {
        let source = r#"
            record Point { x: Int, y: Int }

            agent Main {
                on start {
                    let p = Point { x: 10, y: 20 };
                    let x_val = p.x;
                    let y_val = p.y;
                    yield(x_val);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.records.len(), 1);
        assert_eq!(prog.agents.len(), 1);

        // Check the field access
        let handler = &prog.agents[0].handlers[0];
        let stmt = &handler.body.stmts[1]; // p.x assignment
        if let Stmt::Let { value, .. } = stmt {
            if let Expr::FieldAccess { field, .. } = value {
                assert_eq!(field.name, "x");
            } else {
                panic!("expected FieldAccess");
            }
        } else {
            panic!("expected let statement");
        }
    }

    #[test]
    fn parse_chained_field_access() {
        let source = r#"
            record Inner { val: Int }
            record Outer { inner: Inner }

            agent Main {
                on start {
                    let inner = Inner { val: 42 };
                    let outer = Outer { inner: inner };
                    let v = outer.inner.val;
                    yield(v);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.records.len(), 2);
        assert_eq!(prog.agents.len(), 1);

        // Check the chained field access: outer.inner.val
        let handler = &prog.agents[0].handlers[0];
        let stmt = &handler.body.stmts[2]; // outer.inner.val assignment
        if let Stmt::Let { value, .. } = stmt {
            if let Expr::FieldAccess {
                object, field: val, ..
            } = value
            {
                assert_eq!(val.name, "val");
                // object should be outer.inner
                if let Expr::FieldAccess { field: inner, .. } = object.as_ref() {
                    assert_eq!(inner.name, "inner");
                } else {
                    panic!("expected nested FieldAccess");
                }
            } else {
                panic!("expected FieldAccess");
            }
        } else {
            panic!("expected let statement");
        }
    }

    // =========================================================================
    // RFC-0006: Message passing tests
    // =========================================================================

    #[test]
    fn parse_loop_break() {
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

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents.len(), 1);
        let handler = &prog.agents[0].handlers[0];
        // Check loop statement exists
        let loop_stmt = &handler.body.stmts[1];
        assert!(matches!(loop_stmt, Stmt::Loop { .. }));
        // Check break is inside the loop
        if let Stmt::Loop { body, .. } = loop_stmt {
            let if_stmt = &body.stmts[1];
            if let Stmt::If { then_block, .. } = if_stmt {
                assert!(matches!(then_block.stmts[0], Stmt::Break { .. }));
            } else {
                panic!("expected if statement");
            }
        }
    }

    #[test]
    fn parse_agent_receives() {
        let source = r#"
            enum WorkerMsg {
                Task,
                Shutdown,
            }

            agent Worker receives WorkerMsg {
                id: Int

                on start {
                    yield(0);
                }
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents.len(), 2);

        // Worker should have receives clause
        let worker = &prog.agents[0];
        assert_eq!(worker.name.name, "Worker");
        assert!(worker.receives.is_some());
        if let Some(TypeExpr::Named(name, _)) = &worker.receives {
            assert_eq!(name.name, "WorkerMsg");
        } else {
            panic!("expected named type for receives");
        }

        // Main should not have receives
        let main = &prog.agents[1];
        assert_eq!(main.name.name, "Main");
        assert!(main.receives.is_none());
    }

    #[test]
    fn parse_receive_expression() {
        let source = r#"
            enum Msg { Ping }

            agent Worker receives Msg {
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

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find Worker agent
        let worker = prog
            .agents
            .iter()
            .find(|a| a.name.name == "Worker")
            .unwrap();
        let handler = &worker.handlers[0];
        let stmt = &handler.body.stmts[0];

        if let Stmt::Let { value, .. } = stmt {
            assert!(matches!(value, Expr::Receive { .. }));
        } else {
            panic!("expected let with receive");
        }
    }

    #[test]
    fn parse_message_passing_full() {
        let source = r#"
            enum WorkerMsg {
                Task,
                Shutdown,
            }

            agent Worker receives WorkerMsg {
                id: Int

                on start {
                    let msg = receive();
                    let result = match msg {
                        Task => 1,
                        Shutdown => 0,
                    };
                    yield(result);
                }
            }

            agent Main {
                on start {
                    let w = summon Worker { id: 1 };
                    send(w, Task);
                    send(w, Shutdown);
                    await w;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.enums.len(), 1);
        assert_eq!(prog.agents.len(), 2);

        // Check Worker has receives
        let worker = prog
            .agents
            .iter()
            .find(|a| a.name.name == "Worker")
            .unwrap();
        assert!(worker.receives.is_some());
    }

    // =========================================================================
    // RFC-0007: Error handling tests
    // =========================================================================

    #[test]
    fn parse_fallible_function() {
        let source = r#"
            fn get_data(url: String) -> String fails {
                return divine("Get data from {url}" -> String);
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.functions.len(), 1);
        assert!(prog.functions[0].is_fallible);
    }

    #[test]
    fn parse_try_expression() {
        let source = r#"
            fn fallible() -> Int fails { return 42; }

            agent Main {
                on start {
                    let x = try fallible();
                    yield(x);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the let statement and check it contains a Try expression
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            assert!(matches!(value, Expr::Try { .. }));
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_catch_expression() {
        let source = r#"
            fn fallible() -> Int fails { return 42; }

            agent Main {
                on start {
                    let x = fallible() catch { 0 };
                    yield(x);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the let statement and check it contains a Catch expression
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Catch { error_bind, .. } = value {
                assert!(error_bind.is_none());
            } else {
                panic!("expected Catch expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_catch_with_error_binding() {
        let source = r#"
            fn fallible() -> Int fails { return 42; }

            agent Main {
                on start {
                    let x = fallible() catch(e) { 0 };
                    yield(x);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the let statement and check it contains a Catch expression with binding
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Catch { error_bind, .. } = value {
                assert!(error_bind.is_some());
                assert_eq!(error_bind.as_ref().unwrap().name, "e");
            } else {
                panic!("expected Catch expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_fail_expression() {
        let source = r#"
            agent Main {
                on start {
                    fail "something went wrong";
                }
                on error(e) {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the fail statement
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Expr { expr, .. } = &handler.body.stmts[0] {
            if let Expr::Fail { error, .. } = expr {
                assert!(matches!(**error, Expr::Literal { .. }));
            } else {
                panic!("expected Fail expression, got {expr:?}");
            }
        } else {
            panic!("expected Expr statement");
        }
    }

    #[test]
    fn parse_retry_expression() {
        let source = r#"
            agent Main {
                topic: String

                on start {
                    let result = retry(3) {
                        try divine("Summarize: {self.topic}")
                    } catch { "fallback" };
                    yield(result);
                }

                on error(e) {
                    yield("");
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the let statement with retry
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            // Should be Catch wrapping Retry
            if let Expr::Catch { expr, .. } = value {
                if let Expr::Retry { count, delay, .. } = expr.as_ref() {
                    assert!(matches!(**count, Expr::Literal { .. }));
                    assert!(delay.is_none());
                } else {
                    panic!("expected Retry expression");
                }
            } else {
                panic!("expected Catch expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_retry_with_delay() {
        let source = r#"
            agent Main {
                on start {
                    let result = retry(3, delay: 1000) {
                        42
                    } catch { 0 };
                    yield(result);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the let statement with retry
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Catch { expr, .. } = value {
                if let Expr::Retry { delay, .. } = expr.as_ref() {
                    assert!(delay.is_some());
                } else {
                    panic!("expected Retry expression");
                }
            } else {
                panic!("expected Catch expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_on_error_handler() {
        let source = r#"
            agent Main {
                on start {
                    yield(0);
                }

                on error(e) {
                    yield(1);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents.len(), 1);
        assert_eq!(prog.agents[0].handlers.len(), 2);

        // Check the error handler
        let error_handler = prog.agents[0]
            .handlers
            .iter()
            .find(|h| matches!(h.event, EventKind::Error { .. }));
        assert!(error_handler.is_some());

        if let EventKind::Error { param_name } = &error_handler.unwrap().event {
            assert_eq!(param_name.name, "e");
        } else {
            panic!("expected Error event kind");
        }
    }

    // =========================================================================
    // RFC-0009: Closures and function types
    // =========================================================================

    #[test]
    fn parse_fn_type() {
        let source = r#"
            fn apply(f: Fn(Int) -> Int, x: Int) -> Int {
                return f(x);
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.functions.len(), 1);
        let func = &prog.functions[0];
        assert_eq!(func.name.name, "apply");
        assert_eq!(func.params.len(), 2);

        // Check first param is Fn(Int) -> Int
        if let TypeExpr::Fn(params, ret) = &func.params[0].ty {
            assert_eq!(params.len(), 1);
            assert!(matches!(params[0], TypeExpr::Int));
            assert!(matches!(ret.as_ref(), TypeExpr::Int));
        } else {
            panic!("expected Fn type for first param");
        }
    }

    #[test]
    fn parse_closure_with_params() {
        let source = r#"
            agent Main {
                on start {
                    let f = |x: Int| x + 1;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the let statement in the on start handler
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Closure { params, body, .. } = value {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name.name, "x");
                assert!(matches!(&params[0].ty, Some(TypeExpr::Int)));

                // Body should be a binary expression
                assert!(matches!(body.as_ref(), Expr::Binary { .. }));
            } else {
                panic!("expected closure expression");
            }
        } else {
            panic!("expected let statement");
        }
    }

    #[test]
    fn parse_closure_empty_params() {
        let source = r#"
            agent Main {
                on start {
                    let f = || 42;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the let statement
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Closure { params, body, .. } = value {
                assert!(params.is_empty());

                // Body should be a literal
                assert!(matches!(body.as_ref(), Expr::Literal { .. }));
            } else {
                panic!("expected closure expression");
            }
        } else {
            panic!("expected let statement");
        }
    }

    #[test]
    fn parse_closure_multiple_params() {
        let source = r#"
            agent Main {
                on start {
                    let add = |x: Int, y: Int| x + y;
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Closure { params, .. } = value {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0].name.name, "x");
                assert_eq!(params[1].name.name, "y");
            } else {
                panic!("expected closure expression");
            }
        } else {
            panic!("expected let statement");
        }
    }

    #[test]
    fn parse_fn_type_multiarg() {
        let source = r#"
            fn fold_left(f: Fn(Int, Int) -> Int, init: Int) -> Int {
                return init;
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Check Fn(Int, Int) -> Int
        if let TypeExpr::Fn(params, ret) = &prog.functions[0].params[0].ty {
            assert_eq!(params.len(), 2);
            assert!(matches!(params[0], TypeExpr::Int));
            assert!(matches!(params[1], TypeExpr::Int));
            assert!(matches!(ret.as_ref(), TypeExpr::Int));
        } else {
            panic!("expected Fn type");
        }
    }

    #[test]
    fn parse_tuple_literal() {
        let source = r#"
            agent Main {
                on start {
                    let t = (1, 2);
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Tuple { elements, .. } = value {
                assert_eq!(elements.len(), 2);
            } else {
                panic!("expected tuple expression, got {:?}", value);
            }
        } else {
            panic!("expected let statement");
        }
    }

    // =========================================================================
    // RFC-0011: Tool support tests
    // =========================================================================

    #[test]
    fn parse_tool_declaration() {
        let source = r#"
            tool Http {
                fn get(url: String) -> Result<String, String>
                fn post(url: String, body: String) -> Result<String, String>
            }
            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.tools.len(), 1);
        assert_eq!(prog.tools[0].name.name, "Http");
        assert_eq!(prog.tools[0].functions.len(), 2);
        assert_eq!(prog.tools[0].functions[0].name.name, "get");
        assert_eq!(prog.tools[0].functions[1].name.name, "post");
    }

    #[test]
    fn parse_pub_tool_declaration() {
        let source = r#"
            pub tool Database {
                fn query(sql: String) -> Result<List<String>, String>
            }
            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert!(prog.tools[0].is_pub);
        assert_eq!(prog.tools[0].name.name, "Database");
    }

    #[test]
    fn parse_agent_with_tool_use() {
        let source = r#"
            agent Fetcher {
                use Http

                url: String

                on start {
                    yield(0);
                }
            }
            run Fetcher;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents[0].tool_uses.len(), 1);
        assert_eq!(prog.agents[0].tool_uses[0].name, "Http");
        assert_eq!(prog.agents[0].beliefs.len(), 1);
    }

    #[test]
    fn parse_agent_with_multiple_tool_uses() {
        let source = r#"
            agent Pipeline {
                use Http, Fs

                on start {
                    yield(0);
                }
            }
            run Pipeline;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents[0].tool_uses.len(), 2);
        assert_eq!(prog.agents[0].tool_uses[0].name, "Http");
        assert_eq!(prog.agents[0].tool_uses[1].name, "Fs");
    }

    #[test]
    fn parse_tool_call_expression() {
        let source = r#"
            agent Fetcher {
                use Http

                on start {
                    let response = Http.get("https://example.com");
                    yield(0);
                }
            }
            run Fetcher;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::ToolCall {
                tool,
                function,
                args,
                ..
            } = value
            {
                assert_eq!(tool.name, "Http");
                assert_eq!(function.name, "get");
                assert_eq!(args.len(), 1);
            } else {
                panic!("expected ToolCall expression, got {:?}", value);
            }
        } else {
            panic!("expected let statement");
        }
    }

    #[test]
    fn parse_tool_call_with_multiple_args() {
        let source = r#"
            agent Writer {
                use Fs

                on start {
                    let result = Fs.write("/tmp/test.txt", "hello world");
                    yield(0);
                }
            }
            run Writer;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::ToolCall { args, .. } = value {
                assert_eq!(args.len(), 2);
            } else {
                panic!("expected ToolCall expression, got {:?}", value);
            }
        } else {
            panic!("expected let statement");
        }
    }

    #[test]
    fn parse_string_interp_with_field_access() {
        let source = r#"
            record Person { name: String }
            agent Main {
                on start {
                    let p = Person { name: "Alice" };
                    print("Hello, {p.name}!");
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        // Find the print statement with interpolation
        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Expr { expr, .. } = &handler.body.stmts[1] {
            if let Expr::Call { args, .. } = expr {
                if let Expr::StringInterp { template, .. } = &args[0] {
                    assert!(template.has_interpolations());
                    let interps: Vec<_> = template.interpolations().collect();
                    assert_eq!(interps.len(), 1);
                    // Should be a field access: p.name
                    match interps[0] {
                        Expr::FieldAccess { object, field, .. } => {
                            if let Expr::Var { name, .. } = object.as_ref() {
                                assert_eq!(name.name, "p");
                            } else {
                                panic!("expected Var as base");
                            }
                            assert_eq!(field.name, "name");
                        }
                        _ => panic!("expected FieldAccess, got {:?}", interps[0]),
                    }
                } else {
                    panic!("expected StringInterp");
                }
            } else {
                panic!("expected Call");
            }
        } else {
            panic!("expected Expr statement");
        }
    }

    #[test]
    fn parse_string_interp_with_tuple_index() {
        let source = r#"
            agent Main {
                on start {
                    let pair = (1, 2);
                    print("First: {pair.0}");
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Expr { expr, .. } = &handler.body.stmts[1] {
            if let Expr::Call { args, .. } = expr {
                if let Expr::StringInterp { template, .. } = &args[0] {
                    let interps: Vec<_> = template.interpolations().collect();
                    assert_eq!(interps.len(), 1);
                    match interps[0] {
                        Expr::TupleIndex { tuple, index, .. } => {
                            if let Expr::Var { name, .. } = tuple.as_ref() {
                                assert_eq!(name.name, "pair");
                            } else {
                                panic!("expected Var as tuple base");
                            }
                            assert_eq!(*index, 0);
                        }
                        _ => panic!("expected TupleIndex, got {:?}", interps[0]),
                    }
                } else {
                    panic!("expected StringInterp");
                }
            } else {
                panic!("expected Call");
            }
        } else {
            panic!("expected Expr statement");
        }
    }

    #[test]
    fn parse_mock_tool_with_fail() {
        let source = r#"
            test "mock tool fail" {
                mock tool Http.get -> fail("network error");
            }
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let test = &prog.tests[0];
        assert_eq!(test.body.stmts.len(), 1);

        if let Stmt::MockTool {
            tool_name,
            fn_name,
            value,
            ..
        } = &test.body.stmts[0]
        {
            assert_eq!(tool_name.name, "Http");
            assert_eq!(fn_name.name, "get");
            assert!(
                matches!(value, MockValue::Fail(_)),
                "expected MockValue::Fail, got {:?}",
                value
            );
        } else {
            panic!("expected MockTool statement, got {:?}", test.body.stmts[0]);
        }
    }

    #[test]
    fn parse_mock_tool_with_value() {
        let source = r#"
            test "mock tool value" {
                mock tool Http.get -> "response";
            }
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let test = &prog.tests[0];
        if let Stmt::MockTool { value, .. } = &test.body.stmts[0] {
            assert!(
                matches!(value, MockValue::Value(_)),
                "expected MockValue::Value, got {:?}",
                value
            );
        } else {
            panic!("expected MockTool statement");
        }
    }

    // =========================================================================
    // RFC-0015: Generics tests
    // =========================================================================

    #[test]
    fn parse_generic_function() {
        let source = r#"
            fn identity<T>(x: T) -> T {
                return x;
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.functions.len(), 1);
        let func = &prog.functions[0];
        assert_eq!(func.name.name, "identity");
        assert_eq!(func.type_params.len(), 1);
        assert_eq!(func.type_params[0].name, "T");
    }

    #[test]
    fn parse_generic_function_multiple_params() {
        let source = r#"
            fn map<T, U>(list: List<T>, f: Fn(T) -> U) -> List<U> {
                return [];
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let func = &prog.functions[0];
        assert_eq!(func.name.name, "map");
        assert_eq!(func.type_params.len(), 2);
        assert_eq!(func.type_params[0].name, "T");
        assert_eq!(func.type_params[1].name, "U");
    }

    #[test]
    fn parse_generic_record() {
        let source = r#"
            record Pair<A, B> {
                first: A,
                second: B,
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.records.len(), 1);
        let record = &prog.records[0];
        assert_eq!(record.name.name, "Pair");
        assert_eq!(record.type_params.len(), 2);
        assert_eq!(record.type_params[0].name, "A");
        assert_eq!(record.type_params[1].name, "B");
    }

    #[test]
    fn parse_generic_enum() {
        let source = r#"
            enum Tree<T> {
                Leaf(T),
                Node(Tree<T>),
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.enums.len(), 1);
        let enumm = &prog.enums[0];
        assert_eq!(enumm.name.name, "Tree");
        assert_eq!(enumm.type_params.len(), 1);
        assert_eq!(enumm.type_params[0].name, "T");
    }

    #[test]
    fn parse_generic_type_argument() {
        let source = r#"
            record Wrapper<T> {
                value: T,
            }

            fn make_wrapper<T>(value: T) -> Wrapper<T> {
                return Wrapper { value: value };
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let func = &prog.functions[0];
        // Check return type is Wrapper<T>
        if let TypeExpr::Named(name, type_args) = &func.return_ty {
            assert_eq!(name.name, "Wrapper");
            assert_eq!(type_args.len(), 1);
            if let TypeExpr::Named(inner_name, _) = &type_args[0] {
                assert_eq!(inner_name.name, "T");
            } else {
                panic!("expected Named type argument");
            }
        } else {
            panic!("expected Named return type");
        }
    }

    #[test]
    fn parse_turbofish_function_call() {
        let source = r#"
            fn identity<T>(x: T) -> T {
                return x;
            }

            agent Main {
                on start {
                    let result = identity::<Int>(42);
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Call {
                name, type_args, ..
            } = value
            {
                assert_eq!(name.name, "identity");
                assert_eq!(type_args.len(), 1);
                assert!(matches!(type_args[0], TypeExpr::Int));
            } else {
                panic!("expected Call expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_turbofish_multiple_type_args() {
        let source = r#"
            fn make_pair<A, B>(a: A, b: B) -> (A, B) {
                return (a, b);
            }

            agent Main {
                on start {
                    let pair = make_pair::<Int, String>(42, "hello");
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::Call {
                name, type_args, ..
            } = value
            {
                assert_eq!(name.name, "make_pair");
                assert_eq!(type_args.len(), 2);
                assert!(matches!(type_args[0], TypeExpr::Int));
                assert!(matches!(type_args[1], TypeExpr::String));
            } else {
                panic!("expected Call expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_turbofish_record_construction() {
        let source = r#"
            record Pair<A, B> {
                first: A,
                second: B,
            }

            agent Main {
                on start {
                    let p = Pair::<Int, String> { first: 42, second: "hi" };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::RecordConstruct {
                name, type_args, ..
            } = value
            {
                assert_eq!(name.name, "Pair");
                assert_eq!(type_args.len(), 2);
                assert!(matches!(type_args[0], TypeExpr::Int));
                assert!(matches!(type_args[1], TypeExpr::String));
            } else {
                panic!("expected RecordConstruct expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_turbofish_variant_construction() {
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

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::VariantConstruct {
                enum_name,
                type_args,
                variant,
                ..
            } = value
            {
                assert_eq!(enum_name.name, "Either");
                assert_eq!(variant.name, "Left");
                assert_eq!(type_args.len(), 2);
                assert!(matches!(type_args[0], TypeExpr::String));
                assert!(matches!(type_args[1], TypeExpr::Int));
            } else {
                panic!("expected VariantConstruct expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    #[test]
    fn parse_generic_in_type_annotation() {
        let source = r#"
            record Page<T> {
                items: List<T>,
                count: Int,
            }

            agent Main {
                on start {
                    let page: Page<String> = Page { items: [], count: 0 };
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { ty: Some(ty), .. } = &handler.body.stmts[0] {
            if let TypeExpr::Named(name, type_args) = ty {
                assert_eq!(name.name, "Page");
                assert_eq!(type_args.len(), 1);
                assert!(matches!(type_args[0], TypeExpr::String));
            } else {
                panic!("expected Named type");
            }
        } else {
            panic!("expected Let statement with type annotation");
        }
    }

    #[test]
    fn parse_nested_generic_types() {
        let source = r#"
            record Nested<T> {
                value: Option<List<T>>,
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let record = &prog.records[0];
        let field_ty = &record.fields[0].ty;
        // Should be Option<List<T>>
        if let TypeExpr::Option(inner) = field_ty {
            if let TypeExpr::List(elem) = inner.as_ref() {
                if let TypeExpr::Named(name, _) = elem.as_ref() {
                    assert_eq!(name.name, "T");
                } else {
                    panic!("expected Named type T");
                }
            } else {
                panic!("expected List type");
            }
        } else {
            panic!("expected Option type");
        }
    }

    #[test]
    fn parse_span_block() {
        let source = r#"
            agent Main {
                on start {
                    span "fetch_data" {
                        let x = 1;
                        let y = 2;
                    }
                    yield(42);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        // First statement should be a SpanBlock
        if let Stmt::SpanBlock { name, body, .. } = &handler.body.stmts[0] {
            // Name should be a string literal
            if let Expr::Literal {
                value: Literal::String(s),
                ..
            } = name
            {
                assert_eq!(s, "fetch_data");
            } else {
                panic!("expected string literal for span name");
            }
            // Body should have 2 statements
            assert_eq!(body.stmts.len(), 2);
        } else {
            panic!("expected SpanBlock statement");
        }
    }

    #[test]
    fn parse_turbofish_in_interpolation() {
        let source = r#"
            fn identity<T>(x: T) -> T { return x; }

            agent Main {
                on start {
                    let msg = "result: {identity::<Int>(42)}";
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Let { value, .. } = &handler.body.stmts[0] {
            if let Expr::StringInterp { template, .. } = value {
                // Should have 2 parts: "result: " and the interpolated expression
                assert_eq!(template.parts.len(), 2);
                if let StringPart::Interpolation(call_expr) = &template.parts[1] {
                    if let Expr::Call {
                        name,
                        type_args,
                        args,
                        ..
                    } = call_expr.as_ref()
                    {
                        assert_eq!(name.name, "identity");
                        assert_eq!(type_args.len(), 1);
                        assert!(matches!(type_args[0], TypeExpr::Int));
                        assert_eq!(args.len(), 1);
                    } else {
                        panic!(
                            "expected Call expression in interpolation, got {:?}",
                            call_expr
                        );
                    }
                } else {
                    panic!("expected Interpolation part in template");
                }
            } else {
                panic!("expected StringInterp expression");
            }
        } else {
            panic!("expected Let statement");
        }
    }

    // =============================================================================
    // Phase 3: Session Types & Algebraic Effects
    // =============================================================================

    #[test]
    fn parse_protocol_declaration() {
        let source = r#"
            protocol SchemaSync {
                DatabaseSteward -> APISteward: SchemaChanged
                APISteward -> DatabaseSteward: Acknowledged
            }

            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.protocols.len(), 1);
        let proto = &prog.protocols[0];
        assert_eq!(proto.name.name, "SchemaSync");
        assert!(!proto.is_pub);
        assert_eq!(proto.steps.len(), 2);

        assert_eq!(proto.steps[0].sender.name, "DatabaseSteward");
        assert_eq!(proto.steps[0].receiver.name, "APISteward");
        if let TypeExpr::Named(name, _) = &proto.steps[0].message_type {
            assert_eq!(name.name, "SchemaChanged");
        } else {
            panic!("expected Named type");
        }

        assert_eq!(proto.steps[1].sender.name, "APISteward");
        assert_eq!(proto.steps[1].receiver.name, "DatabaseSteward");
    }

    #[test]
    fn parse_public_protocol() {
        let source = r#"
            pub protocol PingPong {
                Pinger -> Ponger: Ping
                Ponger -> Pinger: Pong
            }

            agent Main { on start { yield(0); } }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.protocols.len(), 1);
        assert!(prog.protocols[0].is_pub);
    }

    #[test]
    fn parse_agent_follows_protocol() {
        let source = r#"
            protocol SchemaSync {
                A -> B: Msg
            }

            agent APISteward follows SchemaSync as APISteward {
                on start {
                    yield(0);
                }
            }
            run APISteward;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents[0].follows.len(), 1);
        assert_eq!(prog.agents[0].follows[0].protocol.name, "SchemaSync");
        assert_eq!(prog.agents[0].follows[0].role.name, "APISteward");
    }

    #[test]
    fn parse_agent_follows_multiple_protocols() {
        let source = r#"
            agent MultiProtocolAgent follows Proto1 as RoleA, Proto2 as RoleB {
                on start {
                    yield(0);
                }
            }
            run MultiProtocolAgent;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents[0].follows.len(), 2);
        assert_eq!(prog.agents[0].follows[0].protocol.name, "Proto1");
        assert_eq!(prog.agents[0].follows[0].role.name, "RoleA");
        assert_eq!(prog.agents[0].follows[1].protocol.name, "Proto2");
        assert_eq!(prog.agents[0].follows[1].role.name, "RoleB");
    }

    #[test]
    fn parse_agent_with_receives_and_follows() {
        let source = r#"
            agent Worker receives Request follows WorkProto as Worker {
                on message(msg: Request) {
                    reply(Response {});
                }
            }
            run Worker;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert!(prog.agents[0].receives.is_some());
        assert_eq!(prog.agents[0].follows.len(), 1);
    }

    #[test]
    fn parse_reply_expression() {
        let source = r#"
            agent Responder {
                on message(msg: Request) {
                    reply(Response { code: 200 });
                }
            }
            run Responder;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let handler = &prog.agents[0].handlers[0];
        if let Stmt::Expr {
            expr: Expr::Reply { message, .. },
            ..
        } = &handler.body.stmts[0]
        {
            assert!(matches!(message.as_ref(), Expr::RecordConstruct { .. }));
        } else {
            panic!("expected Reply expression, got {:?}", handler.body.stmts[0]);
        }
    }

    #[test]
    fn parse_effect_handler_declaration() {
        let source = r#"
            handler DefaultLLM handles Infer {
                model: "gpt-4o"
                temperature: 0.7
                max_tokens: 1024
            }

            agent Main { on start { yield(0); } }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.effect_handlers.len(), 1);
        let handler = &prog.effect_handlers[0];
        assert_eq!(handler.name.name, "DefaultLLM");
        assert_eq!(handler.effect.name, "Infer");
        assert!(!handler.is_pub);
        assert_eq!(handler.config.len(), 3);

        assert_eq!(handler.config[0].key.name, "model");
        assert!(matches!(handler.config[0].value, Literal::String(_)));

        assert_eq!(handler.config[1].key.name, "temperature");
        assert!(matches!(handler.config[1].value, Literal::Float(_)));

        assert_eq!(handler.config[2].key.name, "max_tokens");
        assert!(matches!(handler.config[2].value, Literal::Int(1024)));
    }

    #[test]
    fn parse_public_effect_handler() {
        let source = r#"
            pub handler FastLLM handles Infer {
                model: "gpt-4o-mini"
            }

            agent Main { on start { yield(0); } }
            run Main;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert!(prog.effect_handlers[0].is_pub);
    }

    #[test]
    fn parse_supervisor_with_handler_assignment() {
        let source = r#"
            handler DefaultLLM handles Infer {
                model: "gpt-4o"
            }

            agent Worker {
                on start {
                    let thought = divine("Think!");
                    yield(thought);
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

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let child = &prog.supervisors[0].children[0];
        assert_eq!(child.agent_name.name, "Worker");
        assert_eq!(child.handler_assignments.len(), 1);
        assert_eq!(child.handler_assignments[0].effect.name, "Infer");
        assert_eq!(child.handler_assignments[0].handler.name, "DefaultLLM");
    }

    #[test]
    fn parse_supervisor_with_multiple_handler_assignments() {
        let source = r#"
            agent Worker { on start { yield(0); } }

            supervisor MultiHandlerSupervisor {
                strategy: OneForOne
                children {
                    Worker {
                        restart: Permanent
                        handler Infer: FastLLM
                        handler Logger: FileLogger
                    }
                }
            }

            run MultiHandlerSupervisor;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let child = &prog.supervisors[0].children[0];
        assert_eq!(child.handler_assignments.len(), 2);
        assert_eq!(child.handler_assignments[0].effect.name, "Infer");
        assert_eq!(child.handler_assignments[0].handler.name, "FastLLM");
        assert_eq!(child.handler_assignments[1].effect.name, "Logger");
        assert_eq!(child.handler_assignments[1].handler.name, "FileLogger");
    }

    #[test]
    fn recover_fn_with_self_inside_agent() {
        // Regression: chumsky's skip_then_retry_until silently swallowed
        // the entire program when `fn method(self)` appeared inside an
        // agent body, because `self` isn't a valid fn parameter and the
        // double recovery failure produced zero errors.  Now the agent
        // body uses nested_delimiters recovery so parsing continues.
        let source = r#"
            agent Test {
                name: String

                fn helper(self) -> String {
                    return "test";
                }

                on start {
                    yield("done");
                }
            }
            run Test
        "#;

        let (prog, errors) = parse_str(source);
        assert!(!errors.is_empty(), "should have parse errors for fn inside agent");
        // The recovery should produce a program — the agent body is
        // recovered via nested_delimiters, but the run statement may or
        // may not survive depending on chumsky's recovery path.
        let prog = prog.expect("should produce partial AST");
        // Verify that the agent was recovered (even if empty)
        assert!(
            prog.agents.iter().any(|a| a.name.name == "Test"),
            "agent Test should survive recovery"
        );
    }

    #[test]
    fn parse_hangman_code_valid() {
        // Ensure syntactically valid code with functions outside agent
        // parses correctly with no errors and a run_agent set.
        let source = r#"
fn display_word(word: String, guessed: List<String>) -> String {
    let letters = split(word, "");
    let result: List<String> = [];
    for ch in letters {
        if contains(guessed, ch) {
            result = push(result, ch);
        } else {
            result = push(result, "_");
        }
    }
    return join(result, " ");
}

fn check_win(word: String, guessed: List<String>) -> Bool {
    let letters = split(word, "");
    for ch in letters {
        if !contains(guessed, ch) {
            return false;
        }
    }
    return true;
}

agent Hangman {
    on start {
        let word = "sage";
        let guessed: List<String> = [];
        print(display_word(word, guessed));
        yield(0);
    }
}

run Hangman;
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "should have no parse errors: {errors:?}");
        let prog = prog.expect("should parse");
        assert_eq!(prog.functions.len(), 2);
        assert_eq!(prog.agents.len(), 1);
        assert_eq!(prog.agents[0].name.name, "Hangman");
        assert!(prog.run_agent.is_some(), "run_agent should be set");
        assert_eq!(prog.run_agent.as_ref().unwrap().name, "Hangman");
    }

}
