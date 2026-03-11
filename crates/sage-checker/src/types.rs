//! Type representation for the type checker.
//!
//! This module defines the internal type representation used during type checking,
//! which is distinct from the syntactic `TypeExpr` in the AST.

use std::fmt;

/// A resolved type in the Sage type system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// 64-bit signed integer.
    Int,
    /// 64-bit floating point.
    Float,
    /// Boolean.
    Bool,
    /// UTF-8 string.
    String,
    /// Unit type (void equivalent).
    Unit,
    /// Homogeneous list.
    List(Box<Type>),
    /// Optional value.
    Option(Box<Type>),
    /// Result of an LLM inference call.
    Inferred(Box<Type>),
    /// Handle to a running agent.
    Agent(String),
    /// An error type used when type checking fails.
    /// Propagates through expressions to avoid cascading errors.
    Error,
}

impl Type {
    /// Check if this type is numeric (Int or Float).
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }

    /// Check if this type is an error type.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }

    /// Unwrap an Inferred type to get the inner type.
    /// For non-Inferred types, returns the type itself.
    #[must_use]
    pub fn unwrap_inferred(&self) -> &Type {
        match self {
            Type::Inferred(inner) => inner.unwrap_inferred(),
            other => other,
        }
    }

    /// Get the element type if this is a List, otherwise None.
    #[must_use]
    pub fn list_element(&self) -> Option<&Type> {
        match self {
            Type::List(elem) => Some(elem),
            _ => None,
        }
    }

    /// Get the agent name if this is an Agent type, otherwise None.
    #[must_use]
    pub fn agent_name(&self) -> Option<&str> {
        match self {
            Type::Agent(name) => Some(name),
            _ => None,
        }
    }

    /// Check if two types are compatible for assignment/comparison.
    /// Inferred<T> is compatible with T.
    #[must_use]
    pub fn is_compatible_with(&self, other: &Type) -> bool {
        if self == other {
            return true;
        }
        // Error types are compatible with everything to avoid cascading errors
        if self.is_error() || other.is_error() {
            return true;
        }
        // Inferred<T> is compatible with T
        match (self, other) {
            (Type::Inferred(inner), other) | (other, Type::Inferred(inner)) => {
                inner.as_ref().is_compatible_with(other)
            }
            _ => false,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "Int"),
            Type::Float => write!(f, "Float"),
            Type::Bool => write!(f, "Bool"),
            Type::String => write!(f, "String"),
            Type::Unit => write!(f, "Unit"),
            Type::List(elem) => write!(f, "List<{elem}>"),
            Type::Option(inner) => write!(f, "Option<{inner}>"),
            Type::Inferred(inner) => write!(f, "Inferred<{inner}>"),
            Type::Agent(name) => write!(f, "Agent<{name}>"),
            Type::Error => write!(f, "<error>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_display() {
        assert_eq!(Type::Int.to_string(), "Int");
        assert_eq!(
            Type::List(Box::new(Type::String)).to_string(),
            "List<String>"
        );
        assert_eq!(
            Type::Inferred(Box::new(Type::String)).to_string(),
            "Inferred<String>"
        );
        assert_eq!(Type::Agent("Foo".to_string()).to_string(), "Agent<Foo>");
    }

    #[test]
    fn type_is_numeric() {
        assert!(Type::Int.is_numeric());
        assert!(Type::Float.is_numeric());
        assert!(!Type::String.is_numeric());
        assert!(!Type::Bool.is_numeric());
    }

    #[test]
    fn type_unwrap_inferred() {
        let t = Type::Inferred(Box::new(Type::String));
        assert_eq!(t.unwrap_inferred(), &Type::String);

        let nested = Type::Inferred(Box::new(Type::Inferred(Box::new(Type::Int))));
        assert_eq!(nested.unwrap_inferred(), &Type::Int);

        assert_eq!(Type::Int.unwrap_inferred(), &Type::Int);
    }

    #[test]
    fn type_compatibility() {
        assert!(Type::Int.is_compatible_with(&Type::Int));
        assert!(!Type::Int.is_compatible_with(&Type::String));

        // Inferred<T> is compatible with T
        let inferred_string = Type::Inferred(Box::new(Type::String));
        assert!(inferred_string.is_compatible_with(&Type::String));
        assert!(Type::String.is_compatible_with(&inferred_string));

        // Error is compatible with everything
        assert!(Type::Error.is_compatible_with(&Type::Int));
        assert!(Type::Int.is_compatible_with(&Type::Error));
    }
}
