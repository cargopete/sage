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
    /// User-defined type (record or enum) by name.
    Named(String),
    /// Function type: parameter types and return type.
    Fn(Vec<Type>, Box<Type>),
    /// Map type: `Map<K, V>`.
    Map(Box<Type>, Box<Type>),
    /// Tuple type: `(A, B, C)`.
    Tuple(Vec<Type>),
    /// Result type: `Result<T, E>`.
    Result(Box<Type>, Box<Type>),
    /// Never type: expression that never returns (e.g., `fail`).
    /// Compatible with any type since it diverges.
    Never,
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

    /// Get the key and value types if this is a Map, otherwise None.
    #[must_use]
    pub fn map_key_value(&self) -> Option<(&Type, &Type)> {
        match self {
            Type::Map(key, value) => Some((key, value)),
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
        // Never type is compatible with everything (divergent expression)
        if matches!(self, Type::Never) || matches!(other, Type::Never) {
            return true;
        }
        match (self, other) {
            // Inferred<T> is compatible with T
            (Type::Inferred(inner), other) | (other, Type::Inferred(inner)) => {
                inner.as_ref().is_compatible_with(other)
            }
            // Function types are compatible if params and return types are pairwise compatible
            (Type::Fn(params1, ret1), Type::Fn(params2, ret2)) => {
                if params1.len() != params2.len() {
                    return false;
                }
                params1
                    .iter()
                    .zip(params2.iter())
                    .all(|(p1, p2)| p1.is_compatible_with(p2))
                    && ret1.is_compatible_with(ret2)
            }
            // Map types are compatible if key and value types are compatible
            (Type::Map(k1, v1), Type::Map(k2, v2)) => {
                k1.is_compatible_with(k2) && v1.is_compatible_with(v2)
            }
            // Tuple types are compatible if they have the same arity and element types are compatible
            (Type::Tuple(elems1), Type::Tuple(elems2)) => {
                if elems1.len() != elems2.len() {
                    return false;
                }
                elems1
                    .iter()
                    .zip(elems2.iter())
                    .all(|(e1, e2)| e1.is_compatible_with(e2))
            }
            // Result types are compatible if ok and err types are compatible
            (Type::Result(ok1, err1), Type::Result(ok2, err2)) => {
                ok1.is_compatible_with(ok2) && err1.is_compatible_with(err2)
            }
            _ => false,
        }
    }

    /// Get the parameter types if this is a function type.
    #[must_use]
    pub fn fn_params(&self) -> Option<&[Type]> {
        match self {
            Type::Fn(params, _) => Some(params),
            _ => None,
        }
    }

    /// Get the return type if this is a function type.
    #[must_use]
    pub fn fn_return(&self) -> Option<&Type> {
        match self {
            Type::Fn(_, ret) => Some(ret),
            _ => None,
        }
    }

    /// Check if this is a function type.
    #[must_use]
    pub fn is_fn(&self) -> bool {
        matches!(self, Type::Fn(_, _))
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
            Type::Named(name) => write!(f, "{name}"),
            Type::Fn(params, ret) => {
                write!(f, "Fn(")?;
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{param}")?;
                }
                write!(f, ") -> {ret}")
            }
            Type::Map(key, value) => write!(f, "Map<{key}, {value}>"),
            Type::Tuple(elems) => {
                write!(f, "(")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{elem}")?;
                }
                write!(f, ")")
            }
            Type::Result(ok, err) => write!(f, "Result<{ok}, {err}>"),
            Type::Never => write!(f, "Never"),
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

    #[test]
    fn fn_type_display() {
        let fn_type = Type::Fn(vec![Type::Int], Box::new(Type::Bool));
        assert_eq!(fn_type.to_string(), "Fn(Int) -> Bool");

        let fn_type = Type::Fn(vec![Type::String, Type::Int], Box::new(Type::Unit));
        assert_eq!(fn_type.to_string(), "Fn(String, Int) -> Unit");

        let fn_type = Type::Fn(vec![], Box::new(Type::String));
        assert_eq!(fn_type.to_string(), "Fn() -> String");
    }

    #[test]
    fn fn_type_compatibility() {
        let fn1 = Type::Fn(vec![Type::Int], Box::new(Type::Bool));
        let fn2 = Type::Fn(vec![Type::Int], Box::new(Type::Bool));
        let fn3 = Type::Fn(vec![Type::String], Box::new(Type::Bool));
        let fn4 = Type::Fn(vec![Type::Int, Type::Int], Box::new(Type::Bool));

        // Same types are compatible
        assert!(fn1.is_compatible_with(&fn2));

        // Different param types are not compatible
        assert!(!fn1.is_compatible_with(&fn3));

        // Different param count is not compatible
        assert!(!fn1.is_compatible_with(&fn4));

        // Fn types are not compatible with non-Fn types
        assert!(!fn1.is_compatible_with(&Type::Int));
    }

    #[test]
    fn fn_type_accessors() {
        let fn_type = Type::Fn(vec![Type::Int, Type::String], Box::new(Type::Bool));

        assert!(fn_type.is_fn());
        assert_eq!(fn_type.fn_params(), Some(&[Type::Int, Type::String][..]));
        assert_eq!(fn_type.fn_return(), Some(&Type::Bool));

        // Non-Fn types return None
        assert!(!Type::Int.is_fn());
        assert_eq!(Type::Int.fn_params(), None);
        assert_eq!(Type::Int.fn_return(), None);
    }
}
