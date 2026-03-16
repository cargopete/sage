//! Abstract Syntax Tree definitions for the Sage language.
//!
//! This module defines all AST node types that the parser produces.
//! Every node carries a `Span` for error reporting.

use crate::{Ident, Span, TypeExpr};
use std::fmt;

// =============================================================================
// Program (top-level)
// =============================================================================

/// A complete Sage program (or module).
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// Module declarations (`mod foo`).
    pub mod_decls: Vec<ModDecl>,
    /// Use declarations (`use foo::Bar`).
    pub use_decls: Vec<UseDecl>,
    /// Record type declarations.
    pub records: Vec<RecordDecl>,
    /// Enum type declarations.
    pub enums: Vec<EnumDecl>,
    /// Constant declarations.
    pub consts: Vec<ConstDecl>,
    /// Tool declarations (RFC-0011).
    pub tools: Vec<ToolDecl>,
    /// Agent declarations.
    pub agents: Vec<AgentDecl>,
    /// Function declarations.
    pub functions: Vec<FnDecl>,
    /// Test declarations (RFC-0012). Only valid in `_test.sg` files.
    pub tests: Vec<TestDecl>,
    /// The entry-point agent (from `run AgentName`).
    /// None for library modules that don't have an entry point.
    pub run_agent: Option<Ident>,
    /// Span covering the entire program.
    pub span: Span,
}

// =============================================================================
// Module declarations
// =============================================================================

/// A module declaration: `mod name` or `pub mod name`
#[derive(Debug, Clone, PartialEq)]
pub struct ModDecl {
    /// Whether this module is public.
    pub is_pub: bool,
    /// The module name.
    pub name: Ident,
    /// Span covering the declaration.
    pub span: Span,
}

/// A use declaration: `use path::to::Item`
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    /// Whether this is a public re-export (`pub use`).
    pub is_pub: bool,
    /// The path segments (e.g., `["agents", "Researcher"]`).
    pub path: Vec<Ident>,
    /// The kind of import.
    pub kind: UseKind,
    /// Span covering the declaration.
    pub span: Span,
}

/// The kind of use declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum UseKind {
    /// Simple import: `use a::B` or `use a::B as C`
    /// The Option is the alias (e.g., `C` in `use a::B as C`).
    Simple(Option<Ident>),
    /// Glob import: `use a::*`
    Glob,
    /// Group import: `use a::{B, C as D}`
    /// Each tuple is (name, optional alias).
    Group(Vec<(Ident, Option<Ident>)>),
}

// =============================================================================
// Type declarations (records, enums)
// =============================================================================

/// A record declaration: `record Point { x: Int, y: Int }`
#[derive(Debug, Clone, PartialEq)]
pub struct RecordDecl {
    /// Whether this record is public.
    pub is_pub: bool,
    /// The record's name.
    pub name: Ident,
    /// The record's fields.
    pub fields: Vec<RecordField>,
    /// Span covering the declaration.
    pub span: Span,
}

/// A field in a record declaration: `name: Type`
#[derive(Debug, Clone, PartialEq)]
pub struct RecordField {
    /// The field's name.
    pub name: Ident,
    /// The field's type.
    pub ty: TypeExpr,
    /// Span covering the field.
    pub span: Span,
}

/// An enum variant with optional payload: `Ok(T)` or `None`
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    /// The variant's name.
    pub name: Ident,
    /// Optional payload type (e.g., `T` in `Ok(T)`).
    pub payload: Option<TypeExpr>,
    /// Span covering the variant.
    pub span: Span,
}

/// An enum declaration: `enum Status { Active, Pending, Done }` or `enum Result { Ok(T), Err(E) }`
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDecl {
    /// Whether this enum is public.
    pub is_pub: bool,
    /// The enum's name.
    pub name: Ident,
    /// The enum's variants.
    pub variants: Vec<EnumVariant>,
    /// Span covering the declaration.
    pub span: Span,
}

/// A const declaration: `const MAX_RETRIES: Int = 3`
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    /// Whether this const is public.
    pub is_pub: bool,
    /// The constant's name.
    pub name: Ident,
    /// The constant's type.
    pub ty: TypeExpr,
    /// The constant's value.
    pub value: Expr,
    /// Span covering the declaration.
    pub span: Span,
}

