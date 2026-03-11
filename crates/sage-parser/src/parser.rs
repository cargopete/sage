//! Parser implementation using chumsky.
//!
//! This module transforms a token stream into an AST.

use crate::ast::{
    AgentDecl, BeliefDecl, BinOp, Block, ElseBranch, EventKind, Expr, FieldInit, FnDecl,
    HandlerDecl, Literal, Param, Program, Stmt, StringPart, StringTemplate, UnaryOp,
};
use chumsky::prelude::*;
use chumsky::BoxedParser;
use sage_lexer::{Spanned, Token};
use sage_types::{Ident, Span, TypeExpr};
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

    // Top-level declarations with recovery - skip to next agent/fn/run on error
    let top_level = agent_parser(source.clone())
        .or(fn_parser(source.clone()))
        .recover_with(skip_then_retry_until([
            Token::KwAgent,
            Token::KwFn,
            Token::KwRun,
        ]));

    let run_stmt = just(Token::KwRun).ignore_then(ident_token_parser(src.clone()));

    top_level.repeated().then(run_stmt).map_with_span(
        move |(items, run_agent), span: Range<usize>| {
            let mut agents = Vec::new();
            let mut functions = Vec::new();

            for item in items {
                match item {
                    TopLevel::Agent(a) => agents.push(a),
                    TopLevel::Function(f) => functions.push(f),
                }
            }

            Program {
                agents,
                functions,
                run_agent,
                span: make_span(&src2, span),
            }
        },
    )
}

/// Helper enum for collecting top-level declarations.
enum TopLevel {
    Agent(AgentDecl),
    Function(FnDecl),
}

// =============================================================================
// Agent parsers
// =============================================================================

/// Parser for an agent declaration.
#[allow(clippy::needless_pass_by_value)]
fn agent_parser(source: Arc<str>) -> impl Parser<Token, TopLevel, Error = ParseError> {
    let src = source.clone();
    let src2 = source.clone();
    let src3 = source.clone();
    let src4 = source.clone();

    let belief = just(Token::KwBelief)
        .ignore_then(ident_token_parser(src.clone()))
        .then_ignore(just(Token::Colon))
        .then(type_parser(src.clone()))
        .map_with_span(move |(name, ty), span: Range<usize>| BeliefDecl {
            name,
            ty,
            span: make_span(&src, span),
        });

    let handler = just(Token::KwOn)
        .ignore_then(event_kind_parser(src2.clone()))
        .then(block_parser(src2.clone()))
        .map_with_span(move |(event, body), span: Range<usize>| HandlerDecl {
            event,
            body,
            span: make_span(&src2, span),
        });

    just(Token::KwAgent)
        .ignore_then(ident_token_parser(src3.clone()))
        .then_ignore(just(Token::LBrace))
        .then(belief.repeated())
        .then(handler.repeated())
        .then_ignore(just(Token::RBrace))
        .map_with_span(move |((name, beliefs), handlers), span: Range<usize>| {
            TopLevel::Agent(AgentDecl {
                name,
                beliefs,
                handlers,
                span: make_span(&src4, span),
            })
        })
}

/// Parser for event kinds.
#[allow(clippy::needless_pass_by_value)]
fn event_kind_parser(source: Arc<str>) -> impl Parser<Token, EventKind, Error = ParseError> {
    let src = source.clone();

    let start = just(Token::KwStart).to(EventKind::Start);
    let stop = just(Token::KwStop).to(EventKind::Stop);

    let message = just(Token::KwMessage)
        .ignore_then(just(Token::LParen))
        .ignore_then(ident_token_parser(src.clone()))
        .then_ignore(just(Token::Colon))
        .then(type_parser(src))
        .then_ignore(just(Token::RParen))
        .map(|(param_name, param_ty)| EventKind::Message {
            param_name,
            param_ty,
        });

    start.or(stop).or(message)
}

