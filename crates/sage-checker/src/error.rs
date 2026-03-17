//! Error types for the semantic checker.

use miette::{Diagnostic, SourceSpan};
use sage_parser::Span;
use thiserror::Error;

/// Convert a Sage span to a miette `SourceSpan`.
fn to_source_span(span: &Span) -> SourceSpan {
    (span.start, span.end - span.start).into()
}

/// A semantic error detected during name resolution or type checking.
#[derive(Debug, Clone, Error, Diagnostic)]
pub enum CheckError {
    // =========================================================================
    // Name resolution errors
    // =========================================================================
    #[error("undefined variable `{name}`")]
    #[diagnostic(code(sage::undefined_variable))]
    UndefinedVariable {
        name: String,
        #[label("not found in this scope")]
        span: SourceSpan,
    },

    #[error("undefined agent `{name}`")]
    #[diagnostic(code(sage::undefined_agent))]
    UndefinedAgent {
        name: String,
        #[label("no agent with this name")]
        span: SourceSpan,
    },

    #[error("undefined function `{name}`")]
    #[diagnostic(code(sage::undefined_function))]
    UndefinedFunction {
        name: String,
        #[label("no function with this name")]
        span: SourceSpan,
    },

    #[error("undefined belief `{name}`")]
    #[diagnostic(code(sage::undefined_belief))]
    UndefinedBelief {
        name: String,
        #[label("agent has no belief with this name")]
        span: SourceSpan,
    },

    #[error("duplicate definition of `{name}`")]
    #[diagnostic(code(sage::duplicate_definition))]
    DuplicateDefinition {
        name: String,
        #[label("already defined")]
        span: SourceSpan,
    },

    #[error("`self` used outside of agent context")]
    #[diagnostic(code(sage::self_outside_agent))]
    SelfOutsideAgent {
        #[label("not inside an agent handler")]
        span: SourceSpan,
    },

    // =========================================================================
    // Type errors
    // =========================================================================
    #[error("type mismatch: expected `{expected}`, found `{found}`")]
    #[diagnostic(code(sage::type_mismatch))]
    TypeMismatch {
        expected: String,
        found: String,
        #[label("expected `{expected}`")]
        span: SourceSpan,
    },

    #[error("cannot apply operator `{op}` to types `{left}` and `{right}`")]
    #[diagnostic(code(sage::invalid_binary_op))]
    InvalidBinaryOp {
        op: String,
        left: String,
        right: String,
        #[label("invalid operand types")]
        span: SourceSpan,
    },

    #[error("cannot apply operator `{op}` to type `{operand}`")]
    #[diagnostic(code(sage::invalid_unary_op))]
    InvalidUnaryOp {
        op: String,
        operand: String,
        #[label("invalid operand type")]
        span: SourceSpan,
    },

    #[error("cannot iterate over type `{ty}`")]
    #[diagnostic(code(sage::not_iterable))]
    NotIterable {
        ty: String,
        #[label("expected List<T> or Map<K, V>")]
        span: SourceSpan,
    },

    #[error("cannot await non-agent type `{ty}`")]
    #[diagnostic(code(sage::await_non_agent))]
    AwaitNonAgent {
        ty: String,
        #[label("expected Agent<T>")]
        span: SourceSpan,
    },

    #[error("cannot send to non-agent type `{ty}`")]
    #[diagnostic(code(sage::send_non_agent))]
    SendNonAgent {
        ty: String,
        #[label("expected Agent<T>")]
        span: SourceSpan,
    },

    #[error("function `{name}` expects {expected} arguments, found {found}")]
    #[diagnostic(code(sage::wrong_arg_count))]
    WrongArgCount {
        name: String,
        expected: usize,
        found: usize,
        #[label("wrong number of arguments")]
        span: SourceSpan,
    },

    #[error("missing belief initialization: `{name}`")]
    #[diagnostic(code(sage::missing_belief_init))]
    MissingBeliefInit {
        name: String,
        #[label("agent requires this belief")]
        span: SourceSpan,
    },