// =============================================================================
// Tool declarations (RFC-0011)
// =============================================================================

/// A tool declaration: `tool Http { fn get(url: String) -> Result<String, String> }`
#[derive(Debug, Clone, PartialEq)]
pub struct ToolDecl {
    /// Whether this tool is public.
    pub is_pub: bool,
    /// The tool's name.
    pub name: Ident,
    /// The tool's function signatures.
    pub functions: Vec<ToolFnDecl>,
    /// Span covering the declaration.
    pub span: Span,
}

/// A function signature in a tool declaration (no body).
#[derive(Debug, Clone, PartialEq)]
pub struct ToolFnDecl {
    /// The function's name.
    pub name: Ident,
    /// The function's parameters.
    pub params: Vec<Param>,
    /// The return type.
    pub return_ty: TypeExpr,
    /// Span covering the declaration.
    pub span: Span,
}

// =============================================================================
// Agent declarations
// =============================================================================

/// An agent declaration: `agent Name { ... }` or `pub agent Name receives MsgType { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct AgentDecl {
    /// Whether this agent is public (can be imported by other modules).
    pub is_pub: bool,
    /// The agent's name.
    pub name: Ident,
    /// The message type this agent receives (for message passing).
    pub receives: Option<TypeExpr>,
    /// Tools this agent uses (RFC-0011): `use Http, Fs`
    pub tool_uses: Vec<Ident>,
    /// Belief declarations (agent state).
    pub beliefs: Vec<BeliefDecl>,
    /// Event handlers.
    pub handlers: Vec<HandlerDecl>,
    /// Span covering the entire declaration.
    pub span: Span,
}

/// A belief declaration: `belief name: Type`
#[derive(Debug, Clone, PartialEq)]
pub struct BeliefDecl {
    /// The belief's name.
    pub name: Ident,
    /// The belief's type.
    pub ty: TypeExpr,
    /// Span covering the declaration.
    pub span: Span,
}

/// An event handler: `on start { ... }`, `on message(x: T) { ... }`, `on stop { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct HandlerDecl {
    /// The event kind this handler responds to.
    pub event: EventKind,
    /// The handler body.
    pub body: Block,
    /// Span covering the entire handler.
    pub span: Span,
}

/// The kind of event a handler responds to.
#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    /// `on start` — runs when the agent is spawned.
    Start,
    /// `on message(param: Type)` — runs when a message is received.
    Message {
        /// The parameter name for the incoming message.
        param_name: Ident,
        /// The type of the message.
        param_ty: TypeExpr,
    },
    /// `on stop` — runs during graceful shutdown.
    Stop,
    /// `on error(e)` — runs when an unhandled error occurs in the agent.
    Error {
        /// The parameter name for the error.
        param_name: Ident,
    },
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventKind::Start => write!(f, "start"),
            EventKind::Message {
                param_name,
                param_ty,
            } => {
                write!(f, "message({param_name}: {param_ty})")
            }
            EventKind::Stop => write!(f, "stop"),
            EventKind::Error { param_name } => {
                write!(f, "error({param_name})")
            }
        }
    }
}

// =============================================================================
// Function declarations
// =============================================================================

/// A function declaration: `fn name(params) -> ReturnType { ... }` or `fn name(params) -> ReturnType fails { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    /// Whether this function is public (can be imported by other modules).
    pub is_pub: bool,
    /// The function's name.
    pub name: Ident,
    /// The function's parameters.
    pub params: Vec<Param>,
    /// The return type.
    pub return_ty: TypeExpr,
    /// Whether this function can fail (marked with `fails`).
    pub is_fallible: bool,
    /// The function body.
    pub body: Block,
    /// Span covering the entire declaration.
    pub span: Span,
}

/// A function parameter: `name: Type`
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// The parameter name.
    pub name: Ident,
    /// The parameter type.
    pub ty: TypeExpr,
    /// Span covering the parameter.
    pub span: Span,
}

/// A closure parameter: `name` or `name: Type`
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureParam {
    /// The parameter name.
    pub name: Ident,
    /// Optional type annotation (can be inferred).
    pub ty: Option<TypeExpr>,
    /// Span covering the parameter.
    pub span: Span,
}

