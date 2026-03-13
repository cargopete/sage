//! Scope and symbol table for name resolution.

use crate::types::Type;
use sage_types::TypeExpr;
use std::collections::HashMap;

/// A module path like `["agents", "researcher"]`.
pub type ModulePath = Vec<String>;

/// Information about a declared agent.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    /// The agent's name.
    pub name: String,
    /// Beliefs declared by this agent (name -> type).
    pub beliefs: HashMap<String, Type>,
    /// The type of messages this agent can receive (from `on message` handler).
    pub message_type: Option<Type>,
    /// The type this agent emits (inferred from `emit` calls).
    pub emit_type: Option<Type>,
    /// Whether this agent has an `on start` handler.
    pub has_start_handler: bool,
    /// Whether this agent is public (visible outside its module).
    pub is_pub: bool,
    /// The module path where this agent is defined.
    pub module_path: ModulePath,
}

/// Information about a declared function.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// The function's name.
    pub name: String,
    /// Parameter types in order.
    pub params: Vec<(String, Type)>,
    /// Return type.
    pub return_type: Type,
    /// Whether this function is public (visible outside its module).
    pub is_pub: bool,
    /// The module path where this function is defined.
    pub module_path: ModulePath,
}

/// Information about a built-in function.
#[derive(Debug, Clone)]
pub struct BuiltinInfo {
    /// The function's name.
    pub name: &'static str,
    /// Parameter types (None means variadic).
    pub params: Option<Vec<Type>>,
    /// Return type.
    pub return_type: Type,
}

/// Information about a declared record type.
#[derive(Debug, Clone)]
pub struct RecordInfo {
    /// The record's name.
    pub name: String,
    /// Fields declared by this record (name -> type).
    pub fields: HashMap<String, Type>,
    /// Field order (for positional access if needed).
    pub field_order: Vec<String>,
    /// Whether this record is public.
    pub is_pub: bool,
    /// The module path where this record is defined.
    pub module_path: ModulePath,
}

/// Information about a declared enum type.
#[derive(Debug, Clone)]
pub struct EnumInfo {
    /// The enum's name.
    pub name: String,
    /// Variant names.
    pub variants: Vec<String>,
    /// Whether this enum is public.
    pub is_pub: bool,
    /// The module path where this enum is defined.
    pub module_path: ModulePath,
}

/// Information about a declared constant.
#[derive(Debug, Clone)]
pub struct ConstInfo {
    /// The constant's name.
    pub name: String,
    /// The constant's type.
    pub ty: Type,
    /// Whether this constant is public.
    pub is_pub: bool,
    /// The module path where this constant is defined.
    pub module_path: ModulePath,
}

/// A scope containing variable bindings.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Variables in this scope (name -> type).
    variables: HashMap<String, Type>,
}

impl Scope {
    /// Create a new empty scope.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Define a variable in this scope.
    pub fn define(&mut self, name: impl Into<String>, ty: Type) {
        self.variables.insert(name.into(), ty);
    }

    /// Look up a variable in this scope.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Type> {
        self.variables.get(name)
    }

    /// Check if a variable is defined in this scope.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
}

/// The global symbol table containing all top-level declarations.
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    /// Declared agents.
    agents: HashMap<String, AgentInfo>,
    /// Declared functions.
    functions: HashMap<String, FunctionInfo>,
    /// Built-in functions.
    builtins: HashMap<&'static str, BuiltinInfo>,
    /// Declared record types.
    records: HashMap<String, RecordInfo>,
    /// Declared enum types.
    enums: HashMap<String, EnumInfo>,
    /// Declared constants.
    consts: HashMap<String, ConstInfo>,
}

impl SymbolTable {
    /// Create a new symbol table with built-in functions.
    #[must_use]
    pub fn new() -> Self {
        let mut table = Self::default();
        table.register_builtins();
        table
    }

