//! Scope and symbol table for name resolution.

use crate::types::Type;
use sage_parser::TypeExpr;
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
    /// Names of beliefs that are @persistent.
    pub persistent_beliefs: std::collections::HashSet<String>,
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
    /// Protocols this agent follows (protocol_name -> role_name).
    /// Phase 3: Session Types - tracks which protocols an agent participates in and their role.
    pub protocol_roles: HashMap<String, String>,
}

/// Information about a declared function.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// The function's name.
    pub name: String,
    /// Type parameters for generic functions (e.g., `["T", "U"]` for `fn map<T, U>`).
    pub type_params: Vec<String>,
    /// Parameter types in order.
    pub params: Vec<(String, Type)>,
    /// Return type.
    pub return_type: Type,
    /// Whether this function is public (visible outside its module).
    pub is_pub: bool,
    /// The module path where this function is defined.
    pub module_path: ModulePath,
    /// RFC-0007: Whether this function can fail (has `fails` annotation).
    pub is_fallible: bool,
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
    /// Whether this function can fail (has `fails` annotation).
    pub is_fallible: bool,
}

impl Default for BuiltinInfo {
    fn default() -> Self {
        Self {
            name: "",
            params: None,
            return_type: Type::Unit,
            is_fallible: false,
        }
    }
}

/// Information about a declared record type.
#[derive(Debug, Clone)]
pub struct RecordInfo {
    /// The record's name.
    pub name: String,
    /// Type parameters for generic records (e.g., `["A", "B"]` for `record Pair<A, B>`).
    pub type_params: Vec<String>,
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
    /// Type parameters for generic enums (e.g., `["T"]` for `enum Tree<T>`).
    pub type_params: Vec<String>,
    /// Variants with optional payload types.
    pub variants: Vec<(String, Option<Type>)>,
    /// Whether this enum is public.
    pub is_pub: bool,
    /// The module path where this enum is defined.
    pub module_path: ModulePath,
}

impl EnumInfo {
    /// Check if all variants are unit variants (no payloads).
    #[must_use]
    pub fn all_variants_unit(&self) -> bool {
        self.variants.iter().all(|(_, payload)| payload.is_none())
    }

    /// Get the payload type for a variant, if it exists.
    /// Returns Some(None) if the variant exists but has no payload.
    /// Returns None if the variant doesn't exist.
    #[must_use]
    pub fn get_variant_payload(&self, name: &str) -> Option<Option<&Type>> {
        self.variants
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, payload)| payload.as_ref())
    }

    /// Check if a variant exists.
    #[must_use]
    pub fn has_variant(&self, name: &str) -> bool {
        self.variants.iter().any(|(n, _)| n == name)
    }
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

/// Information about a declared tool (RFC-0011).
#[derive(Debug, Clone)]
pub struct ToolInfo {
    /// The tool's name.
    pub name: String,
    /// Functions provided by this tool.
    pub functions: HashMap<String, ToolFnInfo>,
    /// Whether this tool is public.
    pub is_pub: bool,
}

/// Information about a declared supervisor (v2 supervision trees).
#[derive(Debug, Clone)]
pub struct SupervisorInfo {
    /// The supervisor's name.
    pub name: String,
    /// Names of child agents this supervisor manages.
    pub children: Vec<String>,
    /// Whether this supervisor is public.
    pub is_pub: bool,
    /// The module path where this supervisor is defined.
    pub module_path: ModulePath,
}

/// Information about a protocol step (Phase 3: Session Types).
#[derive(Debug, Clone)]
pub struct ProtocolStepInfo {
    /// The sending role.
    pub sender: String,
    /// The receiving role.
    pub receiver: String,
    /// The message type being sent.
    pub message_type: Type,
}

/// Information about a declared protocol (Phase 3: Session Types).
#[derive(Debug, Clone)]
pub struct ProtocolInfo {
    /// The protocol's name.
    pub name: String,
    /// The steps in the protocol.
    pub steps: Vec<ProtocolStepInfo>,
    /// The roles participating in this protocol.
    pub roles: std::collections::HashSet<String>,
    /// Whether this protocol is public.
    pub is_pub: bool,
    /// The module path where this protocol is defined.
    pub module_path: ModulePath,
}

/// Information about an effect handler (Phase 3: Algebraic Effects).
#[derive(Debug, Clone)]
pub struct EffectHandlerInfo {
    /// The handler's name.
    pub name: String,
    /// The effect this handler handles (e.g., "Infer").
    pub effect: String,
    /// Configuration values (key -> value as string).
    pub config: HashMap<String, String>,
    /// Whether this handler is public.
    pub is_pub: bool,
    /// The module path where this handler is defined.
    pub module_path: ModulePath,
}

/// Information about a tool function (RFC-0011).
#[derive(Debug, Clone)]
pub struct ToolFnInfo {
    /// Parameter names and types.
    pub params: Vec<(String, Type)>,
    /// Return type.
    pub return_ty: Type,
}

/// A scope containing variable bindings and tool declarations.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Variables in this scope (name -> type).
    variables: HashMap<String, Type>,
    /// Tools available in this scope (RFC-0011).
    tools: HashMap<String, ToolInfo>,
}

impl Scope {
    /// Create a new empty scope.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope with built-in tools (RFC-0011).
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut scope = Self::new();

        // Register Http built-in tool
        let mut http_functions = HashMap::new();

        // Http.get(url: String) -> Result<HttpResponse, String>
        http_functions.insert(
            "get".to_string(),
            ToolFnInfo {
                params: vec![("url".to_string(), Type::String)],
                return_ty: Type::Result(
                    Box::new(Type::Named("HttpResponse".to_string())),
                    Box::new(Type::String),
                ),
            },
        );

        // Http.post(url: String, body: String) -> Result<HttpResponse, String>
        http_functions.insert(
            "post".to_string(),
            ToolFnInfo {
                params: vec![
                    ("url".to_string(), Type::String),
                    ("body".to_string(), Type::String),
                ],
                return_ty: Type::Result(
                    Box::new(Type::Named("HttpResponse".to_string())),
                    Box::new(Type::String),
                ),
            },
        );

