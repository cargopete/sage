//! Error types for the semantic checker.

use miette::{Diagnostic, SourceSpan};
use sage_types::Span;
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
        #[label("expected List<T>")]
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
        help("add an `on message(x: T)` handler")
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
        help("entry agents cannot have beliefs since there's no way to initialize them")
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
        help("add `pub` to make it public")
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
        help("field access is only valid on record types")
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
        help("add a wildcard `_` pattern or cover all variants")
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
    pub fn missing_field(
        field: impl Into<String>,
        record: impl Into<String>,
        span: &Span,
    ) -> Self {
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
}