    /// Register the built-in functions.
    fn register_builtins(&mut self) {
        // print(String) -> Unit
        self.builtins.insert(
            "print",
            BuiltinInfo {
                name: "print",
                params: Some(vec![Type::String]),
                return_type: Type::Unit,
            },
        );

        // len(List<T>) -> Int (we'll handle generics specially)
        self.builtins.insert(
            "len",
            BuiltinInfo {
                name: "len",
                params: None, // Special handling for generic
                return_type: Type::Int,
            },
        );

        // push(List<T>, T) -> List<T> (special handling)
        self.builtins.insert(
            "push",
            BuiltinInfo {
                name: "push",
                params: None,
                return_type: Type::Error, // Determined by first arg
            },
        );

        // join(List<String>, String) -> String
        self.builtins.insert(
            "join",
            BuiltinInfo {
                name: "join",
                params: Some(vec![Type::List(Box::new(Type::String)), Type::String]),
                return_type: Type::String,
            },
        );

        // str(T) -> String (accepts any type)
        self.builtins.insert(
            "str",
            BuiltinInfo {
                name: "str",
                params: None, // Special handling - accepts any type
                return_type: Type::String,
            },
        );

        // int_to_str(Int) -> String
        self.builtins.insert(
            "int_to_str",
            BuiltinInfo {
                name: "int_to_str",
                params: Some(vec![Type::Int]),
                return_type: Type::String,
            },
        );

        // str_contains(String, String) -> Bool
        self.builtins.insert(
            "str_contains",
            BuiltinInfo {
                name: "str_contains",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Bool,
            },
        );

        // sleep_ms(Int) -> Unit
        self.builtins.insert(
            "sleep_ms",
            BuiltinInfo {
                name: "sleep_ms",
                params: Some(vec![Type::Int]),
                return_type: Type::Unit,
            },
        );
    }

    /// Define an agent.
    pub fn define_agent(&mut self, info: AgentInfo) {
        self.agents.insert(info.name.clone(), info);
    }

    /// Define a function.
    pub fn define_function(&mut self, info: FunctionInfo) {
        self.functions.insert(info.name.clone(), info);
    }

    /// Look up an agent by name.
    #[must_use]
    pub fn get_agent(&self, name: &str) -> Option<&AgentInfo> {
        self.agents.get(name)
    }

    /// Look up a function by name.
    #[must_use]
    pub fn get_function(&self, name: &str) -> Option<&FunctionInfo> {
        self.functions.get(name)
    }

    /// Look up a built-in function by name.
    #[must_use]
    pub fn get_builtin(&self, name: &str) -> Option<&BuiltinInfo> {
        self.builtins.get(name)
    }

    /// Check if an agent is defined.
    #[must_use]
    pub fn has_agent(&self, name: &str) -> bool {
        self.agents.contains_key(name)
    }

    /// Check if a function is defined.
    #[must_use]
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    /// Check if a built-in function is defined.
    #[must_use]
    pub fn has_builtin(&self, name: &str) -> bool {
        self.builtins.contains_key(name)
    }

    /// Get a mutable reference to an agent.
    pub fn get_agent_mut(&mut self, name: &str) -> Option<&mut AgentInfo> {
        self.agents.get_mut(name)
    }

    /// Iterate over all agents.
    pub fn iter_agents(&self) -> impl Iterator<Item = (&String, &AgentInfo)> {
        self.agents.iter()
    }

    /// Iterate over all functions.
    pub fn iter_functions(&self) -> impl Iterator<Item = (&String, &FunctionInfo)> {
        self.functions.iter()
    }

    /// Define a record type.
    pub fn define_record(&mut self, info: RecordInfo) {
        self.records.insert(info.name.clone(), info);
    }

    /// Define an enum type.
    pub fn define_enum(&mut self, info: EnumInfo) {
        self.enums.insert(info.name.clone(), info);
    }

    /// Define a constant.
    pub fn define_const(&mut self, info: ConstInfo) {
        self.consts.insert(info.name.clone(), info);
    }

