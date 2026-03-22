//! Main code generator.

use crate::emit::Emitter;
use sage_loader::{ModuleTree, SupervisionConfig};
use sage_parser::{
    AgentDecl, BinOp, Block, ConstDecl, EffectHandlerDecl, EnumDecl, EventKind, Expr, ExternFnDecl,
    FnDecl, Ident, Literal, MockValue, Program, ProtocolDecl, RecordDecl, RestartPolicy, Stmt,
    StringPart, SupervisionStrategy, SupervisorDecl, TestDecl, TypeExpr, UnaryOp,
};

/// How to specify the sage-runtime dependency in generated Cargo.toml.
#[derive(Debug, Clone)]
pub enum RuntimeDep {
    /// Use the published crates.io version.
    CratesIo { version: String },
    /// Use a local path (for development).
    Path { path: String },
}

impl Default for RuntimeDep {
    fn default() -> Self {
        // Default to crates.io with the current version
        Self::CratesIo {
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl RuntimeDep {
    /// Generate the Cargo.toml dependency line.
    fn to_cargo_dep(&self, feature: Option<&str>) -> String {
        match self {
            RuntimeDep::CratesIo { version } => {
                if let Some(feat) = feature {
                    format!("sage-runtime = {{ version = \"{version}\", features = [\"{feat}\"] }}")
                } else {
                    format!("sage-runtime = \"{version}\"")
                }
            }
            RuntimeDep::Path { path } => {
                if let Some(feat) = feature {
                    format!("sage-runtime = {{ path = \"{path}\", features = [\"{feat}\"] }}")
                } else {
                    format!("sage-runtime = {{ path = \"{path}\" }}")
                }
            }
        }
    }
}

/// Persistence backend configuration for @persistent fields.
#[derive(Debug, Clone, Default)]
pub enum PersistenceBackend {
    /// In-memory storage (no persistence across restarts).
    #[default]
    Memory,
    /// SQLite database storage.
    Sqlite { path: String },
    /// PostgreSQL database storage.
    Postgres { url: String },
    /// File-based JSON storage.
    File { path: String },
}

impl PersistenceBackend {
    /// Get the feature flag name for this backend, if any.
    fn feature_flag(&self) -> Option<&'static str> {
        match self {
            PersistenceBackend::Memory => None,
            PersistenceBackend::Sqlite { .. } => Some("persistence-sqlite"),
            PersistenceBackend::Postgres { .. } => Some("persistence-postgres"),
            PersistenceBackend::File { .. } => Some("persistence-file"),
        }
    }
}

/// Observability configuration for tracing and metrics.
#[derive(Debug, Clone, Default)]
pub struct ObservabilityConfig {
    /// Backend type: "ndjson", "otlp", or "none".
    pub backend: String,
    /// OTLP endpoint URL (for otlp backend).
    pub otlp_endpoint: Option<String>,
    /// Service name for trace attribution.
    pub service_name: String,
}

/// Full configuration for code generation.
#[derive(Debug, Clone, Default)]
pub struct CodegenConfig {
    /// How to specify the sage-runtime dependency.
    pub runtime_dep: RuntimeDep,
    /// Persistence backend configuration.
    pub persistence: PersistenceBackend,
    /// Supervision configuration (restart intensity limits).
    pub supervision: SupervisionConfig,
    /// Observability configuration for tracing.
    pub observability: ObservabilityConfig,
}

/// Generated Rust project files.
pub struct GeneratedProject {
    /// The main.rs content.
    pub main_rs: String,
    /// The Cargo.toml content.
    pub cargo_toml: String,
}

/// Generate Rust code from a Sage program (single file).
pub fn generate(program: &Program, project_name: &str) -> GeneratedProject {
    generate_with_config(program, project_name, RuntimeDep::default())
}

/// Generate Rust code from a Sage program with custom runtime dependency.
pub fn generate_with_config(
    program: &Program,
    project_name: &str,
    runtime_dep: RuntimeDep,
) -> GeneratedProject {
    generate_with_full_config(
        program,
        project_name,
        CodegenConfig {
            runtime_dep,
            persistence: PersistenceBackend::Memory,
            supervision: SupervisionConfig::default(),
            observability: ObservabilityConfig::default(),
        },
    )
}

/// Generate Rust code from a Sage program with full configuration.
pub fn generate_with_full_config(
    program: &Program,
    project_name: &str,
    config: CodegenConfig,
) -> GeneratedProject {
    let mut gen = Generator::new(config);
    let main_rs = gen.generate_program(program);
    let needs_persistence = Generator::has_persistent_fields(program);
    let cargo_toml = gen.generate_cargo_toml_with_persistence(project_name, needs_persistence);
    GeneratedProject {
        main_rs,
        cargo_toml,
    }
}

/// Generate Rust code from a module tree (multi-file project).
///
/// This flattens all modules into a single Rust file, generating all agents
/// and functions with appropriate visibility modifiers.
pub fn generate_module_tree(tree: &ModuleTree, project_name: &str) -> GeneratedProject {
    generate_module_tree_with_config(tree, project_name, RuntimeDep::default())
}

/// Generate Rust code from a module tree with custom runtime dependency.
pub fn generate_module_tree_with_config(
    tree: &ModuleTree,
    project_name: &str,
    runtime_dep: RuntimeDep,
) -> GeneratedProject {
    generate_module_tree_with_full_config(
        tree,
        project_name,
        CodegenConfig {
            runtime_dep,
            persistence: PersistenceBackend::Memory,
            supervision: SupervisionConfig::default(),
            observability: ObservabilityConfig::default(),
        },
    )
}

/// Generate Rust code from a module tree with full configuration.
pub fn generate_module_tree_with_full_config(
    tree: &ModuleTree,
    project_name: &str,
    config: CodegenConfig,
) -> GeneratedProject {
    let mut gen = Generator::new(config);
    let main_rs = gen.generate_module_tree(tree);
    let needs_persistence = Generator::has_persistent_fields_in_tree(tree);
    let cargo_toml = gen.generate_cargo_toml_with_persistence(project_name, needs_persistence);
    GeneratedProject {
        main_rs,
        cargo_toml,
    }
}

/// Generated test project files (RFC-0012).
pub struct GeneratedTestProject {
    /// The test main.rs content.
    pub main_rs: String,
    /// The Cargo.toml content.
    pub cargo_toml: String,
}

/// Generate a test binary from a Sage test file (RFC-0012).
pub fn generate_test_program(program: &Program, test_name: &str) -> GeneratedTestProject {
    generate_test_program_with_config(program, test_name, RuntimeDep::default())
}

/// Generate a test binary with custom runtime dependency.
pub fn generate_test_program_with_config(
    program: &Program,
    test_name: &str,
    runtime_dep: RuntimeDep,
) -> GeneratedTestProject {
    let mut gen = Generator::with_runtime_dep(runtime_dep);
    let main_rs = gen.generate_test_binary(program);
    let cargo_toml = gen.generate_test_cargo_toml(test_name);
    GeneratedTestProject {
        main_rs,
        cargo_toml,
    }
}

/// Information about an agent's message handlers.
#[derive(Clone)]
struct AgentMessageHandlers {
    /// List of (param_name, param_type) for each message handler.
    handlers: Vec<(Ident, TypeExpr)>,
}

struct Generator {
    emit: Emitter,
    config: CodegenConfig,
    /// Variables that are reassigned in the current scope
    reassigned_vars: std::collections::HashSet<String>,
    /// Agents that have on_error handlers
    agents_with_error_handlers: std::collections::HashSet<String>,
    /// Agents that have message handlers (agent_name -> handler info)
    agents_with_message_handlers: std::collections::HashMap<String, AgentMessageHandlers>,
    /// Phase 3: Current agent's protocol roles (protocol_name -> role_name)
    current_protocol_roles: std::collections::HashMap<String, String>,
    /// v2.0: Current agent's @persistent belief field names (for checkpoint())
    current_agent_persistent_beliefs: Vec<String>,
    /// Agent tool uses (agent_name -> list of tool names) for summon generation
    agent_tool_uses: std::collections::HashMap<String, Vec<String>>,
    /// Extern function names (for sage_extern:: dispatch)
    extern_fn_names: std::collections::HashSet<String>,
    /// Subset of extern fns that are fallible (marked with `fails`)
    extern_fn_fallible: std::collections::HashSet<String>,
    /// String constant names (need .to_string() when referenced)
    string_consts: std::collections::HashSet<String>,
}

impl Generator {
    fn new(config: CodegenConfig) -> Self {
        Self {
            emit: Emitter::new(),
            config,
            reassigned_vars: std::collections::HashSet::new(),
            agents_with_error_handlers: std::collections::HashSet::new(),
            agents_with_message_handlers: std::collections::HashMap::new(),
            current_protocol_roles: std::collections::HashMap::new(),
            current_agent_persistent_beliefs: Vec::new(),
            agent_tool_uses: std::collections::HashMap::new(),
            extern_fn_names: std::collections::HashSet::new(),
            extern_fn_fallible: std::collections::HashSet::new(),
            string_consts: std::collections::HashSet::new(),
        }
    }

    /// Create a generator with only a runtime dependency (backwards compatible).
    fn with_runtime_dep(runtime_dep: RuntimeDep) -> Self {
        Self::new(CodegenConfig {
            runtime_dep,
            persistence: PersistenceBackend::Memory,
            supervision: SupervisionConfig::default(),
            observability: ObservabilityConfig::default(),
        })
    }

    /// Collect extern function names and fallibility from declarations.
    fn collect_extern_fns(&mut self, extern_fns: &[ExternFnDecl]) {
        for ext_fn in extern_fns {
            self.extern_fn_names.insert(ext_fn.name.name.clone());
            if ext_fn.is_fallible {
                self.extern_fn_fallible.insert(ext_fn.name.name.clone());
            }
        }
    }

    /// Emit the checkpoint store initialization based on configured backend.
    fn emit_checkpoint_store_init(&mut self) {
        match &self.config.persistence {
            PersistenceBackend::Memory => {
                self.emit
                    .writeln("sage_runtime::persistence::MemoryCheckpointStore::new()");
            }
            PersistenceBackend::Sqlite { path } => {
                self.emit
                    .write("sage_runtime::persistence::SyncSqliteStore::open(\"");
                self.emit.write(path);
                self.emit
                    .writeln("\").expect(\"Failed to open checkpoint database\")");
            }
            PersistenceBackend::Postgres { url } => {
                self.emit
                    .write("sage_runtime::persistence::SyncPostgresStore::connect(\"");
                self.emit.write(url);
                self.emit
                    .writeln("\").expect(\"Failed to connect to checkpoint database\")");
            }
            PersistenceBackend::File { path } => {
                self.emit
                    .write("sage_runtime::persistence::SyncFileStore::open(\"");
                self.emit.write(path);
                self.emit
                    .writeln("\").expect(\"Failed to open checkpoint directory\")");
            }
        }
    }

    /// Emit tracing initialization based on configured backend.
    fn emit_tracing_init(&mut self) {
        let obs = &self.config.observability;
        if obs.backend.is_empty() || obs.backend == "ndjson" {
            // Default: use environment-based init for NDJSON
            self.emit.writeln("sage_runtime::trace::init();");
        } else {
            // Use config-based init
            self.emit.writeln(
                "sage_runtime::trace::init_with_config(sage_runtime::trace::TracingConfig {",
            );
            self.emit.indent();
            self.emit.write("backend: \"");
            self.emit.write(&obs.backend);
            self.emit.writeln("\".to_string(),");
            if let Some(endpoint) = &obs.otlp_endpoint {
                self.emit.write("otlp_endpoint: Some(\"");
                self.emit.write(endpoint);
                self.emit.writeln("\".to_string()),");
            } else {
                self.emit.writeln("otlp_endpoint: None,");
            }
            self.emit.write("service_name: \"");
            self.emit.write(if obs.service_name.is_empty() {
                "sage-agent"
            } else {
                &obs.service_name
            });
            self.emit.writeln("\".to_string(),");
            self.emit.dedent();
            self.emit.writeln("});");
        }
    }

    /// Scan a block to find all variables that are reassigned (Stmt::Assign)
    fn collect_reassigned_vars(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.collect_reassigned_vars_stmt(stmt);
        }
    }

    fn collect_reassigned_vars_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign { name, .. } => {
                self.reassigned_vars.insert(name.name.clone());
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                self.collect_reassigned_vars(then_block);
                if let Some(else_branch) = else_block {
                    match else_branch {
                        sage_parser::ElseBranch::Block(block) => {
                            self.collect_reassigned_vars(block)
                        }
                        sage_parser::ElseBranch::ElseIf(stmt) => {
                            self.collect_reassigned_vars_stmt(stmt)
                        }
                    }
                }
            }
            Stmt::While { body, .. } | Stmt::Loop { body, .. } => {
                self.collect_reassigned_vars(body);
            }
            Stmt::For { body, .. } => {
                self.collect_reassigned_vars(body);
            }
            _ => {}
        }
    }

    fn generate_program(&mut self, program: &Program) -> String {
        // Collect extern function names before generating any code
        self.collect_extern_fns(&program.extern_fns);

        // Prelude
        self.emit
            .writeln("//! Generated by Sage compiler. Do not edit.");
        self.emit.blank_line();
        self.emit.writeln("use sage_runtime::prelude::*;");
        self.emit.blank_line();

        // Extern module declaration (implementation provided by user Rust code)
        if !program.extern_fns.is_empty() {
            self.emit.writeln("mod sage_extern;");
            self.emit.blank_line();
        }

        // Constants
        for const_decl in &program.consts {
            self.generate_const(const_decl);
            self.emit.blank_line();
        }

        // Enums
        for enum_decl in &program.enums {
            self.generate_enum(enum_decl);
            self.emit.blank_line();
        }

        // Records
        for record in &program.records {
            self.generate_record(record);
            self.emit.blank_line();
        }

        // Functions
        for func in &program.functions {
            self.generate_function(func);
            self.emit.blank_line();
        }

        // Phase 3: Protocols (generate state machine modules)
        for protocol in &program.protocols {
            self.generate_protocol(protocol);
            self.emit.blank_line();
        }

        // Phase 3: Effect handlers (generate InferConfig structs)
        for handler in &program.effect_handlers {
            self.generate_effect_handler(handler);
            self.emit.blank_line();
        }

        // Pre-pass: Collect agent metadata for summon generation
        for agent in &program.agents {
            self.collect_agent_metadata(agent);
        }

        // Agents
        for agent in &program.agents {
            self.generate_agent(agent);
            self.emit.blank_line();
        }

        // Supervisors
        for supervisor in &program.supervisors {
            self.generate_supervisor(supervisor, program);
            self.emit.blank_line();
        }

        // Entry point (required for executables)
        if let Some(run_entry) = &program.run_agent {
            // First check if it's an agent
            if let Some(agent) = program
                .agents
                .iter()
                .find(|a| a.name.name == run_entry.name)
            {
                self.generate_main(agent);
            } else if let Some(supervisor) = program
                .supervisors
                .iter()
                .find(|s| s.name.name == run_entry.name)
            {
                // It's a supervisor entry point
                self.generate_supervisor_main(supervisor, program);
            }
        }

        std::mem::take(&mut self.emit).finish()
    }

    fn generate_module_tree(&mut self, tree: &ModuleTree) -> String {
        // Pre-pass: Collect agent metadata and extern fns from all modules
        let mut has_extern_fns = false;
        for module in tree.modules.values() {
            for agent in &module.program.agents {
                self.collect_agent_metadata(agent);
            }
            if !module.program.extern_fns.is_empty() {
                has_extern_fns = true;
                self.collect_extern_fns(&module.program.extern_fns);
            }
        }

        // Prelude
        self.emit
            .writeln("//! Generated by Sage compiler. Do not edit.");
        self.emit.blank_line();
        self.emit.writeln("use sage_runtime::prelude::*;");
        self.emit.blank_line();

        // Extern module declaration (implementation provided by user Rust code)
        if has_extern_fns {
            self.emit.writeln("mod sage_extern;");
            self.emit.blank_line();
        }

        // Generate all modules, starting with the root
        // We flatten everything into one file for simplicity
        // (A more advanced implementation would generate mod.rs files)

        // First, generate non-root modules
        for (path, module) in &tree.modules {
            if path != &tree.root {
                self.emit.write("// Module: ");
                if path.is_empty() {
                    self.emit.writeln("(root)");
                } else {
                    self.emit.writeln(&path.join("::"));
                }

                for const_decl in &module.program.consts {
                    self.generate_const(const_decl);
                    self.emit.blank_line();
                }

                for enum_decl in &module.program.enums {
                    self.generate_enum(enum_decl);
                    self.emit.blank_line();
                }

                for record in &module.program.records {
                    self.generate_record(record);
                    self.emit.blank_line();
                }

                for func in &module.program.functions {
                    self.generate_function(func);
                    self.emit.blank_line();
                }

                // Phase 3: Protocols
                for protocol in &module.program.protocols {
                    self.generate_protocol(protocol);
                    self.emit.blank_line();
                }

                // Phase 3: Effect handlers
                for handler in &module.program.effect_handlers {
                    self.generate_effect_handler(handler);
                    self.emit.blank_line();
                }

                for agent in &module.program.agents {
                    self.generate_agent(agent);
                    self.emit.blank_line();
                }

                for supervisor in &module.program.supervisors {
                    self.generate_supervisor(supervisor, &module.program);
                    self.emit.blank_line();
                }
            }
        }

        // Then, generate the root module
        if let Some(root_module) = tree.modules.get(&tree.root) {
            self.emit.writeln("// Root module");

            for const_decl in &root_module.program.consts {
                self.generate_const(const_decl);
                self.emit.blank_line();
            }

            for enum_decl in &root_module.program.enums {
                self.generate_enum(enum_decl);
                self.emit.blank_line();
            }

            for record in &root_module.program.records {
                self.generate_record(record);
                self.emit.blank_line();
            }

            for func in &root_module.program.functions {
                self.generate_function(func);
                self.emit.blank_line();
            }

            // Phase 3: Protocols
            for protocol in &root_module.program.protocols {
                self.generate_protocol(protocol);
                self.emit.blank_line();
            }

            // Phase 3: Effect handlers
            for handler in &root_module.program.effect_handlers {
                self.generate_effect_handler(handler);
                self.emit.blank_line();
            }

            for agent in &root_module.program.agents {
                self.generate_agent(agent);
                self.emit.blank_line();
            }

            for supervisor in &root_module.program.supervisors {
                self.generate_supervisor(supervisor, &root_module.program);
                self.emit.blank_line();
            }

            // Entry point (only in root module)
            if let Some(run_entry) = &root_module.program.run_agent {
                // First check if it's an agent
                if let Some(agent) = root_module
                    .program
                    .agents
                    .iter()
                    .find(|a| a.name.name == run_entry.name)
                {
                    self.generate_main(agent);
                } else if let Some(supervisor) = root_module
                    .program
                    .supervisors
                    .iter()
                    .find(|s| s.name.name == run_entry.name)
                {
                    // It's a supervisor entry point
                    self.generate_supervisor_main(supervisor, &root_module.program);
                }
            }
        }

        std::mem::take(&mut self.emit).finish()
    }

    #[allow(dead_code)]
    fn generate_cargo_toml(&self, name: &str) -> String {
        self.generate_cargo_toml_impl(name, false)
    }

    fn generate_cargo_toml_with_persistence(&self, name: &str, needs_persistence: bool) -> String {
        self.generate_cargo_toml_impl(name, needs_persistence)
    }

    fn generate_cargo_toml_impl(&self, name: &str, needs_persistence: bool) -> String {
        // Get the feature flag for the configured persistence backend
        let feature_flag = if needs_persistence {
            self.config.persistence.feature_flag()
        } else {
            None
        };
        let runtime_dep = self.config.runtime_dep.to_cargo_dep(feature_flag);

        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
{runtime_dep}
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"

# Standalone project, not part of parent workspace
[workspace]
"#
        )
    }

    /// Check if a program has any agents with @persistent fields.
    fn has_persistent_fields(program: &Program) -> bool {
        program
            .agents
            .iter()
            .any(|agent| agent.beliefs.iter().any(|b| b.is_persistent))
    }

    /// Check if any module in a tree has agents with @persistent fields.
    fn has_persistent_fields_in_tree(tree: &ModuleTree) -> bool {
        tree.modules
            .values()
            .any(|module| Self::has_persistent_fields(&module.program))
    }

    // =========================================================================
    // RFC-0012: Test generation
    // =========================================================================

    fn generate_test_binary(&mut self, program: &Program) -> String {
        // Test prelude
        self.emit
            .writeln("//! Generated test file by Sage compiler. Do not edit.");
        self.emit.blank_line();
        self.emit.writeln("#![allow(unused_imports, dead_code)]");
        self.emit.blank_line();
        self.emit.writeln("use sage_runtime::prelude::*;");
        self.emit.blank_line();

        // Constants (test files may import types/constants from main code)
        for const_decl in &program.consts {
            self.generate_const(const_decl);
            self.emit.blank_line();
        }

        // Enums
        for enum_decl in &program.enums {
            self.generate_enum(enum_decl);
            self.emit.blank_line();
        }

        // Records
        for record in &program.records {
            self.generate_record(record);
            self.emit.blank_line();
        }

        // Functions
        for func in &program.functions {
            self.generate_function(func);
            self.emit.blank_line();
        }

        // Agents (test files may define helper agents)
        for agent in &program.agents {
            self.generate_agent(agent);
            self.emit.blank_line();
        }

        // Separate serial and concurrent tests
        let (serial_tests, concurrent_tests): (Vec<_>, Vec<_>) =
            program.tests.iter().partition(|t| t.is_serial);

        // Generate concurrent test functions
        for test in &concurrent_tests {
            self.generate_test_function(test);
            self.emit.blank_line();
        }

        // Generate serial test functions (marked with #[serial])
        for test in &serial_tests {
            self.generate_test_function(test);
            self.emit.blank_line();
        }

        // Generate an empty main function (required for bin crates)
        self.emit.writeln("fn main() {}");

        std::mem::take(&mut self.emit).finish()
    }

    fn generate_test_cargo_toml(&self, name: &str) -> String {
        let runtime_dep = self.config.runtime_dep.to_cargo_dep(None);
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
{runtime_dep}
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"

# Standalone project, not part of parent workspace
[workspace]
"#
        )
    }

    fn generate_test_function(&mut self, test: &TestDecl) {
        // Collect mock statements from the test body
        let mock_infers = self.collect_mock_infers(&test.body);
        let mock_tools = self.collect_mock_tools(&test.body);

        // Generate test function
        self.emit.writeln("#[tokio::test]");
        // Convert test name to valid Rust identifier
        let test_fn_name = self.sanitize_test_name(&test.name);
        self.emit.write("async fn ");
        self.emit.write(&test_fn_name);
        self.emit.writeln("() {");
        self.emit.indent();

        // Generate mock LLM client if there are mock divines
        if !mock_infers.is_empty() {
            self.emit
                .writeln("let _mock_client = MockLlmClient::with_responses(vec![");
            self.emit.indent();
            for mock in &mock_infers {
                match mock {
                    MockValue::Value(expr) => {
                        self.emit.write("MockResponse::value(");
                        self.generate_expr(expr);
                        self.emit.writeln("),");
                    }
                    MockValue::Fail(expr) => {
                        self.emit.write("MockResponse::fail(");
                        self.generate_expr(expr);
                        self.emit.writeln("),");
                    }
                }
            }
            self.emit.dedent();
            self.emit.writeln("]);");
            self.emit.blank_line();
        }

        // Generate mock tool registry if there are mock tools
        if !mock_tools.is_empty() {
            self.emit
                .writeln("let _mock_tools = MockToolRegistry::new();");
            for (tool_name, fn_name, value) in &mock_tools {
                self.emit.write("_mock_tools.register(\"");
                self.emit.write(tool_name);
                self.emit.write("\", \"");
                self.emit.write(fn_name);
                self.emit.write("\", ");
                match value {
                    MockValue::Value(expr) => {
                        self.emit.write("MockResponse::value(");
                        self.generate_expr(expr);
                        self.emit.write(")");
                    }
                    MockValue::Fail(expr) => {
                        self.emit.write("MockResponse::fail(");
                        self.generate_expr(expr);
                        self.emit.write(")");
                    }
                }
                self.emit.writeln(");");
            }
            self.emit.blank_line();

            // Wrap test body in with_mock_tools context
            self.emit.writeln("with_mock_tools(_mock_tools, async {");
            self.emit.indent();
            self.generate_test_block(&test.body);
            self.emit.dedent();
            self.emit.writeln("}).await;");
        } else {
            // No tool mocks, generate test body directly
            self.generate_test_block(&test.body);
        }

        self.emit.dedent();
        self.emit.writeln("}");
    }

    fn collect_mock_infers(&self, block: &Block) -> Vec<MockValue> {
        let mut mocks = Vec::new();
        for stmt in &block.stmts {
            if let Stmt::MockDivine { value, .. } = stmt {
                mocks.push(value.clone());
            }
        }
        mocks
    }

    fn collect_mock_tools(&self, block: &Block) -> Vec<(String, String, MockValue)> {
        let mut mocks = Vec::new();
        for stmt in &block.stmts {
            if let Stmt::MockTool {
                tool_name,
                fn_name,
                value,
                ..
            } = stmt
            {
                mocks.push((tool_name.name.clone(), fn_name.name.clone(), value.clone()));
            }
        }
        mocks
    }

    fn generate_test_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            // Skip mock divine and mock tool statements - they were collected separately
            if matches!(stmt, Stmt::MockDivine { .. } | Stmt::MockTool { .. }) {
                continue;
            }
            self.generate_test_stmt(stmt);
        }
    }

    fn generate_test_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            // Handle assertion builtins specially
            Stmt::Expr { expr, .. } => {
                if let Expr::Call { name, args, .. } = expr {
                    if self.is_assertion_builtin(&name.name) {
                        self.generate_assertion(&name.name, args);
                        return;
                    }
                }
                // Regular expression statement
                self.generate_expr(expr);
                self.emit.writeln(";");
            }
            // For other statements, use the normal generation
            _ => self.generate_stmt(stmt),
        }
    }

    fn is_assertion_builtin(&self, name: &str) -> bool {
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

    fn generate_assertion(&mut self, name: &str, args: &[Expr]) {
        match name {
            "assert" | "assert_true" => {
                self.emit.write("assert!(");
                if !args.is_empty() {
                    self.generate_expr(&args[0]);
                }
                self.emit.writeln(");");
            }
            "assert_false" => {
                self.emit.write("assert!(!");
                if !args.is_empty() {
                    self.generate_expr(&args[0]);
                }
                self.emit.writeln(");");
            }
            "assert_eq" => {
                self.emit.write("assert_eq!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(", ");
                    self.generate_expr(&args[1]);
                }
                self.emit.writeln(");");
            }
            "assert_neq" => {
                self.emit.write("assert_ne!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(", ");
                    self.generate_expr(&args[1]);
                }
                self.emit.writeln(");");
            }
            "assert_gt" => {
                self.emit.write("assert!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(" > ");
                    self.generate_expr(&args[1]);
                }
                self.emit.writeln(");");
            }
            "assert_lt" => {
                self.emit.write("assert!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(" < ");
                    self.generate_expr(&args[1]);
                }
                self.emit.writeln(");");
            }
            "assert_gte" => {
                self.emit.write("assert!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(" >= ");
                    self.generate_expr(&args[1]);
                }
                self.emit.writeln(");");
            }
            "assert_lte" => {
                self.emit.write("assert!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(" <= ");
                    self.generate_expr(&args[1]);
                }
                self.emit.writeln(");");
            }
            "assert_contains" => {
                self.emit.write("assert!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(".contains(&");
                    self.generate_expr(&args[1]);
                    self.emit.write(")");
                }
                self.emit.writeln(");");
            }
            "assert_not_contains" => {
                self.emit.write("assert!(!");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(".contains(&");
                    self.generate_expr(&args[1]);
                    self.emit.write(")");
                }
                self.emit.writeln(");");
            }
            "assert_empty" => {
                self.emit.write("assert!(");
                if !args.is_empty() {
                    self.generate_expr(&args[0]);
                }
                self.emit.writeln(".is_empty());");
            }
            "assert_not_empty" => {
                self.emit.write("assert!(!");
                if !args.is_empty() {
                    self.generate_expr(&args[0]);
                }
                self.emit.writeln(".is_empty());");
            }
            "assert_starts_with" => {
                self.emit.write("assert!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(".starts_with(&");
                    self.generate_expr(&args[1]);
                    self.emit.write(")");
                }
                self.emit.writeln(");");
            }
            "assert_ends_with" => {
                self.emit.write("assert!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(".ends_with(&");
                    self.generate_expr(&args[1]);
                    self.emit.write(")");
                }
                self.emit.writeln(");");
            }
            "assert_len" => {
                self.emit.write("assert_eq!(");
                if args.len() >= 2 {
                    self.generate_expr(&args[0]);
                    self.emit.write(".len() as i64, ");
                    self.generate_expr(&args[1]);
                }
                self.emit.writeln(");");
            }
            "assert_empty_list" => {
                self.emit.write("assert!(");
                if !args.is_empty() {
                    self.generate_expr(&args[0]);
                }
                self.emit.writeln(".is_empty());");
            }
            "assert_not_empty_list" => {
                self.emit.write("assert!(!");
                if !args.is_empty() {
                    self.generate_expr(&args[0]);
                }
                self.emit.writeln(".is_empty());");
            }
            "assert_fails" => {
                // assert_fails expects an expression that should fail
                self.emit.writeln("{");
                self.emit.indent();
                self.emit.write("let result = ");
                if !args.is_empty() {
                    self.generate_expr(&args[0]);
                }
                self.emit.writeln(";");
                self.emit.writeln(
                    "assert!(result.is_err(), \"Expected operation to fail but it succeeded\");",
                );
                self.emit.dedent();
                self.emit.writeln("}");
            }
            _ => {
                // Unknown assertion - just call it as a regular function
                self.emit.write(name);
                self.emit.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.generate_expr(arg);
                }
                self.emit.writeln(");");
            }
        }
    }

    fn sanitize_test_name(&self, name: &str) -> String {
        // Convert test name to valid Rust identifier
        name.chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .to_lowercase()
    }

    fn generate_const(&mut self, const_decl: &ConstDecl) {
        if const_decl.is_pub {
            self.emit.write("pub ");
        }
        self.emit.write("const ");
        self.emit.write(&const_decl.name.name);
        self.emit.write(": ");
        // String constants must use &'static str since .to_string() isn't const
        let is_string = matches!(const_decl.ty, TypeExpr::String);
        if is_string {
            self.string_consts.insert(const_decl.name.name.clone());
            self.emit.write("&'static str");
        } else {
            self.emit_type(&const_decl.ty);
        }
        self.emit.write(" = ");
        // For string constants, emit raw string literal without .to_string()
        if is_string {
            if let Expr::Literal {
                value: Literal::String(s),
                ..
            } = &const_decl.value
            {
                self.emit.write("\"");
                self.emit.write(&Self::escape_string_for_rust(s));
                self.emit.write("\"");
            } else {
                self.generate_expr(&const_decl.value);
            }
        } else {
            self.generate_expr(&const_decl.value);
        }
        self.emit.writeln(";");
    }

    fn generate_enum(&mut self, enum_decl: &EnumDecl) {
        if enum_decl.is_pub {
            self.emit.write("pub ");
        }
        // Generic enums can't be Copy since type params may not be Copy
        if enum_decl.type_params.is_empty() {
            self.emit
                .writeln("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
        } else {
            self.emit.writeln("#[derive(Debug, Clone, PartialEq, Eq)]");
        }
        self.emit.write("enum ");
        self.emit.write(&enum_decl.name.name);
        // RFC-0015: Emit type parameters
        if !enum_decl.type_params.is_empty() {
            self.emit.write("<");
            for (i, param) in enum_decl.type_params.iter().enumerate() {
                if i > 0 {
                    self.emit.write(", ");
                }
                self.emit.write(&param.name);
            }
            self.emit.write(">");
        }
        self.emit.writeln(" {");
        self.emit.indent();
        for variant in &enum_decl.variants {
            self.emit.write(&variant.name.name);
            if let Some(payload_ty) = &variant.payload {
                self.emit.write("(");
                self.emit_type(payload_ty);
                self.emit.write(")");
            }
            self.emit.writeln(",");
        }
        self.emit.dedent();
        self.emit.writeln("}");
    }

    fn generate_record(&mut self, record: &RecordDecl) {
        if record.is_pub {
            self.emit.write("pub ");
        }
        // Derive serde traits for message serialization
        self.emit
            .writeln("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]");
        self.emit.write("struct ");
        self.emit.write(&record.name.name);
        // RFC-0015: Emit type parameters
        if !record.type_params.is_empty() {
            self.emit.write("<");
            for (i, param) in record.type_params.iter().enumerate() {
                if i > 0 {
                    self.emit.write(", ");
                }
                self.emit.write(&param.name);
            }
            self.emit.write(">");
        }
        self.emit.writeln(" {");
        self.emit.indent();
        for field in &record.fields {
            self.emit.write(&field.name.name);
            self.emit.write(": ");
            self.emit_type(&field.ty);
            self.emit.writeln(",");
        }
        self.emit.dedent();
        self.emit.writeln("}");
    }

    fn generate_function(&mut self, func: &FnDecl) {
        // Function signature with visibility
        if func.is_pub {
            self.emit.write("pub ");
        }
        self.emit.write("fn ");
        self.emit.write(&func.name.name);
        // RFC-0015: Emit type parameters
        if !func.type_params.is_empty() {
            self.emit.write("<");
            for (i, param) in func.type_params.iter().enumerate() {
                if i > 0 {
                    self.emit.write(", ");
                }
                self.emit.write(&param.name);
            }
            self.emit.write(">");
        }
        self.emit.write("(");

        for (i, param) in func.params.iter().enumerate() {
            if i > 0 {
                self.emit.write(", ");
            }
            self.emit.write(&param.name.name);
            self.emit.write(": ");
            self.emit_type(&param.ty);
        }

        self.emit.write(") -> ");

        // RFC-0007: Wrap return type in SageResult if fallible
        if func.is_fallible {
            self.emit.write("SageResult<");
            self.emit_type(&func.return_ty);
            self.emit.write(">");
        } else {
            self.emit_type(&func.return_ty);
        }

        self.emit.write(" ");
        self.generate_block(&func.body);
    }

    /// Collect metadata about an agent (error handlers, message handlers) for use in summon generation.
    /// This is called in a pre-pass before any code generation.
    fn collect_agent_metadata(&mut self, agent: &AgentDecl) {
        let name = &agent.name.name;

        // Track if this agent has an error handler
        let has_error_handler = agent
            .handlers
            .iter()
            .any(|h| matches!(h.event, EventKind::Error { .. }));
        if has_error_handler {
            self.agents_with_error_handlers.insert(name.clone());
        }

        // Track tool uses for summon generation
        if !agent.tool_uses.is_empty() {
            let tool_names: Vec<String> = agent.tool_uses.iter().map(|t| t.name.clone()).collect();
            self.agent_tool_uses.insert(name.clone(), tool_names);
        }

        // Track message handlers
        let message_handlers: Vec<_> = agent
            .handlers
            .iter()
            .filter_map(|h| {
                if let EventKind::Message {
                    param_name,
                    param_ty,
                } = &h.event
                {
                    Some((param_name.clone(), param_ty.clone()))
                } else {
                    None
                }
            })
            .collect();
        if !message_handlers.is_empty() {
            self.agents_with_message_handlers.insert(
                name.clone(),
                AgentMessageHandlers {
                    handlers: message_handlers,
                },
            );
        }
    }

    fn generate_agent(&mut self, agent: &AgentDecl) {
        let name = &agent.name.name;

        // Phase 3: Set current protocol roles for this agent (used by reply() generation)
        self.current_protocol_roles.clear();
        for pr in &agent.follows {
            self.current_protocol_roles
                .insert(pr.protocol.name.clone(), pr.role.name.clone());
        }

        // v2.0: Set current agent's @persistent beliefs (used by checkpoint() generation)
        self.current_agent_persistent_beliefs = agent
            .beliefs
            .iter()
            .filter(|b| b.is_persistent)
            .map(|b| b.name.name.clone())
            .collect();

        // RFC-0011: Check for tool usage
        let has_tools = !agent.tool_uses.is_empty();
        let needs_struct_body = !agent.beliefs.is_empty() || has_tools;

        // Struct definition with visibility
        if agent.is_pub {
            self.emit.write("pub ");
        }
        self.emit.write("struct ");
        self.emit.write(name);
        if !needs_struct_body {
            self.emit.writeln(";");
        } else {
            self.emit.writeln(" {");
            self.emit.indent();

            // RFC-0011: Generate tool fields
            for tool_use in &agent.tool_uses {
                // Generate field like: http: HttpClient
                self.emit.write(&tool_use.name.to_lowercase());
                self.emit.write(": ");
                self.emit.write(&tool_use.name);
                self.emit.writeln("Client,");
            }

            // Check if this agent has any @persistent fields
            let has_persistent = agent.beliefs.iter().any(|b| b.is_persistent);

            // Add checkpoint store field if agent has persistent beliefs
            if has_persistent {
                self.emit
                    .writeln("_checkpoint: std::sync::Arc<dyn CheckpointStore>,");
                self.emit.writeln("_checkpoint_key: String,");
            }

            // Regular belief fields - wrap @persistent ones in Persisted<T>
            for belief in &agent.beliefs {
                self.emit.write(&belief.name.name);
                self.emit.write(": ");
                if belief.is_persistent {
                    self.emit.write("Persisted<");
                    self.emit_type(&belief.ty);
                    self.emit.write(">");
                } else {
                    self.emit_type(&belief.ty);
                }
                self.emit.writeln(",");
            }
            self.emit.dedent();
            self.emit.writeln("}");
        }
        self.emit.blank_line();

        // Find the output type from the start handler
        let output_type = self.infer_agent_output_type(agent);

        // Impl block
        self.emit.write("impl ");
        self.emit.write(name);
        self.emit.writeln(" {");
        self.emit.indent();

        // Generate handlers
        for handler in &agent.handlers {
            match &handler.event {
                // v2 lifecycle: on waking - runs before start, after persistent state loaded
                EventKind::Waking => {
                    self.emit.writeln("async fn on_waking(&self) {");
                    self.emit.indent();
                    self.generate_block_contents(&handler.body);
                    self.emit.dedent();
                    self.emit.writeln("}");
                }

                EventKind::Start => {
                    self.emit
                        .write("async fn on_start(&self, ctx: &mut AgentContext<");
                    self.emit.write(&output_type);
                    self.emit.write(">) -> SageResult<");
                    self.emit.write(&output_type);
                    self.emit.writeln("> {");
                    self.emit.indent();
                    self.generate_block_contents(&handler.body);
                    self.emit.dedent();
                    self.emit.writeln("}");
                }

                // RFC-0007: Generate on_error handler
                EventKind::Error { param_name } => {
                    self.emit.write("async fn on_error(&self, _");
                    self.emit.write(&param_name.name);
                    self.emit.write(": SageError, ctx: &mut AgentContext<");
                    self.emit.write(&output_type);
                    self.emit.write(">) -> SageResult<");
                    self.emit.write(&output_type);
                    self.emit.writeln("> {");
                    self.emit.indent();
                    self.generate_block_contents(&handler.body);
                    // Fallback: re-raise the error if handler doesn't explicitly return
                    self.emit.write("Err(_");
                    self.emit.write(&param_name.name);
                    self.emit.writeln(")");
                    self.emit.dedent();
                    self.emit.writeln("}");
                }

                // v2 lifecycle: on pause - runs when supervisor signals graceful pause
                EventKind::Pause => {
                    self.emit.writeln("async fn on_pause(&self) {");
                    self.emit.indent();
                    self.generate_block_contents(&handler.body);
                    self.emit.dedent();
                    self.emit.writeln("}");
                }

                // v2 lifecycle: on resume - runs when agent is unpaused
                EventKind::Resume => {
                    self.emit.writeln("async fn on_resume(&self) {");
                    self.emit.indent();
                    self.generate_block_contents(&handler.body);
                    self.emit.dedent();
                    self.emit.writeln("}");
                }

                // on stop handler - cleanup before termination
                EventKind::Stop => {
                    self.emit.writeln("async fn on_stop(&self) {");
                    self.emit.indent();
                    self.generate_block_contents(&handler.body);
                    self.emit.dedent();
                    self.emit.writeln("}");
                }

                // v2 lifecycle: on resting - alias for stop
                EventKind::Resting => {
                    self.emit.writeln("async fn on_stop(&self) {");
                    self.emit.indent();
                    self.generate_block_contents(&handler.body);
                    self.emit.dedent();
                    self.emit.writeln("}");
                }

                // Message handler: on message(param: Type)
                EventKind::Message {
                    param_name,
                    param_ty,
                } => {
                    // Generate method name from type, e.g., on_message_ping for Ping
                    let type_name = self.type_expr_to_string(param_ty);
                    let method_name = format!("on_message_{}", Self::to_snake_case(&type_name));

                    self.emit.write("async fn ");
                    self.emit.write(&method_name);
                    self.emit.write("(&self, ");
                    self.emit.write(&param_name.name);
                    self.emit.write(": ");
                    self.emit_type(param_ty);
                    self.emit.write(", ctx: &mut AgentContext<");
                    self.emit.write(&output_type);
                    self.emit.write(">) -> SageResult<()> ");
                    self.emit.writeln("{");
                    self.emit.indent();
                    self.generate_block_contents(&handler.body);
                    self.emit.writeln("Ok(())");
                    self.emit.dedent();
                    self.emit.writeln("}");
                }
            }
        }

        self.emit.dedent();
        self.emit.writeln("}");
    }

    fn generate_main(&mut self, agent: &AgentDecl) {
        let entry_agent = &agent.name.name;

        // Phase 3: Set current protocol roles for this agent (used by reply() generation)
        self.current_protocol_roles.clear();
        for pr in &agent.follows {
            self.current_protocol_roles
                .insert(pr.protocol.name.clone(), pr.role.name.clone());
        }

        // v2.0: Set current agent's @persistent beliefs (used by checkpoint() generation)
        self.current_agent_persistent_beliefs = agent
            .beliefs
            .iter()
            .filter(|b| b.is_persistent)
            .map(|b| b.name.name.clone())
            .collect();

        let has_error_handler = agent
            .handlers
            .iter()
            .any(|h| matches!(h.event, EventKind::Error { .. }));

        let has_stop_handler = agent
            .handlers
            .iter()
            .any(|h| matches!(h.event, EventKind::Stop | EventKind::Resting));

        // RFC-0011: Check if agent uses tools
        let has_tools = !agent.tool_uses.is_empty();

        // v2.0: Check if agent has @persistent fields
        let has_persistent = agent.beliefs.iter().any(|b| b.is_persistent);

        // v2.0: Check if agent has on_waking handler
        let has_waking = agent
            .handlers
            .iter()
            .any(|h| matches!(h.event, EventKind::Waking));

        // Collect message handlers for dispatch loop
        let message_handlers: Vec<_> = agent
            .handlers
            .iter()
            .filter_map(|h| {
                if let EventKind::Message {
                    param_name,
                    param_ty,
                } = &h.event
                {
                    Some((param_name.clone(), param_ty.clone()))
                } else {
                    None
                }
            })
            .collect();
        let has_message_handlers = !message_handlers.is_empty();

        self.emit.writeln("#[tokio::main]");
        self.emit
            .writeln("async fn main() -> Result<(), Box<dyn std::error::Error>> {");
        self.emit.indent();

        // Initialize tracing with config
        self.emit_tracing_init();
        self.emit.writeln("");

        // v2.0: Initialize checkpoint store if agent has persistent fields
        if has_persistent {
            self.emit
                .writeln("// Initialize persistence checkpoint store");
            self.emit.writeln(
                "let _checkpoint: std::sync::Arc<dyn CheckpointStore> = std::sync::Arc::new(",
            );
            self.emit.indent();
            self.emit_checkpoint_store_init();
            self.emit.dedent();
            self.emit.writeln(");");
            self.emit.write("let _checkpoint_key = \"");
            self.emit.write(entry_agent);
            self.emit.writeln("_entry\".to_string();");
            self.emit.writeln("");
        }

        // Helper to generate agent construction (with or without tool/persistent fields)
        let needs_struct = has_tools || !agent.beliefs.is_empty();
        let agent_construct = if needs_struct {
            let mut s = format!("{entry_agent} {{ ");
            let mut fields = Vec::new();

            // Add checkpoint fields if persistent
            if has_persistent {
                fields.push("_checkpoint: std::sync::Arc::clone(&_checkpoint)".to_string());
                fields.push("_checkpoint_key: _checkpoint_key.clone()".to_string());
            }

            // Add tool fields
            for tool_use in &agent.tool_uses {
                if tool_use.name == "Database" {
                    fields.push(format!("{}: _db_client", tool_use.name.to_lowercase()));
                } else {
                    fields.push(format!(
                        "{}: {}Client::from_env()",
                        tool_use.name.to_lowercase(),
                        tool_use.name
                    ));
                }
            }

            // Add belief fields
            for belief in &agent.beliefs {
                if belief.is_persistent {
                    // Persisted fields load from checkpoint
                    fields.push(format!(
                        "{}: Persisted::new(std::sync::Arc::clone(&_checkpoint), &_checkpoint_key, \"{}\")",
                        belief.name.name,
                        belief.name.name
                    ));
                } else {
                    // Non-persistent beliefs use Default
                    fields.push(format!("{}: Default::default()", belief.name.name));
                }
            }

            s.push_str(&fields.join(", "));
            s.push_str(" }");
            s
        } else {
            entry_agent.to_string()
        };

        // Set up graceful shutdown signal handling
        self.emit
            .writeln("let ctrl_c = async { tokio::signal::ctrl_c().await.ok() };");

        self.emit.writeln("#[cfg(unix)]");
        self.emit.writeln("let terminate = async {");
        self.emit.indent();
        self.emit.writeln(
            "if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {",
        );
        self.emit.indent();
        self.emit.writeln("s.recv().await;");
        self.emit.dedent();
        self.emit.writeln("} else {");
        self.emit.indent();
        self.emit.writeln("std::future::pending::<()>().await;");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.dedent();
        self.emit.writeln("};");
        self.emit.writeln("#[cfg(not(unix))]");
        self.emit
            .writeln("let terminate = std::future::pending::<()>();");
        self.emit.writeln("");

        self.emit
            .writeln("let handle = sage_runtime::spawn(move |mut ctx| async move {");
        self.emit.indent();

        // RFC-0011: Initialize async tools (like Database) before agent construction
        for tool_use in &agent.tool_uses {
            if tool_use.name == "Database" {
                // Database requires async initialization
                self.emit
                    .write("let _db_client = DatabaseClient::from_env().await");
                self.emit
                    .writeln(".expect(\"Failed to connect to database\");");
            }
        }

        self.emit.write("let agent = ");
        self.emit.write(&agent_construct);
        self.emit.writeln(";");

        // v2.0: Call on_waking after persistent state is loaded
        if has_waking {
            self.emit.writeln("");
            self.emit
                .writeln("// on_waking: runs after persistent state loaded, before on_start");
            self.emit.writeln("agent.on_waking().await;");
        }

        if has_error_handler {
            // RFC-0007: Generate error dispatch code
            self.emit
                .writeln("let result = match agent.on_start(&mut ctx).await {");
            self.emit.indent();
            self.emit.writeln("Ok(result) => Ok(result),");
            self.emit
                .writeln("Err(e) => agent.on_error(e, &mut ctx).await,");
            self.emit.dedent();
            self.emit.writeln("};");
        } else {
            // Simple case: no error handler
            self.emit
                .writeln("let result = agent.on_start(&mut ctx).await;");
        }

        // Phase 3: Collect protocol roles for validation
        let protocol_roles: Vec<_> = agent
            .follows
            .iter()
            .map(|pr| (pr.protocol.name.clone(), pr.role.name.clone()))
            .collect();
        let has_protocols = !protocol_roles.is_empty();

        // Message receive loop (if agent has message handlers)
        if has_message_handlers {
            self.emit.writeln("");
            self.emit.writeln("// Message receive loop");
            self.emit.writeln("if result.is_ok() {");
            self.emit.indent();
            self.emit.writeln("loop {");
            self.emit.indent();
            self.emit.writeln("match ctx.receive_raw().await {");
            self.emit.indent();
            self.emit.writeln("Ok(msg) => {");
            self.emit.indent();
            self.emit.writeln("// Dispatch based on message type");
            self.emit.writeln("match msg.type_name.as_deref() {");
            self.emit.indent();

            // Generate match arms for each message type
            for (param_name, param_ty) in &message_handlers {
                let type_name = self.type_expr_to_string(param_ty);
                let method_name = format!("on_message_{}", Self::to_snake_case(&type_name));

                self.emit.write("Some(\"");
                self.emit.write(&type_name);
                self.emit.writeln("\") => {");
                self.emit.indent();

                // Phase 3: Validate protocol state for this message type
                if has_protocols {
                    // Find which protocol role handles this message type
                    // For simplicity, use the first role that might handle it
                    // (in practice, the checker ensures only one protocol/role combo is valid)
                    if let Some((_, role)) = protocol_roles.first() {
                        self.emit.writeln("// Phase 3: Validate protocol state");
                        self.emit.write("ctx.validate_protocol_receive(\"");
                        self.emit.write(&type_name);
                        self.emit.write("\", \"");
                        self.emit.write(role);
                        self.emit.writeln("\").await?;");
                    }
                }

                self.emit.write("let ");
                self.emit.write(&param_name.name);
                self.emit.write(": ");
                self.emit_type(param_ty);
                self.emit.writeln(" = serde_json::from_value(msg.payload)");
                self.emit.indent();
                self.emit.writeln(".map_err(|e| SageError::Agent(format!(\"Failed to deserialize message: {e}\")))?;");
                self.emit.dedent();
                self.emit.write("agent.");
                self.emit.write(&method_name);
                self.emit.write("(");
                self.emit.write(&param_name.name);
                self.emit.writeln(", &mut ctx).await?;");
                self.emit.dedent();
                self.emit.writeln("}");
            }

            // Default case for unknown message types
            self.emit.writeln("_ => {");
            self.emit.indent();
            self.emit.writeln("// Unknown message type, skip");
            self.emit.dedent();
            self.emit.writeln("}");

            self.emit.dedent();
            self.emit.writeln("}"); // close match type_name
            self.emit.dedent();
            self.emit.writeln("}"); // close Ok(msg)
            self.emit.writeln("Err(_) => break, // Channel closed");
            self.emit.dedent();
            self.emit.writeln("}"); // close match receive_raw
            self.emit.dedent();
            self.emit.writeln("}"); // close loop
            self.emit.dedent();
            self.emit.writeln("}"); // close if result.is_ok()
        }

        if has_stop_handler {
            // Call on_stop for cleanup (errors are ignored)
            self.emit.writeln("agent.on_stop().await;");
        }

        self.emit.writeln("result");
        self.emit.dedent();
        self.emit.writeln("});");

        // Use tokio::select! to race between agent completion and shutdown signals
        self.emit.writeln("");
        self.emit.writeln("let _result = tokio::select! {");
        self.emit.indent();
        self.emit.writeln("result = handle.result() => result?,");
        self.emit.writeln("_ = ctrl_c => {");
        self.emit.indent();
        self.emit
            .writeln("eprintln!(\"\\nReceived interrupt signal, shutting down...\");");
        self.emit.writeln("std::process::exit(0);");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.writeln("_ = terminate => {");
        self.emit.indent();
        self.emit
            .writeln("eprintln!(\"Received terminate signal, shutting down...\");");
        self.emit.writeln("std::process::exit(0);");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.dedent();
        self.emit.writeln("};");
        self.emit.writeln("Ok(())");

        self.emit.dedent();
        self.emit.writeln("}");
    }

    /// Generate a supervisor declaration.
    ///
    /// This generates a struct for the supervisor and doesn't generate handlers
    /// since the supervisor itself doesn't have handlers - it manages child agents.
    fn generate_supervisor(&mut self, supervisor: &SupervisorDecl, _program: &Program) {
        let name = &supervisor.name.name;

        // Comment indicating this is a supervisor
        self.emit.write("// Supervisor: ");
        self.emit.writeln(name);

        // Struct definition with visibility (just a marker struct)
        if supervisor.is_pub {
            self.emit.write("pub ");
        }
        self.emit.write("struct ");
        self.emit.write(name);
        self.emit.writeln(";");
    }

    /// Generate the main function for a supervisor entry point.
    ///
    /// This creates a Supervisor instance, adds all child agents with their
    /// spawn functions and restart policies, then runs the supervisor.
    fn generate_supervisor_main(&mut self, supervisor: &SupervisorDecl, program: &Program) {
        let name = &supervisor.name.name;

        self.emit.writeln("#[tokio::main]");
        self.emit
            .writeln("async fn main() -> Result<(), Box<dyn std::error::Error>> {");
        self.emit.indent();

        // Initialize tracing with config
        self.emit_tracing_init();
        self.emit.writeln("");

        // Create the supervisor with the configured strategy
        self.emit
            .write("let mut supervisor = Supervisor::new(Strategy::");
        match supervisor.strategy {
            SupervisionStrategy::OneForOne => self.emit.write("OneForOne"),
            SupervisionStrategy::OneForAll => self.emit.write("OneForAll"),
            SupervisionStrategy::RestForOne => self.emit.write("RestForOne"),
        }
        self.emit.writeln(&format!(
            ", RestartConfig {{ max_restarts: {}, within: std::time::Duration::from_secs({}) }});",
            self.config.supervision.max_restarts, self.config.supervision.within_seconds
        ));
        self.emit.writeln("");

        // Add each child with its spawn function
        for child in &supervisor.children {
            let child_agent_name = &child.agent_name.name;

            // Find the agent declaration to understand its structure
            let agent = program
                .agents
                .iter()
                .find(|a| a.name.name == *child_agent_name);

            // Phase 3: Set current protocol roles for this child agent
            self.current_protocol_roles.clear();
            if let Some(agent) = agent {
                for pr in &agent.follows {
                    self.current_protocol_roles
                        .insert(pr.protocol.name.clone(), pr.role.name.clone());
                }
            }

            // v2.0: Set current agent's @persistent beliefs (used by checkpoint() generation)
            self.current_agent_persistent_beliefs = if let Some(agent) = agent {
                agent
                    .beliefs
                    .iter()
                    .filter(|b| b.is_persistent)
                    .map(|b| b.name.name.clone())
                    .collect()
            } else {
                Vec::new()
            };

            // Determine restart policy
            let restart_policy = match child.restart {
                RestartPolicy::Permanent => "Permanent",
                RestartPolicy::Transient => "Transient",
                RestartPolicy::Temporary => "Temporary",
            };

            self.emit.write("supervisor.add_child(\"");
            self.emit.write(child_agent_name);
            self.emit.write("\", RestartPolicy::");
            self.emit.write(restart_policy);
            self.emit.writeln(", || {");
            self.emit.indent();

            // Generate the spawn closure body
            self.emit.writeln("async {");
            self.emit.indent();

            // Check if agent has tools, beliefs, or persistence
            let (has_tools, has_beliefs, has_persistent) = if let Some(agent) = agent {
                let has_tools = !agent.tool_uses.is_empty();
                let has_beliefs = !agent.beliefs.is_empty();
                let has_persistent = agent.beliefs.iter().any(|b| b.is_persistent);
                (has_tools, has_beliefs, has_persistent)
            } else {
                (false, false, false)
            };

            // Initialize checkpoint store if needed
            if has_persistent {
                self.emit.writeln(
                    "let _checkpoint: std::sync::Arc<dyn CheckpointStore> = std::sync::Arc::new(",
                );
                self.emit.indent();
                self.emit_checkpoint_store_init();
                self.emit.dedent();
                self.emit.writeln(");");
                self.emit.write("let _checkpoint_key = \"");
                self.emit.write(child_agent_name);
                self.emit.writeln("\".to_string();");
            }

            // Initialize async tools (like Database)
            if let Some(agent) = agent {
                for tool_use in &agent.tool_uses {
                    if tool_use.name == "Database" {
                        self.emit
                            .write("let _db_client = DatabaseClient::from_env().await");
                        self.emit
                            .writeln(".expect(\"Failed to connect to database\");");
                    }
                }
            }

            // Construct the agent
            if has_tools || has_beliefs {
                self.emit.write("let agent = ");
                self.emit.write(child_agent_name);
                self.emit.writeln(" {");
                self.emit.indent();

                // Add checkpoint fields if persistent
                if has_persistent {
                    self.emit
                        .writeln("_checkpoint: std::sync::Arc::clone(&_checkpoint),");
                    self.emit
                        .writeln("_checkpoint_key: _checkpoint_key.clone(),");
                }

                // Add tool fields
                if let Some(agent) = agent {
                    for tool_use in &agent.tool_uses {
                        if tool_use.name == "Database" {
                            self.emit.write(&tool_use.name.to_lowercase());
                            self.emit.writeln(": _db_client,");
                        } else {
                            self.emit.write(&tool_use.name.to_lowercase());
                            self.emit.write(": ");
                            self.emit.write(&tool_use.name);
                            self.emit.writeln("Client::from_env(),");
                        }
                    }
                }

                // Add belief fields from child spec's initial values
                if let Some(agent) = agent {
                    for belief in &agent.beliefs {
                        // Check if there's an initial value in child spec
                        let init_value = child
                            .beliefs
                            .iter()
                            .find(|f| f.name.name == belief.name.name);

                        self.emit.write(&belief.name.name);
                        self.emit.write(": ");

                        if belief.is_persistent {
                            if let Some(init) = init_value {
                                // Persisted with initial value from child spec
                                self.emit.write("Persisted::with_initial(std::sync::Arc::clone(&_checkpoint), &_checkpoint_key, \"");
                                self.emit.write(&belief.name.name);
                                self.emit.write("\", ");
                                self.generate_expr(&init.value);
                                self.emit.write(")");
                            } else {
                                // Persisted with default
                                self.emit.write("Persisted::new(std::sync::Arc::clone(&_checkpoint), &_checkpoint_key, \"");
                                self.emit.write(&belief.name.name);
                                self.emit.write("\")");
                            }
                        } else if let Some(init) = init_value {
                            // Non-persistent belief with initial value from child spec
                            self.generate_expr(&init.value);
                        } else {
                            // Non-persistent belief with default
                            self.emit.write("Default::default()");
                        }
                        self.emit.writeln(",");
                    }
                }

                self.emit.dedent();
                self.emit.writeln("};");
            } else {
                self.emit.write("let agent = ");
                self.emit.write(child_agent_name);
                self.emit.writeln(";");
            }

            // Check for waking handler
            let has_waking = agent
                .map(|a| {
                    a.handlers
                        .iter()
                        .any(|h| matches!(h.event, EventKind::Waking))
                })
                .unwrap_or(false);

            // Check for error handler
            let has_error_handler = agent
                .map(|a| {
                    a.handlers
                        .iter()
                        .any(|h| matches!(h.event, EventKind::Error { .. }))
                })
                .unwrap_or(false);

            // Check for stop handler
            let has_stop_handler = agent
                .map(|a| {
                    a.handlers
                        .iter()
                        .any(|h| matches!(h.event, EventKind::Stop | EventKind::Resting))
                })
                .unwrap_or(false);

            // Phase 3: Check for Infer effect handler assignment
            let infer_handler = child
                .handler_assignments
                .iter()
                .find(|ha| ha.effect.name == "Infer")
                .map(|ha| ha.handler.name.clone());

            // Create the agent handle and run it
            if let Some(handler_name) = &infer_handler {
                // Generate LlmConfig from handler
                self.emit
                    .writeln("// Phase 3: Use effect handler configuration");
                self.emit
                    .write("let llm_config = sage_runtime::LlmConfig::with_model(handler_");
                self.emit.write(&Self::to_snake_case(handler_name));
                self.emit.writeln("::CONFIG.model)");
                self.emit.indent();
                // Add temperature if configured
                self.emit.write(".with_temperature(handler_");
                self.emit.write(&Self::to_snake_case(handler_name));
                self.emit.writeln("::CONFIG.temperature)");
                // Add max_tokens if configured
                self.emit.write(".with_max_tokens(handler_");
                self.emit.write(&Self::to_snake_case(handler_name));
                self.emit.writeln("::CONFIG.max_tokens);");
                self.emit.dedent();
                self.emit.writeln(
                    "let handle = sage_runtime::spawn_with_llm_config(move |mut ctx| async move {",
                );
            } else {
                self.emit
                    .writeln("let handle = sage_runtime::spawn(move |mut ctx| async move {");
            }
            self.emit.indent();

            // Call on_waking if present
            if has_waking {
                self.emit.writeln("agent.on_waking().await;");
            }

            // Generate the main execution with error handling
            if has_error_handler {
                self.emit
                    .writeln("let result = match agent.on_start(&mut ctx).await {");
                self.emit.indent();
                self.emit.writeln("Ok(result) => Ok(result),");
                self.emit
                    .writeln("Err(e) => agent.on_error(e, &mut ctx).await,");
                self.emit.dedent();
                self.emit.writeln("};");
            } else {
                self.emit
                    .writeln("let result = agent.on_start(&mut ctx).await;");
            }

            // Call on_stop if present
            if has_stop_handler {
                self.emit.writeln("agent.on_stop().await;");
            }

            self.emit.writeln("result");
            self.emit.dedent();
            self.emit.writeln("});");

            // Wait for the handle result, discarding the value and returning Ok(())
            self.emit
                .writeln("handle.result().await.map_err(|e| SageError::Agent(e.to_string()))?;");
            self.emit.writeln("Ok(())");

            self.emit.dedent();
            self.emit.writeln("}");
            self.emit.dedent();
            self.emit.writeln("});");
            self.emit.writeln("");
        }

        // Set up graceful shutdown signal handling
        self.emit
            .writeln("let ctrl_c = async { tokio::signal::ctrl_c().await.ok() };");
        self.emit.writeln("");
        self.emit.writeln("#[cfg(unix)]");
        self.emit.writeln("let terminate = async {");
        self.emit.indent();
        self.emit.writeln("if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {");
        self.emit.indent();
        self.emit.writeln("s.recv().await;");
        self.emit.dedent();
        self.emit.writeln("} else {");
        self.emit.indent();
        self.emit.writeln("std::future::pending::<()>().await;");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.dedent();
        self.emit.writeln("};");
        self.emit.writeln("#[cfg(not(unix))]");
        self.emit
            .writeln("let terminate = std::future::pending::<()>();");
        self.emit.writeln("");

        // Run the supervisor with signal handling
        let child_count = supervisor.children.len();
        self.emit.write("eprintln!(\"Starting supervisor '");
        self.emit.write(name);
        self.emit.write("' with ");
        self.emit.write(&child_count.to_string());
        self.emit.writeln(" children...\");");
        self.emit.writeln("");
        self.emit.writeln("let result = tokio::select! {");
        self.emit.indent();
        self.emit.writeln("result = supervisor.run() => result,");
        self.emit.writeln("_ = ctrl_c => {");
        self.emit.indent();
        self.emit
            .writeln("eprintln!(\"\\nReceived interrupt signal, shutting down supervisor...\");");
        self.emit.writeln("return Ok(());");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.writeln("_ = terminate => {");
        self.emit.indent();
        self.emit
            .writeln("eprintln!(\"Received terminate signal, shutting down supervisor...\");");
        self.emit.writeln("return Ok(());");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.dedent();
        self.emit.writeln("};");
        self.emit.writeln("");
        self.emit.writeln("if let Err(e) = result {");
        self.emit.indent();
        self.emit.writeln("eprintln!(\"Supervisor error: {}\", e);");
        self.emit.writeln("return Err(e.into());");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.writeln("");
        self.emit.writeln("Ok(())");

        self.emit.dedent();
        self.emit.writeln("}");
    }

    // =========================================================================
    // Phase 3: Protocol generation (Session Types)
    // =========================================================================

    /// Generate a protocol state machine module.
    ///
    /// For a protocol like:
    /// ```sage
    /// protocol PingPong {
    ///     Pinger -> Ponger: Ping
    ///     Ponger -> Pinger: Pong
    /// }
    /// ```
    ///
    /// We generate a module with:
    /// - An enum for the protocol states
    /// - An implementation of `ProtocolStateMachine` trait
    fn generate_protocol(&mut self, protocol: &ProtocolDecl) {
        let name = &protocol.name.name;
        let mod_name = Self::to_snake_case(name);

        // Comment
        self.emit.write("// Protocol: ");
        self.emit.writeln(name);

        // Open module
        if protocol.is_pub {
            self.emit.write("pub ");
        }
        self.emit.write("mod protocol_");
        self.emit.write(&mod_name);
        self.emit.writeln(" {");
        self.emit.indent();

        self.emit.writeln("use super::*;");
        self.emit.blank_line();

        // Generate state enum
        // States are: Initial, then one state after each step, plus Done
        self.emit
            .writeln("#[derive(Debug, Clone, Copy, PartialEq, Eq)]");
        self.emit.writeln("pub enum State {");
        self.emit.indent();
        self.emit.writeln("Initial,");
        for (i, step) in protocol.steps.iter().enumerate() {
            self.emit.write("After");
            self.emit.write(&Self::capitalize(&step.sender.name));
            self.emit.write("Sends");
            self.generate_type_name_for_state(&step.message_type);
            self.emit.write("_");
            self.emit.write(&i.to_string());
            self.emit.writeln(",");
        }
        self.emit.writeln("Done,");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.blank_line();

        // Generate Default impl
        self.emit.writeln("impl Default for State {");
        self.emit.indent();
        self.emit.writeln("fn default() -> Self {");
        self.emit.indent();
        self.emit.writeln("Self::Initial");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.blank_line();

        // Generate ProtocolStateMachine impl
        self.emit.writeln("impl ProtocolStateMachine for State {");
        self.emit.indent();

        // state_name()
        self.emit.writeln("fn state_name(&self) -> &str {");
        self.emit.indent();
        self.emit.writeln("match self {");
        self.emit.indent();
        self.emit.writeln("State::Initial => \"Initial\",");
        for (i, step) in protocol.steps.iter().enumerate() {
            self.emit.write("State::After");
            self.emit.write(&Self::capitalize(&step.sender.name));
            self.emit.write("Sends");
            self.generate_type_name_for_state(&step.message_type);
            self.emit.write("_");
            self.emit.write(&i.to_string());
            self.emit.write(" => \"After");
            self.emit.write(&Self::capitalize(&step.sender.name));
            self.emit.write("Sends");
            self.generate_type_name_for_state(&step.message_type);
            self.emit.writeln("\",");
        }
        self.emit.writeln("State::Done => \"Done\",");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.blank_line();

        // can_send()
        self.emit
            .writeln("fn can_send(&self, msg_type: &str, from_role: &str) -> bool {");
        self.emit.indent();
        self.emit.writeln("match (self, msg_type, from_role) {");
        self.emit.indent();
        for (i, step) in protocol.steps.iter().enumerate() {
            let state_name = if i == 0 {
                "State::Initial".to_string()
            } else {
                let prev = &protocol.steps[i - 1];
                format!(
                    "State::After{}Sends{}_{}",
                    Self::capitalize(&prev.sender.name),
                    Self::type_name_for_state(&prev.message_type),
                    i - 1
                )
            };
            self.emit.write("(");
            self.emit.write(&state_name);
            self.emit.write(", \"");
            self.generate_type_name_for_state(&step.message_type);
            self.emit.write("\", \"");
            self.emit.write(&step.sender.name);
            self.emit.writeln("\") => true,");
        }
        self.emit.writeln("_ => false,");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.blank_line();

        // can_receive()
        self.emit
            .writeln("fn can_receive(&self, msg_type: &str, to_role: &str) -> bool {");
        self.emit.indent();
        self.emit.writeln("match (self, msg_type, to_role) {");
        self.emit.indent();
        for (i, step) in protocol.steps.iter().enumerate() {
            let state_name = if i == 0 {
                "State::Initial".to_string()
            } else {
                let prev = &protocol.steps[i - 1];
                format!(
                    "State::After{}Sends{}_{}",
                    Self::capitalize(&prev.sender.name),
                    Self::type_name_for_state(&prev.message_type),
                    i - 1
                )
            };
            self.emit.write("(");
            self.emit.write(&state_name);
            self.emit.write(", \"");
            self.generate_type_name_for_state(&step.message_type);
            self.emit.write("\", \"");
            self.emit.write(&step.receiver.name);
            self.emit.writeln("\") => true,");
        }
        self.emit.writeln("_ => false,");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.blank_line();

        // transition()
        self.emit
            .writeln("fn transition(&mut self, msg_type: &str) -> Result<(), ProtocolViolation> {");
        self.emit.indent();
        self.emit.writeln("let next = match (&self, msg_type) {");
        self.emit.indent();
        for (i, step) in protocol.steps.iter().enumerate() {
            let state_name = if i == 0 {
                "State::Initial".to_string()
            } else {
                let prev = &protocol.steps[i - 1];
                format!(
                    "State::After{}Sends{}_{}",
                    Self::capitalize(&prev.sender.name),
                    Self::type_name_for_state(&prev.message_type),
                    i - 1
                )
            };
            let next_state = if i + 1 >= protocol.steps.len() {
                "State::Done".to_string()
            } else {
                format!(
                    "State::After{}Sends{}_{}",
                    Self::capitalize(&step.sender.name),
                    Self::type_name_for_state(&step.message_type),
                    i
                )
            };
            self.emit.write("(");
            self.emit.write(&state_name);
            self.emit.write(", \"");
            self.generate_type_name_for_state(&step.message_type);
            self.emit.write("\") => ");
            self.emit.write(&next_state);
            self.emit.writeln(",");
        }
        self.emit
            .writeln("_ => return Err(ProtocolViolation::UnexpectedMessage {");
        self.emit.indent();
        self.emit.write("protocol: \"");
        self.emit.write(name);
        self.emit.writeln("\".to_string(),");
        self.emit.writeln("expected: \"unknown\".to_string(),");
        self.emit.writeln("received: msg_type.to_string(),");
        self.emit.writeln("state: self.state_name().to_string(),");
        self.emit.dedent();
        self.emit.writeln("}),");
        self.emit.dedent();
        self.emit.writeln("};");
        self.emit.writeln("*self = next;");
        self.emit.writeln("Ok(())");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.blank_line();

        // is_terminal()
        self.emit.writeln("fn is_terminal(&self) -> bool {");
        self.emit.indent();
        self.emit.writeln("matches!(self, State::Done)");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.blank_line();

        // protocol_name()
        self.emit.writeln("fn protocol_name(&self) -> &str {");
        self.emit.indent();
        self.emit.write("\"");
        self.emit.write(name);
        self.emit.writeln("\"");
        self.emit.dedent();
        self.emit.writeln("}");
        self.emit.blank_line();

        // clone_box()
        self.emit
            .writeln("fn clone_box(&self) -> Box<dyn ProtocolStateMachine> {");
        self.emit.indent();
        self.emit.writeln("Box::new(*self)");
        self.emit.dedent();
        self.emit.writeln("}");

        self.emit.dedent();
        self.emit.writeln("}"); // impl ProtocolStateMachine

        // Close module
        self.emit.dedent();
        self.emit.writeln("}"); // mod
    }

    /// Convert a name to snake_case.
    fn to_snake_case(name: &str) -> String {
        let mut result = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_uppercase() {
                if i > 0 {
                    result.push('_');
                }
                result.push(c.to_ascii_lowercase());
            } else {
                result.push(c);
            }
        }
        result
    }

    /// Capitalize the first letter of a name.
    fn capitalize(name: &str) -> String {
        let mut chars = name.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Generate a type name suitable for state enum variant names.
    fn generate_type_name_for_state(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Int => self.emit.write("Int"),
            TypeExpr::Float => self.emit.write("Float"),
            TypeExpr::String => self.emit.write("String"),
            TypeExpr::Bool => self.emit.write("Bool"),
            TypeExpr::Unit => self.emit.write("Unit"),
            TypeExpr::Named(ident, _) => self.emit.write(&ident.name),
            TypeExpr::List(_) => self.emit.write("List"),
            TypeExpr::Map(_, _) => self.emit.write("Map"),
            TypeExpr::Option(_) => self.emit.write("Option"),
            TypeExpr::Result(_, _) => self.emit.write("Result"),
            TypeExpr::Oracle(_) => self.emit.write("Oracle"),
            TypeExpr::Agent(_) => self.emit.write("Agent"),
            TypeExpr::Tuple(_) => self.emit.write("Tuple"),
            TypeExpr::Fn(_, _) => self.emit.write("Fn"),
            TypeExpr::Error => self.emit.write("Error"),
        }
    }

    /// Get the type name as a string (for state generation).
    fn type_name_for_state(ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Int => "Int".to_string(),
            TypeExpr::Float => "Float".to_string(),
            TypeExpr::String => "String".to_string(),
            TypeExpr::Bool => "Bool".to_string(),
            TypeExpr::Unit => "Unit".to_string(),
            TypeExpr::Named(ident, _) => ident.name.clone(),
            TypeExpr::List(_) => "List".to_string(),
            TypeExpr::Map(_, _) => "Map".to_string(),
            TypeExpr::Option(_) => "Option".to_string(),
            TypeExpr::Result(_, _) => "Result".to_string(),
            TypeExpr::Oracle(_) => "Oracle".to_string(),
            TypeExpr::Agent(_) => "Agent".to_string(),
            TypeExpr::Tuple(_) => "Tuple".to_string(),
            TypeExpr::Fn(_, _) => "Fn".to_string(),
            TypeExpr::Error => "Error".to_string(),
        }
    }

    // =========================================================================
    // Phase 3: Effect handler generation (Algebraic Effects)
    // =========================================================================

    /// Generate an effect handler struct with configuration.
    ///
    /// For a handler like:
    /// ```sage
    /// handler DefaultLLM handles Infer {
    ///     model: "gpt-4o"
    ///     temperature: 0.7
    /// }
    /// ```
    ///
    /// We generate a struct with the configuration values.
    fn generate_effect_handler(&mut self, handler: &EffectHandlerDecl) {
        let name = &handler.name.name;
        let effect = &handler.effect.name;

        // Comment
        self.emit.write("// Effect handler: ");
        self.emit.write(name);
        self.emit.write(" handles ");
        self.emit.writeln(effect);

        // For Infer effect, generate an InferConfig struct
        if effect == "Infer" {
            // Generate a struct with the config
            if handler.is_pub {
                self.emit.write("pub ");
            }
            self.emit.write("mod handler_");
            self.emit.write(&Self::to_snake_case(name));
            self.emit.writeln(" {");
            self.emit.indent();

            self.emit.writeln("#[derive(Debug, Clone)]");
            self.emit.writeln("pub struct Config {");
            self.emit.indent();

            // Generate fields for each config entry
            for config in &handler.config {
                self.emit.write("pub ");
                self.emit.write(&config.key.name);
                self.emit.write(": ");
                // Infer type from literal
                match &config.value {
                    Literal::Int(_) => self.emit.write("i64"),
                    Literal::Float(_) => self.emit.write("f64"),
                    Literal::String(_) => self.emit.write("&'static str"),
                    Literal::Bool(_) => self.emit.write("bool"),
                }
                self.emit.writeln(",");
            }

            self.emit.dedent();
            self.emit.writeln("}");
            self.emit.blank_line();

            // Generate a const instance with the values
            self.emit.write("pub const CONFIG: Config = Config ");
            self.emit.writeln("{");
            self.emit.indent();

            for config in &handler.config {
                self.emit.write(&config.key.name);
                self.emit.write(": ");
                match &config.value {
                    Literal::Int(n) => self.emit.write(&n.to_string()),
                    Literal::Float(f) => {
                        let s = f.to_string();
                        self.emit.write(&s);
                        if !s.contains('.') {
                            self.emit.write(".0");
                        }
                    }
                    Literal::String(s) => {
                        self.emit.write("\"");
                        self.emit.write(s);
                        self.emit.write("\"");
                    }
                    Literal::Bool(b) => self.emit.write(&b.to_string()),
                }
                self.emit.writeln(",");
            }

            self.emit.dedent();
            self.emit.writeln("};");

            self.emit.dedent();
            self.emit.writeln("}");
        } else {
            // For other effects, generate a placeholder
            self.emit.write("// TODO: Handler for effect '");
            self.emit.write(effect);
            self.emit.writeln("' not yet implemented");
        }
    }

    fn generate_block(&mut self, block: &Block) {
        self.emit.open_brace();
        self.generate_block_contents(block);
        self.emit.close_brace();
    }

    fn generate_block_inline(&mut self, block: &Block) {
        self.emit.open_brace();
        self.generate_block_contents(block);
        self.emit.close_brace_inline();
    }

    fn generate_block_contents(&mut self, block: &Block) {
        // Collect variables that are reassigned in this block
        self.collect_reassigned_vars(block);
        for stmt in &block.stmts {
            self.generate_stmt(stmt);
        }
    }

    fn generate_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                // Only add mut if the variable is reassigned later
                if self.reassigned_vars.contains(&name.name) {
                    self.emit.write("let mut ");
                } else {
                    self.emit.write("let ");
                }
                if ty.is_some() {
                    self.emit.write(&name.name);
                    self.emit.write(": ");
                    self.emit_type(ty.as_ref().unwrap());
                } else {
                    self.emit.write(&name.name);
                }
                self.emit.write(" = ");
                self.generate_expr(value);
                self.emit.writeln(";");
            }

            Stmt::Assign { name, value, .. } => {
                self.emit.write(&name.name);
                self.emit.write(" = ");
                self.generate_expr(value);
                self.emit.writeln(";");
            }

            Stmt::Return { value, .. } => {
                self.emit.write("return ");
                if let Some(expr) = value {
                    self.generate_expr(expr);
                }
                self.emit.writeln(";");
            }

            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.emit.write("if ");
                self.generate_expr(condition);
                self.emit.write(" ");
                if else_block.is_some() {
                    self.generate_block_inline(then_block);
                    self.emit.write(" else ");
                    match else_block.as_ref().unwrap() {
                        sage_parser::ElseBranch::Block(block) => {
                            self.generate_block(block);
                        }
                        sage_parser::ElseBranch::ElseIf(stmt) => {
                            self.generate_stmt(stmt);
                        }
                    }
                } else {
                    self.generate_block(then_block);
                }
            }

            Stmt::For {
                pattern,
                iter,
                body,
                ..
            } => {
                self.emit.write("for ");
                self.emit_pattern(pattern);
                self.emit.write(" in ");
                self.generate_expr(iter);
                self.emit.write(" ");
                self.generate_block(body);
            }

            Stmt::While {
                condition, body, ..
            } => {
                self.emit.write("while ");
                self.generate_expr(condition);
                self.emit.write(" ");
                self.generate_block(body);
            }

            Stmt::Loop { body, .. } => {
                self.emit.write("loop ");
                self.generate_block(body);
            }

            Stmt::Break { .. } => {
                self.emit.writeln("break;");
            }

            Stmt::SpanBlock { name, body, .. } => {
                // Generate a block that:
                // 1. Records start time
                // 2. Emits span_start event
                // 3. Runs body
                // 4. Emits span_end event with duration
                self.emit.writeln("{");
                self.emit.indent();
                self.emit.write("let __span_name = ");
                self.generate_expr(name);
                self.emit.writeln(";");
                self.emit
                    .writeln("let __span_start = std::time::Instant::now();");
                self.emit
                    .writeln("sage_runtime::trace::span_start(&__span_name);");
                // Generate body statements
                self.generate_block(body);
                self.emit.writeln(
                    "sage_runtime::trace::span_end(&__span_name, __span_start.elapsed().as_millis() as u64);",
                );
                self.emit.dedent();
                self.emit.writeln("}");
            }

            Stmt::Checkpoint { .. } => {
                // Generate explicit checkpoint calls for each @persistent belief
                // This forces an immediate save of all @persistent beliefs
                if self.current_agent_persistent_beliefs.is_empty() {
                    // No persistent fields - checkpoint is a no-op
                    self.emit.writeln("// checkpoint() - no @persistent fields");
                } else {
                    self.emit
                        .writeln("// checkpoint() - save all @persistent beliefs");
                    for field_name in &self.current_agent_persistent_beliefs.clone() {
                        self.emit.write("self_");
                        self.emit.write(field_name);
                        self.emit.writeln(".checkpoint();");
                    }
                }
            }

            Stmt::Expr { expr, .. } => {
                // Handle emit specially
                if let Expr::Yield { value, .. } = expr {
                    self.emit.write("return ctx.emit(");
                    self.generate_expr(value);
                    self.emit.writeln(");");
                } else if let Expr::Call { name, args, .. } = expr {
                    // Handle assertion builtins (needed for assertions inside span blocks)
                    if self.is_assertion_builtin(&name.name) {
                        self.generate_assertion(&name.name, args);
                    } else {
                        self.generate_expr(expr);
                        self.emit.writeln(";");
                    }
                } else {
                    self.generate_expr(expr);
                    self.emit.writeln(";");
                }
            }

            Stmt::LetTuple { names, value, .. } => {
                self.emit.write("let (");
                for (i, name) in names.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.emit.write(&name.name);
                }
                self.emit.write(") = ");
                self.generate_expr(value);
                self.emit.writeln(";");
            }

            // RFC-0012: mock divine - codegen will be handled in test harness generation
            Stmt::MockDivine { value, .. } => {
                // Mock statements are collected during test codegen, not emitted inline
                // This placeholder ensures the match is exhaustive
                self.emit.write("// mock divine: ");
                match value {
                    sage_parser::MockValue::Value(expr) => {
                        self.generate_expr(expr);
                    }
                    sage_parser::MockValue::Fail(expr) => {
                        self.emit.write("fail(");
                        self.generate_expr(expr);
                        self.emit.write(")");
                    }
                }
                self.emit.writeln(";");
            }

            // RFC-0012: mock tool - codegen will be handled in test harness generation
            Stmt::MockTool {
                tool_name,
                fn_name,
                value,
                ..
            } => {
                // Mock statements are collected during test codegen, not emitted inline
                // This placeholder ensures the match is exhaustive
                self.emit.write("// mock tool ");
                self.emit.write(&tool_name.name);
                self.emit.write(".");
                self.emit.write(&fn_name.name);
                self.emit.write(": ");
                match value {
                    sage_parser::MockValue::Value(expr) => {
                        self.generate_expr(expr);
                    }
                    sage_parser::MockValue::Fail(expr) => {
                        self.emit.write("fail(");
                        self.generate_expr(expr);
                        self.emit.write(")");
                    }
                }
                self.emit.writeln(";");
            }
        }
    }

    fn generate_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal { value, .. } => {
                self.emit_literal(value);
            }

            Expr::Var { name, .. } => {
                // Handle builtin constants (RFC-0013)
                match name.name.as_str() {
                    "PI" => self.emit.write("std::f64::consts::PI"),
                    "E" => self.emit.write("std::f64::consts::E"),
                    // Time constants
                    "MS_PER_SECOND" => self.emit.write("1000_i64"),
                    "MS_PER_MINUTE" => self.emit.write("60000_i64"),
                    "MS_PER_HOUR" => self.emit.write("3600000_i64"),
                    "MS_PER_DAY" => self.emit.write("86400000_i64"),
                    _ => {
                        self.emit.write(&name.name);
                        if self.string_consts.contains(&name.name) {
                            self.emit.write(".to_string()");
                        }
                    }
                }
            }

            Expr::Binary {
                op, left, right, ..
            } => {
                // Handle string concatenation specially
                if matches!(op, BinOp::Concat) {
                    self.emit.write("format!(\"{}{}\", ");
                    self.generate_expr(left);
                    self.emit.write(", ");
                    self.generate_expr(right);
                    self.emit.write(")");
                } else {
                    self.generate_expr(left);
                    self.emit.write(" ");
                    self.emit_binop(op);
                    self.emit.write(" ");
                    self.generate_expr(right);
                }
            }

            Expr::Unary { op, operand, .. } => {
                self.emit_unaryop(op);
                self.generate_expr(operand);
            }

            Expr::Call {
                name,
                type_args,
                args,
                ..
            } => {
                let fn_name = &name.name;

                // Handle builtins
                match fn_name.as_str() {
                    "print" => {
                        self.emit.write("println!(\"{}\", ");
                        self.generate_expr(&args[0]);
                        self.emit.write(")");
                    }
                    "str" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".to_string()");
                    }
                    "len" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".len() as i64");
                    }

                    // RFC-0013: String functions
                    "split" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".split(&*");
                        self.generate_expr(&args[1]);
                        self.emit.write(").map(str::to_string).collect::<Vec<_>>()");
                    }
                    "lines" => {
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(".lines().map(str::to_string).collect::<Vec<_>>()");
                    }
                    "chars" => {
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(".chars().map(|c| c.to_string()).collect::<Vec<_>>()");
                    }
                    "join" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".join(&*");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "trim" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".trim().to_string()");
                    }
                    "trim_start" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".trim_start().to_string()");
                    }
                    "trim_end" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".trim_end().to_string()");
                    }
                    "starts_with" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".starts_with(&*");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "ends_with" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".ends_with(&*");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "str_contains" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".contains(&*");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "replace" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".replace(&*");
                        self.generate_expr(&args[1]);
                        self.emit.write(", &*");
                        self.generate_expr(&args[2]);
                        self.emit.write(")");
                    }
                    "replace_first" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".replacen(&*");
                        self.generate_expr(&args[1]);
                        self.emit.write(", &*");
                        self.generate_expr(&args[2]);
                        self.emit.write(", 1)");
                    }
                    "to_upper" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".to_uppercase()");
                    }
                    "to_lower" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".to_lowercase()");
                    }
                    "str_len" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".chars().count() as i64");
                    }
                    "str_slice" => {
                        self.emit.write("sage_runtime::stdlib::str_slice(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", ");
                        self.generate_expr(&args[1]);
                        self.emit.write(", ");
                        self.generate_expr(&args[2]);
                        self.emit.write(")");
                    }
                    "str_index_of" => {
                        self.emit.write("sage_runtime::stdlib::str_index_of(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "str_repeat" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".repeat(");
                        self.generate_expr(&args[1]);
                        self.emit.write(" as usize)");
                    }
                    "str_pad_start" => {
                        self.emit.write("sage_runtime::stdlib::str_pad_start(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", ");
                        self.generate_expr(&args[1]);
                        self.emit.write(", &");
                        self.generate_expr(&args[2]);
                        self.emit.write(")");
                    }
                    "str_pad_end" => {
                        self.emit.write("sage_runtime::stdlib::str_pad_end(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", ");
                        self.generate_expr(&args[1]);
                        self.emit.write(", &");
                        self.generate_expr(&args[2]);
                        self.emit.write(")");
                    }

                    "chr" => {
                        self.emit.write("sage_runtime::stdlib::chr(");
                        self.generate_expr(&args[0]);
                        self.emit.write(")");
                    }

                    // RFC-0013: Math functions
                    "abs" => {
                        self.emit.write("(");
                        self.generate_expr(&args[0]);
                        self.emit.write(").abs()");
                    }
                    "abs_float" => {
                        self.emit.write("(");
                        self.generate_expr(&args[0]);
                        self.emit.write(").abs()");
                    }
                    "min" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".min(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "max" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".max(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "min_float" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".min(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "max_float" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".max(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "clamp" => {
                        self.emit.write("(");
                        self.generate_expr(&args[0]);
                        self.emit.write(").clamp(");
                        self.generate_expr(&args[1]);
                        self.emit.write(", ");
                        self.generate_expr(&args[2]);
                        self.emit.write(")");
                    }
                    "clamp_float" => {
                        self.emit.write("(");
                        self.generate_expr(&args[0]);
                        self.emit.write(").clamp(");
                        self.generate_expr(&args[1]);
                        self.emit.write(", ");
                        self.generate_expr(&args[2]);
                        self.emit.write(")");
                    }
                    "floor" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".floor() as i64");
                    }
                    "ceil" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".ceil() as i64");
                    }
                    "round" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".round() as i64");
                    }
                    "floor_float" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".floor()");
                    }
                    "ceil_float" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".ceil()");
                    }
                    "pow" => {
                        // Safe power: handle negative exponents by returning 0
                        self.emit.write("{ let __base = ");
                        self.generate_expr(&args[0]);
                        self.emit.write("; let __exp = ");
                        self.generate_expr(&args[1]);
                        self.emit
                            .write("; if __exp < 0 { 0 } else { __base.pow(__exp as u32) } }");
                    }
                    "pow_float" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".powf(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "sqrt" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".sqrt()");
                    }
                    "int_to_float" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(" as f64");
                    }
                    "float_to_int" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(" as i64");
                    }

                    // RFC-0013: Parsing functions
                    "parse_int" => {
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(".trim().parse::<i64>().map_err(|e| e.to_string())");
                    }
                    "parse_float" => {
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(".trim().parse::<f64>().map_err(|e| e.to_string())");
                    }
                    "parse_bool" => {
                        self.emit.write("sage_runtime::stdlib::parse_bool(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(")");
                    }
                    "float_to_str" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".to_string()");
                    }
                    "bool_to_str" => {
                        self.emit.write("if ");
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(" { \"true\".to_string() } else { \"false\".to_string() }");
                    }
                    "int_to_str" => {
                        self.emit.write("(");
                        self.generate_expr(&args[0]);
                        self.emit.write(").to_string()");
                    }

                    // RFC-0013: List Higher-Order Functions
                    "map" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().map(");
                        self.generate_expr(&args[1]);
                        self.emit.write(").collect::<Vec<_>>()");
                    }
                    "filter" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().filter(|__x| (");
                        self.generate_expr(&args[1]);
                        self.emit.write(")((__x).clone())).collect::<Vec<_>>()");
                    }
                    "reduce" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().fold(");
                        self.generate_expr(&args[1]);
                        self.emit.write(", ");
                        self.generate_expr(&args[2]);
                        self.emit.write(")");
                    }
                    "any" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().any(|__x| (");
                        self.generate_expr(&args[1]);
                        self.emit.write(")((__x).clone()))");
                    }
                    "all" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().all(|__x| (");
                        self.generate_expr(&args[1]);
                        self.emit.write(")((__x).clone()))");
                    }
                    "find" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().find(|__x| (");
                        self.generate_expr(&args[1]);
                        self.emit.write(")((__x).clone()))");
                    }
                    "flat_map" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().flat_map(");
                        self.generate_expr(&args[1]);
                        self.emit.write(").collect::<Vec<_>>()");
                    }
                    "zip" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().zip(");
                        self.generate_expr(&args[1]);
                        self.emit.write(".into_iter()).collect::<Vec<_>>()");
                    }
                    "sort_by" => {
                        self.emit.write("{ let mut __v = ");
                        self.generate_expr(&args[0]);
                        self.emit.write("; __v.sort_by(|__a, __b| { let __cmp = (");
                        self.generate_expr(&args[1]);
                        self.emit.write(")((__a).clone(), (__b).clone()); if __cmp < 0 { std::cmp::Ordering::Less } else if __cmp > 0 { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Equal } }); __v }");
                    }
                    "enumerate" => {
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(".into_iter().enumerate().map(|(__i, __x)| (__i as i64, __x)).collect::<Vec<_>>()");
                    }
                    "take" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().take(");
                        self.generate_expr(&args[1]);
                        self.emit.write(" as usize).collect::<Vec<_>>()");
                    }
                    "drop" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().skip(");
                        self.generate_expr(&args[1]);
                        self.emit.write(" as usize).collect::<Vec<_>>()");
                    }
                    "flatten" => {
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(".into_iter().flatten().collect::<Vec<_>>()");
                    }
                    "reverse" => {
                        self.emit.write("{ let mut __v = ");
                        self.generate_expr(&args[0]);
                        self.emit.write("; __v.reverse(); __v }");
                    }
                    "unique" => {
                        self.emit
                            .write("{ let mut __seen = std::collections::HashSet::new(); ");
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().filter(|__x| __seen.insert(format!(\"{:?}\", __x))).collect::<Vec<_>>() }");
                    }
                    "count_where" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().filter(|__x| (");
                        self.generate_expr(&args[1]);
                        self.emit.write(")((__x).clone())).count() as i64");
                    }
                    "sum" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".iter().sum::<i64>()");
                    }
                    "sum_floats" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".iter().sum::<f64>()");
                    }

                    // =========================================================================
                    // RFC-0010: List Utilities
                    // =========================================================================
                    "range" => {
                        self.emit.write("(");
                        self.generate_expr(&args[0]);
                        self.emit.write("..");
                        self.generate_expr(&args[1]);
                        self.emit.write(").collect::<Vec<_>>()");
                    }
                    "range_step" => {
                        self.emit.write("(");
                        self.generate_expr(&args[0]);
                        self.emit.write("..");
                        self.generate_expr(&args[1]);
                        self.emit.write(").step_by(");
                        self.generate_expr(&args[2]);
                        self.emit.write(" as usize).collect::<Vec<_>>()");
                    }
                    "first" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".first().cloned()");
                    }
                    "last" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".last().cloned()");
                    }
                    "get" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".get(");
                        self.generate_expr(&args[1]);
                        self.emit.write(" as usize).cloned()");
                    }
                    "list_contains" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".contains(&");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "sort" => {
                        self.emit.write("{ let mut __v = ");
                        self.generate_expr(&args[0]);
                        self.emit.write("; __v.sort(); __v }");
                    }
                    "list_slice" => {
                        self.emit.write("sage_runtime::stdlib::list_slice(");
                        self.generate_expr(&args[0]);
                        self.emit.write(", ");
                        self.generate_expr(&args[1]);
                        self.emit.write(", ");
                        self.generate_expr(&args[2]);
                        self.emit.write(")");
                    }
                    "chunk" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".chunks(");
                        self.generate_expr(&args[1]);
                        self.emit
                            .write(" as usize).map(|c| c.to_vec()).collect::<Vec<_>>()");
                    }
                    "pop" => {
                        self.emit.write("{ let mut __v = ");
                        self.generate_expr(&args[0]);
                        self.emit.write("; __v.pop() }");
                    }
                    "push" => {
                        self.emit.write("{ let mut __v = ");
                        self.generate_expr(&args[0]);
                        self.emit.write("; __v.push(");
                        self.generate_expr(&args[1]);
                        self.emit.write("); __v }");
                    }
                    "concat" => {
                        self.emit.write("{ let mut __v = ");
                        self.generate_expr(&args[0]);
                        self.emit.write("; __v.extend(");
                        self.generate_expr(&args[1]);
                        self.emit.write("); __v }");
                    }
                    "take_while" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().take_while(|__x| (");
                        self.generate_expr(&args[1]);
                        self.emit.write(")((__x).clone())).collect::<Vec<_>>()");
                    }
                    "drop_while" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".into_iter().skip_while(|__x| (");
                        self.generate_expr(&args[1]);
                        self.emit.write(")((__x).clone())).collect::<Vec<_>>()");
                    }

                    // =========================================================================
                    // RFC-0010: Option Utilities
                    // =========================================================================
                    "is_some" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".is_some()");
                    }
                    "is_none" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".is_none()");
                    }
                    "unwrap" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".expect(\"unwrap called on None\")");
                    }
                    "unwrap_or" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".unwrap_or(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "unwrap_or_else" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".unwrap_or_else(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "map_option" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".map(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "or_option" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".or(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }

                    // =========================================================================
                    // RFC-0010: Result Utilities
                    // =========================================================================
                    "is_ok" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".is_ok()");
                    }
                    "is_err" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".is_err()");
                    }
                    "unwrap_result" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".expect(\"unwrap_result called on Err\")");
                    }
                    "unwrap_err" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".expect_err(\"unwrap_err called on Ok\")");
                    }
                    "unwrap_or_result" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".unwrap_or(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "map_result" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".map(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "map_err" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".map_err(");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "ok" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".ok()");
                    }
                    "err_value" => {
                        self.generate_expr(&args[0]);
                        self.emit.write(".err()");
                    }

                    // =========================================================================
                    // RFC-0010: I/O Functions
                    // =========================================================================
                    "read_file" => {
                        self.emit.write("sage_runtime::stdlib::read_file(&");
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(").map_err(sage_runtime::SageError::agent)?");
                    }
                    "write_file" => {
                        self.emit.write("sage_runtime::stdlib::write_file(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit
                            .write(").map_err(sage_runtime::SageError::agent)?");
                    }
                    "append_file" => {
                        self.emit.write("sage_runtime::stdlib::append_file(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit
                            .write(").map_err(sage_runtime::SageError::agent)?");
                    }
                    "file_exists" => {
                        self.emit.write("sage_runtime::stdlib::file_exists(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(")");
                    }
                    "delete_file" => {
                        self.emit.write("sage_runtime::stdlib::delete_file(&");
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(").map_err(sage_runtime::SageError::agent)?");
                    }
                    "list_dir" => {
                        self.emit.write("sage_runtime::stdlib::list_dir(&");
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(").map_err(sage_runtime::SageError::agent)?");
                    }
                    "make_dir" => {
                        self.emit.write("sage_runtime::stdlib::make_dir(&");
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(").map_err(sage_runtime::SageError::agent)?");
                    }
                    "read_line" => {
                        self.emit.write("sage_runtime::stdlib::read_line().map_err(sage_runtime::SageError::agent)");
                    }
                    "read_all" => {
                        self.emit.write("sage_runtime::stdlib::read_all().map_err(sage_runtime::SageError::agent)");
                    }
                    "print_err" => {
                        self.emit.write("eprintln!(\"{}\", ");
                        self.generate_expr(&args[0]);
                        self.emit.write(")");
                    }

                    // =========================================================================
                    // RFC-0010: Time Functions
                    // =========================================================================
                    "now_ms" => {
                        self.emit.write("sage_runtime::stdlib::now_ms()");
                    }
                    "now_s" => {
                        self.emit.write("sage_runtime::stdlib::now_s()");
                    }
                    "sleep_ms" => {
                        self.emit
                            .write("tokio::time::sleep(std::time::Duration::from_millis(");
                        self.generate_expr(&args[0]);
                        self.emit.write(" as u64)).await");
                    }
                    "format_timestamp" => {
                        self.emit.write("sage_runtime::stdlib::format_timestamp(");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "parse_timestamp" => {
                        self.emit.write("sage_runtime::stdlib::parse_timestamp(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit
                            .write(").map_err(sage_runtime::SageError::agent)?");
                    }

                    // =========================================================================
                    // RFC-0010: JSON Utilities
                    // =========================================================================
                    "json_parse" => {
                        self.emit.write("sage_runtime::stdlib::json_parse(&");
                        self.generate_expr(&args[0]);
                        self.emit
                            .write(").map_err(sage_runtime::SageError::agent)?");
                    }
                    "json_get" => {
                        self.emit.write("sage_runtime::stdlib::json_get(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "json_get_int" => {
                        self.emit.write("sage_runtime::stdlib::json_get_int(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "json_get_float" => {
                        self.emit.write("sage_runtime::stdlib::json_get_float(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "json_get_bool" => {
                        self.emit.write("sage_runtime::stdlib::json_get_bool(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "json_get_list" => {
                        self.emit.write("sage_runtime::stdlib::json_get_list(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(", &");
                        self.generate_expr(&args[1]);
                        self.emit.write(")");
                    }
                    "json_stringify" => {
                        self.emit
                            .write("sage_runtime::stdlib::json_stringify_string(&");
                        self.generate_expr(&args[0]);
                        self.emit.write(".to_string())");
                    }

                    name if self.extern_fn_names.contains(name) => {
                        // Extern function — call into sage_extern module
                        self.emit.write("sage_extern::");
                        self.emit.write(name);
                        self.emit.write("(");
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                self.emit.write(", ");
                            }
                            // Clone arguments to match user-defined fn convention
                            self.emit.write("(");
                            self.generate_expr(arg);
                            self.emit.write(").clone()");
                        }
                        self.emit.write(")");
                        // Fallible extern fns need error conversion
                        if self.extern_fn_fallible.contains(name) {
                            self.emit.write(".map_err(sage_runtime::SageError::agent)?");
                        }
                    }

                    _ => {
                        // User-defined function call
                        self.emit.write(fn_name);
                        // RFC-0015: Emit type arguments if provided (turbofish syntax)
                        if !type_args.is_empty() {
                            self.emit.write("::<");
                            for (i, arg) in type_args.iter().enumerate() {
                                if i > 0 {
                                    self.emit.write(", ");
                                }
                                self.emit_type(arg);
                            }
                            self.emit.write(">");
                        }
                        self.emit.write("(");
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                self.emit.write(", ");
                            }
                            // Clone arguments to avoid move issues with Strings
                            // (compiler optimizes away unnecessary clones for Copy types)
                            self.emit.write("(");
                            self.generate_expr(arg);
                            self.emit.write(").clone()");
                        }
                        self.emit.write(")");
                    }
                }
            }

            Expr::SelfField { field, .. } => {
                self.emit.write("self.");
                self.emit.write(&field.name);
            }

            Expr::Apply { callee, args, .. } => {
                // Generate callee expression (usually a FieldAccess for method calls)
                self.generate_expr(callee);
                self.emit.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.generate_expr(arg);
                }
                self.emit.write(")");
            }

            Expr::SelfMethodCall { method, args, .. } => {
                self.emit.write("self.");
                self.emit.write(&method.name);
                self.emit.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.generate_expr(arg);
                }
                self.emit.write(")");
            }

            Expr::List { elements, .. } => {
                self.emit.write("vec![");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.generate_expr(elem);
                }
                self.emit.write("]");
            }

            Expr::Paren { inner, .. } => {
                self.emit.write("(");
                self.generate_expr(inner);
                self.emit.write(")");
            }

            Expr::Divine { template, .. } => {
                // Note: No ? here - the try wrapper is responsible for error propagation
                self.emit.write("ctx.infer_string(&");
                self.emit_string_template(template);
                self.emit.write(").await");
            }

            Expr::Summon { agent, fields, .. } => {
                let has_error_handler = self.agents_with_error_handlers.contains(&agent.name);
                let message_handlers = self.agents_with_message_handlers.get(&agent.name).cloned();
                let tool_uses = self
                    .agent_tool_uses
                    .get(&agent.name)
                    .cloned()
                    .unwrap_or_default();

                // Wrap in a block so we can emit clone bindings before the spawn
                self.emit.write("{ ");
                // Pre-clone field values to avoid capturing self/moving loop vars
                for (i, field) in fields.iter().enumerate() {
                    self.emit.write(&format!("let __sf{} = (", i));
                    self.generate_expr(&field.value);
                    self.emit.write(").clone(); ");
                }

                self.emit
                    .write("sage_runtime::spawn(move |mut ctx| async move { ");
                self.emit.write("let agent = ");
                self.emit.write(&agent.name);
                // Always use struct literal syntax when there are fields or tool uses
                if fields.is_empty() && tool_uses.is_empty() {
                    self.emit.write("; ");
                } else {
                    self.emit.write(" { ");
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            self.emit.write(", ");
                        }
                        self.emit.write(&field.name.name);
                        self.emit.write(&format!(": __sf{}", i));
                    }
                    // Add tool fields automatically
                    for tool_name in &tool_uses {
                        if !fields.is_empty()
                            || tool_uses.iter().position(|t| t == tool_name) != Some(0)
                        {
                            self.emit.write(", ");
                        }
                        self.emit.write(&tool_name.to_lowercase());
                        self.emit.write(": ");
                        if tool_name == "Database" {
                            self.emit.write("DatabaseClient::from_env().await.expect(\"Failed to connect to database\")");
                        } else {
                            self.emit.write(tool_name);
                            self.emit.write("Client::from_env()");
                        }
                    }
                    self.emit.write(" }; ");
                }

                // on_start with optional error handling
                if has_error_handler {
                    self.emit
                        .write("let result = match agent.on_start(&mut ctx).await { ");
                    self.emit.write("Ok(result) => Ok(result), ");
                    self.emit
                        .write("Err(e) => agent.on_error(e, &mut ctx).await }; ");
                } else {
                    self.emit
                        .write("let result = agent.on_start(&mut ctx).await; ");
                }

                // Message receive loop (if agent has message handlers)
                if let Some(handlers) = message_handlers {
                    self.emit.write("if result.is_ok() { loop { ");
                    self.emit.write("match ctx.receive_raw().await { ");
                    self.emit.write("Ok(msg) => { ");
                    self.emit.write("match msg.type_name.as_deref() { ");

                    // Generate match arms for each message type
                    for (param_name, param_ty) in &handlers.handlers {
                        let type_name = self.type_expr_to_string(param_ty);
                        let method_name = format!("on_message_{}", Self::to_snake_case(&type_name));

                        self.emit.write("Some(\"");
                        self.emit.write(&type_name);
                        self.emit.write("\") => { ");
                        self.emit.write("if let Ok(");
                        self.emit.write(&param_name.name);
                        self.emit
                            .write(") = serde_json::from_value(msg.payload) { ");
                        self.emit.write("let _ = agent.");
                        self.emit.write(&method_name);
                        self.emit.write("(");
                        self.emit.write(&param_name.name);
                        self.emit.write(", &mut ctx).await; } } ");
                    }

                    self.emit.write("_ => {} } } "); // close match type_name, Ok(msg)
                    self.emit.write("Err(_) => break } } } "); // close match receive, loop, if
                }

                self.emit.write("result }) }");
            }

            Expr::Await {
                handle, timeout, ..
            } => {
                // Note: No ? here - the try wrapper is responsible for error propagation
                if let Some(timeout_expr) = timeout {
                    // With timeout: wrap in tokio::time::timeout
                    self.emit.write("tokio::time::timeout(");
                    self.emit.write("std::time::Duration::from_millis(");
                    self.generate_expr(timeout_expr);
                    self.emit.write(" as u64), ");
                    self.generate_expr(handle);
                    self.emit
                        .write(".result()).await.map_err(|_| sage_runtime::SageError::agent(");
                    self.emit.write("\"await timed out\"))?");
                } else {
                    // Without timeout: simple await
                    self.generate_expr(handle);
                    self.emit.write(".result().await");
                }
            }

            Expr::Send {
                handle, message, ..
            } => {
                self.generate_expr(handle);

                // Phase 3: Include type name if we can determine it (for protocol tracking)
                // Note: Don't add ? here - the Try wrapper handles error propagation
                if let Some(type_name) = Self::extract_message_type_name(message) {
                    // Use send_message() for pre-built Message with metadata
                    self.emit.write(".send_message(Message::new(");
                    self.generate_expr(message);
                    self.emit.write(")?.with_type_name(\"");
                    self.emit.write(&type_name);
                    self.emit.write("\")).await");
                } else {
                    // Use simple send() for raw message
                    self.emit.write(".send(");
                    self.generate_expr(message);
                    self.emit.write(").await");
                }
            }

            Expr::Yield { value, .. } => {
                self.emit.write("ctx.emit(");
                self.generate_expr(value);
                self.emit.write(")");
            }

            Expr::StringInterp { template, .. } => {
                self.emit_string_template(template);
            }

            Expr::Match {
                scrutinee, arms, ..
            } => {
                self.emit.write("match ");
                self.generate_expr(scrutinee);
                self.emit.writeln(" {");
                self.emit.indent();
                for arm in arms {
                    self.emit_pattern(&arm.pattern);
                    self.emit.write(" => ");
                    self.generate_expr(&arm.body);
                    self.emit.writeln(",");
                }
                self.emit.dedent();
                self.emit.write("}");
            }

            Expr::RecordConstruct {
                name,
                type_args,
                fields,
                ..
            } => {
                self.emit.write(&name.name);
                // RFC-0015: Emit type arguments if provided (turbofish syntax)
                if !type_args.is_empty() {
                    self.emit.write("::<");
                    for (i, arg) in type_args.iter().enumerate() {
                        if i > 0 {
                            self.emit.write(", ");
                        }
                        self.emit_type(arg);
                    }
                    self.emit.write(">");
                }
                self.emit.write(" { ");
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.emit.write(&field.name.name);
                    self.emit.write(": ");
                    self.generate_expr(&field.value);
                }
                self.emit.write(" }");
            }

            Expr::FieldAccess { object, field, .. } => {
                self.generate_expr(object);
                self.emit.write(".");
                self.emit.write(&field.name);
            }

            Expr::Receive { .. } => {
                self.emit.write("ctx.receive().await?");
            }

            // RFC-0007: Error handling
            Expr::Try { expr, .. } => {
                // Generate the inner expression with ? for error propagation
                self.generate_expr(expr);
                self.emit.write("?");
            }

            Expr::Catch {
                expr,
                error_bind,
                recovery,
                ..
            } => {
                // Generate a match expression to handle the Result
                // If expr is Try { inner }, skip the Try wrapper since we handle the error here
                self.emit.write("match ");
                if let Expr::Try { expr: inner, .. } = expr.as_ref() {
                    self.generate_expr(inner);
                } else {
                    self.generate_expr(expr);
                }
                self.emit.writeln(" {");
                self.emit.indent();

                // Ok arm - unwrap the value
                self.emit.writeln("Ok(__val) => __val,");

                // Err arm - run recovery
                if let Some(err_name) = error_bind {
                    self.emit.write("Err(");
                    self.emit.write(&err_name.name);
                    self.emit.write(") => ");
                } else {
                    self.emit.write("Err(_) => ");
                }
                self.generate_expr(recovery);
                self.emit.writeln(",");

                self.emit.dedent();
                self.emit.write("}");
            }

            // fail expression - explicit error raising
            Expr::Fail { error, .. } => {
                // Generate: return Err(SageError::user(msg))
                self.emit.write("return Err(sage_runtime::SageError::user(");
                self.generate_expr(error);
                self.emit.write("))");
            }

            // retry expression - retry a fallible operation
            Expr::Retry {
                count,
                delay,
                on_errors: _,
                body,
                ..
            } => {
                // Generate a retry loop with async block
                self.emit.writeln("'_retry: {");
                self.emit.indent();

                self.emit.write("let _retry_max: i64 = ");
                self.generate_expr(count);
                self.emit.writeln(";");

                if let Some(delay_expr) = delay {
                    self.emit.write("let _retry_delay: u64 = ");
                    self.generate_expr(delay_expr);
                    self.emit.writeln(" as u64;");
                }

                self.emit
                    .writeln("let mut _last_error: Option<sage_runtime::SageError> = None;");
                self.emit.writeln("for _attempt in 0.._retry_max {");
                self.emit.indent();

                // Wrap body in async block that returns Result
                self.emit.writeln("let _result = (async {");
                self.emit.indent();
                self.emit.write("Ok::<_, sage_runtime::SageError>(");
                self.generate_expr(body);
                self.emit.writeln(")");
                self.emit.dedent();
                self.emit.writeln("}).await;");

                self.emit.writeln("match _result {");
                self.emit.indent();
                self.emit.writeln("Ok(v) => break '_retry v,");
                self.emit.writeln("Err(e) => {");
                self.emit.indent();
                self.emit.writeln("_last_error = Some(e);");

                // Add delay between retries if specified
                if delay.is_some() {
                    self.emit.writeln("if _attempt < _retry_max - 1 {");
                    self.emit.indent();
                    self.emit.writeln(
                        "tokio::time::sleep(std::time::Duration::from_millis(_retry_delay)).await;",
                    );
                    self.emit.dedent();
                    self.emit.writeln("}");
                }

                self.emit.dedent();
                self.emit.writeln("}");
                self.emit.dedent();
                self.emit.writeln("}");

                self.emit.dedent();
                self.emit.writeln("}");

                // After loop exhausted, return the last error
                self.emit.writeln("return Err(_last_error.unwrap());");

                self.emit.dedent();
                self.emit.write("}");
            }

            // trace(message) - emit a trace event
            Expr::Trace { message, .. } => {
                self.emit.write("sage_runtime::trace::user(&");
                self.generate_expr(message);
                self.emit.write(")");
            }

            // RFC-0009: Closures
            Expr::Closure { params, body, .. } => {
                // Generate: Box::new(move |param1: Type1, param2: Type2| { body })
                self.emit.write("Box::new(move |");
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.emit.write(&param.name.name);
                    if let Some(ty) = &param.ty {
                        self.emit.write(": ");
                        self.emit_type(ty);
                    }
                }
                self.emit.write("| ");
                self.generate_expr(body);
                self.emit.write(")");
            }

            // RFC-0010: Tuples and Maps
            Expr::Tuple { elements, .. } => {
                self.emit.write("(");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.generate_expr(elem);
                }
                self.emit.write(")");
            }

            Expr::TupleIndex { tuple, index, .. } => {
                self.generate_expr(tuple);
                self.emit.write(&format!(".{index}"));
            }

            Expr::Map { entries, .. } => {
                if entries.is_empty() {
                    self.emit.write("std::collections::HashMap::new()");
                } else {
                    self.emit.write("std::collections::HashMap::from([");
                    for (i, entry) in entries.iter().enumerate() {
                        if i > 0 {
                            self.emit.write(", ");
                        }
                        self.emit.write("(");
                        self.generate_expr(&entry.key);
                        self.emit.write(", ");
                        self.generate_expr(&entry.value);
                        self.emit.write(")");
                    }
                    self.emit.write("])");
                }
            }

            Expr::VariantConstruct {
                enum_name,
                type_args,
                variant,
                payload,
                ..
            } => {
                self.emit.write(&enum_name.name);
                // RFC-0015: Emit type arguments if provided (turbofish syntax)
                if !type_args.is_empty() {
                    self.emit.write("::<");
                    for (i, arg) in type_args.iter().enumerate() {
                        if i > 0 {
                            self.emit.write(", ");
                        }
                        self.emit_type(arg);
                    }
                    self.emit.write(">");
                }
                self.emit.write("::");
                self.emit.write(&variant.name);
                if let Some(payload_expr) = payload {
                    self.emit.write("(");
                    self.generate_expr(payload_expr);
                    self.emit.write(")");
                }
            }

            // RFC-0011: Tool calls
            Expr::ToolCall {
                tool,
                function,
                args,
                ..
            } => {
                // Generate: self.tool_name.function(args).await
                // Returns SageResult<T> - must be handled with try/catch
                self.emit.write("self.");
                self.emit.write(&tool.name.to_lowercase());
                self.emit.write(".");
                self.emit.write(&function.name);
                self.emit.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    // Clone arguments to avoid move-out-of-self issues
                    self.emit.write("(");
                    self.generate_expr(arg);
                    self.emit.write(").clone()");
                }
                self.emit.write(").await");
            }

            // Phase 3: Reply expression for session types
            Expr::Reply { message, .. } => {
                // Clone protocol info to avoid borrow issues
                let protocol_role = self
                    .current_protocol_roles
                    .iter()
                    .next()
                    .map(|(_, r)| r.clone());

                if let Some(role) = protocol_role {
                    // Agent follows protocols - use reply_with_protocol for validation
                    // We need to infer the message type from the expression
                    let msg_type = self.infer_expr_type_name(message);
                    self.emit.write("ctx.reply_with_protocol(");
                    self.generate_expr(message);
                    self.emit.write(", \"");
                    self.emit.write(&msg_type);
                    self.emit.write("\", \"");
                    self.emit.write(&role);
                    self.emit.write("\").await?");
                } else {
                    // No protocols - use simple reply
                    self.emit.write("ctx.reply(");
                    self.generate_expr(message);
                    self.emit.write(").await?");
                }
            }
        }
    }

    fn emit_pattern(&mut self, pattern: &sage_parser::Pattern) {
        use sage_parser::Pattern;
        match pattern {
            Pattern::Wildcard { .. } => {
                self.emit.write("_");
            }
            Pattern::Variant {
                enum_name,
                variant,
                payload,
                ..
            } => {
                if let Some(enum_name) = enum_name {
                    self.emit.write(&enum_name.name);
                    self.emit.write("::");
                }
                self.emit.write(&variant.name);
                if let Some(inner_pattern) = payload {
                    self.emit.write("(");
                    self.emit_pattern(inner_pattern);
                    self.emit.write(")");
                }
            }
            Pattern::Literal { value, .. } => {
                self.emit_literal(value);
            }
            Pattern::Binding { name, .. } => {
                self.emit.write(&name.name);
            }
            Pattern::Tuple { elements, .. } => {
                self.emit.write("(");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.emit_pattern(elem);
                }
                self.emit.write(")");
            }
        }
    }

    fn emit_literal(&mut self, lit: &Literal) {
        match lit {
            Literal::Int(n) => {
                self.emit.write(&format!("{n}_i64"));
            }
            Literal::Float(f) => {
                self.emit.write(&format!("{f}_f64"));
            }
            Literal::Bool(b) => {
                self.emit.write(if *b { "true" } else { "false" });
            }
            Literal::String(s) => {
                // Escape the string for Rust
                self.emit.write("\"");
                for c in s.chars() {
                    match c {
                        '"' => self.emit.write_raw("\\\""),
                        '\\' => self.emit.write_raw("\\\\"),
                        '\n' => self.emit.write_raw("\\n"),
                        '\r' => self.emit.write_raw("\\r"),
                        '\t' => self.emit.write_raw("\\t"),
                        c if c.is_control() => {
                            self.emit.write_raw(&format!("\\x{:02x}", c as u32));
                        }
                        _ => self.emit.write_raw(&c.to_string()),
                    }
                }
                self.emit.write("\".to_string()");
            }
        }
    }

    fn escape_string_for_rust(s: &str) -> String {
        let mut out = String::new();
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32)),
                _ => out.push(c),
            }
        }
        out
    }

    fn emit_string_template(&mut self, template: &sage_parser::StringTemplate) {
        if !template.has_interpolations() {
            // Simple string literal
            if let Some(StringPart::Literal(s)) = template.parts.first() {
                self.emit.write("\"");
                self.emit.write_raw(&Self::escape_string_for_rust(s));
                self.emit.write("\".to_string()");
            }
            return;
        }

        // Build format string and args
        self.emit.write("format!(\"");
        for part in &template.parts {
            match part {
                StringPart::Literal(s) => {
                    // Escape for Rust string, then escape braces for format string
                    let escaped = Self::escape_string_for_rust(s)
                        .replace('{', "{{")
                        .replace('}', "}}");
                    self.emit.write_raw(&escaped);
                }
                StringPart::Interpolation(_) => {
                    self.emit.write_raw("{}");
                }
            }
        }
        self.emit.write("\"");

        // Add the interpolation args
        for part in &template.parts {
            if let StringPart::Interpolation(expr) = part {
                self.emit.write(", ");
                self.generate_expr(expr);
            }
        }
        self.emit.write(")");
    }

    fn emit_type(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Int => self.emit.write("i64"),
            TypeExpr::Float => self.emit.write("f64"),
            TypeExpr::Bool => self.emit.write("bool"),
            TypeExpr::String => self.emit.write("String"),
            TypeExpr::Unit => self.emit.write("()"),
            TypeExpr::List(inner) => {
                self.emit.write("Vec<");
                self.emit_type(inner);
                self.emit.write(">");
            }
            TypeExpr::Option(inner) => {
                self.emit.write("Option<");
                self.emit_type(inner);
                self.emit.write(">");
            }
            TypeExpr::Oracle(inner) => {
                // Inferred<T> just becomes T at runtime
                self.emit_type(inner);
            }
            TypeExpr::Agent(agent_name) => {
                // Agent handles use the agent's output type, but we don't know it here
                // For now, just use a generic output type
                self.emit.write("AgentHandle<");
                self.emit.write(&agent_name.name);
                self.emit.write("Output>");
            }
            TypeExpr::Named(name, type_args) => {
                self.emit.write(&name.name);
                if !type_args.is_empty() {
                    self.emit.write("<");
                    for (i, arg) in type_args.iter().enumerate() {
                        if i > 0 {
                            self.emit.write(", ");
                        }
                        self.emit_type(arg);
                    }
                    self.emit.write(">");
                }
            }

            // RFC-0007: Error handling
            TypeExpr::Error => {
                self.emit.write("sage_runtime::SageError");
            }

            // RFC-0009: Function types
            TypeExpr::Fn(params, ret) => {
                self.emit.write("Box<dyn Fn(");
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.emit_type(param);
                }
                self.emit.write(") -> ");
                self.emit_type(ret);
                self.emit.write(" + Send + 'static>");
            }

            // RFC-0010: Maps, tuples, Result
            TypeExpr::Map(key, value) => {
                self.emit.write("std::collections::HashMap<");
                self.emit_type(key);
                self.emit.write(", ");
                self.emit_type(value);
                self.emit.write(">");
            }
            TypeExpr::Tuple(elems) => {
                self.emit.write("(");
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        self.emit.write(", ");
                    }
                    self.emit_type(elem);
                }
                self.emit.write(")");
            }
            TypeExpr::Result(ok, err) => {
                self.emit.write("Result<");
                self.emit_type(ok);
                self.emit.write(", ");
                self.emit_type(err);
                self.emit.write(">");
            }
        }
    }

    fn emit_binop(&mut self, op: &BinOp) {
        let s = match op {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::Concat => "++", // Handled specially above
        };
        self.emit.write(s);
    }

    fn emit_unaryop(&mut self, op: &UnaryOp) {
        let s = match op {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
        };
        self.emit.write(s);
    }

    fn infer_agent_output_type(&self, agent: &AgentDecl) -> String {
        // Look for emit expression in start handler to infer return type
        // For now, default to i64
        for handler in &agent.handlers {
            if let EventKind::Start = &handler.event {
                if let Some(ty) = self.find_emit_type(&handler.body) {
                    return ty;
                }
            }
        }
        "i64".to_string()
    }

    fn find_emit_type(&self, block: &Block) -> Option<String> {
        // Track variable assignments to resolve yield(var) types
        let mut var_types: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for stmt in &block.stmts {
            // Track let bindings
            if let Stmt::Let { name, value, .. } = stmt {
                let ty = self.infer_expr_type_with_vars(value, &var_types);
                var_types.insert(name.name.clone(), ty);
            }

            if let Stmt::Expr { expr, .. } = stmt {
                if let Expr::Yield { value, .. } = expr {
                    return Some(self.infer_expr_type_with_vars(value, &var_types));
                }
            }
            // Check nested blocks
            if let Stmt::If {
                then_block,
                else_block,
                ..
            } = stmt
            {
                if let Some(ty) = self.find_emit_type(then_block) {
                    return Some(ty);
                }
                if let Some(else_branch) = else_block {
                    if let sage_parser::ElseBranch::Block(block) = else_branch {
                        if let Some(ty) = self.find_emit_type(block) {
                            return Some(ty);
                        }
                    }
                }
            }
        }
        None
    }

    fn infer_expr_type_with_vars(
        &self,
        expr: &Expr,
        var_types: &std::collections::HashMap<String, String>,
    ) -> String {
        match expr {
            Expr::Var { name, .. } => {
                // Look up variable type from tracked assignments
                if let Some(ty) = var_types.get(&name.name) {
                    return ty.clone();
                }
                "i64".to_string() // Conservative default
            }
            // Try expression unwraps to inner type
            Expr::Try { expr, .. } => self.infer_expr_type_with_vars(expr, var_types),
            // Catch expression returns the Ok type
            Expr::Catch { expr, .. } => self.infer_expr_type_with_vars(expr, var_types),
            // Delegate to basic type inference for other expressions
            _ => self.infer_expr_type(expr),
        }
    }

    fn infer_expr_type(&self, expr: &Expr) -> String {
        match expr {
            Expr::Literal { value, .. } => match value {
                Literal::Int(_) => "i64".to_string(),
                Literal::Float(_) => "f64".to_string(),
                Literal::Bool(_) => "bool".to_string(),
                Literal::String(_) => "String".to_string(),
            },
            Expr::Var { .. } => "i64".to_string(), // Conservative default
            Expr::Binary { op, .. } => {
                if matches!(
                    op,
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
                ) {
                    "bool".to_string()
                } else if matches!(op, BinOp::Concat) {
                    "String".to_string()
                } else {
                    "i64".to_string()
                }
            }
            Expr::Divine { .. } | Expr::StringInterp { .. } => "String".to_string(),
            Expr::Call { name, .. } if name.name == "str" => "String".to_string(),
            Expr::Call { name, .. } if name.name == "len" => "i64".to_string(),
            _ => "i64".to_string(),
        }
    }

    /// Phase 3: Extract the type name from a message expression (for protocol tracking).
    ///
    /// Returns Some(type_name) if the type can be determined, None otherwise.
    fn extract_message_type_name(expr: &Expr) -> Option<String> {
        match expr {
            // Record construction: `Ping {}` → "Ping"
            Expr::RecordConstruct { name, .. } => Some(name.name.clone()),

            // Enum variant: `Status::Active` → "Status"
            Expr::VariantConstruct { enum_name, .. } => Some(enum_name.name.clone()),

            // Parenthesized expression: unwrap
            Expr::Paren { inner, .. } => Self::extract_message_type_name(inner),

            // For other expressions, we can't determine the type at codegen time
            _ => None,
        }
    }

    /// Phase 3: Infer the type name from an expression.
    ///
    /// Used for protocol validation in reply() - we need to know the message type
    /// being sent. For struct constructions this is straightforward; for other
    /// expressions we use a fallback.
    fn infer_expr_type_name(&self, expr: &Expr) -> String {
        match expr {
            // Record construction: RecordName { ... }
            Expr::RecordConstruct { name, .. } => name.name.clone(),
            // Enum variant: EnumName.Variant or EnumName.Variant(payload)
            Expr::VariantConstruct { enum_name, .. } => enum_name.name.clone(),
            // Variable reference - we'd need type info to resolve this
            Expr::Var { name, .. } => name.name.clone(),
            // For other expressions, use a generic name
            _ => "Message".to_string(),
        }
    }

    /// Convert a TypeExpr to its base type name string.
    ///
    /// Used for generating method names from message types.
    #[allow(clippy::only_used_in_recursion)]
    fn type_expr_to_string(&self, ty: &TypeExpr) -> String {
        match ty {
            TypeExpr::Int => "Int".to_string(),
            TypeExpr::Float => "Float".to_string(),
            TypeExpr::Bool => "Bool".to_string(),
            TypeExpr::String => "String".to_string(),
            TypeExpr::Unit => "Unit".to_string(),
            TypeExpr::Named(name, _) => name.name.clone(),
            TypeExpr::List(inner) => format!("List{}", self.type_expr_to_string(inner)),
            TypeExpr::Option(inner) => format!("Option{}", self.type_expr_to_string(inner)),
            TypeExpr::Oracle(inner) => self.type_expr_to_string(inner),
            TypeExpr::Agent(name) => name.name.clone(),
            TypeExpr::Error => "Error".to_string(),
            TypeExpr::Map(k, v) => {
                format!(
                    "Map{}To{}",
                    self.type_expr_to_string(k),
                    self.type_expr_to_string(v)
                )
            }
            TypeExpr::Fn(_, _) => "Fn".to_string(),
            TypeExpr::Tuple(elems) => {
                let parts: Vec<_> = elems.iter().map(|e| self.type_expr_to_string(e)).collect();
                format!("Tuple{}", parts.join(""))
            }
            TypeExpr::Result(ok, err) => {
                format!(
                    "Result{}Or{}",
                    self.type_expr_to_string(ok),
                    self.type_expr_to_string(err)
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sage_parser::{lex, parse};
    use std::sync::Arc;

    fn generate_source(source: &str) -> String {
        let lex_result = lex(source).expect("lexing failed");
        let source_arc: Arc<str> = Arc::from(source);
        let (program, errors) = parse(lex_result.tokens(), source_arc);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let program = program.expect("should parse");
        generate(&program, "test").main_rs
    }

    #[test]
    fn generate_minimal_program() {
        let source = r#"
            agent Main {
                on start {
                    yield(42);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("struct Main;"));
        assert!(output.contains("async fn on_start"));
        assert!(output.contains("ctx.emit(42_i64)"));
        assert!(output.contains("#[tokio::main]"));
    }

    #[test]
    fn generate_function() {
        let source = r#"
            fn add(a: Int, b: Int) -> Int {
                return a + b;
            }
            agent Main {
                on start {
                    yield(add(1, 2));
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("fn add(a: i64, b: i64) -> i64"));
        assert!(output.contains("return a + b;"));
    }

    #[test]
    fn generate_agent_with_beliefs() {
        let source = r#"
            agent Worker {
                value: Int

                on start {
                    yield(self.value * 2);
                }
            }
            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("struct Worker {"));
        assert!(output.contains("value: i64,"));
        assert!(output.contains("self.value"));
    }

    #[test]
    fn generate_persistent_beliefs() {
        let source = r#"
            agent Counter {
                @persistent count: Int

                on waking {
                    print("woke up");
                }

                on start {
                    yield(self.count.get());
                }
            }
            run Counter;
        "#;

        let output = generate_source(source);
        // Agent struct should have checkpoint fields and Persisted wrapper
        assert!(output.contains("_checkpoint:"), "missing checkpoint field");
        assert!(
            output.contains("_checkpoint_key:"),
            "missing checkpoint key field"
        );
        assert!(
            output.contains("Persisted<i64>"),
            "count should be wrapped in Persisted"
        );
        // Main should initialize checkpoint store
        assert!(
            output.contains("MemoryCheckpointStore"),
            "missing checkpoint store init"
        );
        assert!(
            output.contains("Persisted::new"),
            "missing Persisted::new in construction"
        );
        // on_waking handler should be generated and called
        assert!(
            output.contains("async fn on_waking"),
            "missing on_waking handler"
        );
        assert!(
            output.contains("agent.on_waking().await"),
            "missing on_waking call"
        );
    }

    #[test]
    fn generate_string_interpolation() {
        let source = r#"
            agent Main {
                on start {
                    let name = "World";
                    let msg = "Hello, {name}!";
                    print(msg);
                    yield(0);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("format!(\"Hello, {}!\", name)"));
    }

    #[test]
    fn generate_control_flow() {
        let source = r#"
            agent Main {
                on start {
                    let x = 10;
                    if x > 5 {
                        yield(1);
                    } else {
                        yield(0);
                    }
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("if x > 5_i64"), "output:\n{output}");
        // else is on the same line after close brace
        assert!(output.contains("else"), "output:\n{output}");
    }

    #[test]
    fn generate_loops() {
        let source = r#"
            agent Main {
                on start {
                    for x in [1, 2, 3] {
                        print(str(x));
                    }
                    let n = 0;
                    while n < 5 {
                        n = n + 1;
                    }
                    yield(n);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("for x in vec![1_i64, 2_i64, 3_i64]"));
        assert!(output.contains("while n < 5_i64"));
    }

    #[test]
    fn generate_pub_function() {
        let source = r#"
            pub fn helper(x: Int) -> Int {
                return x * 2;
            }
            agent Main {
                on start {
                    yield(helper(21));
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("pub fn helper(x: i64) -> i64"));
    }

    #[test]
    fn generate_pub_agent() {
        let source = r#"
            pub agent Worker {
                on start {
                    yield(42);
                }
            }
            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("pub struct Worker;"));
    }

    #[test]
    fn generate_module_tree_simple() {
        use sage_loader::load_single_file;
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.sg");
        fs::write(
            &file,
            r#"
agent Main {
    on start {
        yield(42);
    }
}
run Main;
"#,
        )
        .unwrap();

        let tree = load_single_file(&file).unwrap();
        let project = generate_module_tree(&tree, "test");

        assert!(project.main_rs.contains("struct Main;"));
        assert!(project.main_rs.contains("async fn on_start"));
        assert!(project.main_rs.contains("#[tokio::main]"));
    }

    #[test]
    fn generate_module_tree_with_supervisor() {
        use sage_loader::load_single_file;
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.sg");
        fs::write(
            &file,
            r#"
agent Worker {
    on start {
        yield(42);
    }
}
supervisor AppSupervisor {
    strategy: OneForOne
    children {
        Worker { restart: Transient }
    }
}
run AppSupervisor;
"#,
        )
        .unwrap();

        let tree = load_single_file(&file).unwrap();
        let project = generate_module_tree(&tree, "test");

        // Verify supervisor struct is generated
        assert!(
            project.main_rs.contains("struct AppSupervisor;"),
            "Missing supervisor struct. Output:\n{}",
            project.main_rs
        );
        // Verify supervisor main is generated
        assert!(
            project
                .main_rs
                .contains("Supervisor::new(Strategy::OneForOne"),
            "Missing supervisor main. Output:\n{}",
            project.main_rs
        );
    }

    #[test]
    fn generate_supervisor_with_belief_inits() {
        // Tests that supervisor children with belief initializers parse and generate correctly.
        // Belief inits can be written without commas (multiline style).
        let source = r#"
agent QueryMonitor {
    @persistent check_count: Int
    @persistent last_check: String

    on start {
        yield(1);
    }
}

supervisor DbGuardian {
    strategy: OneForOne
    children {
        QueryMonitor {
            restart: Permanent
            check_count: 0
            last_check: "never"
        }
    }
}

run DbGuardian;
"#;

        let output = generate_source(source);

        // Verify supervisor main is generated
        assert!(
            output.contains("#[tokio::main]"),
            "Missing tokio::main. Output:\n{}",
            output
        );
        assert!(
            output.contains("Supervisor::new(Strategy::OneForOne"),
            "Missing supervisor creation. Output:\n{}",
            output
        );
        // Verify belief initializers are set with Persisted::with_initial
        assert!(
            output.contains("check_count: Persisted::with_initial("),
            "Missing check_count with_initial. Output:\n{}",
            output
        );
        assert!(
            output.contains("last_check: Persisted::with_initial("),
            "Missing last_check with_initial. Output:\n{}",
            output
        );
        // Check that values are passed
        assert!(
            output.contains("0_i64"),
            "Missing check_count initial value. Output:\n{}",
            output
        );
        assert!(
            output.contains("\"never\""),
            "Missing last_check initial value. Output:\n{}",
            output
        );
    }

    #[test]
    fn generate_record_declaration() {
        let source = r#"
            record Point {
                x: Int,
                y: Int,
            }
            agent Main {
                on start {
                    let p = Point { x: 10, y: 20 };
                    yield(p.x);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]"));
        assert!(output.contains("struct Point {"));
        assert!(output.contains("x: i64,"));
        assert!(output.contains("y: i64,"));
        assert!(output.contains("Point { x: 10_i64, y: 20_i64 }"));
        assert!(output.contains("p.x"));
    }

    #[test]
    fn generate_enum_declaration() {
        let source = r#"
            enum Status {
                Active,
                Inactive,
                Pending,
            }
            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("#[derive(Debug, Clone, Copy, PartialEq, Eq)]"));
        assert!(output.contains("enum Status {"));
        assert!(output.contains("Active,"));
        assert!(output.contains("Inactive,"));
        assert!(output.contains("Pending,"));
    }

    #[test]
    fn generate_const_declaration() {
        let source = r#"
            const MAX_SIZE: Int = 100;
            const GREETING: String = "Hello";
            agent Main {
                on start {
                    yield(MAX_SIZE);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("const MAX_SIZE: i64 = 100_i64;"));
        // String constants use &'static str since .to_string() isn't const in Rust
        assert!(output.contains("const GREETING: &'static str = \"Hello\";"));
    }

    #[test]
    fn generate_match_expression() {
        let source = r#"
            enum Status {
                Active,
                Inactive,
            }
            fn check_status(s: Status) -> Int {
                return match s {
                    Active => 1,
                    Inactive => 0,
                };
            }
            agent Main {
                on start {
                    yield(0);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        assert!(output.contains("match s {"));
        assert!(output.contains("Active => 1_i64,"));
        assert!(output.contains("Inactive => 0_i64,"));
    }

    // =========================================================================
    // RFC-0007: Error handling codegen tests
    // =========================================================================

    #[test]
    fn generate_fallible_function() {
        let source = r#"
            fn get_data(url: String) -> String fails {
                return url;
            }
            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let output = generate_source(source);
        // Fallible function should return SageResult<T>
        assert!(output.contains("fn get_data(url: String) -> SageResult<String>"));
    }

    #[test]
    fn generate_try_expression() {
        let source = r#"
            fn fallible() -> Int fails { return 42; }
            fn caller() -> Int fails {
                let x = try fallible();
                return x;
            }
            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let output = generate_source(source);
        // try should generate ? operator
        assert!(output.contains("fallible()?"));
    }

    #[test]
    fn generate_catch_expression() {
        let source = r#"
            fn fallible() -> Int fails { return 42; }
            agent Main {
                on start {
                    let x = fallible() catch { 0 };
                    yield(x);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        // catch should generate match expression
        assert!(output.contains("match fallible()"));
        assert!(output.contains("Ok(__val) => __val"));
        assert!(output.contains("Err(_) => 0_i64"));
    }

    #[test]
    fn generate_catch_with_binding() {
        let source = r#"
            fn fallible() -> Int fails { return 42; }
            agent Main {
                on start {
                    let x = fallible() catch(e) { 0 };
                    yield(x);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        // catch with binding should capture the error
        assert!(output.contains("Err(e) => 0_i64"));
    }

    #[test]
    fn generate_on_error_handler() {
        let source = r#"
            agent Main {
                on start {
                    yield(0);
                }
                on error(e) {
                    yield(1);
                }
            }
            run Main;
        "#;

        let output = generate_source(source);
        // Should generate on_error method with &self and &mut ctx
        assert!(output.contains("async fn on_error(&self, _e: SageError, ctx: &mut AgentContext"));
        // Main should dispatch to on_error on failure with &mut ctx
        assert!(output.contains(".on_error(e, &mut ctx)"));
    }

    // =========================================================================
    // RFC-0011: Tool support codegen tests
    // =========================================================================

    #[test]
    fn generate_agent_with_tool_use() {
        let source = r#"
            agent Fetcher {
                use Http

                on start {
                    let r = Http.get("https://example.com");
                    yield(0);
                }
            }
            run Fetcher;
        "#;

        let output = generate_source(source);
        // Should generate struct with http field
        assert!(output.contains("struct Fetcher {"));
        assert!(output.contains("http: HttpClient,"));
        // Should initialize HttpClient in main
        assert!(output.contains("http: HttpClient::from_env()"));
        // Should generate tool call
        assert!(output.contains("self.http.get("));
    }

    #[test]
    fn generate_tool_call_expression() {
        let source = r#"
            agent Fetcher {
                use Http

                on start {
                    let response = Http.get("https://httpbin.org/get");
                    yield(0);
                }
            }
            run Fetcher;
        "#;

        let output = generate_source(source);
        // Tool call should generate self.http.get(...).await (no ?, handled by try/catch)
        assert!(output
            .contains("self.http.get((\"https://httpbin.org/get\".to_string()).clone()).await"));
    }

    fn generate_test_source(source: &str) -> String {
        let lex_result = lex(source).expect("lexing failed");
        let source_arc: Arc<str> = Arc::from(source);
        let (program, errors) = parse(lex_result.tokens(), source_arc);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let program = program.expect("should parse");
        super::generate_test_program(&program, "test").main_rs
    }

    #[test]
    fn generate_mock_tool() {
        let source = r#"
            test "mocks http tool" {
                mock tool Http.get -> "mocked response";
                mock tool Http.post -> fail("network error");
                assert_eq(1, 1);
            }
        "#;

        let output = generate_test_source(source);
        // Should generate MockToolRegistry
        assert!(output.contains("let _mock_tools = MockToolRegistry::new();"));
        // Should register mock responses
        assert!(output.contains("_mock_tools.register(\"Http\", \"get\", MockResponse::value("));
        assert!(output.contains("_mock_tools.register(\"Http\", \"post\", MockResponse::fail("));
    }

    #[test]
    fn generate_mock_infer_and_tool() {
        let source = r#"
            test "mocks both infer and tool" {
                mock divine -> "hello";
                mock tool Http.get -> "response";
                assert_true(true);
            }
        "#;

        let output = generate_test_source(source);
        // Should have both mock client and registry
        assert!(output.contains("MockLlmClient::with_responses"));
        assert!(output.contains("MockToolRegistry::new()"));
    }

    #[test]
    fn generate_supervisor_declaration() {
        let source = r#"
            agent Worker {
                count: Int

                on start {
                    yield(self.count);
                }
            }

            supervisor AppSupervisor {
                strategy: OneForOne

                children {
                    Worker { restart: Transient, count: 0 }
                }
            }

            run AppSupervisor;
        "#;

        let output = generate_source(source);

        // Check supervisor struct is generated
        assert!(output.contains("// Supervisor: AppSupervisor"));
        assert!(output.contains("struct AppSupervisor;"));

        // Check supervisor main is generated with config from grove.toml (defaults)
        assert!(output.contains("Supervisor::new(Strategy::OneForOne"));
        assert!(output.contains("RestartConfig { max_restarts: 5"));
        assert!(output.contains("Duration::from_secs(60)"));
        assert!(output.contains("supervisor.add_child(\"Worker\", RestartPolicy::Transient"));
        assert!(output.contains("supervisor.run()"));
    }

    #[test]
    fn generate_supervisor_with_custom_config() {
        let source = r#"
            agent Worker {
                on start { yield(0); }
            }

            supervisor AppSupervisor {
                strategy: OneForAll

                children {
                    Worker { restart: Permanent }
                }
            }

            run AppSupervisor;
        "#;

        let lex_result = lex(source).expect("lexing failed");
        let source_arc: Arc<str> = Arc::from(source);
        let (program, errors) = parse(lex_result.tokens(), source_arc);
        assert!(errors.is_empty());
        let program = program.expect("should parse");

        // Use custom supervision config (simulating grove.toml values)
        let config = CodegenConfig {
            runtime_dep: RuntimeDep::default(),
            persistence: PersistenceBackend::Memory,
            supervision: SupervisionConfig {
                max_restarts: 10,
                within_seconds: 120,
            },
            observability: ObservabilityConfig::default(),
        };
        let output = generate_with_full_config(&program, "test", config).main_rs;

        // Check custom config values are used
        assert!(output.contains("RestartConfig { max_restarts: 10"));
        assert!(output.contains("Duration::from_secs(120)"));
        assert!(output.contains("Strategy::OneForAll"));
    }

    // =========================================================================
    // Persistence backend configuration tests
    // =========================================================================

    fn generate_with_backend(source: &str, backend: PersistenceBackend) -> String {
        let lex_result = lex(source).expect("lexing failed");
        let source_arc: Arc<str> = Arc::from(source);
        let (program, errors) = parse(lex_result.tokens(), source_arc);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let program = program.expect("should parse");

        let config = CodegenConfig {
            runtime_dep: RuntimeDep::default(),
            persistence: backend,
            supervision: SupervisionConfig::default(),
            observability: ObservabilityConfig::default(),
        };
        generate_with_full_config(&program, "test", config).main_rs
    }

    #[test]
    fn generate_persistence_memory_backend() {
        let source = r#"
            agent Counter {
                @persistent count: Int
                on start {
                    yield(self.count.get());
                }
            }
            run Counter;
        "#;

        let output = generate_with_backend(source, PersistenceBackend::Memory);
        assert!(
            output.contains("MemoryCheckpointStore::new()"),
            "memory backend should use MemoryCheckpointStore"
        );
    }

    #[test]
    fn generate_persistence_sqlite_backend() {
        let source = r#"
            agent Counter {
                @persistent count: Int
                on start {
                    yield(self.count.get());
                }
            }
            run Counter;
        "#;

        let output = generate_with_backend(
            source,
            PersistenceBackend::Sqlite {
                path: ".sage/data.db".to_string(),
            },
        );
        assert!(
            output.contains("SyncSqliteStore::open(\".sage/data.db\")"),
            "sqlite backend should use SyncSqliteStore with correct path"
        );
    }

    #[test]
    fn generate_persistence_postgres_backend() {
        let source = r#"
            agent Counter {
                @persistent count: Int
                on start {
                    yield(self.count.get());
                }
            }
            run Counter;
        "#;

        let output = generate_with_backend(
            source,
            PersistenceBackend::Postgres {
                url: "postgres://localhost/mydb".to_string(),
            },
        );
        assert!(
            output.contains("SyncPostgresStore::connect(\"postgres://localhost/mydb\")"),
            "postgres backend should use SyncPostgresStore with correct url"
        );
    }

    #[test]
    fn generate_persistence_file_backend() {
        let source = r#"
            agent Counter {
                @persistent count: Int
                on start {
                    yield(self.count.get());
                }
            }
            run Counter;
        "#;

        let output = generate_with_backend(
            source,
            PersistenceBackend::File {
                path: "./state".to_string(),
            },
        );
        assert!(
            output.contains("SyncFileStore::open(\"./state\")"),
            "file backend should use SyncFileStore with correct path"
        );
    }

    #[test]
    fn generate_cargo_toml_with_sqlite_feature() {
        let source = r#"
            agent Counter {
                @persistent count: Int
                on start { yield(0); }
            }
            run Counter;
        "#;

        let lex_result = lex(source).expect("lexing failed");
        let source_arc: Arc<str> = Arc::from(source);
        let (program, errors) = parse(lex_result.tokens(), source_arc);
        assert!(errors.is_empty());
        let program = program.expect("should parse");

        let config = CodegenConfig {
            runtime_dep: RuntimeDep::CratesIo {
                version: "1.0.0".to_string(),
            },
            persistence: PersistenceBackend::Sqlite {
                path: ".sage/data.db".to_string(),
            },
            supervision: SupervisionConfig::default(),
            observability: ObservabilityConfig::default(),
        };
        let project = generate_with_full_config(&program, "test", config);

        assert!(
            project.cargo_toml.contains("persistence-sqlite"),
            "Cargo.toml should include persistence-sqlite feature"
        );
    }

    #[test]
    fn generate_cargo_toml_no_feature_for_memory() {
        let source = r#"
            agent Counter {
                @persistent count: Int
                on start { yield(0); }
            }
            run Counter;
        "#;

        let lex_result = lex(source).expect("lexing failed");
        let source_arc: Arc<str> = Arc::from(source);
        let (program, errors) = parse(lex_result.tokens(), source_arc);
        assert!(errors.is_empty());
        let program = program.expect("should parse");

        let config = CodegenConfig {
            runtime_dep: RuntimeDep::CratesIo {
                version: "1.0.0".to_string(),
            },
            persistence: PersistenceBackend::Memory,
            supervision: SupervisionConfig::default(),
            observability: ObservabilityConfig::default(),
        };
        let project = generate_with_full_config(&program, "test", config);

        assert!(
            !project.cargo_toml.contains("persistence-"),
            "Cargo.toml should NOT include any persistence feature for memory backend"
        );
    }

    // =========================================================================
    // Phase 3: Session Types & Algebraic Effects codegen tests
    // =========================================================================

    #[test]
    fn generate_protocol_state_machine() {
        let source = r#"
            protocol PingPong {
                Pinger -> Ponger: Ping
                Ponger -> Pinger: Pong
            }

            record Ping {}
            record Pong {}

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let output = generate_source(source);

        // Check module is generated
        assert!(output.contains("mod protocol_ping_pong"));

        // Check state enum
        assert!(output.contains("pub enum State"));
        assert!(output.contains("Initial"));
        assert!(output.contains("Done"));

        // Check ProtocolStateMachine impl
        assert!(output.contains("impl ProtocolStateMachine for State"));
        assert!(output.contains("fn state_name(&self)"));
        assert!(output.contains("fn can_send(&self, msg_type: &str, from_role: &str)"));
        assert!(output.contains("fn can_receive(&self, msg_type: &str, to_role: &str)"));
        assert!(output.contains("fn transition(&mut self, msg_type: &str)"));
        assert!(output.contains("fn is_terminal(&self)"));
        assert!(output.contains("fn protocol_name(&self)"));
    }

    #[test]
    fn generate_effect_handler_config() {
        let source = r#"
            handler FastLlm handles Infer {
                model: "gpt-4o"
                temperature: 0.7
                max_tokens: 1024
            }

            agent Main {
                on start { yield(0); }
            }
            run Main;
        "#;

        let output = generate_source(source);

        // Check module is generated (FastLlm -> fast_llm)
        assert!(
            output.contains("mod handler_fast_llm"),
            "Should contain handler module: {}",
            output
        );

        // Check Config struct
        assert!(output.contains("pub struct Config"));
        assert!(output.contains("pub model: &'static str"));
        assert!(output.contains("pub temperature: f64"));
        assert!(output.contains("pub max_tokens: i64"));

        // Check CONFIG constant
        assert!(output.contains("pub const CONFIG: Config"));
        assert!(output.contains("model: \"gpt-4o\""));
        assert!(output.contains("temperature: 0.7"));
        assert!(output.contains("max_tokens: 1024"));
    }

    #[test]
    fn generate_reply_expression_parsing() {
        // Note: The current codegen doesn't generate on_message handlers,
        // only on_start. This test verifies the reply expression parses
        // correctly. Full message handler codegen is a future enhancement.
        let source = r#"
            record Request {}
            record Response { code: Int }

            agent Worker receives Request {
                on start { yield(0); }
                on message(msg: Request) {
                    reply(Response { code: 200 });
                }
            }
            run Worker;
        "#;

        // Just verify it compiles and generates something
        let output = generate_source(source);
        assert!(output.contains("struct Worker"));
        assert!(output.contains("struct Request"));
        assert!(output.contains("struct Response"));
    }

    // =========================================================================
    // v2.0: Explicit checkpoint() statement codegen tests
    // =========================================================================

    #[test]
    fn generate_checkpoint_with_persistent_fields() {
        let source = r#"
            agent Counter {
                @persistent count: Int
                @persistent name: String

                on start {
                    checkpoint();
                    yield(0);
                }
            }
            run Counter;
        "#;

        let output = generate_source(source);
        // checkpoint() should generate .checkpoint() calls for each @persistent field
        assert!(
            output.contains("self_count.checkpoint()"),
            "Should generate checkpoint call for count field"
        );
        assert!(
            output.contains("self_name.checkpoint()"),
            "Should generate checkpoint call for name field"
        );
    }

    #[test]
    fn generate_checkpoint_without_persistent_fields() {
        let source = r#"
            agent Worker {
                on start {
                    checkpoint();
                    yield(0);
                }
            }
            run Worker;
        "#;

        let output = generate_source(source);
        // checkpoint() with no @persistent fields is a no-op comment
        assert!(
            output.contains("// checkpoint() - no @persistent fields"),
            "Should generate no-op comment when no persistent fields"
        );
    }
}