// =============================================================================
// Test declarations (RFC-0012)
// =============================================================================

/// A test declaration: `test "description" { ... }` or `@serial test "description" { ... }`
#[derive(Debug, Clone, PartialEq)]
pub struct TestDecl {
    /// The test description (the string after `test`).
    pub name: String,
    /// Whether this test must run serially (marked with `@serial`).
    pub is_serial: bool,
    /// The test body.
    pub body: Block,
    /// Span covering the entire declaration.
    pub span: Span,
}

// =============================================================================
// Blocks and statements
// =============================================================================

/// A block of statements: `{ stmt* }`
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// The statements in this block.
    pub stmts: Vec<Stmt>,
    /// Span covering the entire block (including braces).
    pub span: Span,
}

/// A statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Variable binding: `let name: Type = expr` or `let name = expr`
    Let {
        /// The variable name.
        name: Ident,
        /// Optional type annotation.
        ty: Option<TypeExpr>,
        /// The initial value.
        value: Expr,
        /// Span covering the statement.
        span: Span,
    },

    /// Assignment: `name = expr`
    Assign {
        /// The variable being assigned to.
        name: Ident,
        /// The new value.
        value: Expr,
        /// Span covering the statement.
        span: Span,
    },

    /// Return statement: `return expr?`
    Return {
        /// The optional return value.
        value: Option<Expr>,
        /// Span covering the statement.
        span: Span,
    },

    /// If statement: `if cond { ... } else { ... }`
    If {
        /// The condition (must be Bool).
        condition: Expr,
        /// The then branch.
        then_block: Block,
        /// The optional else branch (can be another If for else-if chains).
        else_block: Option<ElseBranch>,
        /// Span covering the statement.
        span: Span,
    },

    /// For loop: `for x in iter { ... }` or `for (k, v) in map { ... }`
    For {
        /// The loop pattern (can be a simple binding or tuple destructuring).
        pattern: Pattern,
        /// The iterable expression (List<T> or Map<K, V>).
        iter: Expr,
        /// The loop body.
        body: Block,
        /// Span covering the statement.
        span: Span,
    },

    /// While loop: `while cond { ... }`
    While {
        /// The condition (must be Bool).
        condition: Expr,
        /// The loop body.
        body: Block,
        /// Span covering the statement.
        span: Span,
    },

    /// Infinite loop: `loop { ... }`
    Loop {
        /// The loop body.
        body: Block,
        /// Span covering the statement.
        span: Span,
    },

    /// Break statement: `break`
    Break {
        /// Span covering the statement.
        span: Span,
    },

    /// Expression statement: `expr`
    Expr {
        /// The expression.
        expr: Expr,
        /// Span covering the statement.
        span: Span,
    },

    /// Tuple destructuring: `let (a, b) = expr;`
    LetTuple {
        /// The variable names.
        names: Vec<Ident>,
        /// Optional type annotation.
        ty: Option<TypeExpr>,
        /// The value expression.
        value: Expr,
        /// Span covering the statement.
        span: Span,
    },

    /// RFC-0012: Mock infer statement: `mock infer -> expr;`
    MockInfer {
        /// The mock value expression.
        value: MockValue,
        /// Span covering the statement.
        span: Span,
    },

    /// Mock tool statement: `mock tool Http.get -> value;`
    MockTool {
        /// The tool name.
        tool_name: Ident,
        /// The function name.
        fn_name: Ident,
        /// The mock value expression.
        value: MockValue,
        /// Span covering the statement.
        span: Span,
    },
}

/// RFC-0012: A mock value for `mock infer -> value`.
#[derive(Debug, Clone, PartialEq)]
pub enum MockValue {
    /// A literal value: `mock infer -> "string"` or `mock infer -> SomeRecord { ... }`
    Value(Expr),
    /// A failure: `mock infer -> fail("error message")`
    Fail(Expr),
}

