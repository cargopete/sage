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
    /// Result of an LLM oracle call.
    Oracle(Box<Type>),
    /// Handle to a running agent.
    Agent(String),
    /// User-defined type (record or enum) by name.
    Named(String),
    /// Generic type with type arguments: `Pair<Int, String>`.
    /// First field is the type name, second is the type arguments.
    Generic(String, Vec<Type>),
    /// Type parameter reference (e.g., `T` in `fn identity<T>(x: T) -> T`).
    /// Used during type checking of generic definitions.
    TypeParam(String),
    /// Function type: parameter types and return type.
    Fn(Vec<Type>, Box<Type>),
    /// Map type: `Map<K, V>`.
    Map(Box<Type>, Box<Type>),
    /// Tuple type: `(A, B, C)`.
    Tuple(Vec<Type>),
    /// Result type: `Result<T, E>`.
    Result(Box<Type>, Box<Type>),
    /// Persisted type for @persistent beliefs: wraps the inner type.
    /// Has `.get()` returning T and `.set(T)` returning Unit.
    Persisted(Box<Type>),
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

    /// Unwrap an Oracle type to get the inner type.
    /// For non-Oracle types, returns the type itself.
    #[must_use]
    pub fn unwrap_oracle(&self) -> &Type {
        match self {
            Type::Oracle(inner) => inner.unwrap_oracle(),
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

    /// Get the inner type if this is an Option, otherwise None.
    #[must_use]
    pub fn option_inner(&self) -> Option<&Type> {
        match self {
            Type::Option(inner) => Some(inner),
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

    /// Check if this is a type parameter.
    #[must_use]
    pub fn is_type_param(&self) -> bool {
        matches!(self, Type::TypeParam(_))
    }

    /// Get the type parameter name if this is a TypeParam.
    #[must_use]
    pub fn type_param_name(&self) -> Option<&str> {
        match self {
            Type::TypeParam(name) => Some(name),
            _ => None,
        }
    }

    /// Check if two types are compatible for assignment/comparison.
    /// Oracle<T> is compatible with T.
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
            // Oracle<T> is compatible with T
            (Type::Oracle(inner), other) | (other, Type::Oracle(inner)) => {
                inner.as_ref().is_compatible_with(other)
            }
            // Type parameters are compatible with any concrete type during inference
            // (actual compatibility is checked during unification)
            (Type::TypeParam(_), _) | (_, Type::TypeParam(_)) => true,
            // Generic types are compatible if names match and all type args are compatible
            (Type::Generic(name1, args1), Type::Generic(name2, args2)) => {
                name1 == name2
                    && args1.len() == args2.len()
                    && args1
                        .iter()
                        .zip(args2.iter())
                        .all(|(a1, a2)| a1.is_compatible_with(a2))
            }
            // Generic<T> is compatible with Named if it has no type args (non-generic)
            (Type::Generic(name1, args), Type::Named(name2))
            | (Type::Named(name2), Type::Generic(name1, args)) => name1 == name2 && args.is_empty(),
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
            // Persisted types are compatible if inner types are compatible
            (Type::Persisted(inner1), Type::Persisted(inner2)) => inner1.is_compatible_with(inner2),
            // List types are compatible if element types are compatible
            (Type::List(elem1), Type::List(elem2)) => elem1.is_compatible_with(elem2),
            // Option types are compatible if inner types are compatible
            (Type::Option(inner1), Type::Option(inner2)) => inner1.is_compatible_with(inner2),
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

    /// Substitute type parameters with concrete types.
    /// `bindings` maps type parameter names to their concrete types.
    #[must_use]
    pub fn substitute(&self, bindings: &std::collections::HashMap<String, Type>) -> Type {
        match self {
            // Type parameter: look up in bindings, or keep as-is if not found
            Type::TypeParam(name) => bindings.get(name).cloned().unwrap_or_else(|| self.clone()),

            // Compound types: substitute recursively
            Type::List(elem) => Type::List(Box::new(elem.substitute(bindings))),
            Type::Option(inner) => Type::Option(Box::new(inner.substitute(bindings))),
            Type::Oracle(inner) => Type::Oracle(Box::new(inner.substitute(bindings))),
            Type::Map(key, value) => Type::Map(
                Box::new(key.substitute(bindings)),
                Box::new(value.substitute(bindings)),
            ),
            Type::Tuple(elems) => {
                Type::Tuple(elems.iter().map(|e| e.substitute(bindings)).collect())
            }
            Type::Result(ok, err) => Type::Result(
                Box::new(ok.substitute(bindings)),
                Box::new(err.substitute(bindings)),
            ),
            Type::Persisted(inner) => Type::Persisted(Box::new(inner.substitute(bindings))),
            Type::Fn(params, ret) => Type::Fn(
                params.iter().map(|p| p.substitute(bindings)).collect(),
                Box::new(ret.substitute(bindings)),
            ),
            Type::Generic(name, args) => Type::Generic(
                name.clone(),
                args.iter().map(|a| a.substitute(bindings)).collect(),
            ),

            // Leaf types: return as-is
            Type::Int
            | Type::Float
            | Type::Bool
            | Type::String
            | Type::Unit
            | Type::Agent(_)
            | Type::Named(_)
            | Type::Never
            | Type::Error => self.clone(),
        }
    }

    /// Check if this type is serializable (can be persisted).
    /// Serializable types: primitives, List<T>, Option<T>, Map<K,V>, Tuple,
    /// Result<T,E>, Named (records/enums), Generic with serializable args.
    /// NOT serializable: Fn, Agent, Oracle, TypeParam.
    #[must_use]
    pub fn is_serializable(&self) -> bool {
        match self {
            // Primitives are serializable
            Type::Int | Type::Float | Type::Bool | Type::String | Type::Unit => true,

            // Compound types are serializable if their inner types are
            Type::List(elem) | Type::Option(elem) | Type::Persisted(elem) => elem.is_serializable(),
            Type::Map(key, value) | Type::Result(key, value) => {
                key.is_serializable() && value.is_serializable()
            }
            Type::Tuple(elems) => elems.iter().all(Type::is_serializable),
            Type::Generic(_, args) => args.iter().all(Type::is_serializable),

            // Named types (records/enums) are assumed serializable
            // (the user-defined type's fields are checked separately)
            Type::Named(_) => true,

            // Non-serializable types
            Type::Fn(_, _) => false,     // Functions can't be serialized
            Type::Agent(_) => false,     // Agent handles are runtime-only
            Type::Oracle(_) => false,    // Oracle results are transient
            Type::TypeParam(_) => false, // Unresolved type params can't be verified

            // Error and Never are edge cases - treat as non-serializable
            Type::Never | Type::Error => false,
        }
    }

    /// Check if this type contains any type parameters.
    #[must_use]
    pub fn has_type_params(&self) -> bool {
        match self {
            Type::TypeParam(_) => true,
            Type::List(elem) | Type::Option(elem) | Type::Oracle(elem) | Type::Persisted(elem) => {
                elem.has_type_params()
            }
            Type::Map(key, value) | Type::Result(key, value) => {
                key.has_type_params() || value.has_type_params()
            }
            Type::Tuple(elems) => elems.iter().any(Type::has_type_params),
            Type::Fn(params, ret) => {
                params.iter().any(Type::has_type_params) || ret.has_type_params()
            }
            Type::Generic(_, args) => args.iter().any(Type::has_type_params),
            _ => false,
        }
    }

    /// Collect all type parameter names in this type.
    pub fn collect_type_params(&self, params: &mut std::collections::HashSet<String>) {
        match self {
            Type::TypeParam(name) => {
                params.insert(name.clone());
            }
            Type::List(elem) | Type::Option(elem) | Type::Oracle(elem) | Type::Persisted(elem) => {
                elem.collect_type_params(params);
            }
            Type::Map(key, value) | Type::Result(key, value) => {
                key.collect_type_params(params);
                value.collect_type_params(params);
            }
            Type::Tuple(elems) => {
                for elem in elems {
                    elem.collect_type_params(params);
                }
            }
            Type::Fn(param_types, ret) => {
                for param in param_types {
                    param.collect_type_params(params);
                }
                ret.collect_type_params(params);
            }
            Type::Generic(_, args) => {
                for arg in args {
                    arg.collect_type_params(params);
                }
            }
            _ => {}
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
            Type::Oracle(inner) => write!(f, "Oracle<{inner}>"),
            Type::Agent(name) => write!(f, "Agent<{name}>"),
            Type::Named(name) => write!(f, "{name}"),
            Type::Generic(name, args) => {
                write!(f, "{name}<")?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ">")
            }
            Type::TypeParam(name) => write!(f, "{name}"),
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
            Type::Persisted(inner) => write!(f, "Persisted<{inner}>"),
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
            Type::Oracle(Box::new(Type::String)).to_string(),
            "Oracle<String>"
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
    fn type_unwrap_oracle() {
        let t = Type::Oracle(Box::new(Type::String));
        assert_eq!(t.unwrap_oracle(), &Type::String);

        let nested = Type::Oracle(Box::new(Type::Oracle(Box::new(Type::Int))));
        assert_eq!(nested.unwrap_oracle(), &Type::Int);

        assert_eq!(Type::Int.unwrap_oracle(), &Type::Int);
    }

    #[test]
    fn type_compatibility() {
        assert!(Type::Int.is_compatible_with(&Type::Int));
        assert!(!Type::Int.is_compatible_with(&Type::String));

        // Inferred<T> is compatible with T
        let inferred_string = Type::Oracle(Box::new(Type::String));
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

    #[test]
    fn type_serializability() {
        // Primitives are serializable
        assert!(Type::Int.is_serializable());
        assert!(Type::Float.is_serializable());
        assert!(Type::Bool.is_serializable());
        assert!(Type::String.is_serializable());
        assert!(Type::Unit.is_serializable());

        // Compound types with serializable inner types
        assert!(Type::List(Box::new(Type::Int)).is_serializable());
        assert!(Type::Option(Box::new(Type::String)).is_serializable());
        assert!(Type::Map(Box::new(Type::String), Box::new(Type::Int)).is_serializable());
        assert!(Type::Tuple(vec![Type::Int, Type::Bool]).is_serializable());
        assert!(Type::Result(Box::new(Type::Int), Box::new(Type::String)).is_serializable());

        // Named types (records/enums) are serializable
        assert!(Type::Named("MyRecord".to_string()).is_serializable());

        // Non-serializable types
        assert!(!Type::Fn(vec![Type::Int], Box::new(Type::Bool)).is_serializable());
        assert!(!Type::Agent("Foo".to_string()).is_serializable());
        assert!(!Type::Oracle(Box::new(Type::String)).is_serializable());
        assert!(!Type::TypeParam("T".to_string()).is_serializable());

        // Compound types with non-serializable inner types
        assert!(!Type::List(Box::new(Type::Agent("Foo".to_string()))).is_serializable());
        assert!(!Type::Option(Box::new(Type::Fn(vec![], Box::new(Type::Unit)))).is_serializable());
    }
}