    #[error("unknown field `{name}` in agent initialization")]
    #[diagnostic(code(sage::unknown_field))]
    UnknownField {
        name: String,
        #[label("not a belief of this agent")]
        span: SourceSpan,
    },

    #[error("agent `{name}` has no message handler")]
    #[diagnostic(
        code(sage::no_message_handler),
        help("Oswyn suggests: add an `on message(x: T)` handler")
    )]
    NoMessageHandler {
        name: String,
        #[label("sending to this agent")]
        span: SourceSpan,
    },

    // =========================================================================
    // Entry agent errors
    // =========================================================================
    #[error("entry agent `{name}` must have no beliefs")]
    #[diagnostic(
        code(sage::entry_agent_has_beliefs),
        help("Oswyn explains: entry agents cannot have beliefs since there's no way to initialize them")
    )]
    EntryAgentHasBeliefs {
        name: String,
        #[label("this agent has beliefs")]
        span: SourceSpan,
    },

    #[error("entry agent `{name}` must have an `on start` handler")]
    #[diagnostic(code(sage::entry_agent_no_start))]
    EntryAgentNoStart {
        name: String,
        #[label("add an `on start` handler")]
        span: SourceSpan,
    },

    // =========================================================================
    // Module errors
    // =========================================================================
    #[error("module `{path}` not found")]
    #[diagnostic(code(sage::module_not_found))]
    ModuleNotFound {
        path: String,
        #[label("no module at this path")]
        span: SourceSpan,
    },

    #[error("item `{name}` in module `{module}` is private")]
    #[diagnostic(
        code(sage::private_item),
        help("Oswyn suggests: add `pub` to make it public")
    )]
    PrivateItem {
        name: String,
        module: String,
        #[label("cannot access private item")]
        span: SourceSpan,
    },

    #[error("item `{name}` not found in module `{module}`")]
    #[diagnostic(code(sage::item_not_found))]
    ItemNotFound {
        name: String,
        module: String,
        #[label("no such item")]
        span: SourceSpan,
    },

    // =========================================================================
    // User-defined type errors
    // =========================================================================
    #[error("undefined type `{name}`")]
    #[diagnostic(code(sage::undefined_type))]
    UndefinedType {
        name: String,
        #[label("no record or enum with this name")]
        span: SourceSpan,
    },

    #[error("missing field `{field}` in record `{record}`")]
    #[diagnostic(code(sage::missing_field))]
    MissingField {
        field: String,
        record: String,
        #[label("record construction incomplete")]
        span: SourceSpan,
    },

    #[error("cannot access field on type `{ty}`")]
    #[diagnostic(
        code(sage::field_access_on_non_record),
        help("Oswyn explains: field access is only valid on record types")
    )]
    FieldAccessOnNonRecord {
        ty: String,
        #[label("not a record type")]
        span: SourceSpan,
    },

    #[error("undefined field `{field}` in record `{record}`")]
    #[diagnostic(code(sage::undefined_record_field))]
    UndefinedRecordField {
        field: String,
        record: String,
        #[label("record has no field with this name")]
        span: SourceSpan,
    },

    #[error("undefined variant `{variant}` in enum `{enum_name}`")]
    #[diagnostic(code(sage::undefined_enum_variant))]
    UndefinedEnumVariant {
        variant: String,
        enum_name: String,
        #[label("enum has no variant with this name")]
        span: SourceSpan,
    },

    #[error("non-exhaustive match: missing patterns")]
    #[diagnostic(
        code(sage::non_exhaustive_match),
        help("Oswyn suggests: add a wildcard `_` pattern or cover all variants")
    )]
    NonExhaustiveMatch {
        #[label("match is not exhaustive")]
        span: SourceSpan,
    },

    // =========================================================================
    // Warnings
    // =========================================================================
    #[error("unused belief `{name}`")]
    #[diagnostic(code(sage::unused_belief), severity(Warning))]
    UnusedBelief {
        name: String,
        #[label("declared but never accessed")]
        span: SourceSpan,
    },

    // =========================================================================
    // Misc errors
    // =========================================================================
    #[error("return statement outside of function")]
    #[diagnostic(code(sage::return_outside_function))]
    ReturnOutsideFunction {
        #[label("not inside a function")]
        span: SourceSpan,
    },

    #[error("condition must be Bool, found `{found}`")]
    #[diagnostic(code(sage::non_bool_condition))]
    NonBoolCondition {
        found: String,
        #[label("expected Bool")]
        span: SourceSpan,
    },

    #[error("`break` outside of loop")]
    #[diagnostic(code(sage::break_outside_loop))]
    BreakOutsideLoop {
        #[label("not inside a loop")]
        span: SourceSpan,
    },

    #[error("`receive()` called in agent `{name}` which has no `receives` declaration")]
    #[diagnostic(
        code(sage::receive_without_receives),
        help("Oswyn suggests: add `receives MsgType` to agent `{name}`")
    )]
    ReceiveWithoutReceives {
        name: String,
        #[label("no receives clause")]
        span: SourceSpan,
    },

    #[error("`receive()` called outside of agent")]
    #[diagnostic(code(sage::receive_outside_agent))]
    ReceiveOutsideAgent {
        #[label("not inside an agent handler")]
        span: SourceSpan,
    },

    // =========================================================================
    // RFC-0007: Error handling errors
    // =========================================================================
    #[error("unhandled error: calling fallible function `{name}` without `try` or `catch`")]
    #[diagnostic(
        code(sage::E013),
        help("Oswyn suggests: use `try {name}(...)` to propagate, or `catch` to handle it")
    )]
    UnhandledError {
        name: String,
        #[label("fallible function called without error handling")]
        span: SourceSpan,
    },

    #[error("`try` used in non-fallible context")]
    #[diagnostic(
        code(sage::E014),
        help("Oswyn suggests: add `fails` to the function, or use `catch` instead")
    )]
    TryInNonFallible {
        #[label("cannot propagate errors from here")]
        span: SourceSpan,
    },

    #[error("catch recovery type mismatch: expected `{expected}`, found `{found}`")]
    #[diagnostic(code(sage::E015))]
    CatchTypeMismatch {
        expected: String,
        found: String,
        #[label("recovery expression must match fallible expression type")]
        span: SourceSpan,
    },

    #[error("unhandled propagated error in agent without `on error` handler")]
    #[diagnostic(
        code(sage::E016),
        help("Oswyn suggests: add `on error(e) {{ ... }}` handler, or use `catch` instead")
    )]
    MissingErrorHandler {
        agent: String,
        #[label("`try` propagates errors but agent has no error handler")]
        span: SourceSpan,
    },

    #[error("`yield` cannot be called in `on stop` handler")]
    #[diagnostic(
        code(sage::E017),
        help("Oswyn suggests: remove the yield call - on stop is for cleanup only")
    )]
    EmitInStopHandler {
        #[label("yield not allowed in stop handler")]
        span: SourceSpan,
    },

    // =========================================================================
    // RFC-0009: Closure errors
    // =========================================================================
    #[error("closure parameter `{name}` requires type annotation")]
    #[diagnostic(
        code(sage::E040),
        help("Oswyn suggests: add a type annotation: `|{name}: Type|`")
    )]
    ClosureParamNeedsType {
        name: String,
        #[label("type annotation required")]
        span: SourceSpan,
    },

    // =========================================================================
    // RFC-0010: Maps, tuples, and related errors
    // =========================================================================
    #[error("tuple arity mismatch: expected {expected} elements, found {found}")]
    #[diagnostic(code(sage::E028))]
    TupleArityMismatch {
        expected: usize,
        found: usize,
        #[label("expected {expected} elements")]
        span: SourceSpan,
    },

    #[error("tuple index {index} out of bounds for tuple with {len} elements")]
    #[diagnostic(code(sage::E034))]
    TupleIndexOutOfBounds {
        index: usize,
        len: usize,
        #[label("index out of bounds")]
        span: SourceSpan,
    },

    #[error("cannot use tuple index on non-tuple type `{ty}`")]
    #[diagnostic(
        code(sage::E049),
        help("Oswyn suggests: tuple index syntax (.0, .1, etc.) only works on tuple types")
    )]
    TupleIndexOnNonTuple {
        ty: String,
        #[label("expected a tuple")]
        span: SourceSpan,
    },

    #[error("empty map literal requires type annotation")]
    #[diagnostic(
        code(sage::E025),
        help("Oswyn suggests: use `let m: Map<K, V> = {{}}` or provide at least one entry")
    )]
    EmptyMapLiteral {
        #[label("cannot infer key/value types")]
        span: SourceSpan,
    },

    // =========================================================================
    // RFC-0011: Tool errors
    // =========================================================================
    #[error("tool `{tool}` has no function `{function}`")]
    #[diagnostic(
        code(sage::E036),
        help("Oswyn suggests: check the tool declaration for available functions")
    )]
    UndefinedToolFunction {
        tool: String,
        function: String,
        #[label("no such function on tool `{tool}`")]
        span: SourceSpan,
    },

    #[error("agent uses tool `{tool}` without declaring `use {tool}`")]
    #[diagnostic(
        code(sage::E038),
        help("Oswyn suggests: add `use {tool}` at the start of the agent body")
    )]
    UndeclaredToolUse {
        tool: String,
        #[label("tool not declared in agent")]
        span: SourceSpan,
    },

    #[error("tool function `{tool}.{function}` expects {expected} arguments, found {found}")]
    #[diagnostic(code(sage::E039))]
    ToolCallArity {
        tool: String,
        function: String,
        expected: usize,
        found: usize,
        #[label("wrong number of arguments")]
        span: SourceSpan,
    },

    // =========================================================================
    // RFC-0012: Testing framework errors
    // =========================================================================
    #[error("test constructs are only available in `_test.sg` files")]
    #[diagnostic(
        code(sage::E050),
        help("Oswyn explains: `test` blocks and assertions only work in test files")
    )]
    TestOutsideTestFile {
        #[label("not in a test file")]
        span: SourceSpan,
    },

    #[error("`run` statement not allowed in test files")]
    #[diagnostic(
        code(sage::E051),
        help("Oswyn suggests: test files use `test` blocks, not `run` statements")
    )]
    RunInTestFile {
        #[label("cannot use `run` in test file")]
        span: SourceSpan,
    },

    #[error("duplicate test name `{name}`")]
    #[diagnostic(code(sage::E055))]
    DuplicateTestName {
        name: String,
        #[label("test with this name already exists")]
        span: SourceSpan,
    },

    #[error("`mock divine` is only valid inside a `test` block")]
    #[diagnostic(code(sage::E056))]
    MockInferOutsideTest {
        #[label("must be inside a test block")]
        span: SourceSpan,
    },

    #[error("`mock tool` is only valid inside a `test` block")]
    #[diagnostic(code(sage::E057))]
    MockToolOutsideTest {
        #[label("must be inside a test block")]
        span: SourceSpan,
    },

    #[error("`fail` argument must be a String")]
    #[diagnostic(code(sage::E058))]
    MockFailNotString {
        found: String,
        #[label("expected String, found `{found}`")]
        span: SourceSpan,
    },

    // =========================================================================
    // Persistence errors (v2.0)
    // =========================================================================
    #[error("`@persistent` field `{name}` has non-serializable type `{ty}`")]
    #[diagnostic(
        code(sage::E052),
        help("Eskar notes: persistent fields must be serializable (primitives, List, Option, records, enums)")
    )]
    PersistentFieldNotSerializable {
        name: String,
        ty: String,
        #[label("function types and agent handles cannot be persisted")]
        span: SourceSpan,
    },

    #[error("`on waking` handler in agent with no `@persistent` fields")]
    #[diagnostic(
        code(sage::W006),
        severity(warning),
        help("Lyr observes: `on waking` is for loading persisted state — without `@persistent` fields, it serves no purpose")
    )]
    WakingWithoutPersistentFields {
        agent_name: String,
        #[label("this agent has no persistent fields")]
        span: SourceSpan,
    },

    // =========================================================================
    // Generics errors (RFC-0015)
    // =========================================================================
    #[error("{message}")]
    #[diagnostic(code(sage::E100))]
    GenericError {
        message: String,
        #[label("generic error")]
        span: SourceSpan,
    },

    // =========================================================================
    // Supervision tree errors (v2)
    // =========================================================================
    #[error("supervisor `{name}` has no children")]
    #[diagnostic(
        code(sage::E060),
        help("Oswyn explains: a supervisor must have at least one child to supervise")
    )]
    SupervisorNoChildren {
        name: String,
        #[label("add at least one child")]
        span: SourceSpan,
    },

    #[error("child agent `{child}` does not exist")]
    #[diagnostic(code(sage::E061))]
    SupervisorChildNotFound {
        child: String,
        #[label("no agent with this name")]
        span: SourceSpan,
    },

    #[error("child `{child}` is missing required belief `{belief}`")]
    #[diagnostic(code(sage::E062))]
    SupervisorChildMissingBelief {
        child: String,
        belief: String,
        #[label("belief must be initialized")]
        span: SourceSpan,
    },

    #[error("supervisor nesting depth exceeds maximum ({depth} > {max})")]
    #[diagnostic(
        code(sage::E063),
        help("Oswyn suggests: flatten your supervision tree or reconsider architecture")
    )]
    SupervisorNestingTooDeep {
        depth: usize,
        max: usize,
        #[label("nesting too deep")]
        span: SourceSpan,
    },

    #[error("child `{child}` has Permanent restart but no @persistent fields")]
    #[diagnostic(
        code(sage::W004),
        severity(warning),
        help("Lyr observes: Permanent restart without persistence means state is lost on restart")
    )]
    PermanentWithoutPersistence {
        child: String,
        #[label("consider adding @persistent fields or using Transient restart")]
        span: SourceSpan,
    },
}