    /// Look up a record by name.
    #[must_use]
    pub fn get_record(&self, name: &str) -> Option<&RecordInfo> {
        self.records.get(name)
    }

    /// Look up an enum by name.
    #[must_use]
    pub fn get_enum(&self, name: &str) -> Option<&EnumInfo> {
        self.enums.get(name)
    }

    /// Look up a constant by name.
    #[must_use]
    pub fn get_const(&self, name: &str) -> Option<&ConstInfo> {
        self.consts.get(name)
    }

    /// Check if a record is defined.
    #[must_use]
    pub fn has_record(&self, name: &str) -> bool {
        self.records.contains_key(name)
    }

    /// Check if an enum is defined.
    #[must_use]
    pub fn has_enum(&self, name: &str) -> bool {
        self.enums.contains_key(name)
    }

    /// Check if a constant is defined.
    #[must_use]
    pub fn has_const(&self, name: &str) -> bool {
        self.consts.contains_key(name)
    }

    /// Iterate over all records.
    pub fn iter_records(&self) -> impl Iterator<Item = (&String, &RecordInfo)> {
        self.records.iter()
    }

    /// Iterate over all enums.
    pub fn iter_enums(&self) -> impl Iterator<Item = (&String, &EnumInfo)> {
        self.enums.iter()
    }

    /// Iterate over all constants.
    pub fn iter_consts(&self) -> impl Iterator<Item = (&String, &ConstInfo)> {
        self.consts.iter()
    }
}

/// Convert a syntactic `TypeExpr` to a semantic Type.
#[must_use]
pub fn resolve_type(ty: &TypeExpr) -> Type {
    match ty {
        TypeExpr::Int => Type::Int,
        TypeExpr::Float => Type::Float,
        TypeExpr::Bool => Type::Bool,
        TypeExpr::String => Type::String,
        TypeExpr::Unit => Type::Unit,
        TypeExpr::List(inner) => Type::List(Box::new(resolve_type(inner))),
        TypeExpr::Option(inner) => Type::Option(Box::new(resolve_type(inner))),
        TypeExpr::Inferred(inner) => Type::Inferred(Box::new(resolve_type(inner))),
        TypeExpr::Agent(ident) => Type::Agent(ident.name.clone()),
        TypeExpr::Named(ident) => {
            // Named types can be records, enums, or agents
            // We return Type::Named and let the checker validate
            Type::Named(ident.name.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_define_and_get() {
        let mut scope = Scope::new();
        scope.define("x", Type::Int);
        scope.define("y", Type::String);

        assert_eq!(scope.get("x"), Some(&Type::Int));
        assert_eq!(scope.get("y"), Some(&Type::String));
        assert_eq!(scope.get("z"), None);
    }

    #[test]
    fn symbol_table_has_builtins() {
        let table = SymbolTable::new();

        assert!(table.has_builtin("print"));
        assert!(table.has_builtin("len"));
        assert!(table.has_builtin("join"));
        assert!(!table.has_builtin("nonexistent"));
    }

    #[test]
    fn resolve_type_primitives() {
        assert_eq!(resolve_type(&TypeExpr::Int), Type::Int);
        assert_eq!(resolve_type(&TypeExpr::Float), Type::Float);
        assert_eq!(resolve_type(&TypeExpr::Bool), Type::Bool);
        assert_eq!(resolve_type(&TypeExpr::String), Type::String);
        assert_eq!(resolve_type(&TypeExpr::Unit), Type::Unit);
    }

    #[test]
    fn resolve_type_compound() {
        let list_int = TypeExpr::List(Box::new(TypeExpr::Int));
        assert_eq!(resolve_type(&list_int), Type::List(Box::new(Type::Int)));

        let inferred_string = TypeExpr::Inferred(Box::new(TypeExpr::String));
        assert_eq!(
            resolve_type(&inferred_string),
            Type::Inferred(Box::new(Type::String))
        );
    }
}