        // Http.put(url: String, body: String) -> Result<HttpResponse, String>
        http_functions.insert(
            "put".to_string(),
            ToolFnInfo {
                params: vec![
                    ("url".to_string(), Type::String),
                    ("body".to_string(), Type::String),
                ],
                return_ty: Type::Result(
                    Box::new(Type::Named("HttpResponse".to_string())),
                    Box::new(Type::String),
                ),
            },
        );

        // Http.delete(url: String) -> Result<HttpResponse, String>
        http_functions.insert(
            "delete".to_string(),
            ToolFnInfo {
                params: vec![("url".to_string(), Type::String)],
                return_ty: Type::Result(
                    Box::new(Type::Named("HttpResponse".to_string())),
                    Box::new(Type::String),
                ),
            },
        );

        scope.tools.insert(
            "Http".to_string(),
            ToolInfo {
                name: "Http".to_string(),
                functions: http_functions,
                is_pub: true,
            },
        );

        // Register Fs built-in tool (FileSystem)
        let mut fs_functions = HashMap::new();

        // Fs.read(path: String) -> Result<String, String>
        fs_functions.insert(
            "read".to_string(),
            ToolFnInfo {
                params: vec![("path".to_string(), Type::String)],
                return_ty: Type::Result(Box::new(Type::String), Box::new(Type::String)),
            },
        );

        // Fs.write(path: String, content: String) -> Result<Unit, String>
        fs_functions.insert(
            "write".to_string(),
            ToolFnInfo {
                params: vec![
                    ("path".to_string(), Type::String),
                    ("content".to_string(), Type::String),
                ],
                return_ty: Type::Result(Box::new(Type::Unit), Box::new(Type::String)),
            },
        );

        // Fs.exists(path: String) -> Result<Bool, String>
        fs_functions.insert(
            "exists".to_string(),
            ToolFnInfo {
                params: vec![("path".to_string(), Type::String)],
                return_ty: Type::Result(Box::new(Type::Bool), Box::new(Type::String)),
            },
        );

        // Fs.list(path: String) -> Result<List<String>, String>
        fs_functions.insert(
            "list".to_string(),
            ToolFnInfo {
                params: vec![("path".to_string(), Type::String)],
                return_ty: Type::Result(
                    Box::new(Type::List(Box::new(Type::String))),
                    Box::new(Type::String),
                ),
            },
        );

        // Fs.delete(path: String) -> Result<Unit, String>
        fs_functions.insert(
            "delete".to_string(),
            ToolFnInfo {
                params: vec![("path".to_string(), Type::String)],
                return_ty: Type::Result(Box::new(Type::Unit), Box::new(Type::String)),
            },
        );

        scope.tools.insert(
            "Fs".to_string(),
            ToolInfo {
                name: "Fs".to_string(),
                functions: fs_functions,
                is_pub: true,
            },
        );

        // Register Shell built-in tool
        let mut shell_functions = HashMap::new();

        // Shell.run(command: String) -> Result<ShellResult, String>
        shell_functions.insert(
            "run".to_string(),
            ToolFnInfo {
                params: vec![("command".to_string(), Type::String)],
                return_ty: Type::Result(
                    Box::new(Type::Named("ShellResult".to_string())),
                    Box::new(Type::String),
                ),
            },
        );

        scope.tools.insert(
            "Shell".to_string(),
            ToolInfo {
                name: "Shell".to_string(),
                functions: shell_functions,
                is_pub: true,
            },
        );

        // Register Database built-in tool
        let mut database_functions = HashMap::new();

        // Database.query(sql: String) -> Result<List<DbRow>, String>
        database_functions.insert(
            "query".to_string(),
            ToolFnInfo {
                params: vec![("sql".to_string(), Type::String)],
                return_ty: Type::Result(
                    Box::new(Type::List(Box::new(Type::Named("DbRow".to_string())))),
                    Box::new(Type::String),
                ),
            },
        );

        // Database.execute(sql: String) -> Result<Int, String>
        database_functions.insert(
            "execute".to_string(),
            ToolFnInfo {
                params: vec![("sql".to_string(), Type::String)],
                return_ty: Type::Result(Box::new(Type::Int), Box::new(Type::String)),
            },
        );

        scope.tools.insert(
            "Database".to_string(),
            ToolInfo {
                name: "Database".to_string(),
                functions: database_functions,
                is_pub: true,
            },
        );

        scope
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

    /// Look up a tool in this scope (RFC-0011).
    #[must_use]
    pub fn lookup_tool(&self, name: &str) -> Option<&ToolInfo> {
        self.tools.get(name)
    }
}