// =============================================================================
// Function parsers
// =============================================================================

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

    just(Token::KwFn)
        .ignore_then(ident_token_parser(src2.clone()))
        .then(params)
        .then_ignore(just(Token::Arrow))
        .then(type_parser(src2.clone()))
        .then(block_parser(src2))
        .map_with_span(
            move |(((name, params), return_ty), body), span: Range<usize>| {
                TopLevel::Function(FnDecl {
                    name,
                    params,
                    return_ty,
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

    let let_stmt = just(Token::KwLet)
        .ignore_then(ident_token_parser(src.clone()))
        .then(
            just(Token::Colon)
                .ignore_then(type_parser(src.clone()))
                .or_not(),
        )
        .then_ignore(just(Token::Eq))
        .then(expr_parser(src.clone()))
        .map_with_span(move |((name, ty), value), span: Range<usize>| Stmt::Let {
            name,
            ty,
            value,
            span: make_span(&src, span),
        });

    let return_stmt = just(Token::KwReturn)
        .ignore_then(expr_parser(src2.clone()).or_not())
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
        .ignore_then(ident_token_parser(src4.clone()))
        .then_ignore(just(Token::KwIn))
        .then(expr_parser(src4.clone()))
        .then(block.clone())
        .map_with_span(move |((var, iter), body), span: Range<usize>| Stmt::For {
            var,
            iter,
            body,
            span: make_span(&src4, span),
        });

    let assign_stmt = ident_token_parser(src5.clone())
        .then_ignore(just(Token::Eq))
        .then(expr_parser(src5.clone()))
        .map_with_span(move |(name, value), span: Range<usize>| Stmt::Assign {
            name,
            value,
            span: make_span(&src5, span),
        });

    let expr_stmt =
        expr_parser(src6.clone()).map_with_span(move |expr, span: Range<usize>| Stmt::Expr {
            expr,
            span: make_span(&src6, span),
        });

    let_stmt
        .or(return_stmt)
        .or(if_stmt)
        .or(for_stmt)
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

        let paren = expr
            .clone()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |inner, span: Range<usize>| Expr::Paren {
                    inner: Box::new(inner),
                    span: make_span(&src, span),
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

        // infer("template") or infer("template" -> Type)
        let infer_expr = just(Token::KwInfer)
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
                move |(template, result_ty), span: Range<usize>| Expr::Infer {
                    template,
                    result_ty,
                    span: make_span(&src, span),
                }
            });

        // spawn Agent { field: value, ... }
        let field_init = ident_token_parser(src.clone())
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

        let spawn_expr = just(Token::KwSpawn)
            .ignore_then(ident_token_parser(src.clone()))
            .then_ignore(just(Token::LBrace))
            .then(field_init.separated_by(just(Token::Comma)).allow_trailing())
            .then_ignore(just(Token::RBrace))
            .map_with_span({
                let src = src.clone();
                move |(agent, fields), span: Range<usize>| Expr::Spawn {
                    agent,
                    fields,
                    span: make_span(&src, span),
                }
            });

        // await expr - we need to handle this carefully to avoid left recursion
        let await_expr = just(Token::KwAwait)
            .ignore_then(ident_token_parser(src.clone()).map_with_span({
                let src = src.clone();
                move |name, span: Range<usize>| Expr::Var {
                    name,
                    span: make_span(&src, span),
                }
            }))
            .map_with_span({
                let src = src.clone();
                move |handle, span: Range<usize>| Expr::Await {
                    handle: Box::new(handle),
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

        // emit(value)
        let emit_expr = just(Token::KwEmit)
            .ignore_then(just(Token::LParen))
            .ignore_then(expr.clone())
            .then_ignore(just(Token::RParen))
            .map_with_span({
                let src = src.clone();
                move |value, span: Range<usize>| Expr::Emit {
                    value: Box::new(value),
                    span: make_span(&src, span),
                }
            });

        // function call: name(args)
        let call_expr = ident_token_parser(src.clone())
            .then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .map_with_span({
                let src = src.clone();
                move |(name, args), span: Range<usize>| Expr::Call {
                    name,
                    args,
                    span: make_span(&src, span),
                }
            });

        // Atom: the base expression without binary ops
        // Box early to cut type complexity
        let atom = infer_expr
            .or(spawn_expr)
            .or(await_expr)
            .or(send_expr)
            .or(emit_expr)
            .or(self_access)
            .or(call_expr)
            .or(list)
            .or(paren)
            .or(literal)
            .or(var)
            .boxed();

        // Unary expressions
        let unary = just(Token::Minus)
            .to(UnaryOp::Neg)
            .or(just(Token::Bang).to(UnaryOp::Not))
            .repeated()
            .then(atom)
            .foldr(|op, operand| {
                let span = operand.span().clone();
                Expr::Unary {
                    op,
                    operand: Box::new(operand),
                    span,
                }
            })
            .boxed();

        // Binary operators with precedence levels
        // Level 7: * /
        let mul_div_op = just(Token::Star)
            .to(BinOp::Mul)
            .or(just(Token::Slash).to(BinOp::Div));

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

        and.clone().then(or_op.then(and).repeated()).foldl({
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

        let inferred_ty = just(Token::TyInferred)
            .ignore_then(just(Token::Lt))
            .ignore_then(ty.clone())
            .then_ignore(just(Token::Gt))
            .map(|inner| TypeExpr::Inferred(Box::new(inner)));

        let agent_ty = just(Token::TyAgent)
            .ignore_then(just(Token::Lt))
            .ignore_then(ident_token_parser(src.clone()))
            .then_ignore(just(Token::Gt))
            .map(TypeExpr::Agent);

        let named_ty = ident_token_parser(src).map(TypeExpr::Named);

        primitive
            .or(list_ty)
            .or(option_ty)
            .or(inferred_ty)
            .or(agent_ty)
            .or(named_ty)
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

    let string_lit = filter_map(move |span: Range<usize>, token| match token {
        Token::StringLit => {
            let text = &src5[span.start..span.end];
            let inner = &text[1..text.len() - 1];
            let unescaped = unescape_string(inner);
            Ok(Literal::String(unescaped))
        }
        _ => Err(Simple::expected_input_found(
            span,
            vec![Some(Token::StringLit)],
            Some(token),
        )),
    })
    .map_with_span(move |value, span: Range<usize>| Expr::Literal {
        value,
        span: make_span(&source, span),
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

/// Parse a string into template parts, handling `{ident}` interpolations.
fn parse_string_template(s: &str, span: &Span) -> Vec<StringPart> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' {
            if !current.is_empty() {
                parts.push(StringPart::Literal(std::mem::take(&mut current)));
            }

            let mut ident_name = String::new();
            while let Some(&c) = chars.peek() {
                if c == '}' {
                    chars.next();
                    break;
                }
                ident_name.push(c);
                chars.next();
            }

            if !ident_name.is_empty() {
                parts.push(StringPart::Interpolation(Ident::new(
                    ident_name,
                    span.clone(),
                )));
            }
        } else if ch == '\\' {
            if let Some(escaped) = chars.next() {
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

/// Unescape a string literal.
fn unescape_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                result.push(match escaped {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    '\\' => '\\',
                    '"' => '"',
                    other => other,
                });
            }
        } else {
            result.push(ch);
        }
    }

    result
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use sage_lexer::lex;

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
                    emit(42)
                }
            }
            run Main
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        assert_eq!(prog.agents.len(), 1);
        assert_eq!(prog.agents[0].name.name, "Main");
        assert_eq!(prog.run_agent.name, "Main");
    }

    #[test]
    fn parse_agent_with_beliefs() {
        let source = r#"
            agent Researcher {
                belief topic: String
                belief max_words: Int

                on start {
                    emit(self.topic)
                }
            }
            run Researcher
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
                    print("started")
                }

                on message(msg: String) {
                    print(msg)
                }

                on stop {
                    print("stopped")
                }
            }
            run Worker
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
    fn parse_function() {
        let source = r#"
            fn greet(name: String) -> String {
                return "Hello, " ++ name
            }

            agent Main {
                on start {
                    emit(greet("World"))
                }
            }
            run Main
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
                    let x: Int = 42
                    let y = "hello"
                    emit(x)
                }
            }
            run Main
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
                        emit(1)
                    } else {
                        emit(2)
                    }
                }
            }
            run Main
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
                        print(x)
                    }
                    emit(0)
                }
            }
            run Main
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
                belief name: String
                on start {
                    emit(self.name)
                }
            }

            agent Main {
                on start {
                    let w = spawn Worker { name: "test" }
                    let result = await w
                    emit(result)
                }
            }
            run Main
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        prog.expect("should parse");
    }

    #[test]
    fn parse_infer() {
        let source = r#"
            agent Main {
                on start {
                    let result = infer("What is 2+2?")
                    emit(result)
                }
            }
            run Main
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
                    let x = 2 + 3 * 4
                    emit(x)
                }
            }
            run Main
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
                    let name = "World"
                    let msg = infer("Greet {name}")
                    emit(msg)
                }
            }
            run Main
        "#;

        let (prog, errors) = parse_str(source);
        assert!(errors.is_empty(), "errors: {errors:?}");
        let prog = prog.expect("should parse");

        let stmts = &prog.agents[0].handlers[0].body.stmts;
        if let Stmt::Let { value, .. } = &stmts[1] {
            if let Expr::Infer { template, .. } = value {
                assert!(template.has_interpolations());
            } else {
                panic!("expected infer expression");
            }
        }
    }

    // =========================================================================
    // Error recovery tests
    // =========================================================================

    #[test]
    fn recover_from_malformed_agent_continues_to_next() {
        // First agent has syntax error, second is valid
        let source = r#"
            agent Broken {
                belief x
            }

            agent Main {
                on start {
                    emit(42)
                }
            }
            run Main
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
                    let x = [1, 2, 3
                    emit(42)
                }
            }
            run Main
        "#;

        let (prog, errors) = parse_str(source);
        // Should have errors but still produce an AST
        assert!(!errors.is_empty(), "should have parse errors");
        assert!(prog.is_some(), "should produce partial AST despite errors");
    }

    #[test]
    fn recover_multiple_errors_reported() {
        // Multiple errors in different places
        let source = r#"
            agent A {
                belief
            }

            agent B {
                belief
            }

            agent Main {
                on start {
                    emit(42)
                }
            }
            run Main
        "#;

        let (_prog, errors) = parse_str(source);
        // Should report at least one error from the malformed agents
        assert!(!errors.is_empty(), "should report errors");
        // Recovery allows parsing to continue even with errors
    }
}