impl CheckError {
    /// Create an undefined variable error.
    pub fn undefined_variable(name: impl Into<String>, span: &Span) -> Self {
        Self::UndefinedVariable {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create an undefined agent error.
    pub fn undefined_agent(name: impl Into<String>, span: &Span) -> Self {
        Self::UndefinedAgent {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create an undefined function error.
    pub fn undefined_function(name: impl Into<String>, span: &Span) -> Self {
        Self::UndefinedFunction {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create an undefined belief error.
    pub fn undefined_belief(name: impl Into<String>, span: &Span) -> Self {
        Self::UndefinedBelief {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a duplicate definition error.
    pub fn duplicate_definition(name: impl Into<String>, span: &Span) -> Self {
        Self::DuplicateDefinition {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a self-outside-agent error.
    #[must_use]
    pub fn self_outside_agent(span: &Span) -> Self {
        Self::SelfOutsideAgent {
            span: to_source_span(span),
        }
    }

    /// Create a type mismatch error.
    pub fn type_mismatch(
        expected: impl Into<String>,
        found: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::TypeMismatch {
            expected: expected.into(),
            found: found.into(),
            span: to_source_span(span),
        }
    }

    /// Create an invalid binary operator error.
    pub fn invalid_binary_op(
        op: impl Into<String>,
        left: impl Into<String>,
        right: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::InvalidBinaryOp {
            op: op.into(),
            left: left.into(),
            right: right.into(),
            span: to_source_span(span),
        }
    }

    /// Create an invalid unary operator error.
    pub fn invalid_unary_op(
        op: impl Into<String>,
        operand: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::InvalidUnaryOp {
            op: op.into(),
            operand: operand.into(),
            span: to_source_span(span),
        }
    }

    /// Create a not-iterable error.
    pub fn not_iterable(ty: impl Into<String>, span: &Span) -> Self {
        Self::NotIterable {
            ty: ty.into(),
            span: to_source_span(span),
        }
    }

    /// Create an await-non-agent error.
    pub fn await_non_agent(ty: impl Into<String>, span: &Span) -> Self {
        Self::AwaitNonAgent {
            ty: ty.into(),
            span: to_source_span(span),
        }
    }

    /// Create a send-non-agent error.
    pub fn send_non_agent(ty: impl Into<String>, span: &Span) -> Self {
        Self::SendNonAgent {
            ty: ty.into(),
            span: to_source_span(span),
        }
    }

    /// Create a wrong argument count error.
    pub fn wrong_arg_count(
        name: impl Into<String>,
        expected: usize,
        found: usize,
        span: &Span,
    ) -> Self {
        Self::WrongArgCount {
            name: name.into(),
            expected,
            found,
            span: to_source_span(span),
        }
    }

    /// Create a missing belief initialization error.
    pub fn missing_belief_init(name: impl Into<String>, span: &Span) -> Self {
        Self::MissingBeliefInit {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create an unknown field error.
    pub fn unknown_field(name: impl Into<String>, span: &Span) -> Self {
        Self::UnknownField {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a no-message-handler error.
    pub fn no_message_handler(name: impl Into<String>, span: &Span) -> Self {
        Self::NoMessageHandler {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create an entry-agent-has-beliefs error.
    pub fn entry_agent_has_beliefs(name: impl Into<String>, span: &Span) -> Self {
        Self::EntryAgentHasBeliefs {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create an entry-agent-no-start error.
    pub fn entry_agent_no_start(name: impl Into<String>, span: &Span) -> Self {
        Self::EntryAgentNoStart {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a return-outside-function error.
    #[must_use]
    pub fn return_outside_function(span: &Span) -> Self {
        Self::ReturnOutsideFunction {
            span: to_source_span(span),
        }
    }

    /// Create a non-bool-condition error.
    pub fn non_bool_condition(found: impl Into<String>, span: &Span) -> Self {
        Self::NonBoolCondition {
            found: found.into(),
            span: to_source_span(span),
        }
    }

    /// Create an unused belief warning.
    pub fn unused_belief(name: impl Into<String>, span: &Span) -> Self {
        Self::UnusedBelief {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a module-not-found error.
    pub fn module_not_found(path: impl Into<String>, span: &Span) -> Self {
        Self::ModuleNotFound {
            path: path.into(),
            span: to_source_span(span),
        }
    }

    /// Create a private-item error.
    pub fn private_item(name: impl Into<String>, module: impl Into<String>, span: &Span) -> Self {
        Self::PrivateItem {
            name: name.into(),
            module: module.into(),
            span: to_source_span(span),
        }
    }

    /// Create an item-not-found error.
    pub fn item_not_found(name: impl Into<String>, module: impl Into<String>, span: &Span) -> Self {
        Self::ItemNotFound {
            name: name.into(),
            module: module.into(),
            span: to_source_span(span),
        }
    }

    /// Create an undefined type error.
    pub fn undefined_type(name: impl Into<String>, span: &Span) -> Self {
        Self::UndefinedType {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a missing field error.
    pub fn missing_field(field: impl Into<String>, record: impl Into<String>, span: &Span) -> Self {
        Self::MissingField {
            field: field.into(),
            record: record.into(),
            span: to_source_span(span),
        }
    }

    /// Create a field-access-on-non-record error.
    pub fn field_access_on_non_record(ty: impl Into<String>, span: &Span) -> Self {
        Self::FieldAccessOnNonRecord {
            ty: ty.into(),
            span: to_source_span(span),
        }
    }

    /// Create an undefined record field error.
    pub fn undefined_record_field(
        field: impl Into<String>,
        record: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::UndefinedRecordField {
            field: field.into(),
            record: record.into(),
            span: to_source_span(span),
        }
    }

    /// Create an undefined enum variant error.
    pub fn undefined_enum_variant(
        variant: impl Into<String>,
        enum_name: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::UndefinedEnumVariant {
            variant: variant.into(),
            enum_name: enum_name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a non-exhaustive match error.
    #[must_use]
    pub fn non_exhaustive_match(span: &Span) -> Self {
        Self::NonExhaustiveMatch {
            span: to_source_span(span),
        }
    }

    /// Create a break-outside-loop error.
    #[must_use]
    pub fn break_outside_loop(span: &Span) -> Self {
        Self::BreakOutsideLoop {
            span: to_source_span(span),
        }
    }

    /// Create a receive-without-receives error.
    pub fn receive_without_receives(name: impl Into<String>, span: &Span) -> Self {
        Self::ReceiveWithoutReceives {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a receive-outside-agent error.
    #[must_use]
    pub fn receive_outside_agent(span: &Span) -> Self {
        Self::ReceiveOutsideAgent {
            span: to_source_span(span),
        }
    }

    // =========================================================================
    // RFC-0007: Error handling helpers
    // =========================================================================

    /// Create an unhandled error (E013).
    pub fn unhandled_error(name: impl Into<String>, span: &Span) -> Self {
        Self::UnhandledError {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a try-in-non-fallible error (E014).
    #[must_use]
    pub fn try_in_non_fallible(span: &Span) -> Self {
        Self::TryInNonFallible {
            span: to_source_span(span),
        }
    }

    /// Create a catch type mismatch error (E015).
    pub fn catch_type_mismatch(
        expected: impl Into<String>,
        found: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::CatchTypeMismatch {
            expected: expected.into(),
            found: found.into(),
            span: to_source_span(span),
        }
    }

    /// Create a missing error handler error (E016).
    pub fn missing_error_handler(agent: impl Into<String>, span: &Span) -> Self {
        Self::MissingErrorHandler {
            agent: agent.into(),
            span: to_source_span(span),
        }
    }

    /// Create a yield in stop handler error (E017).
    pub fn yield_in_stop_handler(span: &Span) -> Self {
        Self::EmitInStopHandler {
            span: to_source_span(span),
        }
    }

    // =========================================================================
    // RFC-0009: Closure helpers
    // =========================================================================

    /// Create a closure param needs type error (E040).
    pub fn closure_param_needs_type(name: impl Into<String>, span: &Span) -> Self {
        Self::ClosureParamNeedsType {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    // =========================================================================
    // RFC-0010: Maps, tuples, and related helpers
    // =========================================================================

    /// Create a tuple arity mismatch error (E028).
    #[must_use]
    pub fn tuple_arity_mismatch(expected: usize, found: usize, span: &Span) -> Self {
        Self::TupleArityMismatch {
            expected,
            found,
            span: to_source_span(span),
        }
    }

    /// Create a tuple index out of bounds error (E034).
    #[must_use]
    pub fn tuple_index_out_of_bounds(index: usize, len: usize, span: &Span) -> Self {
        Self::TupleIndexOutOfBounds {
            index,
            len,
            span: to_source_span(span),
        }
    }

    /// Create a tuple index on non-tuple error (E049).
    #[must_use]
    pub fn tuple_index_on_non_tuple(ty: impl Into<String>, span: &Span) -> Self {
        Self::TupleIndexOnNonTuple {
            ty: ty.into(),
            span: to_source_span(span),
        }
    }

    /// Create an empty map literal error (E025).
    #[must_use]
    pub fn empty_map_literal(span: &Span) -> Self {
        Self::EmptyMapLiteral {
            span: to_source_span(span),
        }
    }

    /// Create an undefined tool function error (E036).
    pub fn undefined_tool_function(
        tool: impl Into<String>,
        function: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::UndefinedToolFunction {
            tool: tool.into(),
            function: function.into(),
            span: to_source_span(span),
        }
    }

    /// Create an undeclared tool use error (E038).
    pub fn undeclared_tool_use(tool: impl Into<String>, span: &Span) -> Self {
        Self::UndeclaredToolUse {
            tool: tool.into(),
            span: to_source_span(span),
        }
    }

    /// Create a tool call arity error (E039).
    pub fn tool_call_arity(
        tool: impl Into<String>,
        function: impl Into<String>,
        expected: usize,
        found: usize,
        span: &Span,
    ) -> Self {
        Self::ToolCallArity {
            tool: tool.into(),
            function: function.into(),
            expected,
            found,
            span: to_source_span(span),
        }
    }

    // =========================================================================
    // RFC-0012: Testing framework helpers
    // =========================================================================

    /// Create a test-outside-test-file error (E050).
    #[must_use]
    pub fn test_outside_test_file(span: &Span) -> Self {
        Self::TestOutsideTestFile {
            span: to_source_span(span),
        }
    }

    /// Create a run-in-test-file error (E051).
    #[must_use]
    pub fn run_in_test_file(span: &Span) -> Self {
        Self::RunInTestFile {
            span: to_source_span(span),
        }
    }

    /// Create a duplicate test name error (E055).
    pub fn duplicate_test_name(name: impl Into<String>, span: &Span) -> Self {
        Self::DuplicateTestName {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a mock-divine-outside-test error (E056).
    #[must_use]
    pub fn mock_divine_outside_test(span: &Span) -> Self {
        Self::MockInferOutsideTest {
            span: to_source_span(span),
        }
    }

    /// Create a mock-tool-outside-test error (E057).
    #[must_use]
    pub fn mock_tool_outside_test(span: &Span) -> Self {
        Self::MockToolOutsideTest {
            span: to_source_span(span),
        }
    }

    /// Create a mock-fail-not-string error (E058).
    pub fn mock_fail_not_string(found: impl Into<String>, span: &Span) -> Self {
        Self::MockFailNotString {
            found: found.into(),
            span: to_source_span(span),
        }
    }

    // =========================================================================
    // RFC-0015: Generics helpers
    // =========================================================================

    // =========================================================================
    // Persistence helpers (v2.0)
    // =========================================================================

    /// Create a persistent field not serializable error (E052).
    #[must_use]
    pub fn persistent_field_not_serializable(
        name: impl Into<String>,
        ty: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::PersistentFieldNotSerializable {
            name: name.into(),
            ty: ty.into(),
            span: to_source_span(span),
        }
    }

    /// Create a waking without persistent fields warning (W006).
    #[must_use]
    pub fn waking_without_persistent_fields(
        agent_name: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::WakingWithoutPersistentFields {
            agent_name: agent_name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a generic-related error (E100).
    pub fn generic(message: impl Into<String>, span: &Span) -> Self {
        Self::GenericError {
            message: message.into(),
            span: to_source_span(span),
        }
    }

    // =========================================================================
    // Supervision tree helpers (v2)
    // =========================================================================

    /// Create a supervisor-no-children error (E060).
    pub fn supervisor_no_children(name: impl Into<String>, span: &Span) -> Self {
        Self::SupervisorNoChildren {
            name: name.into(),
            span: to_source_span(span),
        }
    }

    /// Create a supervisor-child-not-found error (E061).
    pub fn supervisor_child_not_found(child: impl Into<String>, span: &Span) -> Self {
        Self::SupervisorChildNotFound {
            child: child.into(),
            span: to_source_span(span),
        }
    }

    /// Create a supervisor-child-missing-belief error (E062).
    pub fn supervisor_child_missing_belief(
        child: impl Into<String>,
        belief: impl Into<String>,
        span: &Span,
    ) -> Self {
        Self::SupervisorChildMissingBelief {
            child: child.into(),
            belief: belief.into(),
            span: to_source_span(span),
        }
    }

    /// Create a supervisor-nesting-too-deep error (E063).
    #[must_use]
    pub fn supervisor_nesting_too_deep(depth: usize, max: usize, span: &Span) -> Self {
        Self::SupervisorNestingTooDeep {
            depth,
            max,
            span: to_source_span(span),
        }
    }

    /// Create a permanent-without-persistence warning (W004).
    pub fn permanent_without_persistence(child: impl Into<String>, span: &Span) -> Self {
        Self::PermanentWithoutPersistence {
            child: child.into(),
            span: to_source_span(span),
        }
    }
}