/// The global symbol table containing all top-level declarations.
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    /// Declared agents.
    agents: HashMap<String, AgentInfo>,
    /// Declared supervisors (v2).
    supervisors: HashMap<String, SupervisorInfo>,
    /// Declared protocols (Phase 3: Session Types).
    protocols: HashMap<String, ProtocolInfo>,
    /// Declared effect handlers (Phase 3: Algebraic Effects).
    effect_handlers: HashMap<String, EffectHandlerInfo>,
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
        table.register_builtin_enums();
        table
    }

    /// Register built-in enum types (Option, Result).
    fn register_builtin_enums(&mut self) {
        // Option<T> = Some(T) | None
        self.enums.insert(
            "Option".to_string(),
            EnumInfo {
                name: "Option".to_string(),
                type_params: vec!["T".to_string()],
                variants: vec![
                    ("Some".to_string(), Some(Type::TypeParam("T".to_string()))),
                    ("None".to_string(), None),
                ],
                is_pub: true,
                module_path: vec![],
            },
        );

        // Result<T, E> = Ok(T) | Err(E)
        self.enums.insert(
            "Result".to_string(),
            EnumInfo {
                name: "Result".to_string(),
                type_params: vec!["T".to_string(), "E".to_string()],
                variants: vec![
                    ("Ok".to_string(), Some(Type::TypeParam("T".to_string()))),
                    ("Err".to_string(), Some(Type::TypeParam("E".to_string()))),
                ],
                is_pub: true,
                module_path: vec![],
            },
        );
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
                is_fallible: false,
            },
        );

        // len(List<T>) -> Int (we'll handle generics specially)
        self.builtins.insert(
            "len",
            BuiltinInfo {
                name: "len",
                params: None, // Special handling for generic
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // push(List<T>, T) -> List<T> (special handling)
        self.builtins.insert(
            "push",
            BuiltinInfo {
                name: "push",
                params: None,
                return_type: Type::Error, // Determined by first arg
                is_fallible: false,
            },
        );

        // join(List<String>, String) -> String
        self.builtins.insert(
            "join",
            BuiltinInfo {
                name: "join",
                params: Some(vec![Type::List(Box::new(Type::String)), Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // lines(String) -> List<String>
        self.builtins.insert(
            "lines",
            BuiltinInfo {
                name: "lines",
                params: Some(vec![Type::String]),
                return_type: Type::List(Box::new(Type::String)),
                is_fallible: false,
            },
        );

        // chars(String) -> List<String>
        self.builtins.insert(
            "chars",
            BuiltinInfo {
                name: "chars",
                params: Some(vec![Type::String]),
                return_type: Type::List(Box::new(Type::String)),
                is_fallible: false,
            },
        );

        // str(T) -> String (accepts any type)
        self.builtins.insert(
            "str",
            BuiltinInfo {
                name: "str",
                params: None, // Special handling - accepts any type
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // int_to_str(Int) -> String
        self.builtins.insert(
            "int_to_str",
            BuiltinInfo {
                name: "int_to_str",
                params: Some(vec![Type::Int]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // str_contains(String, String) -> Bool
        self.builtins.insert(
            "str_contains",
            BuiltinInfo {
                name: "str_contains",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // sleep_ms(Int) -> Unit
        self.builtins.insert(
            "sleep_ms",
            BuiltinInfo {
                name: "sleep_ms",
                params: Some(vec![Type::Int]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // Map builtins - all use special handling for generics
        // map_get(Map<K, V>, K) -> Option<V>
        self.builtins.insert(
            "map_get",
            BuiltinInfo {
                name: "map_get",
                params: None,             // Special handling for generics
                return_type: Type::Error, // Determined by first arg
                is_fallible: false,
            },
        );

        // map_set(Map<K, V>, K, V) -> Unit
        self.builtins.insert(
            "map_set",
            BuiltinInfo {
                name: "map_set",
                params: None,
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // map_delete(Map<K, V>, K) -> Unit
        self.builtins.insert(
            "map_delete",
            BuiltinInfo {
                name: "map_delete",
                params: None,
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // map_has(Map<K, V>, K) -> Bool
        self.builtins.insert(
            "map_has",
            BuiltinInfo {
                name: "map_has",
                params: None,
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // map_keys(Map<K, V>) -> List<K>
        self.builtins.insert(
            "map_keys",
            BuiltinInfo {
                name: "map_keys",
                params: None,
                return_type: Type::Error, // Determined by first arg
                is_fallible: false,
            },
        );

        // map_values(Map<K, V>) -> List<V>
        self.builtins.insert(
            "map_values",
            BuiltinInfo {
                name: "map_values",
                params: None,
                return_type: Type::Error, // Determined by first arg
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0013: Standard Library - String Functions
        // =========================================================================

        // split(String, String) -> List<String>
        self.builtins.insert(
            "split",
            BuiltinInfo {
                name: "split",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::List(Box::new(Type::String)),
                is_fallible: false,
            },
        );

        // trim(String) -> String
        self.builtins.insert(
            "trim",
            BuiltinInfo {
                name: "trim",
                params: Some(vec![Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // trim_start(String) -> String
        self.builtins.insert(
            "trim_start",
            BuiltinInfo {
                name: "trim_start",
                params: Some(vec![Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // trim_end(String) -> String
        self.builtins.insert(
            "trim_end",
            BuiltinInfo {
                name: "trim_end",
                params: Some(vec![Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // starts_with(String, String) -> Bool
        self.builtins.insert(
            "starts_with",
            BuiltinInfo {
                name: "starts_with",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // ends_with(String, String) -> Bool
        self.builtins.insert(
            "ends_with",
            BuiltinInfo {
                name: "ends_with",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // replace(String, String, String) -> String
        self.builtins.insert(
            "replace",
            BuiltinInfo {
                name: "replace",
                params: Some(vec![Type::String, Type::String, Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // replace_first(String, String, String) -> String
        self.builtins.insert(
            "replace_first",
            BuiltinInfo {
                name: "replace_first",
                params: Some(vec![Type::String, Type::String, Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // to_upper(String) -> String
        self.builtins.insert(
            "to_upper",
            BuiltinInfo {
                name: "to_upper",
                params: Some(vec![Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // to_lower(String) -> String
        self.builtins.insert(
            "to_lower",
            BuiltinInfo {
                name: "to_lower",
                params: Some(vec![Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // str_len(String) -> Int
        self.builtins.insert(
            "str_len",
            BuiltinInfo {
                name: "str_len",
                params: Some(vec![Type::String]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // str_slice(String, Int, Int) -> String
        self.builtins.insert(
            "str_slice",
            BuiltinInfo {
                name: "str_slice",
                params: Some(vec![Type::String, Type::Int, Type::Int]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // str_index_of(String, String) -> Option<Int>
        self.builtins.insert(
            "str_index_of",
            BuiltinInfo {
                name: "str_index_of",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Option(Box::new(Type::Int)),
                is_fallible: false,
            },
        );

        // str_repeat(String, Int) -> String
        self.builtins.insert(
            "str_repeat",
            BuiltinInfo {
                name: "str_repeat",
                params: Some(vec![Type::String, Type::Int]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // str_pad_start(String, Int, String) -> String
        self.builtins.insert(
            "str_pad_start",
            BuiltinInfo {
                name: "str_pad_start",
                params: Some(vec![Type::String, Type::Int, Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // str_pad_end(String, Int, String) -> String
        self.builtins.insert(
            "str_pad_end",
            BuiltinInfo {
                name: "str_pad_end",
                params: Some(vec![Type::String, Type::Int, Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0013: Standard Library - Math Functions
        // =========================================================================

        // abs(Int) -> Int
        self.builtins.insert(
            "abs",
            BuiltinInfo {
                name: "abs",
                params: Some(vec![Type::Int]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // abs_float(Float) -> Float
        self.builtins.insert(
            "abs_float",
            BuiltinInfo {
                name: "abs_float",
                params: Some(vec![Type::Float]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // min(Int, Int) -> Int
        self.builtins.insert(
            "min",
            BuiltinInfo {
                name: "min",
                params: Some(vec![Type::Int, Type::Int]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // max(Int, Int) -> Int
        self.builtins.insert(
            "max",
            BuiltinInfo {
                name: "max",
                params: Some(vec![Type::Int, Type::Int]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // min_float(Float, Float) -> Float
        self.builtins.insert(
            "min_float",
            BuiltinInfo {
                name: "min_float",
                params: Some(vec![Type::Float, Type::Float]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // max_float(Float, Float) -> Float
        self.builtins.insert(
            "max_float",
            BuiltinInfo {
                name: "max_float",
                params: Some(vec![Type::Float, Type::Float]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // clamp(Int, Int, Int) -> Int
        self.builtins.insert(
            "clamp",
            BuiltinInfo {
                name: "clamp",
                params: Some(vec![Type::Int, Type::Int, Type::Int]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // clamp_float(Float, Float, Float) -> Float
        self.builtins.insert(
            "clamp_float",
            BuiltinInfo {
                name: "clamp_float",
                params: Some(vec![Type::Float, Type::Float, Type::Float]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // floor(Float) -> Int
        self.builtins.insert(
            "floor",
            BuiltinInfo {
                name: "floor",
                params: Some(vec![Type::Float]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // ceil(Float) -> Int
        self.builtins.insert(
            "ceil",
            BuiltinInfo {
                name: "ceil",
                params: Some(vec![Type::Float]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // round(Float) -> Int
        self.builtins.insert(
            "round",
            BuiltinInfo {
                name: "round",
                params: Some(vec![Type::Float]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // floor_float(Float) -> Float
        self.builtins.insert(
            "floor_float",
            BuiltinInfo {
                name: "floor_float",
                params: Some(vec![Type::Float]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // ceil_float(Float) -> Float
        self.builtins.insert(
            "ceil_float",
            BuiltinInfo {
                name: "ceil_float",
                params: Some(vec![Type::Float]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // pow(Int, Int) -> Int
        self.builtins.insert(
            "pow",
            BuiltinInfo {
                name: "pow",
                params: Some(vec![Type::Int, Type::Int]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // pow_float(Float, Float) -> Float
        self.builtins.insert(
            "pow_float",
            BuiltinInfo {
                name: "pow_float",
                params: Some(vec![Type::Float, Type::Float]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // sqrt(Float) -> Float
        self.builtins.insert(
            "sqrt",
            BuiltinInfo {
                name: "sqrt",
                params: Some(vec![Type::Float]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // int_to_float(Int) -> Float
        self.builtins.insert(
            "int_to_float",
            BuiltinInfo {
                name: "int_to_float",
                params: Some(vec![Type::Int]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // float_to_int(Float) -> Int
        self.builtins.insert(
            "float_to_int",
            BuiltinInfo {
                name: "float_to_int",
                params: Some(vec![Type::Float]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0013: Standard Library - Parsing Functions
        // =========================================================================

        // parse_int(String) -> Result<Int, String>
        self.builtins.insert(
            "parse_int",
            BuiltinInfo {
                name: "parse_int",
                params: Some(vec![Type::String]),
                return_type: Type::Result(Box::new(Type::Int), Box::new(Type::String)),
                is_fallible: false,
            },
        );

        // parse_float(String) -> Result<Float, String>
        self.builtins.insert(
            "parse_float",
            BuiltinInfo {
                name: "parse_float",
                params: Some(vec![Type::String]),
                return_type: Type::Result(Box::new(Type::Float), Box::new(Type::String)),
                is_fallible: false,
            },
        );

        // parse_bool(String) -> Result<Bool, String>
        self.builtins.insert(
            "parse_bool",
            BuiltinInfo {
                name: "parse_bool",
                params: Some(vec![Type::String]),
                return_type: Type::Result(Box::new(Type::Bool), Box::new(Type::String)),
                is_fallible: false,
            },
        );

        // float_to_str(Float) -> String
        self.builtins.insert(
            "float_to_str",
            BuiltinInfo {
                name: "float_to_str",
                params: Some(vec![Type::Float]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // bool_to_str(Bool) -> String
        self.builtins.insert(
            "bool_to_str",
            BuiltinInfo {
                name: "bool_to_str",
                params: Some(vec![Type::Bool]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0013: Standard Library - List Higher-Order Functions
        // =========================================================================

        // map(List<A>, Fn(A) -> B) -> List<B>
        self.builtins.insert(
            "map",
            BuiltinInfo {
                name: "map",
                params: None, // Generic - special handling
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // filter(List<A>, Fn(A) -> Bool) -> List<A>
        self.builtins.insert(
            "filter",
            BuiltinInfo {
                name: "filter",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // reduce(List<A>, B, Fn(B, A) -> B) -> B
        self.builtins.insert(
            "reduce",
            BuiltinInfo {
                name: "reduce",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // any(List<A>, Fn(A) -> Bool) -> Bool
        self.builtins.insert(
            "any",
            BuiltinInfo {
                name: "any",
                params: None,
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // all(List<A>, Fn(A) -> Bool) -> Bool
        self.builtins.insert(
            "all",
            BuiltinInfo {
                name: "all",
                params: None,
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // find(List<A>, Fn(A) -> Bool) -> Option<A>
        self.builtins.insert(
            "find",
            BuiltinInfo {
                name: "find",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // flat_map(List<A>, Fn(A) -> List<B>) -> List<B>
        self.builtins.insert(
            "flat_map",
            BuiltinInfo {
                name: "flat_map",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // zip(List<A>, List<B>) -> List<(A, B)>
        self.builtins.insert(
            "zip",
            BuiltinInfo {
                name: "zip",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // sort_by(List<A>, Fn(A, A) -> Int) -> List<A>
        self.builtins.insert(
            "sort_by",
            BuiltinInfo {
                name: "sort_by",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // enumerate(List<A>) -> List<(Int, A)>
        self.builtins.insert(
            "enumerate",
            BuiltinInfo {
                name: "enumerate",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // take(List<A>, Int) -> List<A>
        self.builtins.insert(
            "take",
            BuiltinInfo {
                name: "take",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // drop(List<A>, Int) -> List<A>
        self.builtins.insert(
            "drop",
            BuiltinInfo {
                name: "drop",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // flatten(List<List<A>>) -> List<A>
        self.builtins.insert(
            "flatten",
            BuiltinInfo {
                name: "flatten",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // reverse(List<A>) -> List<A>
        self.builtins.insert(
            "reverse",
            BuiltinInfo {
                name: "reverse",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // unique(List<A>) -> List<A>
        self.builtins.insert(
            "unique",
            BuiltinInfo {
                name: "unique",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // count_where(List<A>, Fn(A) -> Bool) -> Int
        self.builtins.insert(
            "count_where",
            BuiltinInfo {
                name: "count_where",
                params: None,
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // sum(List<Int>) -> Int
        self.builtins.insert(
            "sum",
            BuiltinInfo {
                name: "sum",
                params: Some(vec![Type::List(Box::new(Type::Int))]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // sum_floats(List<Float>) -> Float
        self.builtins.insert(
            "sum_floats",
            BuiltinInfo {
                name: "sum_floats",
                params: Some(vec![Type::List(Box::new(Type::Float))]),
                return_type: Type::Float,
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0010: List Utilities
        // =========================================================================

        // range(Int, Int) -> List<Int>
        self.builtins.insert(
            "range",
            BuiltinInfo {
                name: "range",
                params: Some(vec![Type::Int, Type::Int]),
                return_type: Type::List(Box::new(Type::Int)),
                is_fallible: false,
            },
        );

        // range_step(Int, Int, Int) -> List<Int>
        self.builtins.insert(
            "range_step",
            BuiltinInfo {
                name: "range_step",
                params: Some(vec![Type::Int, Type::Int, Type::Int]),
                return_type: Type::List(Box::new(Type::Int)),
                is_fallible: false,
            },
        );

        // first(List<T>) -> Option<T> (generic)
        self.builtins.insert(
            "first",
            BuiltinInfo {
                name: "first",
                params: None,
                return_type: Type::Error, // Determined by first arg
                is_fallible: false,
            },
        );

        // last(List<T>) -> Option<T> (generic)
        self.builtins.insert(
            "last",
            BuiltinInfo {
                name: "last",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // get(List<T>, Int) -> Option<T> (generic)
        self.builtins.insert(
            "get",
            BuiltinInfo {
                name: "get",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // list_contains(List<T>, T) -> Bool (generic)
        self.builtins.insert(
            "list_contains",
            BuiltinInfo {
                name: "list_contains",
                params: None,
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // sort(List<T>) -> List<T> (generic, requires Ord)
        self.builtins.insert(
            "sort",
            BuiltinInfo {
                name: "sort",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // list_slice(List<T>, Int, Int) -> List<T> (generic)
        self.builtins.insert(
            "list_slice",
            BuiltinInfo {
                name: "list_slice",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // chunk(List<T>, Int) -> List<List<T>> (generic)
        self.builtins.insert(
            "chunk",
            BuiltinInfo {
                name: "chunk",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // pop(List<T>) -> Option<T> (generic)
        self.builtins.insert(
            "pop",
            BuiltinInfo {
                name: "pop",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // concat(List<T>, List<T>) -> List<T> (generic)
        self.builtins.insert(
            "concat",
            BuiltinInfo {
                name: "concat",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // take_while(List<T>, Fn(T)->Bool) -> List<T> (generic)
        self.builtins.insert(
            "take_while",
            BuiltinInfo {
                name: "take_while",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // drop_while(List<T>, Fn(T)->Bool) -> List<T> (generic)
        self.builtins.insert(
            "drop_while",
            BuiltinInfo {
                name: "drop_while",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0010: I/O Functions
        // =========================================================================

        // read_file(String) -> String fails
        self.builtins.insert(
            "read_file",
            BuiltinInfo {
                name: "read_file",
                params: Some(vec![Type::String]),
                return_type: Type::String,
                is_fallible: true,
            },
        );

        // write_file(String, String) -> Unit fails
        self.builtins.insert(
            "write_file",
            BuiltinInfo {
                name: "write_file",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Unit,
                is_fallible: true,
            },
        );

        // append_file(String, String) -> Unit fails
        self.builtins.insert(
            "append_file",
            BuiltinInfo {
                name: "append_file",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Unit,
                is_fallible: true,
            },
        );

        // file_exists(String) -> Bool
        self.builtins.insert(
            "file_exists",
            BuiltinInfo {
                name: "file_exists",
                params: Some(vec![Type::String]),
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // delete_file(String) -> Unit fails
        self.builtins.insert(
            "delete_file",
            BuiltinInfo {
                name: "delete_file",
                params: Some(vec![Type::String]),
                return_type: Type::Unit,
                is_fallible: true,
            },
        );

        // list_dir(String) -> List<String> fails
        self.builtins.insert(
            "list_dir",
            BuiltinInfo {
                name: "list_dir",
                params: Some(vec![Type::String]),
                return_type: Type::List(Box::new(Type::String)),
                is_fallible: true,
            },
        );

        // make_dir(String) -> Unit fails
        self.builtins.insert(
            "make_dir",
            BuiltinInfo {
                name: "make_dir",
                params: Some(vec![Type::String]),
                return_type: Type::Unit,
                is_fallible: true,
            },
        );

        // read_line() -> String fails
        self.builtins.insert(
            "read_line",
            BuiltinInfo {
                name: "read_line",
                params: Some(vec![]),
                return_type: Type::String,
                is_fallible: true,
            },
        );

        // read_all() -> String fails
        self.builtins.insert(
            "read_all",
            BuiltinInfo {
                name: "read_all",
                params: Some(vec![]),
                return_type: Type::String,
                is_fallible: true,
            },
        );

        // print_err(T) -> Unit (generic)
        self.builtins.insert(
            "print_err",
            BuiltinInfo {
                name: "print_err",
                params: None, // Generic - accepts any type
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0010: Time Functions
        // =========================================================================

        // now_ms() -> Int
        self.builtins.insert(
            "now_ms",
            BuiltinInfo {
                name: "now_ms",
                params: Some(vec![]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // now_s() -> Int
        self.builtins.insert(
            "now_s",
            BuiltinInfo {
                name: "now_s",
                params: Some(vec![]),
                return_type: Type::Int,
                is_fallible: false,
            },
        );

        // format_timestamp(Int, String) -> String
        self.builtins.insert(
            "format_timestamp",
            BuiltinInfo {
                name: "format_timestamp",
                params: Some(vec![Type::Int, Type::String]),
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // parse_timestamp(String, String) -> Int fails
        self.builtins.insert(
            "parse_timestamp",
            BuiltinInfo {
                name: "parse_timestamp",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Int,
                is_fallible: true,
            },
        );

        // =========================================================================
        // RFC-0010: JSON Utilities
        // =========================================================================

        // json_parse(String) -> String fails
        self.builtins.insert(
            "json_parse",
            BuiltinInfo {
                name: "json_parse",
                params: Some(vec![Type::String]),
                return_type: Type::String,
                is_fallible: true,
            },
        );

        // json_get(String, String) -> Option<String>
        self.builtins.insert(
            "json_get",
            BuiltinInfo {
                name: "json_get",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Option(Box::new(Type::String)),
                is_fallible: false,
            },
        );

        // json_get_int(String, String) -> Option<Int>
        self.builtins.insert(
            "json_get_int",
            BuiltinInfo {
                name: "json_get_int",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Option(Box::new(Type::Int)),
                is_fallible: false,
            },
        );

        // json_get_float(String, String) -> Option<Float>
        self.builtins.insert(
            "json_get_float",
            BuiltinInfo {
                name: "json_get_float",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Option(Box::new(Type::Float)),
                is_fallible: false,
            },
        );

        // json_get_bool(String, String) -> Option<Bool>
        self.builtins.insert(
            "json_get_bool",
            BuiltinInfo {
                name: "json_get_bool",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Option(Box::new(Type::Bool)),
                is_fallible: false,
            },
        );

        // json_get_list(String, String) -> Option<List<String>>
        self.builtins.insert(
            "json_get_list",
            BuiltinInfo {
                name: "json_get_list",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Option(Box::new(Type::List(Box::new(Type::String)))),
                is_fallible: false,
            },
        );

        // json_stringify(T) -> String (generic)
        self.builtins.insert(
            "json_stringify",
            BuiltinInfo {
                name: "json_stringify",
                params: None, // Generic - accepts any type
                return_type: Type::String,
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0010: Option Utilities
        // =========================================================================

        // is_some(Option<T>) -> Bool (generic)
        self.builtins.insert(
            "is_some",
            BuiltinInfo {
                name: "is_some",
                params: None,
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // is_none(Option<T>) -> Bool (generic)
        self.builtins.insert(
            "is_none",
            BuiltinInfo {
                name: "is_none",
                params: None,
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // unwrap(Option<T>) -> T fails (generic)
        self.builtins.insert(
            "unwrap",
            BuiltinInfo {
                name: "unwrap",
                params: None,
                return_type: Type::Error, // Determined by first arg
                is_fallible: false,
            },
        );

        // unwrap_or(Option<T>, T) -> T (generic)
        self.builtins.insert(
            "unwrap_or",
            BuiltinInfo {
                name: "unwrap_or",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // unwrap_or_else(Option<T>, Fn()->T) -> T (generic)
        self.builtins.insert(
            "unwrap_or_else",
            BuiltinInfo {
                name: "unwrap_or_else",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // map_option(Option<T>, Fn(T)->U) -> Option<U> (generic)
        self.builtins.insert(
            "map_option",
            BuiltinInfo {
                name: "map_option",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // or_option(Option<T>, Option<T>) -> Option<T> (generic)
        self.builtins.insert(
            "or_option",
            BuiltinInfo {
                name: "or_option",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0010: Result Utilities
        // =========================================================================

        // is_ok(Result<T, E>) -> Bool (generic)
        self.builtins.insert(
            "is_ok",
            BuiltinInfo {
                name: "is_ok",
                params: None,
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // is_err(Result<T, E>) -> Bool (generic)
        self.builtins.insert(
            "is_err",
            BuiltinInfo {
                name: "is_err",
                params: None,
                return_type: Type::Bool,
                is_fallible: false,
            },
        );

        // unwrap_result(Result<T, E>) -> T (generic, panics on Err)
        self.builtins.insert(
            "unwrap_result",
            BuiltinInfo {
                name: "unwrap_result",
                params: None,
                return_type: Type::Error, // Determined by first arg
                is_fallible: false,
            },
        );

        // unwrap_err(Result<T, E>) -> E (generic, panics on Ok)
        self.builtins.insert(
            "unwrap_err",
            BuiltinInfo {
                name: "unwrap_err",
                params: None,
                return_type: Type::Error, // Determined by first arg
                is_fallible: false,
            },
        );

        // unwrap_or_result(Result<T, E>, T) -> T (generic)
        self.builtins.insert(
            "unwrap_or_result",
            BuiltinInfo {
                name: "unwrap_or_result",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // map_result(Result<T, E>, Fn(T)->U) -> Result<U, E> (generic)
        self.builtins.insert(
            "map_result",
            BuiltinInfo {
                name: "map_result",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // map_err(Result<T, E>, Fn(E)->F) -> Result<T, F> (generic)
        self.builtins.insert(
            "map_err",
            BuiltinInfo {
                name: "map_err",
                params: None,
                return_type: Type::Error,
                is_fallible: false,
            },
        );

        // ok(Result<T, E>) -> Option<T> (generic)
        self.builtins.insert(
            "ok",
            BuiltinInfo {
                name: "ok",
                params: None,
                return_type: Type::Error, // Option<T>
                is_fallible: false,
            },
        );

        // err_value(Result<T, E>) -> Option<E> (generic)
        self.builtins.insert(
            "err_value",
            BuiltinInfo {
                name: "err_value",
                params: None,
                return_type: Type::Error, // Option<E>
                is_fallible: false,
            },
        );

        // =========================================================================
        // RFC-0012: Testing Framework - Assertion Builtins
        // These are only valid in _test.sg files (enforced by checker)
        // =========================================================================

        // assert(Bool) -> Unit
        self.builtins.insert(
            "assert",
            BuiltinInfo {
                name: "assert",
                params: Some(vec![Type::Bool]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_eq(T, T) -> Unit (generic)
        self.builtins.insert(
            "assert_eq",
            BuiltinInfo {
                name: "assert_eq",
                params: None, // Generic - special handling
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_neq(T, T) -> Unit (generic)
        self.builtins.insert(
            "assert_neq",
            BuiltinInfo {
                name: "assert_neq",
                params: None,
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_gt(T, T) -> Unit (generic - requires ordering)
        self.builtins.insert(
            "assert_gt",
            BuiltinInfo {
                name: "assert_gt",
                params: None,
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_lt(T, T) -> Unit
        self.builtins.insert(
            "assert_lt",
            BuiltinInfo {
                name: "assert_lt",
                params: None,
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_gte(T, T) -> Unit
        self.builtins.insert(
            "assert_gte",
            BuiltinInfo {
                name: "assert_gte",
                params: None,
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_lte(T, T) -> Unit
        self.builtins.insert(
            "assert_lte",
            BuiltinInfo {
                name: "assert_lte",
                params: None,
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_true(Bool) -> Unit
        self.builtins.insert(
            "assert_true",
            BuiltinInfo {
                name: "assert_true",
                params: Some(vec![Type::Bool]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_false(Bool) -> Unit
        self.builtins.insert(
            "assert_false",
            BuiltinInfo {
                name: "assert_false",
                params: Some(vec![Type::Bool]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_contains(String, String) -> Unit
        self.builtins.insert(
            "assert_contains",
            BuiltinInfo {
                name: "assert_contains",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_not_contains(String, String) -> Unit
        self.builtins.insert(
            "assert_not_contains",
            BuiltinInfo {
                name: "assert_not_contains",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_empty(String) -> Unit
        self.builtins.insert(
            "assert_empty",
            BuiltinInfo {
                name: "assert_empty",
                params: Some(vec![Type::String]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_not_empty(String) -> Unit
        self.builtins.insert(
            "assert_not_empty",
            BuiltinInfo {
                name: "assert_not_empty",
                params: Some(vec![Type::String]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_starts_with(String, String) -> Unit
        self.builtins.insert(
            "assert_starts_with",
            BuiltinInfo {
                name: "assert_starts_with",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_ends_with(String, String) -> Unit
        self.builtins.insert(
            "assert_ends_with",
            BuiltinInfo {
                name: "assert_ends_with",
                params: Some(vec![Type::String, Type::String]),
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_len(List<T>, Int) -> Unit
        self.builtins.insert(
            "assert_len",
            BuiltinInfo {
                name: "assert_len",
                params: None, // Generic
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_empty_list(List<T>) -> Unit
        self.builtins.insert(
            "assert_empty_list",
            BuiltinInfo {
                name: "assert_empty_list",
                params: None, // Generic
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_not_empty_list(List<T>) -> Unit
        self.builtins.insert(
            "assert_not_empty_list",
            BuiltinInfo {
                name: "assert_not_empty_list",
                params: None, // Generic
                return_type: Type::Unit,
                is_fallible: false,
            },
        );

        // assert_fails(T) -> Unit (for testing expected failures)
        self.builtins.insert(
            "assert_fails",
            BuiltinInfo {
                name: "assert_fails",
                params: None, // Generic
                return_type: Type::Unit,
                is_fallible: false,
            },
        );
    }

    /// Register extern function declarations as built-in functions.
    ///
    /// Extern fns have a Sage type signature but no body — the implementation
    /// is provided by external Rust code.  We resolve each parameter and return
    /// type from their AST `TypeExpr` and insert them into the builtins table
    /// so they are callable with full type checking.
    pub fn register_extern_fns(&mut self, extern_fns: &[sage_parser::ExternFnDecl]) {
        for ext_fn in extern_fns {
            let params: Vec<Type> = ext_fn
                .params
                .iter()
                .map(|p| resolve_type(&p.ty))
                .collect();
            let return_type = resolve_type(&ext_fn.return_ty);

            // BuiltinInfo requires &'static str — leak the heap String so it
            // lives for the duration of the process.  This is the same pattern
            // used for all other builtins whose names are string literals.
            let name: &'static str = ext_fn.name.name.clone().leak();

            self.builtins.insert(
                name,
                BuiltinInfo {
                    name,
                    params: Some(params),
                    return_type,
                    is_fallible: ext_fn.is_fallible,
                },
            );
        }
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

    /// Define a supervisor (v2).
    pub fn define_supervisor(&mut self, info: SupervisorInfo) {
        self.supervisors.insert(info.name.clone(), info);
    }

    /// Look up a supervisor by name.
    #[must_use]
    pub fn get_supervisor(&self, name: &str) -> Option<&SupervisorInfo> {
        self.supervisors.get(name)
    }

    /// Check if a supervisor is defined.
    #[must_use]
    pub fn has_supervisor(&self, name: &str) -> bool {
        self.supervisors.contains_key(name)
    }

    /// Iterate over all supervisors.
    pub fn iter_supervisors(&self) -> impl Iterator<Item = (&String, &SupervisorInfo)> {
        self.supervisors.iter()
    }

    /// Define a protocol (Phase 3: Session Types).
    pub fn define_protocol(&mut self, info: ProtocolInfo) {
        self.protocols.insert(info.name.clone(), info);
    }

    /// Look up a protocol by name.
    #[must_use]
    pub fn get_protocol(&self, name: &str) -> Option<&ProtocolInfo> {
        self.protocols.get(name)
    }

    /// Check if a protocol is defined.
    #[must_use]
    pub fn has_protocol(&self, name: &str) -> bool {
        self.protocols.contains_key(name)
    }

    /// Iterate over all protocols.
    pub fn iter_protocols(&self) -> impl Iterator<Item = (&String, &ProtocolInfo)> {
        self.protocols.iter()
    }

    /// Define an effect handler (Phase 3: Algebraic Effects).
    pub fn define_effect_handler(&mut self, info: EffectHandlerInfo) {
        self.effect_handlers.insert(info.name.clone(), info);
    }

    /// Look up an effect handler by name.
    #[must_use]
    pub fn get_effect_handler(&self, name: &str) -> Option<&EffectHandlerInfo> {
        self.effect_handlers.get(name)
    }

    /// Check if an effect handler is defined.
    #[must_use]
    pub fn has_effect_handler(&self, name: &str) -> bool {
        self.effect_handlers.contains_key(name)
    }

    /// Iterate over all effect handlers.
    pub fn iter_effect_handlers(&self) -> impl Iterator<Item = (&String, &EffectHandlerInfo)> {
        self.effect_handlers.iter()
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

    /// Check if a builtin is a test-only assertion (only valid in _test.sg files).
    #[must_use]
    pub fn is_test_assertion(&self, name: &str) -> bool {
        matches!(
            name,
            "assert"
                | "assert_eq"
                | "assert_neq"
                | "assert_gt"
                | "assert_lt"
                | "assert_gte"
                | "assert_lte"
                | "assert_true"
                | "assert_false"
                | "assert_contains"
                | "assert_not_contains"
                | "assert_empty"
                | "assert_not_empty"
                | "assert_starts_with"
                | "assert_ends_with"
                | "assert_len"
                | "assert_empty_list"
                | "assert_not_empty_list"
                | "assert_fails"
        )
    }
}

/// Convert a syntactic `TypeExpr` to a semantic Type.
/// This version doesn't handle type parameters - use `resolve_type_with_params` when in a generic context.
#[must_use]
pub fn resolve_type(ty: &TypeExpr) -> Type {
    resolve_type_with_params(ty, &std::collections::HashSet::new())
}

/// Convert a syntactic `TypeExpr` to a semantic Type, recognizing type parameters in scope.
/// `type_params` is the set of type parameter names that are currently in scope.
#[must_use]
pub fn resolve_type_with_params(
    ty: &TypeExpr,
    type_params: &std::collections::HashSet<String>,
) -> Type {
    match ty {
        TypeExpr::Int => Type::Int,
        TypeExpr::Float => Type::Float,
        TypeExpr::Bool => Type::Bool,
        TypeExpr::String => Type::String,
        TypeExpr::Unit => Type::Unit,
        TypeExpr::List(inner) => {
            Type::List(Box::new(resolve_type_with_params(inner, type_params)))
        }
        TypeExpr::Option(inner) => {
            Type::Option(Box::new(resolve_type_with_params(inner, type_params)))
        }
        TypeExpr::Oracle(inner) => {
            Type::Oracle(Box::new(resolve_type_with_params(inner, type_params)))
        }
        TypeExpr::Agent(ident) => Type::Agent(ident.name.clone()),
        TypeExpr::Named(ident, type_args) => {
            let name = &ident.name;

            // Check if this is a type parameter reference
            if type_params.contains(name) {
                return Type::TypeParam(name.clone());
            }

            // If there are type arguments, create a Generic type
            if !type_args.is_empty() {
                let resolved_args: Vec<Type> = type_args
                    .iter()
                    .map(|arg| resolve_type_with_params(arg, type_params))
                    .collect();
                return Type::Generic(name.clone(), resolved_args);
            }

            // Otherwise, it's a regular named type
            Type::Named(name.clone())
        }
        TypeExpr::Fn(params, ret) => {
            let param_types = params
                .iter()
                .map(|p| resolve_type_with_params(p, type_params))
                .collect();
            let ret_type = Box::new(resolve_type_with_params(ret, type_params));
            Type::Fn(param_types, ret_type)
        }
        TypeExpr::Map(key, value) => Type::Map(
            Box::new(resolve_type_with_params(key, type_params)),
            Box::new(resolve_type_with_params(value, type_params)),
        ),
        TypeExpr::Tuple(elems) => Type::Tuple(
            elems
                .iter()
                .map(|e| resolve_type_with_params(e, type_params))
                .collect(),
        ),
        TypeExpr::Result(ok, err) => Type::Result(
            Box::new(resolve_type_with_params(ok, type_params)),
            Box::new(resolve_type_with_params(err, type_params)),
        ),

        // RFC-0007: Error type (represented as named type for simplicity)
        TypeExpr::Error => Type::Named("Error".to_string()),
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

        let inferred_string = TypeExpr::Oracle(Box::new(TypeExpr::String));
        assert_eq!(
            resolve_type(&inferred_string),
            Type::Oracle(Box::new(Type::String))
        );
    }

    #[test]
    fn resolve_type_fn() {
        // Fn(Int) -> Bool
        let fn_type = TypeExpr::Fn(vec![TypeExpr::Int], Box::new(TypeExpr::Bool));
        assert_eq!(
            resolve_type(&fn_type),
            Type::Fn(vec![Type::Int], Box::new(Type::Bool))
        );

        // Fn(String, Int) -> Unit
        let fn_type = TypeExpr::Fn(
            vec![TypeExpr::String, TypeExpr::Int],
            Box::new(TypeExpr::Unit),
        );
        assert_eq!(
            resolve_type(&fn_type),
            Type::Fn(vec![Type::String, Type::Int], Box::new(Type::Unit))
        );

        // Fn() -> String (no parameters)
        let fn_type = TypeExpr::Fn(vec![], Box::new(TypeExpr::String));
        assert_eq!(
            resolve_type(&fn_type),
            Type::Fn(vec![], Box::new(Type::String))
        );
    }
}