impl Stmt {
    /// Get the span of this statement.
    #[must_use]
    pub fn span(&self) -> &Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::If { span, .. }
            | Stmt::For { span, .. }
            | Stmt::While { span, .. }
            | Stmt::Loop { span, .. }
            | Stmt::Break { span, .. }
            | Stmt::Expr { span, .. }
            | Stmt::LetTuple { span, .. }
            | Stmt::MockInfer { span, .. }
            | Stmt::MockTool { span, .. } => span,
        }
    }
}

/// The else branch of an if statement.
#[derive(Debug, Clone, PartialEq)]
pub enum ElseBranch {
    /// `else { ... }`
    Block(Block),
    /// `else if ...` (chained if)
    ElseIf(Box<Stmt>),
}

// =============================================================================
// Expressions
// =============================================================================

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// LLM inference: `infer("template")` or `infer("template" -> Type)`
    Infer {
        /// The prompt template (may contain `{ident}` interpolations).
        template: StringTemplate,
        /// Optional result type annotation.
        result_ty: Option<TypeExpr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Agent spawning: `spawn AgentName { field: value, ... }`
    Spawn {
        /// The agent type to spawn.
        agent: Ident,
        /// Initial belief values.
        fields: Vec<FieldInit>,
        /// Span covering the expression.
        span: Span,
    },

    /// Await: `await expr` or `await expr timeout(ms)`
    Await {
        /// The agent handle to await.
        handle: Box<Expr>,
        /// Optional timeout in milliseconds.
        timeout: Option<Box<Expr>>,
        /// Span covering the expression.
        span: Span,
    },

    /// Send message: `send(handle, message)`
    Send {
        /// The agent handle to send to.
        handle: Box<Expr>,
        /// The message to send.
        message: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Emit value: `emit(value)`
    Emit {
        /// The value to emit to the awaiter.
        value: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Function call: `name(args)`
    Call {
        /// The function name.
        name: Ident,
        /// The arguments.
        args: Vec<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Method call on self: `self.method(args)`
    SelfMethodCall {
        /// The method name.
        method: Ident,
        /// The arguments.
        args: Vec<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Self field access: `self.field`
    SelfField {
        /// The field (belief) name.
        field: Ident,
        /// Span covering the expression.
        span: Span,
    },

    /// Binary operation: `left op right`
    Binary {
        /// The operator.
        op: BinOp,
        /// The left operand.
        left: Box<Expr>,
        /// The right operand.
        right: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Unary operation: `op operand`
    Unary {
        /// The operator.
        op: UnaryOp,
        /// The operand.
        operand: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// List literal: `[a, b, c]`
    List {
        /// The list elements.
        elements: Vec<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Literal value.
    Literal {
        /// The literal value.
        value: Literal,
        /// Span covering the expression.
        span: Span,
    },

    /// Variable reference.
    Var {
        /// The variable name.
        name: Ident,
        /// Span covering the expression.
        span: Span,
    },

    /// Parenthesized expression: `(expr)`
    Paren {
        /// The inner expression.
        inner: Box<Expr>,
        /// Span covering the expression (including parens).
        span: Span,
    },

    /// Interpolated string: `"Hello, {name}!"`
    StringInterp {
        /// The string template with interpolations.
        template: StringTemplate,
        /// Span covering the expression.
        span: Span,
    },

    /// Match expression: `match expr { Pattern => expr, ... }`
    Match {
        /// The scrutinee expression.
        scrutinee: Box<Expr>,
        /// The match arms.
        arms: Vec<MatchArm>,
        /// Span covering the expression.
        span: Span,
    },

    /// Record construction: `Point { x: 1, y: 2 }`
    RecordConstruct {
        /// The record type name.
        name: Ident,
        /// Field initializations.
        fields: Vec<FieldInit>,
        /// Span covering the expression.
        span: Span,
    },

    /// Field access: `record.field`
    FieldAccess {
        /// The record expression.
        object: Box<Expr>,
        /// The field name.
        field: Ident,
        /// Span covering the expression.
        span: Span,
    },

    /// Receive message from mailbox: `receive()`
    Receive {
        /// Span covering the expression.
        span: Span,
    },

    /// Try expression: `try expr` — propagates failure upward.
    Try {
        /// The expression that may fail.
        expr: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Catch expression: `expr catch { recovery }` or `expr catch(e) { recovery }`.
    Catch {
        /// The expression that may fail.
        expr: Box<Expr>,
        /// The optional error binding (e.g., `e` in `catch(e)`).
        error_bind: Option<Ident>,
        /// The recovery expression.
        recovery: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Fail expression: `fail "message"` or `fail Error { ... }`.
    /// Type is `Never` - this expression never returns.
    Fail {
        /// The error value (either a string message or an Error record).
        error: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Retry expression: `retry(3) { ... }` or `retry(3, delay: 1000) { ... }`
    Retry {
        /// Number of retry attempts (1-10).
        count: Box<Expr>,
        /// Optional delay between attempts in milliseconds.
        delay: Option<Box<Expr>>,
        /// Optional list of error kinds to retry on.
        on_errors: Option<Vec<Expr>>,
        /// The body to retry.
        body: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Trace expression: `trace("message")` for emitting trace events.
    Trace {
        /// The message to trace (must be a string).
        message: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Closure expression: `|params| body`
    Closure {
        /// The closure parameters.
        params: Vec<ClosureParam>,
        /// The closure body (single expression).
        body: Box<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Tuple literal: `(a, b, c)`
    Tuple {
        /// The tuple elements (at least 2).
        elements: Vec<Expr>,
        /// Span covering the expression.
        span: Span,
    },

    /// Tuple index access: `tuple.0`
    TupleIndex {
        /// The tuple expression.
        tuple: Box<Expr>,
        /// The index (0-based).
        index: usize,
        /// Span covering the expression.
        span: Span,
    },

    /// Map literal: `{ key: value, ... }` or `{}`
    Map {
        /// The map entries.
        entries: Vec<MapEntry>,
        /// Span covering the expression.
        span: Span,
    },

    /// Enum variant construction: `MyEnum.Variant` or `MyEnum.Variant(payload)`
    VariantConstruct {
        /// The enum type name.
        enum_name: Ident,
        /// The variant name.
        variant: Ident,
        /// The optional payload expression.
        payload: Option<Box<Expr>>,
        /// Span covering the expression.
        span: Span,
    },

    /// Tool function call (RFC-0011): `Http.get(url)`
    ToolCall {
        /// The tool name.
        tool: Ident,
        /// The function name.
        function: Ident,
        /// The arguments.
        args: Vec<Expr>,
        /// Span covering the expression.
        span: Span,
    },
}

/// A map entry: `key: value`
#[derive(Debug, Clone, PartialEq)]
pub struct MapEntry {
    /// The key expression.
    pub key: Expr,
    /// The value expression.
    pub value: Expr,
    /// Span covering the entry.
    pub span: Span,
}

impl Expr {
    /// Get the span of this expression.
    #[must_use]
    pub fn span(&self) -> &Span {
        match self {
            Expr::Infer { span, .. }
            | Expr::Spawn { span, .. }
            | Expr::Await { span, .. }
            | Expr::Send { span, .. }
            | Expr::Emit { span, .. }
            | Expr::Call { span, .. }
            | Expr::SelfMethodCall { span, .. }
            | Expr::SelfField { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Unary { span, .. }
            | Expr::List { span, .. }
            | Expr::Literal { span, .. }
            | Expr::Var { span, .. }
            | Expr::Paren { span, .. }
            | Expr::StringInterp { span, .. }
            | Expr::Match { span, .. }
            | Expr::RecordConstruct { span, .. }
            | Expr::FieldAccess { span, .. }
            | Expr::Receive { span, .. }
            | Expr::Try { span, .. }
            | Expr::Catch { span, .. }
            | Expr::Fail { span, .. }
            | Expr::Retry { span, .. }
            | Expr::Trace { span, .. }
            | Expr::Closure { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::TupleIndex { span, .. }
            | Expr::Map { span, .. }
            | Expr::VariantConstruct { span, .. }
            | Expr::ToolCall { span, .. } => span,
        }
    }
}

/// A field initialization in a spawn or record construction expression: `field: value`
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    /// The field name.
    pub name: Ident,
    /// The initial value.
    pub value: Expr,
    /// Span covering the field initialization.
    pub span: Span,
}

/// A match arm: `Pattern => expr`
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    /// The pattern to match.
    pub pattern: Pattern,
    /// The expression to evaluate if the pattern matches.
    pub body: Expr,
    /// Span covering the arm.
    pub span: Span,
}

/// A pattern in a match expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// Wildcard pattern: `_`
    Wildcard {
        /// Span covering the pattern.
        span: Span,
    },
    /// Enum variant pattern: `Status::Active`, `Ok(x)`, or just `Active`
    Variant {
        /// Optional enum type name (for qualified patterns).
        enum_name: Option<Ident>,
        /// The variant name.
        variant: Ident,
        /// Optional payload binding pattern (e.g., `x` in `Ok(x)`).
        payload: Option<Box<Pattern>>,
        /// Span covering the pattern.
        span: Span,
    },
    /// Literal pattern: `42`, `"hello"`, `true`
    Literal {
        /// The literal value.
        value: Literal,
        /// Span covering the pattern.
        span: Span,
    },
    /// Binding pattern: `x` (binds the matched value to a variable)
    Binding {
        /// The variable name.
        name: Ident,
        /// Span covering the pattern.
        span: Span,
    },
    /// Tuple pattern: `(a, b, c)`
    Tuple {
        /// The element patterns.
        elements: Vec<Pattern>,
        /// Span covering the pattern.
        span: Span,
    },
}

impl Pattern {
    /// Get the span of this pattern.
    #[must_use]
    pub fn span(&self) -> &Span {
        match self {
            Pattern::Wildcard { span }
            | Pattern::Variant { span, .. }
            | Pattern::Literal { span, .. }
            | Pattern::Binding { span, .. }
            | Pattern::Tuple { span, .. } => span,
        }
    }
}

// =============================================================================
// Operators
// =============================================================================

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    // Arithmetic
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%` (remainder/modulo)
    Rem,

    // Comparison
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,

    // Logical
    /// `&&`
    And,
    /// `||`
    Or,

    // String
    /// `++` (string concatenation)
    Concat,
}

impl BinOp {
    /// Get the precedence of this operator (higher = binds tighter).
    #[must_use]
    pub fn precedence(self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Ne => 3,
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => 4,
            BinOp::Concat => 5,
            BinOp::Add | BinOp::Sub => 6,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 7,
        }
    }

    /// Check if this operator is left-associative.
    #[must_use]
    pub fn is_left_assoc(self) -> bool {
        // All our operators are left-associative
        true
    }
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Rem => write!(f, "%"),
            BinOp::Eq => write!(f, "=="),
            BinOp::Ne => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Gt => write!(f, ">"),
            BinOp::Le => write!(f, "<="),
            BinOp::Ge => write!(f, ">="),
            BinOp::And => write!(f, "&&"),
            BinOp::Or => write!(f, "||"),
            BinOp::Concat => write!(f, "++"),
        }
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// `-` (negation)
    Neg,
    /// `!` (logical not)
    Not,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
        }
    }
}

// =============================================================================
// Literals
// =============================================================================

/// A literal value.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Integer literal: `42`, `-7`
    Int(i64),
    /// Float literal: `3.14`, `-0.5`
    Float(f64),
    /// Boolean literal: `true`, `false`
    Bool(bool),
    /// String literal: `"hello"`
    String(String),
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::Int(n) => write!(f, "{n}"),
            Literal::Float(n) => write!(f, "{n}"),
            Literal::Bool(b) => write!(f, "{b}"),
            Literal::String(s) => write!(f, "\"{s}\""),
        }
    }
}

// =============================================================================
// String templates (for interpolation)
// =============================================================================

/// A string template that may contain interpolations.
///
/// For example, `"Hello, {name}!"` becomes:
/// ```text
/// StringTemplate {
///     parts: [
///         StringPart::Literal("Hello, "),
///         StringPart::Interpolation(Ident("name")),
///         StringPart::Literal("!"),
///     ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StringTemplate {
    /// The parts of the template.
    pub parts: Vec<StringPart>,
    /// Span covering the entire template string.
    pub span: Span,
}

impl StringTemplate {
    /// Create a simple template with no interpolations.
    #[must_use]
    pub fn literal(s: String, span: Span) -> Self {
        Self {
            parts: vec![StringPart::Literal(s)],
            span,
        }
    }

    /// Check if this template has any interpolations.
    #[must_use]
    pub fn has_interpolations(&self) -> bool {
        self.parts
            .iter()
            .any(|p| matches!(p, StringPart::Interpolation(_)))
    }

    /// Get all interpolation expressions.
    pub fn interpolations(&self) -> impl Iterator<Item = &Expr> {
        self.parts.iter().filter_map(|p| match p {
            StringPart::Interpolation(expr) => Some(expr.as_ref()),
            StringPart::Literal(_) => None,
        })
    }
}

impl fmt::Display for StringTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;
        for part in &self.parts {
            match part {
                StringPart::Literal(s) => write!(f, "{s}")?,
                StringPart::Interpolation(_) => write!(f, "{{...}}")?,
            }
        }
        write!(f, "\"")
    }
}

/// A part of a string template.
#[derive(Debug, Clone, PartialEq)]
pub enum StringPart {
    /// A literal string segment.
    Literal(String),
    /// An interpolated expression: `{ident}`, `{a + b}`, `{foo()}`
    Interpolation(Box<Expr>),
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binop_precedence() {
        // Mul/Div > Add/Sub > Comparison > And > Or
        assert!(BinOp::Mul.precedence() > BinOp::Add.precedence());
        assert!(BinOp::Add.precedence() > BinOp::Lt.precedence());
        assert!(BinOp::Lt.precedence() > BinOp::And.precedence());
        assert!(BinOp::And.precedence() > BinOp::Or.precedence());
    }

    #[test]
    fn binop_display() {
        assert_eq!(format!("{}", BinOp::Add), "+");
        assert_eq!(format!("{}", BinOp::Eq), "==");
        assert_eq!(format!("{}", BinOp::Concat), "++");
        assert_eq!(format!("{}", BinOp::And), "&&");
    }

    #[test]
    fn unaryop_display() {
        assert_eq!(format!("{}", UnaryOp::Neg), "-");
        assert_eq!(format!("{}", UnaryOp::Not), "!");
    }

    #[test]
    fn literal_display() {
        assert_eq!(format!("{}", Literal::Int(42)), "42");
        assert_eq!(format!("{}", Literal::Float(3.14)), "3.14");
        assert_eq!(format!("{}", Literal::Bool(true)), "true");
        assert_eq!(format!("{}", Literal::String("hello".into())), "\"hello\"");
    }

    #[test]
    fn event_kind_display() {
        assert_eq!(format!("{}", EventKind::Start), "start");
        assert_eq!(format!("{}", EventKind::Stop), "stop");

        let msg = EventKind::Message {
            param_name: Ident::dummy("msg"),
            param_ty: TypeExpr::String,
        };
        assert_eq!(format!("{msg}"), "message(msg: String)");
    }

    #[test]
    fn string_template_literal() {
        let template = StringTemplate::literal("hello".into(), Span::dummy());
        assert!(!template.has_interpolations());
        assert_eq!(format!("{template}"), "\"hello\"");
    }

    #[test]
    fn string_template_with_interpolation() {
        let template = StringTemplate {
            parts: vec![
                StringPart::Literal("Hello, ".into()),
                StringPart::Interpolation(Box::new(Expr::Var {
                    name: Ident::dummy("name"),
                    span: Span::dummy(),
                })),
                StringPart::Literal("!".into()),
            ],
            span: Span::dummy(),
        };
        assert!(template.has_interpolations());
        assert_eq!(format!("{template}"), "\"Hello, {...}!\"");

        let interps: Vec<_> = template.interpolations().collect();
        assert_eq!(interps.len(), 1);
        // Verify it's a Var expression with name "name"
        if let Expr::Var { name, .. } = interps[0] {
            assert_eq!(name.name, "name");
        } else {
            panic!("Expected Var expression");
        }
    }

    #[test]
    fn expr_span() {
        let span = Span::dummy();
        let expr = Expr::Literal {
            value: Literal::Int(42),
            span: span.clone(),
        };
        assert_eq!(expr.span(), &span);
    }

    #[test]
    fn stmt_span() {
        let span = Span::dummy();
        let stmt = Stmt::Return {
            value: None,
            span: span.clone(),
        };
        assert_eq!(stmt.span(), &span);
    }
}
