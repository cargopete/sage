//! Type expressions for the Sage language.

use crate::Ident;
use std::fmt;

/// A type expression as it appears in source code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeExpr {
    /// 64-bit signed integer.
    Int,
    /// 64-bit IEEE 754 floating point.
    Float,
    /// Boolean.
    Bool,
    /// UTF-8 string.
    String,
    /// Unit type (void equivalent).
    Unit,
    /// Homogeneous list: `List<T>`.
    List(Box<TypeExpr>),
    /// Optional value: `Option<T>`.
    Option(Box<TypeExpr>),
    /// LLM inference result: `Inferred<T>`.
    Inferred(Box<TypeExpr>),
    /// Agent handle: `Agent<AgentName>`.
    Agent(Ident),
    /// Named type (agent name or future user-defined types).
    Named(Ident),
}

impl TypeExpr {
    /// Check if this is a primitive type.
    #[must_use]
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            TypeExpr::Int | TypeExpr::Float | TypeExpr::Bool | TypeExpr::String | TypeExpr::Unit
        )
    }

    /// Check if this is a compound type.
    #[must_use]
    pub fn is_compound(&self) -> bool {
        matches!(
            self,
            TypeExpr::List(_) | TypeExpr::Option(_) | TypeExpr::Inferred(_) | TypeExpr::Agent(_)
        )
    }

    /// Get the inner type for generic types, if any.
    #[must_use]
    pub fn inner_type(&self) -> Option<&TypeExpr> {
        match self {
            TypeExpr::List(inner) | TypeExpr::Option(inner) | TypeExpr::Inferred(inner) => {
                Some(inner)
            }
            _ => None,
        }
    }
}

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeExpr::Int => write!(f, "Int"),
            TypeExpr::Float => write!(f, "Float"),
            TypeExpr::Bool => write!(f, "Bool"),
            TypeExpr::String => write!(f, "String"),
            TypeExpr::Unit => write!(f, "Unit"),
            TypeExpr::List(inner) => write!(f, "List<{inner}>"),
            TypeExpr::Option(inner) => write!(f, "Option<{inner}>"),
            TypeExpr::Inferred(inner) => write!(f, "Inferred<{inner}>"),
            TypeExpr::Agent(name) => write!(f, "Agent<{name}>"),
            TypeExpr::Named(name) => write!(f, "{name}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_display() {
        assert_eq!(format!("{}", TypeExpr::Int), "Int");
        assert_eq!(format!("{}", TypeExpr::Float), "Float");
        assert_eq!(format!("{}", TypeExpr::Bool), "Bool");
        assert_eq!(format!("{}", TypeExpr::String), "String");
        assert_eq!(format!("{}", TypeExpr::Unit), "Unit");
    }

    #[test]
    fn compound_display() {
        let list_str = TypeExpr::List(Box::new(TypeExpr::String));
        assert_eq!(format!("{list_str}"), "List<String>");

        let option_int = TypeExpr::Option(Box::new(TypeExpr::Int));
        assert_eq!(format!("{option_int}"), "Option<Int>");

        let inferred_str = TypeExpr::Inferred(Box::new(TypeExpr::String));
        assert_eq!(format!("{inferred_str}"), "Inferred<String>");

        let agent = TypeExpr::Agent(Ident::dummy("Researcher"));
        assert_eq!(format!("{agent}"), "Agent<Researcher>");
    }

    #[test]
    fn nested_compound_display() {
        // List<List<Int>>
        let nested = TypeExpr::List(Box::new(TypeExpr::List(Box::new(TypeExpr::Int))));
        assert_eq!(format!("{nested}"), "List<List<Int>>");

        // Option<List<String>>
        let nested = TypeExpr::Option(Box::new(TypeExpr::List(Box::new(TypeExpr::String))));
        assert_eq!(format!("{nested}"), "Option<List<String>>");
    }

    #[test]
    fn is_primitive() {
        assert!(TypeExpr::Int.is_primitive());
        assert!(TypeExpr::Float.is_primitive());
        assert!(TypeExpr::Bool.is_primitive());
        assert!(TypeExpr::String.is_primitive());
        assert!(TypeExpr::Unit.is_primitive());

        assert!(!TypeExpr::List(Box::new(TypeExpr::Int)).is_primitive());
        assert!(!TypeExpr::Option(Box::new(TypeExpr::Int)).is_primitive());
    }

    #[test]
    fn is_compound() {
        assert!(!TypeExpr::Int.is_compound());

        assert!(TypeExpr::List(Box::new(TypeExpr::Int)).is_compound());
        assert!(TypeExpr::Option(Box::new(TypeExpr::Int)).is_compound());
        assert!(TypeExpr::Inferred(Box::new(TypeExpr::String)).is_compound());
        assert!(TypeExpr::Agent(Ident::dummy("Foo")).is_compound());
    }

    #[test]
    fn inner_type() {
        let list = TypeExpr::List(Box::new(TypeExpr::String));
        assert_eq!(list.inner_type(), Some(&TypeExpr::String));

        let option = TypeExpr::Option(Box::new(TypeExpr::Int));
        assert_eq!(option.inner_type(), Some(&TypeExpr::Int));

        assert_eq!(TypeExpr::Int.inner_type(), None);
    }

    #[test]
    fn equality() {
        assert_eq!(TypeExpr::Int, TypeExpr::Int);
        assert_ne!(TypeExpr::Int, TypeExpr::Float);

        let list1 = TypeExpr::List(Box::new(TypeExpr::String));
        let list2 = TypeExpr::List(Box::new(TypeExpr::String));
        let list3 = TypeExpr::List(Box::new(TypeExpr::Int));

        assert_eq!(list1, list2);
        assert_ne!(list1, list3);
    }
}
