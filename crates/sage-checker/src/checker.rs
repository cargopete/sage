//! Type checker and name resolver for Sage programs.

use crate::error::CheckError;
use crate::scope::{
    resolve_type, resolve_type_with_params, AgentInfo, ConstInfo, EnumInfo, FunctionInfo,
    RecordInfo, Scope, SupervisorInfo, SymbolTable,
};
use crate::types::Type;
use sage_parser::{
    AgentDecl, BinOp, Block, ConstDecl, EventKind, Expr, FnDecl, Literal, Pattern, Program, Stmt,
    SupervisorDecl, UnaryOp,
};
use std::collections::{HashMap, HashSet};

/// Result of type checking a program.
pub struct CheckResult {
    /// The symbol table with all resolved declarations.
    pub symbols: SymbolTable,
    /// Any errors encountered during checking.
    pub errors: Vec<CheckError>,
}

/// A module path like `["agents", "researcher"]`.
pub type ModulePath = Vec<String>;

/// The type checker state.
pub struct Checker {
    /// Global symbol table.
    symbols: SymbolTable,
    /// Stack of local scopes.
    scopes: Vec<Scope>,
    /// Collected errors.
    errors: Vec<CheckError>,
    /// The current agent being checked (if any).
    current_agent: Option<String>,
    /// Whether we're inside a function.
    in_function: bool,
    /// The expected return type of the current function.
    expected_return: Option<Type>,
    /// Beliefs accessed in the current agent (for unused belief warnings).
    used_beliefs: HashSet<String>,
    /// The current module path being checked.
    current_module: ModulePath,
    /// Whether we're inside a loop (for break validation).
    in_loop: bool,
    /// The receives type of the current agent (for receive validation).
    receives_type: Option<Type>,
    /// RFC-0007: Whether we're in a fallible context (function/handler marked fails).
    in_fallible_context: bool,
    /// RFC-0007: Whether the current agent has an error handler.
    agent_has_error_handler: bool,
    /// RFC-0007: Whether we're inside a try or catch expression (for E013 enforcement).
    in_error_handling: bool,
    /// Whether we're inside an on_stop handler (emit is prohibited).
    in_stop_handler: bool,
    /// RFC-0011: Tools declared by the current agent via `use`.
    current_agent_tools: HashSet<String>,
    /// RFC-0011: Reference to scope for tool lookups.
    scope: Scope,
    /// RFC-0012: Whether this is a test file (_test.sg).
    is_test_file: bool,
    /// RFC-0012: Whether we're inside a test block.
    in_test_block: bool,
}

impl Checker {
    /// Create a new type checker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            scopes: vec![Scope::new()],
            errors: Vec::new(),
            current_agent: None,
            in_function: false,
            expected_return: None,
            used_beliefs: HashSet::new(),
            current_module: vec![],
            in_loop: false,
            receives_type: None,
            in_fallible_context: false,
            agent_has_error_handler: false,
            in_error_handling: false,
            in_stop_handler: false,
            current_agent_tools: HashSet::new(),
            scope: Scope::with_builtins(),
            is_test_file: false,
            in_test_block: false,
        }
    }

    /// Create a new type checker for a test file.
    #[must_use]
    pub fn for_test_file() -> Self {
        Self {
            symbols: SymbolTable::new(),
            scopes: vec![Scope::new()],
            errors: Vec::new(),
            current_agent: None,
            in_function: false,
            expected_return: None,
            used_beliefs: HashSet::new(),
            current_module: vec![],
            in_loop: false,
            receives_type: None,
            in_fallible_context: false,
            agent_has_error_handler: false,
            in_error_handling: false,
            in_stop_handler: false,
            current_agent_tools: HashSet::new(),
            scope: Scope::with_builtins(),
            is_test_file: true,
            in_test_block: false,
        }
    }

    /// Create a new type checker for a specific module.
    #[must_use]
    pub fn for_module(module_path: ModulePath) -> Self {
        Self {
            symbols: SymbolTable::new(),
            scopes: vec![Scope::new()],
            errors: Vec::new(),
            current_agent: None,
            in_function: false,
            expected_return: None,
            used_beliefs: HashSet::new(),
            current_module: module_path,
            in_loop: false,
            receives_type: None,
            in_fallible_context: false,
            agent_has_error_handler: false,
            in_error_handling: false,
            in_stop_handler: false,
            current_agent_tools: HashSet::new(),
            scope: Scope::with_builtins(),
            is_test_file: false,
            in_test_block: false,
        }
    }

    /// Check a complete program.
    #[must_use]
    pub fn check(mut self, program: &Program) -> CheckResult {
        // First pass: collect all agent and function declarations
        self.collect_declarations(program);

        // Second pass: type check all declarations
        for agent in &program.agents {
            self.check_agent(agent);
        }

        for supervisor in &program.supervisors {
            self.check_supervisor(supervisor);
        }

        for func in &program.functions {
            self.check_function(func);
        }

        for const_decl in &program.consts {
            self.check_const(const_decl);
        }

        // RFC-0012: Check tests (if any)
        self.check_tests(program);

        // Validate the entry agent (only for non-test files)
        if !self.is_test_file {
            self.validate_entry_agent(program);
        } else if let Some(run_agent) = &program.run_agent {
            // E051: run statement in test file
            self.errors
                .push(CheckError::run_in_test_file(&run_agent.span));
        }

        CheckResult {
            symbols: self.symbols,
            errors: self.errors,
        }
    }

    /// RFC-0012: Check test declarations.
    fn check_tests(&mut self, program: &Program) {
        // E050: Test blocks outside test files
        if !self.is_test_file && !program.tests.is_empty() {
            for test in &program.tests {
                self.errors
                    .push(CheckError::test_outside_test_file(&test.span));
            }
            return;
        }

        // Check for duplicate test names (E055)
        let mut seen_names: HashMap<&str, &sage_parser::TestDecl> = HashMap::new();
        for test in &program.tests {
            if let Some(_existing) = seen_names.get(test.name.as_str()) {
                self.errors
                    .push(CheckError::duplicate_test_name(&test.name, &test.span));
            } else {
                seen_names.insert(&test.name, test);
            }
        }

        // Type check each test block
        for test in &program.tests {
            self.in_test_block = true;
            self.push_scope();
            self.check_block(&test.body);
            self.pop_scope();
            self.in_test_block = false;
        }
    }

    // =========================================================================
    // First pass: collect declarations
    // =========================================================================

    fn collect_declarations(&mut self, program: &Program) {
        // Collect agents
        for agent in &program.agents {
            if self.symbols.has_agent(&agent.name.name) {
                self.errors.push(CheckError::duplicate_definition(
                    &agent.name.name,
                    &agent.span,
                ));
                continue;
            }

            let mut beliefs = HashMap::new();
            for belief in &agent.beliefs {
                let ty = resolve_type(&belief.ty);
                beliefs.insert(belief.name.name.clone(), ty);
            }

            // Find message handler type
            let message_type = agent.handlers.iter().find_map(|h| {
                if let EventKind::Message { param_ty, .. } = &h.event {
                    Some(resolve_type(param_ty))
                } else {
                    None
                }
            });

            let has_start_handler = agent
                .handlers
                .iter()
                .any(|h| matches!(h.event, EventKind::Start));

            self.symbols.define_agent(AgentInfo {
                name: agent.name.name.clone(),
                beliefs,
                message_type,
                emit_type: None, // Will be inferred during checking
                has_start_handler,
                is_pub: agent.is_pub,
                module_path: self.current_module.clone(),
            });
        }

        // Collect functions
        for func in &program.functions {
            if self.symbols.has_function(&func.name.name) {
                self.errors.push(CheckError::duplicate_definition(
                    &func.name.name,
                    &func.span,
                ));
                continue;
            }

            // Build set of type parameters for this function
            let type_param_names: HashSet<String> =
                func.type_params.iter().map(|p| p.name.clone()).collect();

            let params: Vec<(String, Type)> = func
                .params
                .iter()
                .map(|p| {
                    (
                        p.name.name.clone(),
                        resolve_type_with_params(&p.ty, &type_param_names),
                    )
                })
                .collect();

            let return_type = resolve_type_with_params(&func.return_ty, &type_param_names);

            self.symbols.define_function(FunctionInfo {
                name: func.name.name.clone(),
                type_params: func.type_params.iter().map(|p| p.name.clone()).collect(),
                params,
                return_type,
                is_pub: func.is_pub,
                module_path: self.current_module.clone(),
                is_fallible: func.is_fallible,
            });
        }

        // Collect records
        for record in &program.records {
            if self.symbols.has_record(&record.name.name) {
                self.errors.push(CheckError::duplicate_definition(
                    &record.name.name,
                    &record.span,
                ));
                continue;
            }

            // Build set of type parameters for this record
            let type_param_names: HashSet<String> =
                record.type_params.iter().map(|p| p.name.clone()).collect();

            let mut fields = HashMap::new();
            let mut field_order = Vec::new();
            for field in &record.fields {
                let ty = resolve_type_with_params(&field.ty, &type_param_names);
                fields.insert(field.name.name.clone(), ty);
                field_order.push(field.name.name.clone());
            }

            self.symbols.define_record(RecordInfo {
                name: record.name.name.clone(),
                type_params: record.type_params.iter().map(|p| p.name.clone()).collect(),
                fields,
                field_order,
                is_pub: record.is_pub,
                module_path: self.current_module.clone(),
            });
        }

        // Collect enums
        for enum_decl in &program.enums {
            if self.symbols.has_enum(&enum_decl.name.name) {
                self.errors.push(CheckError::duplicate_definition(
                    &enum_decl.name.name,
                    &enum_decl.span,
                ));
                continue;
            }

            // Build set of type parameters for this enum
            let type_param_names: HashSet<String> =
                enum_decl.type_params.iter().map(|p| p.name.clone()).collect();

            let variants: Vec<(String, Option<Type>)> = enum_decl
                .variants
                .iter()
                .map(|v| {
                    let payload = v
                        .payload
                        .as_ref()
                        .map(|ty| resolve_type_with_params(ty, &type_param_names));
                    (v.name.name.clone(), payload)
                })
                .collect();

            self.symbols.define_enum(EnumInfo {
                name: enum_decl.name.name.clone(),
                type_params: enum_decl.type_params.iter().map(|p| p.name.clone()).collect(),
                variants,
                is_pub: enum_decl.is_pub,
                module_path: self.current_module.clone(),
            });
        }

        // Collect consts
        for const_decl in &program.consts {
            if self.symbols.has_const(&const_decl.name.name) {
                self.errors.push(CheckError::duplicate_definition(
                    &const_decl.name.name,
                    &const_decl.span,
                ));
                continue;
            }

            let ty = resolve_type(&const_decl.ty);

            self.symbols.define_const(ConstInfo {
                name: const_decl.name.name.clone(),
                ty,
                is_pub: const_decl.is_pub,
                module_path: self.current_module.clone(),
            });
        }

        // Collect supervisors (v2)
        for supervisor in &program.supervisors {
            if self.symbols.has_supervisor(&supervisor.name.name) {
                self.errors.push(CheckError::duplicate_definition(
                    &supervisor.name.name,
                    &supervisor.span,
                ));
                continue;
            }

            let children: Vec<String> = supervisor
                .children
                .iter()
                .map(|c| c.agent_name.name.clone())
                .collect();

            self.symbols.define_supervisor(SupervisorInfo {
                name: supervisor.name.name.clone(),
                children,
                is_pub: supervisor.is_pub,
                module_path: self.current_module.clone(),
            });
        }
    }

    // =========================================================================
    // Second pass: type checking
    // =========================================================================

    fn check_agent(&mut self, agent: &AgentDecl) {
        self.current_agent = Some(agent.name.name.clone());
        self.used_beliefs.clear();

        // Set receives type from the agent's receives clause
        self.receives_type = agent.receives.as_ref().map(resolve_type);

        // RFC-0007: Check if agent has an error handler
        self.agent_has_error_handler = agent
            .handlers
            .iter()
            .any(|h| matches!(h.event, EventKind::Error { .. }));

        for handler in &agent.handlers {
            self.push_scope();

            // Add message parameter to scope if this is a message handler
            if let EventKind::Message {
                param_name,
                param_ty,
            } = &handler.event
            {
                let ty = resolve_type(param_ty);
                self.define_var(&param_name.name, ty);
            }

            // RFC-0007: Add error parameter to scope if this is an error handler
            if let EventKind::Error { param_name } = &handler.event {
                self.define_var(&param_name.name, Type::Named("Error".to_string()));
            }

            // Track if we're in a stop/resting handler (yield is prohibited)
            let old_in_stop_handler = self.in_stop_handler;
            if matches!(handler.event, EventKind::Stop | EventKind::Resting) {
                self.in_stop_handler = true;
            }

            self.check_block(&handler.body);

            self.in_stop_handler = old_in_stop_handler;
            self.pop_scope();
        }

        // Check for unused beliefs
        for belief in &agent.beliefs {
            if !self.used_beliefs.contains(&belief.name.name) {
                self.errors
                    .push(CheckError::unused_belief(&belief.name.name, &belief.span));
            }
        }

        // Check @persistent fields have serializable types (E052)
        for belief in &agent.beliefs {
            if belief.is_persistent {
                let ty = resolve_type(&belief.ty);
                if !ty.is_serializable() {
                    self.errors
                        .push(CheckError::persistent_field_not_serializable(
                            &belief.name.name,
                            ty.to_string(),
                            &belief.span,
                        ));
                }
            }
        }

        // Check for `on waking` without persistent fields (W006)
        let has_persistent = agent.beliefs.iter().any(|b| b.is_persistent);
        let has_waking = agent
            .handlers
            .iter()
            .any(|h| matches!(h.event, EventKind::Waking));
        if has_waking && !has_persistent {
            // Find the waking handler's span for the warning
            if let Some(waking_handler) = agent
                .handlers
                .iter()
                .find(|h| matches!(h.event, EventKind::Waking))
            {
                self.errors
                    .push(CheckError::waking_without_persistent_fields(
                        &agent.name.name,
                        &waking_handler.body.span,
                    ));
            }
        }

        self.current_agent = None;
        self.receives_type = None;
        self.agent_has_error_handler = false;
    }

    /// Check a supervisor declaration (v2 supervision trees).
    fn check_supervisor(&mut self, supervisor: &SupervisorDecl) {
        // E060: Supervisor has no children
        if supervisor.children.is_empty() {
            self.errors.push(CheckError::supervisor_no_children(
                &supervisor.name.name,
                &supervisor.span,
            ));
            return;
        }

        // Check each child specification
        for child in &supervisor.children {
            // E061: Child agent doesn't exist
            let agent_info = if let Some(info) = self.symbols.get_agent(&child.agent_name.name) {
                info.clone()
            } else {
                self.errors.push(CheckError::supervisor_child_not_found(
                    &child.agent_name.name,
                    &child.span,
                ));
                continue;
            };

            // Check that all required beliefs are initialized
            let provided_beliefs: HashSet<String> =
                child.beliefs.iter().map(|b| b.name.name.clone()).collect();

            for (belief_name, _belief_ty) in &agent_info.beliefs {
                if !provided_beliefs.contains(belief_name) {
                    // E062: Child missing required belief
                    self.errors.push(CheckError::supervisor_child_missing_belief(
                        &child.agent_name.name,
                        belief_name,
                        &child.span,
                    ));
                }
            }

            // Check belief types match
            for field_init in &child.beliefs {
                if let Some(expected_ty) = agent_info.beliefs.get(&field_init.name.name) {
                    let found_ty = self.check_expr(&field_init.value);
                    if !found_ty.is_compatible_with(expected_ty) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_ty.to_string(),
                            found_ty.to_string(),
                            field_init.value.span(),
                        ));
                    }
                } else {
                    // Unknown belief for this agent
                    self.errors.push(CheckError::unknown_field(
                        &field_init.name.name,
                        &field_init.span,
                    ));
                }
            }

            // W004: Permanent restart without @persistent fields
            // Note: Currently we don't track which agents have @persistent fields
            // in AgentInfo. This would require extending AgentInfo.
            // For now, we skip this warning.
        }

        // E063: Supervisor nesting depth
        // Currently, we don't support tracking nesting depth of supervisors.
        // This would require a more complex analysis to detect nested supervisors.
        // For now, we allow one level of supervisor-manages-supervisor.
    }

    fn check_function(&mut self, func: &FnDecl) {
        self.in_function = true;
        self.expected_return = Some(resolve_type(&func.return_ty));

        // RFC-0007: Track if we're in a fallible function
        self.in_fallible_context = func.is_fallible;

        self.push_scope();

        // Add parameters to scope
        for param in &func.params {
            let ty = resolve_type(&param.ty);
            self.define_var(&param.name.name, ty);
        }

        self.check_block(&func.body);

        self.pop_scope();
        self.in_function = false;
        self.expected_return = None;
        self.in_fallible_context = false;
    }

    fn check_const(&mut self, const_decl: &ConstDecl) {
        let declared_ty = resolve_type(&const_decl.ty);
        let value_ty = self.check_expr(&const_decl.value);

        if !value_ty.is_compatible_with(&declared_ty) {
            self.errors.push(CheckError::type_mismatch(
                declared_ty.to_string(),
                value_ty.to_string(),
                const_decl.value.span(),
            ));
        }

        // Verify the value is a constant expression (for now, just literals)
        if !Self::is_const_expr(&const_decl.value) {
            self.errors.push(CheckError::type_mismatch(
                "constant expression",
                "non-constant expression",
                const_decl.value.span(),
            ));
        }
    }

    /// Check if an expression is a constant expression (evaluable at compile time).
    fn is_const_expr(expr: &Expr) -> bool {
        match expr {
            Expr::Literal { .. } => true,
            Expr::Unary { operand, .. } => Self::is_const_expr(operand),
            Expr::Paren { inner, .. } => Self::is_const_expr(inner),
            // For now, we don't allow complex constant expressions
            _ => false,
        }
    }

    fn check_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let value_ty = self.check_expr(value);

                let declared_ty = ty.as_ref().map(resolve_type);

                if let Some(ref decl) = declared_ty {
                    if !value_ty.is_compatible_with(decl) {
                        self.errors.push(CheckError::type_mismatch(
                            decl.to_string(),
                            value_ty.to_string(),
                            value.span(),
                        ));
                    }
                }

                let final_ty = declared_ty.unwrap_or(value_ty);
                self.define_var(&name.name, final_ty);
            }

            Stmt::Assign { name, value, span } => {
                let expected = self.lookup_var(&name.name, &name.span);
                let actual = self.check_expr(value);

                if !actual.is_compatible_with(&expected) {
                    self.errors.push(CheckError::type_mismatch(
                        expected.to_string(),
                        actual.to_string(),
                        span,
                    ));
                }
            }

            Stmt::Return { value, span } => {
                if !self.in_function {
                    self.errors.push(CheckError::return_outside_function(span));
                    return;
                }

                let return_ty = match value {
                    Some(e) => self.check_expr(e),
                    None => Type::Unit,
                };

                if let Some(ref expected) = self.expected_return {
                    if !return_ty.is_compatible_with(expected) {
                        self.errors.push(CheckError::type_mismatch(
                            expected.to_string(),
                            return_ty.to_string(),
                            span,
                        ));
                    }
                }
            }

            Stmt::If {
                condition,
                then_block,
                else_block,
                span,
            } => {
                let cond_ty = self.check_expr(condition);
                if !cond_ty.is_compatible_with(&Type::Bool) {
                    self.errors
                        .push(CheckError::non_bool_condition(cond_ty.to_string(), span));
                }

                self.push_scope();
                self.check_block(then_block);
                self.pop_scope();

                if let Some(else_branch) = else_block {
                    match else_branch {
                        sage_parser::ElseBranch::Block(block) => {
                            self.push_scope();
                            self.check_block(block);
                            self.pop_scope();
                        }
                        sage_parser::ElseBranch::ElseIf(stmt) => {
                            self.check_stmt(stmt);
                        }
                    }
                }
            }

            Stmt::For {
                pattern,
                iter,
                body,
                span,
            } => {
                let iter_ty = self.check_expr(iter);

                // Determine the element type based on the iterable type
                let elem_ty = if let Some(elem) = iter_ty.list_element() {
                    elem.clone()
                } else if let Some((key_ty, value_ty)) = iter_ty.map_key_value() {
                    // Map iteration yields (K, V) tuples
                    Type::Tuple(vec![key_ty.clone(), value_ty.clone()])
                } else {
                    if !iter_ty.is_error() {
                        self.errors
                            .push(CheckError::not_iterable(iter_ty.to_string(), span));
                    }
                    Type::Error
                };

                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.push_scope();
                self.check_pattern(pattern, &elem_ty);
                self.check_block(body);
                self.pop_scope();
                self.in_loop = was_in_loop;
            }

            Stmt::While {
                condition,
                body,
                span,
            } => {
                let cond_ty = self.check_expr(condition);
                if !cond_ty.is_compatible_with(&Type::Bool) {
                    self.errors
                        .push(CheckError::non_bool_condition(cond_ty.to_string(), span));
                }

                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.push_scope();
                self.check_block(body);
                self.pop_scope();
                self.in_loop = was_in_loop;
            }

            Stmt::Loop { body, .. } => {
                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.push_scope();
                self.check_block(body);
                self.pop_scope();
                self.in_loop = was_in_loop;
            }

            Stmt::Break { span } => {
                if !self.in_loop {
                    self.errors.push(CheckError::break_outside_loop(span));
                }
            }

            Stmt::Expr { expr, .. } => {
                self.check_expr(expr);
            }

            Stmt::LetTuple {
                names,
                ty,
                value,
                span,
            } => {
                let value_ty = self.check_expr(value);

                // Value must be a tuple type
                match &value_ty {
                    Type::Tuple(elems) => {
                        if names.len() != elems.len() {
                            self.errors.push(CheckError::tuple_arity_mismatch(
                                names.len(),
                                elems.len(),
                                span,
                            ));
                        } else {
                            // Bind each name to its corresponding element type
                            for (name, elem_ty) in names.iter().zip(elems.iter()) {
                                self.define_var(&name.name, elem_ty.clone());
                            }
                        }
                    }
                    Type::Error => {
                        // Don't cascade errors; bind all to Error
                        for name in names {
                            self.define_var(&name.name, Type::Error);
                        }
                    }
                    _ => {
                        self.errors.push(CheckError::type_mismatch(
                            format!("tuple with {} elements", names.len()),
                            value_ty.to_string(),
                            span,
                        ));
                        // Bind all names to Error to avoid cascading
                        for name in names {
                            self.define_var(&name.name, Type::Error);
                        }
                    }
                }

                // If there's an explicit type annotation, check it matches
                if let Some(type_expr) = ty {
                    let declared_ty = resolve_type(type_expr);
                    if !value_ty.is_compatible_with(&declared_ty) {
                        self.errors.push(CheckError::type_mismatch(
                            declared_ty.to_string(),
                            value_ty.to_string(),
                            span,
                        ));
                    }
                }
            }

            // RFC-0012: mock infer statement - type check the mock value
            Stmt::MockDivine { value, span } => {
                // E056: mock infer outside test block
                if !self.in_test_block {
                    self.errors.push(CheckError::mock_divine_outside_test(span));
                }

                match value {
                    sage_parser::MockValue::Value(expr) => {
                        self.check_expr(expr);
                    }
                    sage_parser::MockValue::Fail(expr) => {
                        let ty = self.check_expr(expr);
                        if !ty.is_compatible_with(&Type::String) {
                            self.errors.push(CheckError::mock_fail_not_string(
                                ty.to_string(),
                                expr.span(),
                            ));
                        }
                    }
                }
            }

            // mock tool statement - type check the mock value
            Stmt::MockTool { value, span, .. } => {
                // E057: mock tool outside test block
                if !self.in_test_block {
                    self.errors.push(CheckError::mock_tool_outside_test(span));
                }

                match value {
                    sage_parser::MockValue::Value(expr) => {
                        self.check_expr(expr);
                    }
                    sage_parser::MockValue::Fail(expr) => {
                        let ty = self.check_expr(expr);
                        if !ty.is_compatible_with(&Type::String) {
                            self.errors.push(CheckError::mock_fail_not_string(
                                ty.to_string(),
                                expr.span(),
                            ));
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal { value, .. } => match value {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::Bool(_) => Type::Bool,
                Literal::String(_) => Type::String,
            },

            Expr::Var { name, .. } => self.lookup_var(&name.name, &name.span),

            Expr::List { elements, .. } => {
                if elements.is_empty() {
                    // Empty list - type is unknown, default to Error
                    // In a real implementation, we'd use type inference
                    Type::List(Box::new(Type::Error))
                } else {
                    let first_ty = self.check_expr(&elements[0]);
                    for elem in &elements[1..] {
                        let elem_ty = self.check_expr(elem);
                        if !elem_ty.is_compatible_with(&first_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                first_ty.to_string(),
                                elem_ty.to_string(),
                                elem.span(),
                            ));
                        }
                    }
                    Type::List(Box::new(first_ty))
                }
            }

            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                let left_ty = self.check_expr(left);
                let right_ty = self.check_expr(right);
                self.check_binary_op(*op, &left_ty, &right_ty, span)
            }

            Expr::Unary { op, operand, span } => {
                let operand_ty = self.check_expr(operand);
                self.check_unary_op(*op, &operand_ty, span)
            }

            Expr::Call {
                name,
                type_args,
                args,
                span,
            } => self.check_call(&name.name, type_args, args, span),

            Expr::SelfField { field, span } => {
                let Some(agent_name) = &self.current_agent else {
                    self.errors.push(CheckError::self_outside_agent(span));
                    return Type::Error;
                };

                let Some(agent) = self.symbols.get_agent(agent_name) else {
                    return Type::Error; // Agent should exist
                };

                if let Some(ty) = agent.beliefs.get(&field.name) {
                    // Mark this belief as used
                    self.used_beliefs.insert(field.name.clone());
                    ty.clone()
                } else {
                    self.errors
                        .push(CheckError::undefined_belief(&field.name, span));
                    Type::Error
                }
            }

            Expr::SelfMethodCall { method, span, .. } => {
                // self.method() is not supported in POC - only self.field
                self.errors
                    .push(CheckError::undefined_function(&method.name, span));
                Type::Error
            }

            Expr::Divine {
                template,
                result_ty,
                span,
            } => {
                // RFC-0007: E013 - infer is a fallible operation, must be wrapped in try or catch
                if !self.in_error_handling {
                    self.errors.push(CheckError::unhandled_error("infer", span));
                }

                // Track belief usage and type-check interpolated expressions
                for part in &template.parts {
                    if let sage_parser::StringPart::Interpolation(expr) = part {
                        self.check_expr(expr);
                    }
                }
                // infer returns Inferred<T>, default to Inferred<String>
                let inner = result_ty.as_ref().map_or(Type::String, resolve_type);
                Type::Oracle(Box::new(inner))
            }

            Expr::Summon {
                agent,
                fields,
                span,
            } => {
                let Some(agent_info) = self.symbols.get_agent(&agent.name).cloned() else {
                    self.errors
                        .push(CheckError::undefined_agent(&agent.name, span));
                    return Type::Error;
                };

                // Check that all required beliefs are provided
                let mut provided: HashMap<String, bool> = agent_info
                    .beliefs
                    .keys()
                    .map(|k| (k.clone(), false))
                    .collect();

                for field in fields {
                    let field_name = &field.name.name;

                    if let Some(expected_ty) = agent_info.beliefs.get(field_name) {
                        provided.insert(field_name.clone(), true);
                        let actual_ty = self.check_expr(&field.value);

                        if !actual_ty.is_compatible_with(expected_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                expected_ty.to_string(),
                                actual_ty.to_string(),
                                field.value.span(),
                            ));
                        }
                    } else {
                        self.errors
                            .push(CheckError::unknown_field(field_name, &field.span));
                    }
                }

                // Check for missing beliefs
                for (name, was_provided) in &provided {
                    if !was_provided {
                        self.errors
                            .push(CheckError::missing_belief_init(name, span));
                    }
                }

                Type::Agent(agent.name.clone())
            }

            Expr::Await {
                handle,
                timeout,
                span,
            } => {
                let handle_ty = self.check_expr(handle);

                // Check timeout if present (must be Int)
                if let Some(timeout_expr) = timeout {
                    let timeout_ty = self.check_expr(timeout_expr);
                    if !matches!(timeout_ty, Type::Int) && !timeout_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Int".to_string(),
                            timeout_ty.to_string(),
                            span,
                        ));
                    }
                }

                // RFC-0007: E013 - await is a fallible operation, must be wrapped in try or catch
                if !self.in_error_handling {
                    self.errors.push(CheckError::unhandled_error("await", span));
                }

                if let Some(agent_name) = handle_ty.agent_name() {
                    // The result type is the emit type of the agent
                    // For now, default to String since emit_type inference isn't implemented
                    self.symbols
                        .get_agent(agent_name)
                        .and_then(|a| a.emit_type.clone())
                        .unwrap_or(Type::String)
                } else {
                    if !handle_ty.is_error() {
                        self.errors
                            .push(CheckError::await_non_agent(handle_ty.to_string(), span));
                    }
                    Type::Error
                }
            }

            Expr::Send {
                handle,
                message,
                span,
            } => {
                // RFC-0007: E013 - send is a fallible operation, must be wrapped in try or catch
                if !self.in_error_handling {
                    self.errors.push(CheckError::unhandled_error("send", span));
                }

                let handle_ty = self.check_expr(handle);
                let msg_ty = self.check_expr(message);

                if let Some(agent_name) = handle_ty.agent_name() {
                    if let Some(agent_info) = self.symbols.get_agent(agent_name) {
                        if let Some(expected) = &agent_info.message_type {
                            if !msg_ty.is_compatible_with(expected) {
                                self.errors.push(CheckError::type_mismatch(
                                    expected.to_string(),
                                    msg_ty.to_string(),
                                    message.span(),
                                ));
                            }
                        } else {
                            self.errors
                                .push(CheckError::no_message_handler(agent_name, span));
                        }
                    }
                } else if !handle_ty.is_error() {
                    self.errors
                        .push(CheckError::send_non_agent(handle_ty.to_string(), span));
                }

                Type::Unit
            }

            Expr::Yield { value, span } => {
                // Check for emit in stop handler
                if self.in_stop_handler {
                    self.errors.push(CheckError::yield_in_stop_handler(span));
                }

                let value_ty = self.check_expr(value);

                // Record the emit type for the current agent
                if let Some(agent_name) = &self.current_agent {
                    if let Some(agent) = self.symbols.get_agent_mut(agent_name) {
                        agent.emit_type = Some(value_ty.clone());
                    }
                }

                Type::Unit
            }

            Expr::Paren { inner, .. } => self.check_expr(inner),

            Expr::StringInterp { template, .. } => {
                // Check all interpolated expressions (RFC-0013)
                for part in &template.parts {
                    if let sage_parser::StringPart::Interpolation(expr) = part {
                        self.check_expr(expr);
                    }
                }
                Type::String
            }

            Expr::Match {
                scrutinee,
                arms,
                span,
            } => {
                let scrutinee_ty = self.check_expr(scrutinee);

                // Track covered patterns for exhaustiveness
                let mut has_wildcard = false;
                let mut covered_variants: HashSet<String> = HashSet::new();
                let mut covered_bool_true = false;
                let mut covered_bool_false = false;

                let mut result_ty = Type::Error;
                for arm in arms {
                    // Check pattern and get any bindings
                    self.push_scope();
                    self.check_pattern(&arm.pattern, &scrutinee_ty);

                    // Track coverage for exhaustiveness
                    match &arm.pattern {
                        Pattern::Wildcard { .. } | Pattern::Binding { .. } => {
                            has_wildcard = true;
                        }
                        Pattern::Variant { variant, .. } => {
                            covered_variants.insert(variant.name.clone());
                        }
                        Pattern::Literal {
                            value: Literal::Bool(b),
                            ..
                        } => {
                            if *b {
                                covered_bool_true = true;
                            } else {
                                covered_bool_false = true;
                            }
                        }
                        Pattern::Literal { .. } => {
                            // Literal patterns don't guarantee coverage
                        }
                        Pattern::Tuple { .. } => {
                            // Tuple patterns don't guarantee exhaustive coverage
                        }
                    }

                    // Check body expression
                    let arm_ty = self.check_expr(&arm.body);
                    self.pop_scope();

                    if result_ty.is_error() {
                        result_ty = arm_ty;
                    }
                }

                // Check exhaustiveness
                if !has_wildcard {
                    let is_exhaustive = match &scrutinee_ty {
                        Type::Named(name) | Type::Generic(name, _) => {
                            // Check if it's an enum and all variants are covered
                            if let Some(enum_info) = self.symbols.get_enum(name) {
                                enum_info
                                    .variants
                                    .iter()
                                    .all(|(v, _)| covered_variants.contains(v))
                            } else {
                                // Not an enum - needs wildcard
                                false
                            }
                        }
                        // Option<T> requires Some and None
                        Type::Option(_) => {
                            covered_variants.contains("Some") && covered_variants.contains("None")
                        }
                        // Result<T, E> requires Ok and Err
                        Type::Result(_, _) => {
                            covered_variants.contains("Ok") && covered_variants.contains("Err")
                        }
                        Type::Bool => covered_bool_true && covered_bool_false,
                        Type::Error => true, // Don't report exhaustiveness errors on error types
                        _ => false,          // Other types need a wildcard to be exhaustive
                    };

                    if !is_exhaustive {
                        self.errors.push(CheckError::non_exhaustive_match(span));
                    }
                }

                result_ty
            }

            Expr::RecordConstruct {
                name,
                type_args,
                fields,
                span,
            } => {
                let Some(record_info) = self.symbols.get_record(&name.name).cloned() else {
                    self.errors
                        .push(CheckError::undefined_type(&name.name, span));
                    return Type::Error;
                };

                // RFC-0015: Handle generic records
                let bindings = if !record_info.type_params.is_empty() {
                    self.resolve_record_type_bindings(
                        &record_info.type_params,
                        type_args,
                        &record_info.fields,
                        fields,
                        span,
                    )
                } else {
                    // Non-generic record: warn if type args provided
                    if !type_args.is_empty() {
                        self.errors.push(CheckError::generic(
                            format!(
                                "record `{}` is not generic but was constructed with type arguments",
                                name.name
                            ),
                            span,
                        ));
                    }
                    HashMap::new()
                };

                // Track which fields have been provided
                let mut provided: HashMap<String, bool> = record_info
                    .fields
                    .keys()
                    .map(|k| (k.clone(), false))
                    .collect();

                for field in fields {
                    let field_name = &field.name.name;

                    if let Some(expected_ty) = record_info.fields.get(field_name) {
                        provided.insert(field_name.clone(), true);
                        let actual_ty = self.check_expr(&field.value);
                        // Substitute type params in expected type
                        let expected_ty = expected_ty.substitute(&bindings);

                        if !actual_ty.is_compatible_with(&expected_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                expected_ty.to_string(),
                                actual_ty.to_string(),
                                field.value.span(),
                            ));
                        }
                    } else {
                        self.errors
                            .push(CheckError::unknown_field(field_name, &field.span));
                    }
                }

                // Check for missing fields
                for (field_name, was_provided) in &provided {
                    if !was_provided {
                        self.errors
                            .push(CheckError::missing_field(field_name, &name.name, span));
                    }
                }

                // Return Generic type if record has type params, otherwise Named
                if record_info.type_params.is_empty() {
                    Type::Named(name.name.clone())
                } else {
                    let type_args: Vec<Type> = record_info
                        .type_params
                        .iter()
                        .map(|p| bindings.get(p).cloned().unwrap_or(Type::Error))
                        .collect();
                    Type::Generic(name.name.clone(), type_args)
                }
            }

            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let obj_ty = self.check_expr(object);

                // Get the record name from the type
                let record_name = match &obj_ty {
                    Type::Named(name) => name.clone(),
                    Type::Error => return Type::Error,
                    _ => {
                        self.errors.push(CheckError::field_access_on_non_record(
                            obj_ty.to_string(),
                            span,
                        ));
                        return Type::Error;
                    }
                };

                // Look up the record and get field type
                if let Some(record_info) = self.symbols.get_record(&record_name) {
                    if let Some(field_ty) = record_info.fields.get(&field.name) {
                        field_ty.clone()
                    } else {
                        self.errors
                            .push(CheckError::unknown_field(&field.name, span));
                        Type::Error
                    }
                } else {
                    // It's a Named type but not a record - could be enum
                    self.errors.push(CheckError::field_access_on_non_record(
                        obj_ty.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            Expr::Receive { span } => {
                // Must be inside an agent
                if self.current_agent.is_none() {
                    self.errors.push(CheckError::receive_outside_agent(span));
                    return Type::Error;
                }

                // Must have a receives type
                match &self.receives_type {
                    Some(ty) => ty.clone(),
                    None => {
                        self.errors.push(CheckError::receive_without_receives(
                            self.current_agent.as_ref().unwrap(),
                            span,
                        ));
                        Type::Error
                    }
                }
            }

            // RFC-0007: Error handling
            Expr::Try { expr, span } => {
                // Check that we're in a fallible context (can propagate errors)
                if !self.in_fallible_context {
                    // In an agent, check for error handler
                    if self.current_agent.is_some() && !self.agent_has_error_handler {
                        self.errors.push(CheckError::missing_error_handler(
                            self.current_agent.as_ref().unwrap().clone(),
                            span,
                        ));
                    } else if self.current_agent.is_none() {
                        self.errors.push(CheckError::try_in_non_fallible(span));
                    }
                }

                // Set error handling context for inner expression (suppresses E013)
                let old_in_error_handling = self.in_error_handling;
                self.in_error_handling = true;

                // Check the inner expression
                let inner_ty = self.check_expr(expr);

                // Restore error handling context
                self.in_error_handling = old_in_error_handling;

                // Return the inner type (unwrapped from potential error)
                inner_ty
            }

            Expr::Catch {
                expr,
                error_bind,
                recovery,
                span,
            } => {
                // Set error handling context for inner expression (suppresses E013)
                let old_in_error_handling = self.in_error_handling;
                self.in_error_handling = true;

                // Check the inner (fallible) expression
                let expr_ty = self.check_expr(expr);

                // Restore error handling context
                self.in_error_handling = old_in_error_handling;

                // Create a new scope for the recovery block (for error binding)
                self.push_scope();

                // If there's an error binding, add it to scope
                if let Some(err_ident) = error_bind {
                    // Error type has .message (String) and .kind (ErrorKind)
                    // For now, use a simple Named type
                    self.define_var(&err_ident.name, Type::Named("Error".to_string()));
                }

                // Check the recovery expression
                let recovery_ty = self.check_expr(recovery);

                self.pop_scope();

                // Recovery type must match the expression type
                if !recovery_ty.is_compatible_with(&expr_ty) && !expr_ty.is_error() {
                    self.errors.push(CheckError::catch_type_mismatch(
                        expr_ty.to_string(),
                        recovery_ty.to_string(),
                        span,
                    ));
                }

                // Return the expression type (catch handles the error internally)
                expr_ty
            }

            // fail expression - explicit error raising
            Expr::Fail { error, span } => {
                // Check that we're in a fallible context (can propagate errors)
                if !self.in_fallible_context {
                    // In an agent, check for error handler
                    if self.current_agent.is_some() && !self.agent_has_error_handler {
                        self.errors.push(CheckError::missing_error_handler(
                            self.current_agent.as_ref().unwrap().clone(),
                            span,
                        ));
                    } else if self.current_agent.is_none() {
                        self.errors.push(CheckError::try_in_non_fallible(span));
                    }
                }

                // Check the error expression
                let error_ty = self.check_expr(error);

                // Error must be either a String or the Error type
                let is_valid_error = matches!(error_ty, Type::String)
                    || matches!(&error_ty, Type::Named(n) if n == "Error")
                    || error_ty.is_error();
                if !is_valid_error {
                    self.errors.push(CheckError::type_mismatch(
                        "String or Error".to_string(),
                        error_ty.to_string(),
                        span,
                    ));
                }

                // Return Never type - this expression never returns
                Type::Never
            }

            // retry expression - retry a fallible operation
            Expr::Retry {
                count,
                delay,
                on_errors,
                body,
                span,
            } => {
                // Check that count is an Int
                let count_ty = self.check_expr(count);
                if !matches!(count_ty, Type::Int) && !count_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int".to_string(),
                        count_ty.to_string(),
                        count.span(),
                    ));
                }

                // Check delay if present (must be Int)
                if let Some(delay_expr) = delay {
                    let delay_ty = self.check_expr(delay_expr);
                    if !matches!(delay_ty, Type::Int) && !delay_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Int".to_string(),
                            delay_ty.to_string(),
                            delay_expr.span(),
                        ));
                    }
                }

                // Check on_errors if present (each should be an ErrorKind variant)
                if let Some(errors) = on_errors {
                    for err_expr in errors {
                        let _err_ty = self.check_expr(err_expr);
                        // For now just type check - could validate ErrorKind later
                    }
                }

                // The body is checked in a fallible context (errors can propagate)
                let prev_fallible = self.in_fallible_context;
                let prev_error_handling = self.in_error_handling;
                self.in_fallible_context = true;
                self.in_error_handling = true;

                let body_ty = self.check_expr(body);

                self.in_fallible_context = prev_fallible;
                self.in_error_handling = prev_error_handling;

                // If retry itself is not in an error handling context, emit warning
                // since all retries could fail
                if !self.in_error_handling {
                    self.errors.push(CheckError::unhandled_error("retry", span));
                }

                // Return the body type
                body_ty
            }

            // trace(message) - emit a trace event
            Expr::Trace { message, span } => {
                let msg_ty = self.check_expr(message);
                if !matches!(msg_ty, Type::String) && !msg_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "String".to_string(),
                        msg_ty.to_string(),
                        span,
                    ));
                }
                Type::Unit
            }

            // RFC-0009: Closures
            Expr::Closure { params, body, .. } => {
                // Push a new scope for closure parameters
                self.push_scope();

                let mut param_types = Vec::new();
                for param in params {
                    let param_ty = if let Some(ty_expr) = &param.ty {
                        resolve_type(ty_expr)
                    } else {
                        // For now require explicit types - inference comes later
                        self.errors.push(CheckError::closure_param_needs_type(
                            param.name.name.clone(),
                            &param.span,
                        ));
                        Type::Error
                    };
                    self.define_var(&param.name.name, param_ty.clone());
                    param_types.push(param_ty);
                }

                // Type check the body
                let body_ty = self.check_expr(body);

                self.pop_scope();

                // Return Fn type
                Type::Fn(param_types, Box::new(body_ty))
            }

            Expr::Tuple { elements, .. } => {
                let elem_types: Vec<Type> = elements.iter().map(|e| self.check_expr(e)).collect();
                Type::Tuple(elem_types)
            }

            Expr::TupleIndex { tuple, index, span } => {
                let tuple_ty = self.check_expr(tuple);
                match &tuple_ty {
                    Type::Tuple(elems) => {
                        if *index < elems.len() {
                            elems[*index].clone()
                        } else {
                            self.errors.push(CheckError::tuple_index_out_of_bounds(
                                *index,
                                elems.len(),
                                span,
                            ));
                            Type::Error
                        }
                    }
                    Type::Error => Type::Error,
                    _ => {
                        self.errors.push(CheckError::type_mismatch(
                            "tuple",
                            tuple_ty.to_string(),
                            span,
                        ));
                        Type::Error
                    }
                }
            }

            Expr::Map { entries, span } => {
                if entries.is_empty() {
                    // Empty map - we can't infer the types, report an error
                    // or use a placeholder. For now, require at least one entry.
                    self.errors.push(CheckError::empty_map_literal(span));
                    Type::Error
                } else {
                    // Check all keys have the same type and all values have the same type
                    let first_key_ty = self.check_expr(&entries[0].key);
                    let first_val_ty = self.check_expr(&entries[0].value);

                    for entry in entries.iter().skip(1) {
                        let key_ty = self.check_expr(&entry.key);
                        let val_ty = self.check_expr(&entry.value);

                        if !key_ty.is_compatible_with(&first_key_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                first_key_ty.to_string(),
                                key_ty.to_string(),
                                &entry.span,
                            ));
                        }
                        if !val_ty.is_compatible_with(&first_val_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                first_val_ty.to_string(),
                                val_ty.to_string(),
                                &entry.span,
                            ));
                        }
                    }

                    Type::Map(Box::new(first_key_ty), Box::new(first_val_ty))
                }
            }

            Expr::VariantConstruct {
                enum_name,
                type_args,
                variant,
                payload,
                span,
            } => {
                // Look up the enum
                let Some(enum_info) = self.symbols.get_enum(&enum_name.name).cloned() else {
                    self.errors
                        .push(CheckError::undefined_type(&enum_name.name, span));
                    return Type::Error;
                };

                // RFC-0015: Handle generic enums
                let bindings = if !enum_info.type_params.is_empty() {
                    self.resolve_enum_type_bindings(
                        &enum_info.type_params,
                        type_args,
                        &enum_info.variants,
                        &variant.name,
                        payload.as_deref(),
                        span,
                    )
                } else {
                    // Non-generic enum: warn if type args provided
                    if !type_args.is_empty() {
                        self.errors.push(CheckError::generic(
                            format!(
                                "enum `{}` is not generic but was constructed with type arguments",
                                enum_name.name
                            ),
                            span,
                        ));
                    }
                    HashMap::new()
                };

                // Check if the variant exists and get its expected payload type
                let Some(expected_payload) = enum_info.get_variant_payload(&variant.name) else {
                    self.errors.push(CheckError::undefined_enum_variant(
                        &variant.name,
                        &enum_name.name,
                        span,
                    ));
                    return Type::Error;
                };

                // Substitute type params in expected payload type
                let expected_payload = expected_payload.map(|ty| ty.substitute(&bindings));

                // Check payload matches expectation
                match (payload, &expected_payload) {
                    (Some(payload_expr), Some(expected_ty)) => {
                        // Variant expects payload and we have one
                        let payload_ty = self.check_expr(payload_expr);
                        if !payload_ty.is_compatible_with(expected_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                expected_ty.to_string(),
                                payload_ty.to_string(),
                                span,
                            ));
                        }
                    }
                    (None, Some(expected_ty)) => {
                        // Variant expects payload but we don't have one
                        self.errors.push(CheckError::type_mismatch(
                            format!("{}({})", variant.name, expected_ty),
                            variant.name.clone(),
                            span,
                        ));
                    }
                    (Some(_), None) => {
                        // Variant doesn't expect payload but we have one
                        self.errors.push(CheckError::type_mismatch(
                            variant.name.clone(),
                            format!("{}(...)", variant.name),
                            span,
                        ));
                    }
                    (None, None) => {
                        // Unit variant - all good
                    }
                }

                // Return Generic type if enum has type params, otherwise Named
                // Special case: Option and Result use their built-in Type variants
                if enum_info.type_params.is_empty() {
                    Type::Named(enum_name.name.clone())
                } else {
                    let resolved_type_args: Vec<Type> = enum_info
                        .type_params
                        .iter()
                        .map(|p| bindings.get(p).cloned().unwrap_or(Type::Error))
                        .collect();

                    // Map built-in enum types to their native Type variants
                    match enum_name.name.as_str() {
                        "Option" if resolved_type_args.len() == 1 => {
                            Type::Option(Box::new(resolved_type_args[0].clone()))
                        }
                        "Result" if resolved_type_args.len() == 2 => {
                            Type::Result(
                                Box::new(resolved_type_args[0].clone()),
                                Box::new(resolved_type_args[1].clone()),
                            )
                        }
                        _ => Type::Generic(enum_name.name.clone(), resolved_type_args),
                    }
                }
            }

            // RFC-0011: Tool calls
            Expr::ToolCall {
                tool,
                function,
                args,
                span,
            } => {
                // Check that the agent has declared this tool via `use`
                if !self.current_agent_tools.contains(&tool.name) {
                    self.errors
                        .push(CheckError::undeclared_tool_use(&tool.name, span));
                    return Type::Error;
                }

                // Look up the tool in scope and extract function info
                // (clone to avoid borrow conflicts with check_expr)
                let fn_lookup = self.scope.lookup_tool(&tool.name).and_then(|t| {
                    t.functions
                        .get(&function.name)
                        .map(|f| (f.params.clone(), f.return_ty.clone()))
                });
                let tool_exists = self.scope.lookup_tool(&tool.name).is_some();

                if !tool_exists {
                    // Tool not in scope - shouldn't happen for builtins
                    self.errors
                        .push(CheckError::undeclared_tool_use(&tool.name, span));
                    return Type::Error;
                }

                if let Some((params, return_ty)) = fn_lookup {
                    // Check argument count
                    if args.len() != params.len() {
                        self.errors.push(CheckError::tool_call_arity(
                            &tool.name,
                            &function.name,
                            params.len(),
                            args.len(),
                            span,
                        ));
                        return Type::Error;
                    }

                    // Check argument types
                    for (arg, (_, expected_ty)) in args.iter().zip(params.iter()) {
                        let arg_ty = self.check_expr(arg);
                        if !arg_ty.is_compatible_with(expected_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                expected_ty.to_string(),
                                arg_ty.to_string(),
                                arg.span(),
                            ));
                        }
                    }

                    return_ty
                } else {
                    self.errors.push(CheckError::undefined_tool_function(
                        &tool.name,
                        &function.name,
                        span,
                    ));
                    Type::Error
                }
            }
        }
    }

    fn check_binary_op(
        &mut self,
        op: BinOp,
        left: &Type,
        right: &Type,
        span: &sage_parser::Span,
    ) -> Type {
        // Handle error propagation
        if left.is_error() || right.is_error() {
            return Type::Error;
        }

        // Unwrap inferred types for comparison
        let left = left.unwrap_oracle();
        let right = right.unwrap_oracle();

        match op {
            // Arithmetic: Int/Float
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                if left.is_numeric() && left == right {
                    left.clone()
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        format!("{op}"),
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            // String concatenation
            BinOp::Concat => {
                if matches!(left, Type::String) && matches!(right, Type::String) {
                    Type::String
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        "++",
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            // Comparison: same types
            BinOp::Eq | BinOp::Ne => {
                if left == right {
                    Type::Bool
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        format!("{op}"),
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            // Ordering: numeric only
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                if left.is_numeric() && left == right {
                    Type::Bool
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        format!("{op}"),
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            // Logical: Bool only
            BinOp::And | BinOp::Or => {
                if matches!(left, Type::Bool) && matches!(right, Type::Bool) {
                    Type::Bool
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        format!("{op}"),
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }
        }
    }

    fn check_unary_op(&mut self, op: UnaryOp, operand: &Type, span: &sage_parser::Span) -> Type {
        if operand.is_error() {
            return Type::Error;
        }

        let operand = operand.unwrap_oracle();

        match op {
            UnaryOp::Neg => {
                if operand.is_numeric() {
                    operand.clone()
                } else {
                    self.errors
                        .push(CheckError::invalid_unary_op("-", operand.to_string(), span));
                    Type::Error
                }
            }
            UnaryOp::Not => {
                if matches!(operand, Type::Bool) {
                    Type::Bool
                } else {
                    self.errors
                        .push(CheckError::invalid_unary_op("!", operand.to_string(), span));
                    Type::Error
                }
            }
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, scrutinee_ty: &Type) {
        match pattern {
            Pattern::Wildcard { .. } => {
                // Wildcard matches anything - no bindings introduced
            }
            Pattern::Binding { name, .. } => {
                // Binding pattern introduces a variable with the scrutinee's type
                self.define_var(&name.name, scrutinee_ty.clone());
            }
            Pattern::Variant {
                enum_name,
                variant,
                payload,
                span,
            } => {
                // Check that the scrutinee is the correct enum type
                // Handle built-in Option/Result types specially
                let (expected_enum, type_bindings): (Option<String>, std::collections::HashMap<String, Type>) = match scrutinee_ty {
                    Type::Named(name) => (Some(name.clone()), std::collections::HashMap::new()),
                    Type::Option(inner) => {
                        let mut bindings = std::collections::HashMap::new();
                        bindings.insert("T".to_string(), (**inner).clone());
                        (Some("Option".to_string()), bindings)
                    }
                    Type::Result(ok, err) => {
                        let mut bindings = std::collections::HashMap::new();
                        bindings.insert("T".to_string(), (**ok).clone());
                        bindings.insert("E".to_string(), (**err).clone());
                        (Some("Result".to_string()), bindings)
                    }
                    Type::Generic(name, args) => {
                        // For generic enums, build type bindings from args
                        if let Some(enum_info) = self.symbols.get_enum(name) {
                            let mut bindings = std::collections::HashMap::new();
                            for (param, arg) in enum_info.type_params.iter().zip(args.iter()) {
                                bindings.insert(param.clone(), arg.clone());
                            }
                            (Some(name.clone()), bindings)
                        } else {
                            (Some(name.clone()), std::collections::HashMap::new())
                        }
                    }
                    Type::Error => return, // Don't cascade errors
                    _ => (None, std::collections::HashMap::new()),
                };

                // Determine the enum name to use for lookup
                let enum_name_for_lookup = enum_name
                    .as_ref()
                    .map(|e| e.name.clone())
                    .or_else(|| expected_enum.clone());

                if let Some(enum_name_str) = &enum_name {
                    // Qualified variant: Status::Active
                    if let Some(expected) = &expected_enum {
                        if enum_name_str.name != *expected {
                            self.errors.push(CheckError::type_mismatch(
                                expected,
                                &enum_name_str.name,
                                span,
                            ));
                            return;
                        }
                    }

                    // Check that the variant exists in the enum
                    if let Some(enum_info) = self.symbols.get_enum(&enum_name_str.name) {
                        if !enum_info.has_variant(&variant.name) {
                            self.errors.push(CheckError::undefined_enum_variant(
                                &variant.name,
                                &enum_name_str.name,
                                span,
                            ));
                        }
                    } else {
                        self.errors
                            .push(CheckError::undefined_type(&enum_name_str.name, span));
                    }
                } else {
                    // Unqualified variant: just `Active`
                    // Need to check against the scrutinee's enum type
                    if let Some(ref enum_name_str) = expected_enum {
                        if let Some(enum_info) = self.symbols.get_enum(enum_name_str) {
                            if !enum_info.has_variant(&variant.name) {
                                self.errors.push(CheckError::undefined_enum_variant(
                                    &variant.name,
                                    enum_name_str,
                                    span,
                                ));
                            }
                        }
                    } else if !scrutinee_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "enum type",
                            scrutinee_ty.to_string(),
                            span,
                        ));
                    }
                }

                // Handle payload binding
                if let Some(enum_name_str) = enum_name_for_lookup {
                    if let Some(enum_info) = self.symbols.get_enum(&enum_name_str).cloned() {
                        if let Some(expected_payload_ty) =
                            enum_info.get_variant_payload(&variant.name)
                        {
                            match (payload, expected_payload_ty) {
                                (Some(inner_pattern), Some(payload_ty)) => {
                                    // Substitute type parameters in the payload type
                                    let resolved_payload_ty = if type_bindings.is_empty() {
                                        payload_ty.clone()
                                    } else {
                                        payload_ty.substitute(&type_bindings)
                                    };
                                    // Recursively check the inner pattern
                                    self.check_pattern(inner_pattern, &resolved_payload_ty);
                                }
                                (None, Some(_)) => {
                                    // Variant has payload but pattern doesn't bind it
                                    // This is OK - we just ignore the payload value
                                }
                                (Some(_), None) => {
                                    // Pattern tries to bind but variant has no payload
                                    self.errors.push(CheckError::type_mismatch(
                                        variant.name.clone(),
                                        format!("{}(...)", variant.name),
                                        span,
                                    ));
                                }
                                (None, None) => {
                                    // Unit variant, no payload - all good
                                }
                            }
                        }
                    }
                }
            }
            Pattern::Literal { value, span } => {
                // Check that the literal type matches the scrutinee type
                let lit_ty = match value {
                    Literal::Int(_) => Type::Int,
                    Literal::Float(_) => Type::Float,
                    Literal::Bool(_) => Type::Bool,
                    Literal::String(_) => Type::String,
                };

                if !lit_ty.is_compatible_with(scrutinee_ty) && !scrutinee_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        scrutinee_ty.to_string(),
                        lit_ty.to_string(),
                        span,
                    ));
                }
            }

            Pattern::Tuple { elements, span } => {
                // Scrutinee must be a tuple with matching arity
                match scrutinee_ty {
                    Type::Tuple(elem_types) => {
                        if elements.len() != elem_types.len() {
                            self.errors.push(CheckError::tuple_arity_mismatch(
                                elements.len(),
                                elem_types.len(),
                                span,
                            ));
                        } else {
                            // Recursively check each element pattern
                            for (pattern, elem_ty) in elements.iter().zip(elem_types.iter()) {
                                self.check_pattern(pattern, elem_ty);
                            }
                        }
                    }
                    Type::Error => {
                        // Don't cascade errors
                    }
                    _ => {
                        self.errors.push(CheckError::type_mismatch(
                            "tuple",
                            scrutinee_ty.to_string(),
                            span,
                        ));
                    }
                }
            }
        }
    }

    fn check_call(
        &mut self,
        name: &str,
        type_args: &[sage_parser::TypeExpr],
        args: &[Expr],
        span: &sage_parser::Span,
    ) -> Type {
        // Check for user-defined function
        if let Some(func) = self.symbols.get_function(name).cloned() {
            // RFC-0007: E013 - fallible functions must be wrapped in try or catch
            if func.is_fallible && !self.in_error_handling {
                self.errors.push(CheckError::unhandled_error(name, span));
            }

            if args.len() != func.params.len() {
                self.errors.push(CheckError::wrong_arg_count(
                    name,
                    func.params.len(),
                    args.len(),
                    span,
                ));
                return Type::Error;
            }

            // RFC-0015: Handle generic functions
            let bindings = if !func.type_params.is_empty() {
                self.resolve_type_bindings(&func.type_params, type_args, &func.params, args, span)
            } else {
                // Non-generic function: warn if type args provided
                if !type_args.is_empty() {
                    self.errors.push(CheckError::generic(
                        format!(
                            "function `{}` is not generic but was called with type arguments",
                            name
                        ),
                        span,
                    ));
                }
                HashMap::new()
            };

            // Check arguments against (possibly substituted) parameter types
            for (arg, (_, param_ty)) in args.iter().zip(func.params.iter()) {
                let arg_ty = self.check_expr(arg);
                let expected_ty = param_ty.substitute(&bindings);
                if !arg_ty.is_compatible_with(&expected_ty) {
                    self.errors.push(CheckError::type_mismatch(
                        expected_ty.to_string(),
                        arg_ty.to_string(),
                        arg.span(),
                    ));
                }
            }

            return func.return_type.substitute(&bindings);
        }

        // Check for built-in function
        if let Some(builtin) = self.symbols.get_builtin(name).cloned() {
            return self.check_builtin_call(&builtin, args, span);
        }

        self.errors.push(CheckError::undefined_function(name, span));
        Type::Error
    }

    /// Resolve type parameter bindings for a generic record construction.
    fn resolve_record_type_bindings(
        &mut self,
        type_params: &[String],
        type_args: &[sage_parser::TypeExpr],
        field_types: &HashMap<String, Type>,
        fields: &[sage_parser::FieldInit],
        span: &sage_parser::Span,
    ) -> HashMap<String, Type> {
        let mut bindings = HashMap::new();

        if !type_args.is_empty() {
            // Explicit type arguments via turbofish syntax
            if type_args.len() != type_params.len() {
                self.errors.push(CheckError::generic(
                    format!(
                        "expected {} type argument(s), found {}",
                        type_params.len(),
                        type_args.len()
                    ),
                    span,
                ));
                return bindings;
            }

            // Convert TypeExpr to Type and create bindings
            let empty_params = HashSet::new();
            for (param, arg) in type_params.iter().zip(type_args.iter()) {
                let ty = resolve_type_with_params(arg, &empty_params);
                bindings.insert(param.clone(), ty);
            }
        } else {
            // Infer type arguments from field values
            for field in fields {
                let field_name = &field.name.name;
                if let Some(expected_ty) = field_types.get(field_name) {
                    let actual_ty = self.check_expr(&field.value);
                    self.unify_types(expected_ty, &actual_ty, &mut bindings);
                }
            }

            // Check all type parameters were inferred
            for param in type_params {
                if !bindings.contains_key(param) {
                    self.errors.push(CheckError::generic(
                        format!("could not infer type for type parameter `{}`", param),
                        span,
                    ));
                }
            }
        }

        bindings
    }

    /// Resolve type parameter bindings for a generic enum variant construction.
    fn resolve_enum_type_bindings(
        &mut self,
        type_params: &[String],
        type_args: &[sage_parser::TypeExpr],
        variants: &[(String, Option<Type>)],
        variant_name: &str,
        payload: Option<&Expr>,
        span: &sage_parser::Span,
    ) -> HashMap<String, Type> {
        let mut bindings = HashMap::new();

        if !type_args.is_empty() {
            // Explicit type arguments via turbofish syntax
            if type_args.len() != type_params.len() {
                self.errors.push(CheckError::generic(
                    format!(
                        "expected {} type argument(s), found {}",
                        type_params.len(),
                        type_args.len()
                    ),
                    span,
                ));
                return bindings;
            }

            // Convert TypeExpr to Type and create bindings
            let empty_params = HashSet::new();
            for (param, arg) in type_params.iter().zip(type_args.iter()) {
                let ty = resolve_type_with_params(arg, &empty_params);
                bindings.insert(param.clone(), ty);
            }
        } else {
            // Infer type arguments from payload
            if let Some(payload_expr) = payload {
                // Find the variant's payload type
                if let Some((_, Some(expected_ty))) =
                    variants.iter().find(|(name, _)| name == variant_name)
                {
                    let actual_ty = self.check_expr(payload_expr);
                    self.unify_types(expected_ty, &actual_ty, &mut bindings);
                }
            }

            // For enum variants, all type params must be inferrable or explicitly provided
            // since generated Rust code needs concrete types
            for param in type_params {
                if !bindings.contains_key(param) {
                    self.errors.push(CheckError::generic(
                        format!(
                            "type parameter `{}` cannot be inferred; use turbofish syntax",
                            param
                        ),
                        span,
                    ));
                    // Insert Error to avoid cascading errors
                    bindings.insert(param.clone(), Type::Error);
                }
            }
        }

        bindings
    }

    /// Resolve type parameter bindings for a generic function call.
    /// Uses explicit type arguments (turbofish) if provided, otherwise infers from arguments.
    fn resolve_type_bindings(
        &mut self,
        type_params: &[String],
        type_args: &[sage_parser::TypeExpr],
        params: &[(String, Type)],
        args: &[Expr],
        span: &sage_parser::Span,
    ) -> HashMap<String, Type> {
        let mut bindings = HashMap::new();

        if !type_args.is_empty() {
            // Explicit type arguments via turbofish syntax
            if type_args.len() != type_params.len() {
                self.errors.push(CheckError::generic(
                    format!(
                        "expected {} type argument(s), found {}",
                        type_params.len(),
                        type_args.len()
                    ),
                    span,
                ));
                return bindings;
            }

            // Convert TypeExpr to Type and create bindings
            let empty_params = HashSet::new();
            for (param, arg) in type_params.iter().zip(type_args.iter()) {
                let ty = resolve_type_with_params(arg, &empty_params);
                bindings.insert(param.clone(), ty);
            }
        } else {
            // Infer type arguments from actual arguments
            for (arg, (_, param_ty)) in args.iter().zip(params.iter()) {
                let arg_ty = self.check_expr(arg);
                self.unify_types(param_ty, &arg_ty, &mut bindings);
            }

            // Check all type parameters were inferred
            for param in type_params {
                if !bindings.contains_key(param) {
                    self.errors.push(CheckError::generic(
                        format!("could not infer type for type parameter `{}`", param),
                        span,
                    ));
                }
            }
        }

        bindings
    }

    /// Attempt to unify a pattern type with a concrete type, extracting type parameter bindings.
    fn unify_types(&mut self, pattern: &Type, concrete: &Type, bindings: &mut HashMap<String, Type>) {
        match (pattern, concrete) {
            // Type parameter: bind to concrete type
            (Type::TypeParam(name), concrete) => {
                if let Some(existing) = bindings.get(name) {
                    // Already bound - check consistency
                    if !existing.is_compatible_with(concrete) {
                        // Conflicting types for same type parameter - handled elsewhere
                    }
                } else {
                    bindings.insert(name.clone(), concrete.clone());
                }
            }

            // Recurse into compound types
            (Type::List(p_elem), Type::List(c_elem)) => {
                self.unify_types(p_elem, c_elem, bindings);
            }
            (Type::Option(p_inner), Type::Option(c_inner)) => {
                self.unify_types(p_inner, c_inner, bindings);
            }
            (Type::Map(p_k, p_v), Type::Map(c_k, c_v)) => {
                self.unify_types(p_k, c_k, bindings);
                self.unify_types(p_v, c_v, bindings);
            }
            (Type::Tuple(p_elems), Type::Tuple(c_elems)) if p_elems.len() == c_elems.len() => {
                for (p, c) in p_elems.iter().zip(c_elems.iter()) {
                    self.unify_types(p, c, bindings);
                }
            }
            (Type::Result(p_ok, p_err), Type::Result(c_ok, c_err)) => {
                self.unify_types(p_ok, c_ok, bindings);
                self.unify_types(p_err, c_err, bindings);
            }
            (Type::Fn(p_params, p_ret), Type::Fn(c_params, c_ret))
                if p_params.len() == c_params.len() =>
            {
                for (p, c) in p_params.iter().zip(c_params.iter()) {
                    self.unify_types(p, c, bindings);
                }
                self.unify_types(p_ret, c_ret, bindings);
            }
            (Type::Generic(p_name, p_args), Type::Generic(c_name, c_args))
                if p_name == c_name && p_args.len() == c_args.len() =>
            {
                for (p, c) in p_args.iter().zip(c_args.iter()) {
                    self.unify_types(p, c, bindings);
                }
            }
            (Type::Oracle(p_inner), Type::Oracle(c_inner)) => {
                self.unify_types(p_inner, c_inner, bindings);
            }

            // Base types: no unification needed
            _ => {}
        }
    }

    fn check_builtin_call(
        &mut self,
        builtin: &crate::scope::BuiltinInfo,
        args: &[Expr],
        span: &sage_parser::Span,
    ) -> Type {
        // RFC-0012: E050 - assertion builtins only valid in test files
        if self.symbols.is_test_assertion(builtin.name) && !self.is_test_file {
            self.errors.push(CheckError::test_outside_test_file(span));
            return Type::Unit;
        }

        match builtin.name {
            "len" => {
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("len", 1, args.len(), span));
                    return Type::Error;
                }
                let arg_ty = self.check_expr(&args[0]);
                // len() accepts List<T> or Map<K, V>
                if arg_ty.list_element().is_none()
                    && arg_ty.map_key_value().is_none()
                    && !arg_ty.is_error()
                {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T> or Map<K, V>",
                        arg_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Int
            }

            "push" => {
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("push", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let elem_ty = self.check_expr(&args[1]);

                if let Some(expected_elem) = list_ty.list_element() {
                    if !elem_ty.is_compatible_with(expected_elem) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_elem.to_string(),
                            elem_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    list_ty.clone()
                } else {
                    if !list_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "List<T>",
                            list_ty.to_string(),
                            args[0].span(),
                        ));
                    }
                    Type::Error
                }
            }

            "str" => {
                // str() accepts any single value and returns String
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("str", 1, args.len(), span));
                    return Type::Error;
                }
                // Check the argument (any type is valid)
                self.check_expr(&args[0]);
                Type::String
            }

            "map_get" => {
                // map_get(Map<K, V>, K) -> Option<V>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map_get", 2, args.len(), span));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);
                let key_ty = self.check_expr(&args[1]);

                if let Some((expected_key, value_ty)) = map_ty.map_key_value() {
                    if !key_ty.is_compatible_with(expected_key) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_key.to_string(),
                            key_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    Type::Option(Box::new(value_ty.clone()))
                } else {
                    if !map_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Map<K, V>",
                            map_ty.to_string(),
                            args[0].span(),
                        ));
                    }
                    Type::Error
                }
            }

            "map_set" => {
                // map_set(Map<K, V>, K, V) -> Unit
                if args.len() != 3 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map_set", 3, args.len(), span));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);
                let key_ty = self.check_expr(&args[1]);
                let value_ty = self.check_expr(&args[2]);

                if let Some((expected_key, expected_value)) = map_ty.map_key_value() {
                    if !key_ty.is_compatible_with(expected_key) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_key.to_string(),
                            key_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    if !value_ty.is_compatible_with(expected_value) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_value.to_string(),
                            value_ty.to_string(),
                            args[2].span(),
                        ));
                    }
                } else if !map_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Map<K, V>",
                        map_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Unit
            }

            "map_delete" => {
                // map_delete(Map<K, V>, K) -> Unit
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "map_delete",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);
                let key_ty = self.check_expr(&args[1]);

                if let Some((expected_key, _)) = map_ty.map_key_value() {
                    if !key_ty.is_compatible_with(expected_key) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_key.to_string(),
                            key_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !map_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Map<K, V>",
                        map_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Unit
            }

            "map_has" => {
                // map_has(Map<K, V>, K) -> Bool
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map_has", 2, args.len(), span));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);
                let key_ty = self.check_expr(&args[1]);

                if let Some((expected_key, _)) = map_ty.map_key_value() {
                    if !key_ty.is_compatible_with(expected_key) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_key.to_string(),
                            key_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !map_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Map<K, V>",
                        map_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Bool
            }

            "map_keys" => {
                // map_keys(Map<K, V>) -> List<K>
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map_keys", 1, args.len(), span));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);

                if let Some((key_ty, _)) = map_ty.map_key_value() {
                    Type::List(Box::new(key_ty.clone()))
                } else {
                    if !map_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Map<K, V>",
                            map_ty.to_string(),
                            args[0].span(),
                        ));
                    }
                    Type::Error
                }
            }

            "map_values" => {
                // map_values(Map<K, V>) -> List<V>
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "map_values",
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);

                if let Some((_, value_ty)) = map_ty.map_key_value() {
                    Type::List(Box::new(value_ty.clone()))
                } else {
                    if !map_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Map<K, V>",
                            map_ty.to_string(),
                            args[0].span(),
                        ));
                    }
                    Type::Error
                }
            }

            // =========================================================================
            // RFC-0013: List Higher-Order Functions
            // =========================================================================
            "map" => {
                // map(List<A>, Fn(A) -> B) -> List<B>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1 && params[0].is_compatible_with(elem_ty) {
                            return Type::List(ret.clone());
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> B"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "filter" => {
                // filter(List<A>, Fn(A) -> Bool) -> List<A>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("filter", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1
                            && params[0].is_compatible_with(elem_ty)
                            && **ret == Type::Bool
                        {
                            return list_ty.clone();
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> Bool"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "reduce" => {
                // reduce(List<A>, B, Fn(B, A) -> B) -> B
                if args.len() != 3 {
                    self.errors
                        .push(CheckError::wrong_arg_count("reduce", 3, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let init_ty = self.check_expr(&args[1]);
                let fn_ty = self.check_expr(&args[2]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 2
                            && params[0].is_compatible_with(&init_ty)
                            && params[1].is_compatible_with(elem_ty)
                            && ret.is_compatible_with(&init_ty)
                        {
                            return init_ty;
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({init_ty}, {elem_ty}) -> {init_ty}"),
                            fn_ty.to_string(),
                            args[2].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "any" | "all" | "count_where" => {
                // any/all(List<A>, Fn(A) -> Bool) -> Bool
                // count_where(List<A>, Fn(A) -> Bool) -> Int
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1
                            && params[0].is_compatible_with(elem_ty)
                            && **ret == Type::Bool
                        {
                            return if builtin.name == "count_where" {
                                Type::Int
                            } else {
                                Type::Bool
                            };
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> Bool"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "find" => {
                // find(List<A>, Fn(A) -> Bool) -> Option<A>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("find", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1
                            && params[0].is_compatible_with(elem_ty)
                            && **ret == Type::Bool
                        {
                            return Type::Option(Box::new(elem_ty.clone()));
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> Bool"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "flat_map" => {
                // flat_map(List<A>, Fn(A) -> List<B>) -> List<B>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("flat_map", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1 && params[0].is_compatible_with(elem_ty) {
                            // Return type must be List<B>
                            if ret.list_element().is_some() {
                                return (**ret).clone();
                            }
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> List<B>"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "zip" => {
                // zip(List<A>, List<B>) -> List<(A, B)>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("zip", 2, args.len(), span));
                    return Type::Error;
                }
                let list1_ty = self.check_expr(&args[0]);
                let list2_ty = self.check_expr(&args[1]);

                if let (Some(elem1), Some(elem2)) =
                    (list1_ty.list_element(), list2_ty.list_element())
                {
                    return Type::List(Box::new(Type::Tuple(vec![elem1.clone(), elem2.clone()])));
                }
                if !list1_ty.is_error() && list1_ty.list_element().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list1_ty.to_string(),
                        args[0].span(),
                    ));
                }
                if !list2_ty.is_error() && list2_ty.list_element().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<B>",
                        list2_ty.to_string(),
                        args[1].span(),
                    ));
                }
                Type::Error
            }

            "sort_by" => {
                // sort_by(List<A>, Fn(A, A) -> Int) -> List<A>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("sort_by", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 2
                            && params[0].is_compatible_with(elem_ty)
                            && params[1].is_compatible_with(elem_ty)
                            && **ret == Type::Int
                        {
                            return list_ty.clone();
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}, {elem_ty}) -> Int"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "enumerate" => {
                // enumerate(List<A>) -> List<(Int, A)>
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "enumerate",
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if let Some(elem_ty) = list_ty.list_element() {
                    return Type::List(Box::new(Type::Tuple(vec![Type::Int, elem_ty.clone()])));
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "take" | "drop" => {
                // take/drop(List<A>, Int) -> List<A>
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let n_ty = self.check_expr(&args[1]);

                if n_ty != Type::Int && !n_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        n_ty.to_string(),
                        args[1].span(),
                    ));
                }
                if list_ty.list_element().is_some() {
                    return list_ty;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "flatten" => {
                // flatten(List<List<A>>) -> List<A>
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("flatten", 1, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if let Some(inner) = list_ty.list_element() {
                    if let Some(elem_ty) = inner.list_element() {
                        return Type::List(Box::new(elem_ty.clone()));
                    }
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<List<A>>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "reverse" | "unique" => {
                // reverse/unique(List<A>) -> List<A>
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if list_ty.list_element().is_some() {
                    return list_ty;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            // =========================================================================
            // RFC-0010: List Utilities
            // =========================================================================
            "first" | "last" | "pop" => {
                // first/last/pop(List<T>) -> Option<T>
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if let Some(elem_ty) = list_ty.list_element() {
                    return Type::Option(Box::new(elem_ty.clone()));
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "get" => {
                // get(List<T>, Int) -> Option<T>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("get", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let idx_ty = self.check_expr(&args[1]);

                if idx_ty != Type::Int && !idx_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        idx_ty.to_string(),
                        args[1].span(),
                    ));
                }
                if let Some(elem_ty) = list_ty.list_element() {
                    return Type::Option(Box::new(elem_ty.clone()));
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "list_contains" => {
                // list_contains(List<T>, T) -> Bool
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "list_contains",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let elem_ty = self.check_expr(&args[1]);

                if let Some(expected_elem) = list_ty.list_element() {
                    if !elem_ty.is_compatible_with(expected_elem) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_elem.to_string(),
                            elem_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    return Type::Bool;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "sort" => {
                // sort(List<T>) -> List<T>
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("sort", 1, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if list_ty.list_element().is_some() {
                    return list_ty;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "list_slice" => {
                // list_slice(List<T>, Int, Int) -> List<T>
                if args.len() != 3 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "list_slice",
                        3,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let start_ty = self.check_expr(&args[1]);
                let end_ty = self.check_expr(&args[2]);

                if start_ty != Type::Int && !start_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        start_ty.to_string(),
                        args[1].span(),
                    ));
                }
                if end_ty != Type::Int && !end_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        end_ty.to_string(),
                        args[2].span(),
                    ));
                }
                if list_ty.list_element().is_some() {
                    return list_ty;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "chunk" => {
                // chunk(List<T>, Int) -> List<List<T>>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("chunk", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let size_ty = self.check_expr(&args[1]);

                if size_ty != Type::Int && !size_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        size_ty.to_string(),
                        args[1].span(),
                    ));
                }
                if list_ty.list_element().is_some() {
                    return Type::List(Box::new(list_ty));
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "concat" => {
                // concat(List<T>, List<T>) -> List<T>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("concat", 2, args.len(), span));
                    return Type::Error;
                }
                let list1_ty = self.check_expr(&args[0]);
                let list2_ty = self.check_expr(&args[1]);

                if let (Some(elem1), Some(elem2)) =
                    (list1_ty.list_element(), list2_ty.list_element())
                {
                    if !elem1.is_compatible_with(elem2) {
                        self.errors.push(CheckError::type_mismatch(
                            list1_ty.to_string(),
                            list2_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    return list1_ty;
                }
                if !list1_ty.is_error() && list1_ty.list_element().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list1_ty.to_string(),
                        args[0].span(),
                    ));
                }
                if !list2_ty.is_error() && list2_ty.list_element().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list2_ty.to_string(),
                        args[1].span(),
                    ));
                }
                Type::Error
            }

            "take_while" | "drop_while" => {
                // take_while/drop_while(List<T>, Fn(T)->Bool) -> List<T>
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1
                            && params[0].is_compatible_with(elem_ty)
                            && **ret == Type::Bool
                        {
                            return list_ty;
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> Bool"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            // =========================================================================
            // RFC-0010: Option Utilities
            // =========================================================================
            "is_some" | "is_none" => {
                // is_some/is_none(Option<T>) -> Bool
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);

                if opt_ty.option_inner().is_some() || opt_ty.is_error() {
                    return Type::Bool;
                }
                self.errors.push(CheckError::type_mismatch(
                    "Option<T>",
                    opt_ty.to_string(),
                    args[0].span(),
                ));
                Type::Error
            }

            "unwrap" => {
                // unwrap(Option<T>) -> T (fallible)
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("unwrap", 1, args.len(), span));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);

                if let Some(inner_ty) = opt_ty.option_inner() {
                    return inner_ty.clone();
                }
                if !opt_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "unwrap_or" => {
                // unwrap_or(Option<T>, T) -> T
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "unwrap_or",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);
                let default_ty = self.check_expr(&args[1]);

                if let Some(inner_ty) = opt_ty.option_inner() {
                    if !default_ty.is_compatible_with(inner_ty) {
                        self.errors.push(CheckError::type_mismatch(
                            inner_ty.to_string(),
                            default_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    return inner_ty.clone();
                }
                if !opt_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "unwrap_or_else" => {
                // unwrap_or_else(Option<T>, Fn()->T) -> T
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "unwrap_or_else",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(inner_ty) = opt_ty.option_inner() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.is_empty() && ret.is_compatible_with(inner_ty) {
                            return inner_ty.clone();
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn() -> {inner_ty}"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !opt_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "map_option" => {
                // map_option(Option<T>, Fn(T)->U) -> Option<U>
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "map_option",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(inner_ty) = opt_ty.option_inner() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1 && params[0].is_compatible_with(inner_ty) {
                            return Type::Option(ret.clone());
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({inner_ty}) -> U"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !opt_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "or_option" => {
                // or_option(Option<T>, Option<T>) -> Option<T>
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "or_option",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt1_ty = self.check_expr(&args[0]);
                let opt2_ty = self.check_expr(&args[1]);

                if let (Some(inner1), Some(inner2)) =
                    (opt1_ty.option_inner(), opt2_ty.option_inner())
                {
                    if !inner1.is_compatible_with(inner2) {
                        self.errors.push(CheckError::type_mismatch(
                            opt1_ty.to_string(),
                            opt2_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    return opt1_ty;
                }
                if !opt1_ty.is_error() && opt1_ty.option_inner().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt1_ty.to_string(),
                        args[0].span(),
                    ));
                }
                if !opt2_ty.is_error() && opt2_ty.option_inner().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt2_ty.to_string(),
                        args[1].span(),
                    ));
                }
                Type::Error
            }

            _ => {
                // Standard built-in with fixed signature
                if let Some(ref params) = builtin.params {
                    if args.len() != params.len() {
                        self.errors.push(CheckError::wrong_arg_count(
                            builtin.name,
                            params.len(),
                            args.len(),
                            span,
                        ));
                        return Type::Error;
                    }

                    for (arg, param_ty) in args.iter().zip(params.iter()) {
                        let arg_ty = self.check_expr(arg);
                        if !arg_ty.is_compatible_with(param_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                param_ty.to_string(),
                                arg_ty.to_string(),
                                arg.span(),
                            ));
                        }
                    }
                }

                builtin.return_type.clone()
            }
        }
    }

    fn validate_entry_agent(&mut self, program: &Program) {
        // If there's no run statement, this is a library module - no entry validation needed
        let Some(run_entry) = &program.run_agent else {
            return;
        };

        let entry_name = &run_entry.name;

        // First check if it's an agent
        if let Some(agent) = self.symbols.get_agent(entry_name).cloned() {
            // Entry agent must have no beliefs
            if !agent.beliefs.is_empty() {
                self.errors.push(CheckError::entry_agent_has_beliefs(
                    entry_name,
                    &run_entry.span,
                ));
            }

            // Entry agent must have on start handler
            if !agent.has_start_handler {
                self.errors.push(CheckError::entry_agent_no_start(
                    entry_name,
                    &run_entry.span,
                ));
            }
            return;
        }

        // Check if it's a supervisor
        if self.symbols.get_supervisor(entry_name).is_some() {
            // Supervisors are valid entry points, no additional validation needed
            return;
        }

        // Neither agent nor supervisor found
        self.errors
            .push(CheckError::undefined_agent(entry_name, &run_entry.span));
    }

    // =========================================================================
    // Scope management
    // =========================================================================

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define_var(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.define(name, ty);
        }
    }

    fn lookup_var(&mut self, name: &str, span: &sage_parser::Span) -> Type {
        // Search from innermost to outermost scope
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return ty.clone();
            }
        }

        // Check if it's a constant
        if let Some(const_info) = self.symbols.get_const(name) {
            return const_info.ty.clone();
        }

        // Check for builtin constants (RFC-0013)
        if name == "PI" || name == "E" {
            return Type::Float;
        }

        // Time constants (RFC-0010)
        if matches!(
            name,
            "MS_PER_SECOND" | "MS_PER_MINUTE" | "MS_PER_HOUR" | "MS_PER_DAY"
        ) {
            return Type::Int;
        }

        self.errors.push(CheckError::undefined_variable(name, span));
        Type::Error
    }
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}

/// Check a program for semantic errors.
///
/// # Errors
///
/// Returns errors if the program contains semantic errors such as
/// undefined variables, type mismatches, or invalid operations.
#[must_use]
pub fn check(program: &Program) -> CheckResult {
    Checker::new().check(program)
}

/// Type-check a test file (`*_test.sg`).
///
/// RFC-0012: Test files may contain `test` blocks and assertion builtins.
///
/// # Errors
///
/// Returns errors if the program contains semantic errors such as
/// undefined variables, type mismatches, or invalid operations.
#[must_use]
pub fn check_test_file(program: &Program) -> CheckResult {
    Checker::for_test_file().check(program)
}

// =============================================================================
// Multi-module checking
// =============================================================================

use sage_loader::ModuleTree;

/// Result of checking a complete module tree.
pub struct ModuleCheckResult {
    /// The combined symbol table from all modules.
    pub symbols: SymbolTable,
    /// Any errors encountered during checking.
    pub errors: Vec<CheckError>,
}

/// Check an entire module tree for semantic errors.
///
/// This function:
/// 1. Collects all declarations from all modules (with visibility tracking)
/// 2. Resolves `use` declarations to find imported symbols
/// 3. Type checks all modules with proper cross-module resolution
///
/// # Errors
///
/// Returns errors if any module contains semantic errors such as
/// undefined variables, type mismatches, invalid imports, or visibility violations.
#[must_use]
pub fn check_module_tree(tree: &ModuleTree) -> ModuleCheckResult {
    let checker = MultiModuleChecker::new();
    checker.check(tree)
}

/// Checker for multi-module projects.
struct MultiModuleChecker {
    /// Combined symbol table from all modules.
    symbols: SymbolTable,
    /// Collected errors.
    errors: Vec<CheckError>,
    /// Imports resolved for each module: `module_path` -> (`local_name` -> (`defining_module`, `original_name`))
    imports: HashMap<ModulePath, HashMap<String, (ModulePath, String)>>,
}

impl MultiModuleChecker {
    fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            errors: Vec::new(),
            imports: HashMap::new(),
        }
    }

    fn check(mut self, tree: &ModuleTree) -> ModuleCheckResult {
        // Pass 1: Collect all declarations from all modules
        for (path, module) in &tree.modules {
            self.collect_module_declarations(path, &module.program);
        }

        // Pass 2: Resolve imports
        for (path, module) in &tree.modules {
            self.resolve_imports(path, &module.program, tree);
        }

        // Pass 3: Type check each module
        for (path, module) in &tree.modules {
            self.check_module(path, &module.program);
        }

        // Pass 4: Validate entry agent (only for the root module)
        if let Some(root_module) = tree.modules.get(&tree.root) {
            self.validate_entry_agent(&root_module.program);
        }

        ModuleCheckResult {
            symbols: self.symbols,
            errors: self.errors,
        }
    }

    fn collect_module_declarations(&mut self, module_path: &ModulePath, program: &Program) {
        // Collect agents
        for agent in &program.agents {
            let full_name = Self::make_qualified_name(module_path, &agent.name.name);

            if self.symbols.has_agent(&full_name) {
                self.errors
                    .push(CheckError::duplicate_definition(&full_name, &agent.span));
                continue;
            }

            let mut beliefs = HashMap::new();
            for belief in &agent.beliefs {
                let ty = resolve_type(&belief.ty);
                beliefs.insert(belief.name.name.clone(), ty);
            }

            let message_type = agent.handlers.iter().find_map(|h| {
                if let EventKind::Message { param_ty, .. } = &h.event {
                    Some(resolve_type(param_ty))
                } else {
                    None
                }
            });

            let has_start_handler = agent
                .handlers
                .iter()
                .any(|h| matches!(h.event, EventKind::Start));

            self.symbols.define_agent(AgentInfo {
                name: agent.name.name.clone(),
                beliefs,
                message_type,
                emit_type: None,
                has_start_handler,
                is_pub: agent.is_pub,
                module_path: module_path.clone(),
            });
        }

        // Collect functions
        for func in &program.functions {
            let full_name = Self::make_qualified_name(module_path, &func.name.name);

            if self.symbols.has_function(&full_name) {
                self.errors
                    .push(CheckError::duplicate_definition(&full_name, &func.span));
                continue;
            }

            // Build set of type parameters for this function
            let type_param_names: HashSet<String> =
                func.type_params.iter().map(|p| p.name.clone()).collect();

            let params: Vec<(String, Type)> = func
                .params
                .iter()
                .map(|p| {
                    (
                        p.name.name.clone(),
                        resolve_type_with_params(&p.ty, &type_param_names),
                    )
                })
                .collect();

            let return_type = resolve_type_with_params(&func.return_ty, &type_param_names);

            self.symbols.define_function(FunctionInfo {
                name: func.name.name.clone(),
                type_params: func.type_params.iter().map(|p| p.name.clone()).collect(),
                params,
                return_type,
                is_pub: func.is_pub,
                module_path: module_path.clone(),
                is_fallible: func.is_fallible,
            });
        }

        // Collect records
        for record in &program.records {
            let full_name = Self::make_qualified_name(module_path, &record.name.name);

            if self.symbols.has_record(&full_name) {
                self.errors
                    .push(CheckError::duplicate_definition(&full_name, &record.span));
                continue;
            }

            // Build set of type parameters for this record
            let type_param_names: HashSet<String> =
                record.type_params.iter().map(|p| p.name.clone()).collect();

            let mut fields = HashMap::new();
            let mut field_order = Vec::new();
            for field in &record.fields {
                let ty = resolve_type_with_params(&field.ty, &type_param_names);
                fields.insert(field.name.name.clone(), ty);
                field_order.push(field.name.name.clone());
            }

            self.symbols.define_record(RecordInfo {
                name: record.name.name.clone(),
                type_params: record.type_params.iter().map(|p| p.name.clone()).collect(),
                fields,
                field_order,
                is_pub: record.is_pub,
                module_path: module_path.clone(),
            });
        }

        // Collect enums
        for enum_decl in &program.enums {
            let full_name = Self::make_qualified_name(module_path, &enum_decl.name.name);

            if self.symbols.has_enum(&full_name) {
                self.errors.push(CheckError::duplicate_definition(
                    &full_name,
                    &enum_decl.span,
                ));
                continue;
            }

            // Build set of type parameters for this enum
            let type_param_names: HashSet<String> =
                enum_decl.type_params.iter().map(|p| p.name.clone()).collect();

            let variants: Vec<(String, Option<Type>)> = enum_decl
                .variants
                .iter()
                .map(|v| {
                    let payload = v
                        .payload
                        .as_ref()
                        .map(|p| resolve_type_with_params(p, &type_param_names));
                    (v.name.name.clone(), payload)
                })
                .collect();

            self.symbols.define_enum(EnumInfo {
                name: enum_decl.name.name.clone(),
                type_params: enum_decl.type_params.iter().map(|p| p.name.clone()).collect(),
                variants,
                is_pub: enum_decl.is_pub,
                module_path: module_path.clone(),
            });
        }

        // Collect consts
        for const_decl in &program.consts {
            let full_name = Self::make_qualified_name(module_path, &const_decl.name.name);

            if self.symbols.has_const(&full_name) {
                self.errors.push(CheckError::duplicate_definition(
                    &full_name,
                    &const_decl.span,
                ));
                continue;
            }

            let ty = resolve_type(&const_decl.ty);

            self.symbols.define_const(ConstInfo {
                name: const_decl.name.name.clone(),
                ty,
                is_pub: const_decl.is_pub,
                module_path: module_path.clone(),
            });
        }

        // Collect supervisors (v2)
        for supervisor in &program.supervisors {
            let full_name = Self::make_qualified_name(module_path, &supervisor.name.name);

            if self.symbols.has_supervisor(&full_name) {
                self.errors.push(CheckError::duplicate_definition(
                    &full_name,
                    &supervisor.span,
                ));
                continue;
            }

            let children: Vec<String> = supervisor
                .children
                .iter()
                .map(|c| c.agent_name.name.clone())
                .collect();

            self.symbols.define_supervisor(SupervisorInfo {
                name: supervisor.name.name.clone(),
                children,
                is_pub: supervisor.is_pub,
                module_path: module_path.clone(),
            });
        }
    }

    fn resolve_imports(&mut self, module_path: &ModulePath, program: &Program, tree: &ModuleTree) {
        let mut module_imports: HashMap<String, (ModulePath, String)> = HashMap::new();

        for use_decl in &program.use_decls {
            // Resolve the module path from the use declaration
            let target_path: ModulePath = use_decl.path.iter().map(|i| i.name.clone()).collect();

            match &use_decl.kind {
                sage_parser::UseKind::Simple(alias) => {
                    // `use foo::bar` or `use foo::bar as baz`
                    if let Some(name) = target_path.last() {
                        let local_name = alias
                            .as_ref()
                            .map_or_else(|| name.clone(), |a| a.name.clone());

                        // The target module is everything except the last segment
                        let (target_module, item_name) = if target_path.len() > 1 {
                            (
                                target_path[..target_path.len() - 1].to_vec(),
                                target_path.last().unwrap().clone(),
                            )
                        } else {
                            // Importing from a sibling module
                            (target_path.clone(), name.clone())
                        };

                        // Verify the import is valid
                        if self.verify_import(
                            &target_module,
                            &item_name,
                            use_decl.is_pub,
                            module_path,
                            &use_decl.span,
                        ) {
                            module_imports.insert(local_name, (target_module, item_name));
                        }
                    }
                }
                sage_parser::UseKind::Group(items) => {
                    // `use foo::{a, b as c}`
                    for (item, alias) in items {
                        let local_name = alias
                            .as_ref()
                            .map_or_else(|| item.name.clone(), |a| a.name.clone());

                        if self.verify_import(
                            &target_path,
                            &item.name,
                            use_decl.is_pub,
                            module_path,
                            &use_decl.span,
                        ) {
                            module_imports
                                .insert(local_name, (target_path.clone(), item.name.clone()));
                        }
                    }
                }
                sage_parser::UseKind::Glob => {
                    // `use foo::*` - import all public items from target module
                    if let Some(target_module) = tree.modules.get(&target_path) {
                        for agent in &target_module.program.agents {
                            if agent.is_pub {
                                module_imports.insert(
                                    agent.name.name.clone(),
                                    (target_path.clone(), agent.name.name.clone()),
                                );
                            }
                        }
                        for func in &target_module.program.functions {
                            if func.is_pub {
                                module_imports.insert(
                                    func.name.name.clone(),
                                    (target_path.clone(), func.name.name.clone()),
                                );
                            }
                        }
                        for record in &target_module.program.records {
                            if record.is_pub {
                                module_imports.insert(
                                    record.name.name.clone(),
                                    (target_path.clone(), record.name.name.clone()),
                                );
                            }
                        }
                        for enum_decl in &target_module.program.enums {
                            if enum_decl.is_pub {
                                module_imports.insert(
                                    enum_decl.name.name.clone(),
                                    (target_path.clone(), enum_decl.name.name.clone()),
                                );
                            }
                        }
                        for const_decl in &target_module.program.consts {
                            if const_decl.is_pub {
                                module_imports.insert(
                                    const_decl.name.name.clone(),
                                    (target_path.clone(), const_decl.name.name.clone()),
                                );
                            }
                        }
                    } else {
                        self.errors.push(CheckError::module_not_found(
                            target_path.join("::"),
                            &use_decl.span,
                        ));
                    }
                }
            }
        }

        self.imports.insert(module_path.clone(), module_imports);
    }

    fn verify_import(
        &mut self,
        target_module: &ModulePath,
        item_name: &str,
        _is_pub_use: bool,
        from_module: &ModulePath,
        span: &sage_parser::Span,
    ) -> bool {
        // Check if the item exists and is accessible

        // Check agents
        for (_, agent_info) in self.symbols.iter_agents() {
            if &agent_info.module_path == target_module && agent_info.name == item_name {
                if !agent_info.is_pub && &agent_info.module_path != from_module {
                    self.errors.push(CheckError::private_item(
                        item_name,
                        target_module.join("::"),
                        span,
                    ));
                    return false;
                }
                return true;
            }
        }

        // Check functions
        for (_, func_info) in self.symbols.iter_functions() {
            if &func_info.module_path == target_module && func_info.name == item_name {
                if !func_info.is_pub && &func_info.module_path != from_module {
                    self.errors.push(CheckError::private_item(
                        item_name,
                        target_module.join("::"),
                        span,
                    ));
                    return false;
                }
                return true;
            }
        }

        // Check records
        for (_, record_info) in self.symbols.iter_records() {
            if &record_info.module_path == target_module && record_info.name == item_name {
                if !record_info.is_pub && &record_info.module_path != from_module {
                    self.errors.push(CheckError::private_item(
                        item_name,
                        target_module.join("::"),
                        span,
                    ));
                    return false;
                }
                return true;
            }
        }

        // Check enums
        for (_, enum_info) in self.symbols.iter_enums() {
            if &enum_info.module_path == target_module && enum_info.name == item_name {
                if !enum_info.is_pub && &enum_info.module_path != from_module {
                    self.errors.push(CheckError::private_item(
                        item_name,
                        target_module.join("::"),
                        span,
                    ));
                    return false;
                }
                return true;
            }
        }

        // Check consts
        for (_, const_info) in self.symbols.iter_consts() {
            if &const_info.module_path == target_module && const_info.name == item_name {
                if !const_info.is_pub && &const_info.module_path != from_module {
                    self.errors.push(CheckError::private_item(
                        item_name,
                        target_module.join("::"),
                        span,
                    ));
                    return false;
                }
                return true;
            }
        }

        self.errors.push(CheckError::item_not_found(
            item_name,
            target_module.join("::"),
            span,
        ));
        false
    }

    fn check_module(&mut self, module_path: &ModulePath, program: &Program) {
        let module_imports = self.imports.get(module_path).cloned().unwrap_or_default();

        let (errors, inferred_emit_types) = {
            let mut module_checker =
                ModuleChecker::new(&self.symbols, module_path.clone(), module_imports);

            module_checker.check_program(program);

            (module_checker.errors, module_checker.inferred_emit_types)
        };

        // Now update the symbols with inferred emit types (no longer borrowing)
        for (agent_name, emit_type) in inferred_emit_types {
            if let Some(agent) = self.symbols.get_agent_mut(&agent_name) {
                agent.emit_type = Some(emit_type);
            }
        }

        self.errors.extend(errors);
    }

    fn validate_entry_agent(&mut self, program: &Program) {
        let Some(run_entry) = &program.run_agent else {
            return;
        };

        let entry_name = &run_entry.name;

        // Look up the agent (it should be in the root module)
        let agent = self
            .symbols
            .iter_agents()
            .find(|(_, info)| info.module_path.is_empty() && info.name == *entry_name);

        if let Some((_, agent)) = agent {
            let agent = agent.clone();

            if !agent.beliefs.is_empty() {
                self.errors.push(CheckError::entry_agent_has_beliefs(
                    entry_name,
                    &run_entry.span,
                ));
            }

            if !agent.has_start_handler {
                self.errors.push(CheckError::entry_agent_no_start(
                    entry_name,
                    &run_entry.span,
                ));
            }
            return;
        }

        // Check if it's a supervisor in the root module
        let supervisor = self
            .symbols
            .iter_supervisors()
            .find(|(_, info)| info.module_path.is_empty() && info.name == *entry_name);

        if supervisor.is_some() {
            // Supervisors are valid entry points
            return;
        }

        // Neither agent nor supervisor found
        self.errors
            .push(CheckError::undefined_agent(entry_name, &run_entry.span));
    }

    fn make_qualified_name(module_path: &ModulePath, name: &str) -> String {
        if module_path.is_empty() {
            name.to_string()
        } else {
            format!("{}::{}", module_path.join("::"), name)
        }
    }
}

/// Per-module checker that uses a shared symbol table.
struct ModuleChecker<'a> {
    symbols: &'a SymbolTable,
    module_path: ModulePath,
    imports: HashMap<String, (ModulePath, String)>,
    scopes: Vec<Scope>,
    errors: Vec<CheckError>,
    current_agent: Option<String>,
    in_function: bool,
    expected_return: Option<Type>,
    used_beliefs: HashSet<String>,
    /// Emit types inferred during checking.
    inferred_emit_types: HashMap<String, Type>,
    /// Whether we're inside a loop (for break validation).
    in_loop: bool,
    /// The receives type of the current agent (for receive validation).
    receives_type: Option<Type>,
    /// RFC-0007: Whether we're in a fallible context (function/handler marked fails).
    in_fallible_context: bool,
    /// RFC-0007: Whether the current agent has an error handler.
    agent_has_error_handler: bool,
    /// RFC-0007: Whether we're inside a try or catch expression (for E013 enforcement).
    in_error_handling: bool,
    /// Whether we're inside an on_stop handler (emit is prohibited).
    in_stop_handler: bool,
    /// RFC-0012: Whether this is a test file (_test.sg).
    is_test_file: bool,
    /// RFC-0012: Whether we're inside a test block.
    in_test_block: bool,
}

impl<'a> ModuleChecker<'a> {
    fn new(
        symbols: &'a SymbolTable,
        module_path: ModulePath,
        imports: HashMap<String, (ModulePath, String)>,
    ) -> Self {
        Self {
            symbols,
            module_path,
            imports,
            scopes: vec![Scope::new()],
            errors: Vec::new(),
            current_agent: None,
            in_function: false,
            expected_return: None,
            used_beliefs: HashSet::new(),
            inferred_emit_types: HashMap::new(),
            in_loop: false,
            receives_type: None,
            in_fallible_context: false,
            agent_has_error_handler: false,
            in_error_handling: false,
            in_stop_handler: false,
            is_test_file: false,
            in_test_block: false,
        }
    }

    fn check_program(&mut self, program: &Program) {
        for agent in &program.agents {
            self.check_agent(agent);
        }

        for func in &program.functions {
            self.check_function(func);
        }
    }

    fn check_agent(&mut self, agent: &AgentDecl) {
        self.current_agent = Some(agent.name.name.clone());
        self.used_beliefs.clear();

        // Set receives type from the agent's receives clause
        self.receives_type = agent.receives.as_ref().map(resolve_type);

        // RFC-0007: Check if agent has an error handler
        self.agent_has_error_handler = agent
            .handlers
            .iter()
            .any(|h| matches!(h.event, EventKind::Error { .. }));

        for handler in &agent.handlers {
            self.push_scope();

            if let EventKind::Message {
                param_name,
                param_ty,
            } = &handler.event
            {
                let ty = resolve_type(param_ty);
                self.define_var(&param_name.name, ty);
            }

            // RFC-0007: Add error parameter to scope if this is an error handler
            if let EventKind::Error { param_name } = &handler.event {
                self.define_var(&param_name.name, Type::Named("Error".to_string()));
            }

            // Track if we're in a stop/resting handler (yield is prohibited)
            let old_in_stop_handler = self.in_stop_handler;
            if matches!(handler.event, EventKind::Stop | EventKind::Resting) {
                self.in_stop_handler = true;
            }

            self.check_block(&handler.body);

            self.in_stop_handler = old_in_stop_handler;
            self.pop_scope();
        }

        // Check for unused beliefs
        for belief in &agent.beliefs {
            if !self.used_beliefs.contains(&belief.name.name) {
                self.errors
                    .push(CheckError::unused_belief(&belief.name.name, &belief.span));
            }
        }

        // Check @persistent fields have serializable types (E052)
        for belief in &agent.beliefs {
            if belief.is_persistent {
                let ty = resolve_type(&belief.ty);
                if !ty.is_serializable() {
                    self.errors
                        .push(CheckError::persistent_field_not_serializable(
                            &belief.name.name,
                            ty.to_string(),
                            &belief.span,
                        ));
                }
            }
        }

        // Check for `on waking` without persistent fields (W006)
        let has_persistent = agent.beliefs.iter().any(|b| b.is_persistent);
        let has_waking = agent
            .handlers
            .iter()
            .any(|h| matches!(h.event, EventKind::Waking));
        if has_waking && !has_persistent {
            // Find the waking handler's span for the warning
            if let Some(waking_handler) = agent
                .handlers
                .iter()
                .find(|h| matches!(h.event, EventKind::Waking))
            {
                self.errors
                    .push(CheckError::waking_without_persistent_fields(
                        &agent.name.name,
                        &waking_handler.body.span,
                    ));
            }
        }

        self.current_agent = None;
        self.receives_type = None;
        self.agent_has_error_handler = false;
    }

    fn check_function(&mut self, func: &FnDecl) {
        self.in_function = true;
        self.expected_return = Some(resolve_type(&func.return_ty));

        // RFC-0007: Track if we're in a fallible function
        self.in_fallible_context = func.is_fallible;

        self.push_scope();

        for param in &func.params {
            let ty = resolve_type(&param.ty);
            self.define_var(&param.name.name, ty);
        }

        self.check_block(&func.body);

        self.pop_scope();
        self.in_function = false;
        self.expected_return = None;
        self.in_fallible_context = false;
    }

    fn check_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let value_ty = self.check_expr(value);
                let declared_ty = ty.as_ref().map(resolve_type);

                if let Some(ref decl) = declared_ty {
                    if !value_ty.is_compatible_with(decl) {
                        self.errors.push(CheckError::type_mismatch(
                            decl.to_string(),
                            value_ty.to_string(),
                            value.span(),
                        ));
                    }
                }

                let final_ty = declared_ty.unwrap_or(value_ty);
                self.define_var(&name.name, final_ty);
            }

            Stmt::Assign { name, value, span } => {
                let expected = self.lookup_var(&name.name, &name.span);
                let actual = self.check_expr(value);

                if !actual.is_compatible_with(&expected) {
                    self.errors.push(CheckError::type_mismatch(
                        expected.to_string(),
                        actual.to_string(),
                        span,
                    ));
                }
            }

            Stmt::Return { value, span } => {
                if !self.in_function {
                    self.errors.push(CheckError::return_outside_function(span));
                    return;
                }

                let return_ty = match value {
                    Some(e) => self.check_expr(e),
                    None => Type::Unit,
                };

                if let Some(ref expected) = self.expected_return {
                    if !return_ty.is_compatible_with(expected) {
                        self.errors.push(CheckError::type_mismatch(
                            expected.to_string(),
                            return_ty.to_string(),
                            span,
                        ));
                    }
                }
            }

            Stmt::If {
                condition,
                then_block,
                else_block,
                span,
            } => {
                let cond_ty = self.check_expr(condition);
                if !cond_ty.is_compatible_with(&Type::Bool) {
                    self.errors
                        .push(CheckError::non_bool_condition(cond_ty.to_string(), span));
                }

                self.push_scope();
                self.check_block(then_block);
                self.pop_scope();

                if let Some(else_branch) = else_block {
                    match else_branch {
                        sage_parser::ElseBranch::Block(block) => {
                            self.push_scope();
                            self.check_block(block);
                            self.pop_scope();
                        }
                        sage_parser::ElseBranch::ElseIf(stmt) => {
                            self.check_stmt(stmt);
                        }
                    }
                }
            }

            Stmt::For {
                pattern,
                iter,
                body,
                span,
            } => {
                let iter_ty = self.check_expr(iter);

                // Determine the element type based on the iterable type
                let elem_ty = if let Some(elem) = iter_ty.list_element() {
                    elem.clone()
                } else if let Some((key_ty, value_ty)) = iter_ty.map_key_value() {
                    // Map iteration yields (K, V) tuples
                    Type::Tuple(vec![key_ty.clone(), value_ty.clone()])
                } else {
                    if !iter_ty.is_error() {
                        self.errors
                            .push(CheckError::not_iterable(iter_ty.to_string(), span));
                    }
                    Type::Error
                };

                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.push_scope();
                self.check_pattern(pattern, &elem_ty);
                self.check_block(body);
                self.pop_scope();
                self.in_loop = was_in_loop;
            }

            Stmt::While {
                condition,
                body,
                span,
            } => {
                let cond_ty = self.check_expr(condition);
                if !cond_ty.is_compatible_with(&Type::Bool) {
                    self.errors
                        .push(CheckError::non_bool_condition(cond_ty.to_string(), span));
                }

                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.push_scope();
                self.check_block(body);
                self.pop_scope();
                self.in_loop = was_in_loop;
            }

            Stmt::Loop { body, .. } => {
                let was_in_loop = self.in_loop;
                self.in_loop = true;
                self.push_scope();
                self.check_block(body);
                self.pop_scope();
                self.in_loop = was_in_loop;
            }

            Stmt::Break { span } => {
                if !self.in_loop {
                    self.errors.push(CheckError::break_outside_loop(span));
                }
            }

            Stmt::Expr { expr, .. } => {
                self.check_expr(expr);
            }

            Stmt::LetTuple {
                names,
                ty,
                value,
                span,
            } => {
                let value_ty = self.check_expr(value);

                // Value must be a tuple type
                match &value_ty {
                    Type::Tuple(elems) => {
                        if names.len() != elems.len() {
                            self.errors.push(CheckError::tuple_arity_mismatch(
                                names.len(),
                                elems.len(),
                                span,
                            ));
                        } else {
                            // Bind each name to its corresponding element type
                            for (name, elem_ty) in names.iter().zip(elems.iter()) {
                                self.define_var(&name.name, elem_ty.clone());
                            }
                        }
                    }
                    Type::Error => {
                        // Don't cascade errors; bind all to Error
                        for name in names {
                            self.define_var(&name.name, Type::Error);
                        }
                    }
                    _ => {
                        self.errors.push(CheckError::type_mismatch(
                            format!("tuple with {} elements", names.len()),
                            value_ty.to_string(),
                            span,
                        ));
                        // Bind all names to Error to avoid cascading
                        for name in names {
                            self.define_var(&name.name, Type::Error);
                        }
                    }
                }

                // If there's an explicit type annotation, check it matches
                if let Some(type_expr) = ty {
                    let declared_ty = resolve_type(type_expr);
                    if !value_ty.is_compatible_with(&declared_ty) {
                        self.errors.push(CheckError::type_mismatch(
                            declared_ty.to_string(),
                            value_ty.to_string(),
                            span,
                        ));
                    }
                }
            }

            // RFC-0012: mock infer statement - type check the mock value
            Stmt::MockDivine { value, span } => {
                // E056: mock infer outside test block
                if !self.in_test_block {
                    self.errors.push(CheckError::mock_divine_outside_test(span));
                }

                match value {
                    sage_parser::MockValue::Value(expr) => {
                        self.check_expr(expr);
                    }
                    sage_parser::MockValue::Fail(expr) => {
                        let ty = self.check_expr(expr);
                        if !ty.is_compatible_with(&Type::String) {
                            self.errors.push(CheckError::mock_fail_not_string(
                                ty.to_string(),
                                expr.span(),
                            ));
                        }
                    }
                }
            }

            // mock tool statement - type check the mock value
            Stmt::MockTool { value, span, .. } => {
                // E057: mock tool outside test block
                if !self.in_test_block {
                    self.errors.push(CheckError::mock_tool_outside_test(span));
                }

                match value {
                    sage_parser::MockValue::Value(expr) => {
                        self.check_expr(expr);
                    }
                    sage_parser::MockValue::Fail(expr) => {
                        let ty = self.check_expr(expr);
                        if !ty.is_compatible_with(&Type::String) {
                            self.errors.push(CheckError::mock_fail_not_string(
                                ty.to_string(),
                                expr.span(),
                            ));
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn check_expr(&mut self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal { value, .. } => match value {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Float,
                Literal::Bool(_) => Type::Bool,
                Literal::String(_) => Type::String,
            },

            Expr::Var { name, .. } => self.lookup_var(&name.name, &name.span),

            Expr::List { elements, .. } => {
                if elements.is_empty() {
                    Type::List(Box::new(Type::Error))
                } else {
                    let first_ty = self.check_expr(&elements[0]);
                    for elem in &elements[1..] {
                        let elem_ty = self.check_expr(elem);
                        if !elem_ty.is_compatible_with(&first_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                first_ty.to_string(),
                                elem_ty.to_string(),
                                elem.span(),
                            ));
                        }
                    }
                    Type::List(Box::new(first_ty))
                }
            }

            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                let left_ty = self.check_expr(left);
                let right_ty = self.check_expr(right);
                self.check_binary_op(*op, &left_ty, &right_ty, span)
            }

            Expr::Unary { op, operand, span } => {
                let operand_ty = self.check_expr(operand);
                self.check_unary_op(*op, &operand_ty, span)
            }

            Expr::Call {
                name,
                type_args,
                args,
                span,
            } => self.check_call(&name.name, type_args, args, span),

            Expr::SelfField { field, span } => {
                let Some(agent_name) = &self.current_agent else {
                    self.errors.push(CheckError::self_outside_agent(span));
                    return Type::Error;
                };

                // Clone the belief type to avoid holding borrow across mutation
                let belief_type = self
                    .lookup_agent(agent_name)
                    .and_then(|agent| agent.beliefs.get(&field.name).cloned());

                if let Some(ty) = belief_type {
                    self.used_beliefs.insert(field.name.clone());
                    ty
                } else {
                    // Check if agent exists at all
                    if self.lookup_agent(agent_name).is_some() {
                        self.errors
                            .push(CheckError::undefined_belief(&field.name, span));
                    }
                    Type::Error
                }
            }

            Expr::SelfMethodCall { method, span, .. } => {
                self.errors
                    .push(CheckError::undefined_function(&method.name, span));
                Type::Error
            }

            Expr::Divine {
                template,
                result_ty,
                ..
            } => {
                // RFC-0013: Check interpolation expressions
                for part in &template.parts {
                    if let sage_parser::StringPart::Interpolation(expr) = part {
                        self.check_expr(expr);
                    }
                }
                let inner = result_ty.as_ref().map_or(Type::String, resolve_type);
                Type::Oracle(Box::new(inner))
            }

            Expr::Summon {
                agent,
                fields,
                span,
            } => {
                let agent_info = self.lookup_agent(&agent.name);
                let Some(agent_info) = agent_info else {
                    self.errors
                        .push(CheckError::undefined_agent(&agent.name, span));
                    return Type::Error;
                };
                let agent_info = agent_info.clone();

                let mut provided: HashMap<String, bool> = agent_info
                    .beliefs
                    .keys()
                    .map(|k| (k.clone(), false))
                    .collect();

                for field in fields {
                    let field_name = &field.name.name;

                    if let Some(expected_ty) = agent_info.beliefs.get(field_name) {
                        provided.insert(field_name.clone(), true);
                        let actual_ty = self.check_expr(&field.value);

                        if !actual_ty.is_compatible_with(expected_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                expected_ty.to_string(),
                                actual_ty.to_string(),
                                field.value.span(),
                            ));
                        }
                    } else {
                        self.errors
                            .push(CheckError::unknown_field(field_name, &field.span));
                    }
                }

                for (name, was_provided) in &provided {
                    if !was_provided {
                        self.errors
                            .push(CheckError::missing_belief_init(name, span));
                    }
                }

                Type::Agent(agent.name.clone())
            }

            Expr::Await {
                handle,
                timeout,
                span,
            } => {
                let handle_ty = self.check_expr(handle);

                // RFC-0007: E013 - await is a fallible operation, must be wrapped in try or catch
                if !self.in_error_handling {
                    self.errors.push(CheckError::unhandled_error("await", span));
                }

                // Check timeout is an Int if provided
                if let Some(timeout_expr) = timeout {
                    let timeout_ty = self.check_expr(timeout_expr);
                    if !matches!(timeout_ty, Type::Int) && !timeout_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Int".to_string(),
                            timeout_ty.to_string(),
                            timeout_expr.span(),
                        ));
                    }
                }

                if let Some(agent_name) = handle_ty.agent_name() {
                    self.lookup_agent(agent_name)
                        .and_then(|a| a.emit_type.clone())
                        .unwrap_or(Type::String)
                } else {
                    if !handle_ty.is_error() {
                        self.errors
                            .push(CheckError::await_non_agent(handle_ty.to_string(), span));
                    }
                    Type::Error
                }
            }

            Expr::Send {
                handle,
                message,
                span,
            } => {
                // RFC-0007: E013 - send is a fallible operation, must be wrapped in try or catch
                if !self.in_error_handling {
                    self.errors.push(CheckError::unhandled_error("send", span));
                }

                let handle_ty = self.check_expr(handle);
                let msg_ty = self.check_expr(message);

                if let Some(agent_name) = handle_ty.agent_name() {
                    if let Some(agent_info) = self.lookup_agent(agent_name) {
                        if let Some(expected) = &agent_info.message_type {
                            if !msg_ty.is_compatible_with(expected) {
                                self.errors.push(CheckError::type_mismatch(
                                    expected.to_string(),
                                    msg_ty.to_string(),
                                    message.span(),
                                ));
                            }
                        } else {
                            self.errors
                                .push(CheckError::no_message_handler(agent_name, span));
                        }
                    }
                } else if !handle_ty.is_error() {
                    self.errors
                        .push(CheckError::send_non_agent(handle_ty.to_string(), span));
                }

                Type::Unit
            }

            Expr::Yield { value, span } => {
                // Check for emit in stop handler
                if self.in_stop_handler {
                    self.errors.push(CheckError::yield_in_stop_handler(span));
                }

                let value_ty = self.check_expr(value);

                if let Some(agent_name) = &self.current_agent {
                    self.inferred_emit_types
                        .insert(agent_name.clone(), value_ty.clone());
                }

                Type::Unit
            }

            Expr::Paren { inner, .. } => self.check_expr(inner),

            Expr::StringInterp { template, .. } => {
                // RFC-0013: Check all interpolated expressions
                for part in &template.parts {
                    if let sage_parser::StringPart::Interpolation(expr) = part {
                        self.check_expr(expr);
                    }
                }
                Type::String
            }

            Expr::Match {
                scrutinee,
                arms,
                span,
            } => {
                let scrutinee_ty = self.check_expr(scrutinee);

                // Track covered patterns for exhaustiveness
                let mut has_wildcard = false;
                let mut covered_variants: HashSet<String> = HashSet::new();
                let mut covered_bool_true = false;
                let mut covered_bool_false = false;

                let mut result_ty = Type::Error;
                for arm in arms {
                    // Check pattern and get any bindings
                    self.push_scope();
                    self.check_pattern(&arm.pattern, &scrutinee_ty);

                    // Track coverage for exhaustiveness
                    match &arm.pattern {
                        Pattern::Wildcard { .. } | Pattern::Binding { .. } => {
                            has_wildcard = true;
                        }
                        Pattern::Variant { variant, .. } => {
                            covered_variants.insert(variant.name.clone());
                        }
                        Pattern::Literal {
                            value: Literal::Bool(b),
                            ..
                        } => {
                            if *b {
                                covered_bool_true = true;
                            } else {
                                covered_bool_false = true;
                            }
                        }
                        Pattern::Literal { .. } => {
                            // Literal patterns don't guarantee coverage
                        }
                        Pattern::Tuple { .. } => {
                            // Tuple patterns don't guarantee exhaustive coverage
                        }
                    }

                    // Check body expression
                    let arm_ty = self.check_expr(&arm.body);
                    self.pop_scope();

                    if result_ty.is_error() {
                        result_ty = arm_ty;
                    }
                }

                // Check exhaustiveness
                if !has_wildcard {
                    let is_exhaustive = match &scrutinee_ty {
                        Type::Named(name) | Type::Generic(name, _) => {
                            // Check if it's an enum and all variants are covered
                            if let Some(enum_info) = self.lookup_enum(name) {
                                enum_info
                                    .variants
                                    .iter()
                                    .all(|(v, _)| covered_variants.contains(v))
                            } else {
                                // Not an enum - needs wildcard
                                false
                            }
                        }
                        // Option<T> requires Some and None
                        Type::Option(_) => {
                            covered_variants.contains("Some") && covered_variants.contains("None")
                        }
                        // Result<T, E> requires Ok and Err
                        Type::Result(_, _) => {
                            covered_variants.contains("Ok") && covered_variants.contains("Err")
                        }
                        Type::Bool => covered_bool_true && covered_bool_false,
                        Type::Error => true, // Don't report exhaustiveness errors on error types
                        _ => false,          // Other types need a wildcard to be exhaustive
                    };

                    if !is_exhaustive {
                        self.errors.push(CheckError::non_exhaustive_match(span));
                    }
                }

                result_ty
            }

            Expr::RecordConstruct {
                name,
                type_args,
                fields,
                span,
            } => {
                let record_info = self.lookup_record(&name.name);
                let Some(record_info) = record_info else {
                    self.errors
                        .push(CheckError::undefined_type(&name.name, span));
                    return Type::Error;
                };
                let record_info = record_info.clone();

                // RFC-0015: Handle generic records
                let bindings = if !record_info.type_params.is_empty() {
                    self.resolve_record_type_bindings(
                        &record_info.type_params,
                        type_args,
                        &record_info.fields,
                        fields,
                        span,
                    )
                } else {
                    // Non-generic record: warn if type args provided
                    if !type_args.is_empty() {
                        self.errors.push(CheckError::generic(
                            format!(
                                "record `{}` is not generic but was constructed with type arguments",
                                name.name
                            ),
                            span,
                        ));
                    }
                    HashMap::new()
                };

                // Track which fields have been provided
                let mut provided: HashMap<String, bool> = record_info
                    .fields
                    .keys()
                    .map(|k| (k.clone(), false))
                    .collect();

                for field in fields {
                    let field_name = &field.name.name;

                    if let Some(expected_ty) = record_info.fields.get(field_name) {
                        provided.insert(field_name.clone(), true);
                        let actual_ty = self.check_expr(&field.value);
                        // Substitute type params in expected type
                        let expected_ty = expected_ty.substitute(&bindings);

                        if !actual_ty.is_compatible_with(&expected_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                expected_ty.to_string(),
                                actual_ty.to_string(),
                                field.value.span(),
                            ));
                        }
                    } else {
                        self.errors
                            .push(CheckError::unknown_field(field_name, &field.span));
                    }
                }

                // Check for missing fields
                for (field_name, was_provided) in &provided {
                    if !was_provided {
                        self.errors
                            .push(CheckError::missing_field(field_name, &name.name, span));
                    }
                }

                // Return Generic type if record has type params, otherwise Named
                if record_info.type_params.is_empty() {
                    Type::Named(name.name.clone())
                } else {
                    let type_args: Vec<Type> = record_info
                        .type_params
                        .iter()
                        .map(|p| bindings.get(p).cloned().unwrap_or(Type::Error))
                        .collect();
                    Type::Generic(name.name.clone(), type_args)
                }
            }

            Expr::FieldAccess {
                object,
                field,
                span,
            } => {
                let obj_ty = self.check_expr(object);

                // Get the record name from the type
                let record_name = match &obj_ty {
                    Type::Named(name) => name.clone(),
                    Type::Error => return Type::Error,
                    _ => {
                        self.errors.push(CheckError::field_access_on_non_record(
                            obj_ty.to_string(),
                            span,
                        ));
                        return Type::Error;
                    }
                };

                // Look up the record and get field type
                if let Some(record_info) = self.lookup_record(&record_name) {
                    if let Some(field_ty) = record_info.fields.get(&field.name) {
                        field_ty.clone()
                    } else {
                        self.errors
                            .push(CheckError::unknown_field(&field.name, span));
                        Type::Error
                    }
                } else {
                    // It's a Named type but not a record - could be enum
                    self.errors.push(CheckError::field_access_on_non_record(
                        obj_ty.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            Expr::Receive { span } => {
                // Must be inside an agent
                if self.current_agent.is_none() {
                    self.errors.push(CheckError::receive_outside_agent(span));
                    return Type::Error;
                }

                // Must have a receives type
                match &self.receives_type {
                    Some(ty) => ty.clone(),
                    None => {
                        self.errors.push(CheckError::receive_without_receives(
                            self.current_agent.as_ref().unwrap(),
                            span,
                        ));
                        Type::Error
                    }
                }
            }

            // RFC-0007: Error handling
            Expr::Try { expr, span } => {
                // Check that we're in a fallible context (can propagate errors)
                if !self.in_fallible_context {
                    // In an agent, check for error handler
                    if self.current_agent.is_some() && !self.agent_has_error_handler {
                        self.errors.push(CheckError::missing_error_handler(
                            self.current_agent.as_ref().unwrap().clone(),
                            span,
                        ));
                    } else if self.current_agent.is_none() {
                        self.errors.push(CheckError::try_in_non_fallible(span));
                    }
                }

                // Set error handling context for inner expression (suppresses E013)
                let old_in_error_handling = self.in_error_handling;
                self.in_error_handling = true;

                // Check the inner expression
                let inner_ty = self.check_expr(expr);

                // Restore error handling context
                self.in_error_handling = old_in_error_handling;

                // Return the inner type (unwrapped from potential error)
                inner_ty
            }

            Expr::Catch {
                expr,
                error_bind,
                recovery,
                span,
            } => {
                // Set error handling context for inner expression (suppresses E013)
                let old_in_error_handling = self.in_error_handling;
                self.in_error_handling = true;

                // Check the inner (fallible) expression
                let expr_ty = self.check_expr(expr);

                // Restore error handling context
                self.in_error_handling = old_in_error_handling;

                // Create a new scope for the recovery block (for error binding)
                self.push_scope();

                // If there's an error binding, add it to scope
                if let Some(err_ident) = error_bind {
                    // Error type has .message (String) and .kind (ErrorKind)
                    // For now, use a simple Named type
                    self.define_var(&err_ident.name, Type::Named("Error".to_string()));
                }

                // Check the recovery expression
                let recovery_ty = self.check_expr(recovery);

                self.pop_scope();

                // Recovery type must match the expression type
                if !recovery_ty.is_compatible_with(&expr_ty) && !expr_ty.is_error() {
                    self.errors.push(CheckError::catch_type_mismatch(
                        expr_ty.to_string(),
                        recovery_ty.to_string(),
                        span,
                    ));
                }

                // Return the expression type (catch handles the error internally)
                expr_ty
            }

            // fail expression - explicit error raising
            Expr::Fail { error, span } => {
                // Check that we're in a fallible context (can propagate errors)
                if !self.in_fallible_context {
                    // In an agent, check for error handler
                    if self.current_agent.is_some() && !self.agent_has_error_handler {
                        self.errors.push(CheckError::missing_error_handler(
                            self.current_agent.as_ref().unwrap().clone(),
                            span,
                        ));
                    } else if self.current_agent.is_none() {
                        self.errors.push(CheckError::try_in_non_fallible(span));
                    }
                }

                // Check the error expression
                let error_ty = self.check_expr(error);

                // Error must be either a String or the Error type
                let is_valid_error = matches!(error_ty, Type::String)
                    || matches!(&error_ty, Type::Named(n) if n == "Error")
                    || error_ty.is_error();
                if !is_valid_error {
                    self.errors.push(CheckError::type_mismatch(
                        "String or Error".to_string(),
                        error_ty.to_string(),
                        span,
                    ));
                }

                // Return Never type - this expression never returns
                Type::Never
            }

            // retry expression - retry a fallible operation
            Expr::Retry {
                count,
                delay,
                on_errors,
                body,
                span,
            } => {
                // Check that count is an Int
                let count_ty = self.check_expr(count);
                if !matches!(count_ty, Type::Int) && !count_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int".to_string(),
                        count_ty.to_string(),
                        count.span(),
                    ));
                }

                // Check delay if present (must be Int)
                if let Some(delay_expr) = delay {
                    let delay_ty = self.check_expr(delay_expr);
                    if !matches!(delay_ty, Type::Int) && !delay_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Int".to_string(),
                            delay_ty.to_string(),
                            delay_expr.span(),
                        ));
                    }
                }

                // Check on_errors if present (each should be an ErrorKind variant)
                if let Some(errors) = on_errors {
                    for err_expr in errors {
                        let _err_ty = self.check_expr(err_expr);
                        // For now just type check - could validate ErrorKind later
                    }
                }

                // The body is checked in a fallible context (errors can propagate)
                let prev_fallible = self.in_fallible_context;
                let prev_error_handling = self.in_error_handling;
                self.in_fallible_context = true;
                self.in_error_handling = true;

                let body_ty = self.check_expr(body);

                self.in_fallible_context = prev_fallible;
                self.in_error_handling = prev_error_handling;

                // If retry itself is not in an error handling context, emit warning
                // since all retries could fail
                if !self.in_error_handling {
                    self.errors.push(CheckError::unhandled_error("retry", span));
                }

                // Return the body type
                body_ty
            }

            // trace(message) - emit a trace event
            Expr::Trace { message, span } => {
                let msg_ty = self.check_expr(message);
                if !matches!(msg_ty, Type::String) && !msg_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "String".to_string(),
                        msg_ty.to_string(),
                        span,
                    ));
                }
                Type::Unit
            }

            // RFC-0009: Closures
            Expr::Closure { params, body, .. } => {
                // Push a new scope for closure parameters
                self.push_scope();

                let mut param_types = Vec::new();
                for param in params {
                    let param_ty = if let Some(ty_expr) = &param.ty {
                        resolve_type(ty_expr)
                    } else {
                        // For now require explicit types - inference comes later
                        self.errors.push(CheckError::closure_param_needs_type(
                            param.name.name.clone(),
                            &param.span,
                        ));
                        Type::Error
                    };
                    self.define_var(&param.name.name, param_ty.clone());
                    param_types.push(param_ty);
                }

                // Type check the body
                let body_ty = self.check_expr(body);

                self.pop_scope();

                // Return Fn type
                Type::Fn(param_types, Box::new(body_ty))
            }

            Expr::Tuple { elements, .. } => {
                let elem_types: Vec<Type> = elements.iter().map(|e| self.check_expr(e)).collect();
                Type::Tuple(elem_types)
            }

            Expr::TupleIndex { tuple, index, span } => {
                let tuple_ty = self.check_expr(tuple);
                match &tuple_ty {
                    Type::Tuple(elems) => {
                        if *index < elems.len() {
                            elems[*index].clone()
                        } else {
                            self.errors.push(CheckError::tuple_index_out_of_bounds(
                                *index,
                                elems.len(),
                                span,
                            ));
                            Type::Error
                        }
                    }
                    Type::Error => Type::Error,
                    _ => {
                        self.errors.push(CheckError::type_mismatch(
                            "tuple",
                            tuple_ty.to_string(),
                            span,
                        ));
                        Type::Error
                    }
                }
            }

            Expr::Map { entries, span } => {
                if entries.is_empty() {
                    // Empty map - we can't infer the types, report an error
                    // or use a placeholder. For now, require at least one entry.
                    self.errors.push(CheckError::empty_map_literal(span));
                    Type::Error
                } else {
                    // Check all keys have the same type and all values have the same type
                    let first_key_ty = self.check_expr(&entries[0].key);
                    let first_val_ty = self.check_expr(&entries[0].value);

                    for entry in entries.iter().skip(1) {
                        let key_ty = self.check_expr(&entry.key);
                        let val_ty = self.check_expr(&entry.value);

                        if !key_ty.is_compatible_with(&first_key_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                first_key_ty.to_string(),
                                key_ty.to_string(),
                                &entry.span,
                            ));
                        }
                        if !val_ty.is_compatible_with(&first_val_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                first_val_ty.to_string(),
                                val_ty.to_string(),
                                &entry.span,
                            ));
                        }
                    }

                    Type::Map(Box::new(first_key_ty), Box::new(first_val_ty))
                }
            }

            Expr::VariantConstruct {
                enum_name,
                type_args,
                variant,
                payload,
                span,
            } => {
                // Look up the enum
                let Some(enum_info) = self.symbols.get_enum(&enum_name.name).cloned() else {
                    self.errors
                        .push(CheckError::undefined_type(&enum_name.name, span));
                    return Type::Error;
                };

                // RFC-0015: Handle generic enums
                let bindings = if !enum_info.type_params.is_empty() {
                    self.resolve_enum_type_bindings(
                        &enum_info.type_params,
                        type_args,
                        &enum_info.variants,
                        &variant.name,
                        payload.as_deref(),
                        span,
                    )
                } else {
                    // Non-generic enum: warn if type args provided
                    if !type_args.is_empty() {
                        self.errors.push(CheckError::generic(
                            format!(
                                "enum `{}` is not generic but was constructed with type arguments",
                                enum_name.name
                            ),
                            span,
                        ));
                    }
                    HashMap::new()
                };

                // Check if the variant exists and get its expected payload type
                let Some(expected_payload) = enum_info.get_variant_payload(&variant.name) else {
                    self.errors.push(CheckError::undefined_enum_variant(
                        &variant.name,
                        &enum_name.name,
                        span,
                    ));
                    return Type::Error;
                };

                // Substitute type params in expected payload type
                let expected_payload = expected_payload.map(|ty| ty.substitute(&bindings));

                // Check payload matches expectation
                match (payload, &expected_payload) {
                    (Some(payload_expr), Some(expected_ty)) => {
                        // Variant expects payload and we have one
                        let payload_ty = self.check_expr(payload_expr);
                        if !payload_ty.is_compatible_with(expected_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                expected_ty.to_string(),
                                payload_ty.to_string(),
                                span,
                            ));
                        }
                    }
                    (None, Some(expected_ty)) => {
                        // Variant expects payload but we don't have one
                        self.errors.push(CheckError::type_mismatch(
                            format!("{}({})", variant.name, expected_ty),
                            variant.name.clone(),
                            span,
                        ));
                    }
                    (Some(_), None) => {
                        // Variant doesn't expect payload but we have one
                        self.errors.push(CheckError::type_mismatch(
                            variant.name.clone(),
                            format!("{}(...)", variant.name),
                            span,
                        ));
                    }
                    (None, None) => {
                        // Unit variant - all good
                    }
                }

                // Return Generic type if enum has type params, otherwise Named
                // Special case: Option and Result use their built-in Type variants
                if enum_info.type_params.is_empty() {
                    Type::Named(enum_name.name.clone())
                } else {
                    let resolved_type_args: Vec<Type> = enum_info
                        .type_params
                        .iter()
                        .map(|p| bindings.get(p).cloned().unwrap_or(Type::Error))
                        .collect();

                    // Map built-in enum types to their native Type variants
                    match enum_name.name.as_str() {
                        "Option" if resolved_type_args.len() == 1 => {
                            Type::Option(Box::new(resolved_type_args[0].clone()))
                        }
                        "Result" if resolved_type_args.len() == 2 => {
                            Type::Result(
                                Box::new(resolved_type_args[0].clone()),
                                Box::new(resolved_type_args[1].clone()),
                            )
                        }
                        _ => Type::Generic(enum_name.name.clone(), resolved_type_args),
                    }
                }
            }

            // RFC-0011: Tool calls - just check inner expressions
            Expr::ToolCall { args, .. } => {
                for arg in args {
                    self.check_expr(arg);
                }
                // Return a placeholder type - actual checking is done in first pass
                Type::Error
            }
        }
    }

    fn check_binary_op(
        &mut self,
        op: BinOp,
        left: &Type,
        right: &Type,
        span: &sage_parser::Span,
    ) -> Type {
        if left.is_error() || right.is_error() {
            return Type::Error;
        }

        let left = left.unwrap_oracle();
        let right = right.unwrap_oracle();

        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                if left.is_numeric() && left == right {
                    left.clone()
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        format!("{op}"),
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            BinOp::Concat => {
                if matches!(left, Type::String) && matches!(right, Type::String) {
                    Type::String
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        "++",
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            BinOp::Eq | BinOp::Ne => {
                if left == right {
                    Type::Bool
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        format!("{op}"),
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                if left.is_numeric() && left == right {
                    Type::Bool
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        format!("{op}"),
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }

            BinOp::And | BinOp::Or => {
                if matches!(left, Type::Bool) && matches!(right, Type::Bool) {
                    Type::Bool
                } else {
                    self.errors.push(CheckError::invalid_binary_op(
                        format!("{op}"),
                        left.to_string(),
                        right.to_string(),
                        span,
                    ));
                    Type::Error
                }
            }
        }
    }

    fn check_unary_op(&mut self, op: UnaryOp, operand: &Type, span: &sage_parser::Span) -> Type {
        if operand.is_error() {
            return Type::Error;
        }

        let operand = operand.unwrap_oracle();

        match op {
            UnaryOp::Neg => {
                if operand.is_numeric() {
                    operand.clone()
                } else {
                    self.errors
                        .push(CheckError::invalid_unary_op("-", operand.to_string(), span));
                    Type::Error
                }
            }
            UnaryOp::Not => {
                if matches!(operand, Type::Bool) {
                    Type::Bool
                } else {
                    self.errors
                        .push(CheckError::invalid_unary_op("!", operand.to_string(), span));
                    Type::Error
                }
            }
        }
    }

    fn check_pattern(&mut self, pattern: &Pattern, scrutinee_ty: &Type) {
        match pattern {
            Pattern::Wildcard { .. } => {
                // Wildcard matches anything - no bindings introduced
            }
            Pattern::Binding { name, .. } => {
                // Binding pattern introduces a variable with the scrutinee's type
                self.define_var(&name.name, scrutinee_ty.clone());
            }
            Pattern::Variant {
                enum_name,
                variant,
                payload,
                span,
            } => {
                // Check that the scrutinee is the correct enum type
                // Handle built-in Option/Result types specially
                let (expected_enum, type_bindings): (Option<String>, std::collections::HashMap<String, Type>) = match scrutinee_ty {
                    Type::Named(name) => (Some(name.clone()), std::collections::HashMap::new()),
                    Type::Option(inner) => {
                        let mut bindings = std::collections::HashMap::new();
                        bindings.insert("T".to_string(), (**inner).clone());
                        (Some("Option".to_string()), bindings)
                    }
                    Type::Result(ok, err) => {
                        let mut bindings = std::collections::HashMap::new();
                        bindings.insert("T".to_string(), (**ok).clone());
                        bindings.insert("E".to_string(), (**err).clone());
                        (Some("Result".to_string()), bindings)
                    }
                    Type::Generic(name, args) => {
                        // For generic enums, build type bindings from args
                        if let Some(enum_info) = self.symbols.get_enum(name) {
                            let mut bindings = std::collections::HashMap::new();
                            for (param, arg) in enum_info.type_params.iter().zip(args.iter()) {
                                bindings.insert(param.clone(), arg.clone());
                            }
                            (Some(name.clone()), bindings)
                        } else {
                            (Some(name.clone()), std::collections::HashMap::new())
                        }
                    }
                    Type::Error => return, // Don't cascade errors
                    _ => (None, std::collections::HashMap::new()),
                };

                // Determine the enum name to use for lookup
                let enum_name_for_lookup = enum_name
                    .as_ref()
                    .map(|e| e.name.clone())
                    .or_else(|| expected_enum.clone());

                if let Some(enum_name_str) = &enum_name {
                    // Qualified variant: Status::Active
                    if let Some(expected) = &expected_enum {
                        if enum_name_str.name != *expected {
                            self.errors.push(CheckError::type_mismatch(
                                expected,
                                &enum_name_str.name,
                                span,
                            ));
                            return;
                        }
                    }

                    // Check that the variant exists in the enum
                    if let Some(enum_info) = self.lookup_enum(&enum_name_str.name) {
                        if !enum_info.has_variant(&variant.name) {
                            self.errors.push(CheckError::undefined_enum_variant(
                                &variant.name,
                                &enum_name_str.name,
                                span,
                            ));
                        }
                    } else {
                        self.errors
                            .push(CheckError::undefined_type(&enum_name_str.name, span));
                    }
                } else {
                    // Unqualified variant: just `Active`
                    // Need to check against the scrutinee's enum type
                    if let Some(ref enum_name_str) = expected_enum {
                        if let Some(enum_info) = self.lookup_enum(enum_name_str) {
                            if !enum_info.has_variant(&variant.name) {
                                self.errors.push(CheckError::undefined_enum_variant(
                                    &variant.name,
                                    enum_name_str,
                                    span,
                                ));
                            }
                        }
                    } else if !scrutinee_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "enum type",
                            scrutinee_ty.to_string(),
                            span,
                        ));
                    }
                }

                // Handle payload binding
                if let Some(enum_name_str) = enum_name_for_lookup {
                    if let Some(enum_info) = self.lookup_enum(&enum_name_str).cloned() {
                        if let Some(expected_payload_ty) =
                            enum_info.get_variant_payload(&variant.name)
                        {
                            match (payload, expected_payload_ty) {
                                (Some(inner_pattern), Some(payload_ty)) => {
                                    // Substitute type parameters with concrete types
                                    let substituted_ty = payload_ty.substitute(&type_bindings);
                                    // Recursively check the inner pattern
                                    self.check_pattern(inner_pattern, &substituted_ty);
                                }
                                (None, Some(_)) => {
                                    // Variant has payload but pattern doesn't bind it
                                    // This is OK - we just ignore the payload value
                                }
                                (Some(_), None) => {
                                    // Pattern tries to bind but variant has no payload
                                    self.errors.push(CheckError::type_mismatch(
                                        variant.name.clone(),
                                        format!("{}(...)", variant.name),
                                        span,
                                    ));
                                }
                                (None, None) => {
                                    // Unit variant, no payload - all good
                                }
                            }
                        }
                    }
                }
            }
            Pattern::Literal { value, span } => {
                // Check that the literal type matches the scrutinee type
                let lit_ty = match value {
                    Literal::Int(_) => Type::Int,
                    Literal::Float(_) => Type::Float,
                    Literal::Bool(_) => Type::Bool,
                    Literal::String(_) => Type::String,
                };

                if !lit_ty.is_compatible_with(scrutinee_ty) && !scrutinee_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        scrutinee_ty.to_string(),
                        lit_ty.to_string(),
                        span,
                    ));
                }
            }

            Pattern::Tuple { elements, span } => {
                // Scrutinee must be a tuple with matching arity
                match scrutinee_ty {
                    Type::Tuple(elem_types) => {
                        if elements.len() != elem_types.len() {
                            self.errors.push(CheckError::tuple_arity_mismatch(
                                elements.len(),
                                elem_types.len(),
                                span,
                            ));
                        } else {
                            // Recursively check each element pattern
                            for (pattern, elem_ty) in elements.iter().zip(elem_types.iter()) {
                                self.check_pattern(pattern, elem_ty);
                            }
                        }
                    }
                    Type::Error => {
                        // Don't cascade errors
                    }
                    _ => {
                        self.errors.push(CheckError::type_mismatch(
                            "tuple",
                            scrutinee_ty.to_string(),
                            span,
                        ));
                    }
                }
            }
        }
    }

    fn check_call(
        &mut self,
        name: &str,
        type_args: &[sage_parser::TypeExpr],
        args: &[Expr],
        span: &sage_parser::Span,
    ) -> Type {
        // Check imports first
        if let Some((module_path, original_name)) = self.imports.get(name) {
            // Look up the function in the imported module
            for (_, func) in self.symbols.iter_functions() {
                if &func.module_path == module_path && func.name == *original_name {
                    return self.check_function_call(&func.clone(), type_args, args, span);
                }
            }
        }

        // Check local functions (in current module)
        for (_, func) in self.symbols.iter_functions() {
            if func.module_path == self.module_path && func.name == name {
                return self.check_function_call(&func.clone(), type_args, args, span);
            }
        }

        // Check built-in functions
        if let Some(builtin) = self.symbols.get_builtin(name).cloned() {
            return self.check_builtin_call(&builtin, args, span);
        }

        self.errors.push(CheckError::undefined_function(name, span));
        Type::Error
    }

    fn check_function_call(
        &mut self,
        func: &FunctionInfo,
        type_args: &[sage_parser::TypeExpr],
        args: &[Expr],
        span: &sage_parser::Span,
    ) -> Type {
        if args.len() != func.params.len() {
            self.errors.push(CheckError::wrong_arg_count(
                &func.name,
                func.params.len(),
                args.len(),
                span,
            ));
            return Type::Error;
        }

        // RFC-0015: Handle generic functions
        let bindings = if !func.type_params.is_empty() {
            self.resolve_type_bindings(&func.type_params, type_args, &func.params, args, span)
        } else {
            // Non-generic function: warn if type args provided
            if !type_args.is_empty() {
                self.errors.push(CheckError::generic(
                    format!(
                        "function `{}` is not generic but was called with type arguments",
                        func.name
                    ),
                    span,
                ));
            }
            HashMap::new()
        };

        // Check arguments against (possibly substituted) parameter types
        for (arg, (_, param_ty)) in args.iter().zip(func.params.iter()) {
            let arg_ty = self.check_expr(arg);
            let expected_ty = param_ty.substitute(&bindings);
            if !arg_ty.is_compatible_with(&expected_ty) {
                self.errors.push(CheckError::type_mismatch(
                    expected_ty.to_string(),
                    arg_ty.to_string(),
                    arg.span(),
                ));
            }
        }

        func.return_type.substitute(&bindings)
    }

    /// Resolve type parameter bindings for a generic record construction.
    fn resolve_record_type_bindings(
        &mut self,
        type_params: &[String],
        type_args: &[sage_parser::TypeExpr],
        field_types: &HashMap<String, Type>,
        fields: &[sage_parser::FieldInit],
        span: &sage_parser::Span,
    ) -> HashMap<String, Type> {
        let mut bindings = HashMap::new();

        if !type_args.is_empty() {
            // Explicit type arguments via turbofish syntax
            if type_args.len() != type_params.len() {
                self.errors.push(CheckError::generic(
                    format!(
                        "expected {} type argument(s), found {}",
                        type_params.len(),
                        type_args.len()
                    ),
                    span,
                ));
                return bindings;
            }

            // Convert TypeExpr to Type and create bindings
            let empty_params = HashSet::new();
            for (param, arg) in type_params.iter().zip(type_args.iter()) {
                let ty = resolve_type_with_params(arg, &empty_params);
                bindings.insert(param.clone(), ty);
            }
        } else {
            // Infer type arguments from field values
            for field in fields {
                let field_name = &field.name.name;
                if let Some(expected_ty) = field_types.get(field_name) {
                    let actual_ty = self.check_expr(&field.value);
                    self.unify_types(expected_ty, &actual_ty, &mut bindings);
                }
            }

            // Check all type parameters were inferred
            for param in type_params {
                if !bindings.contains_key(param) {
                    self.errors.push(CheckError::generic(
                        format!("could not infer type for type parameter `{}`", param),
                        span,
                    ));
                }
            }
        }

        bindings
    }

    /// Resolve type parameter bindings for a generic enum variant construction.
    fn resolve_enum_type_bindings(
        &mut self,
        type_params: &[String],
        type_args: &[sage_parser::TypeExpr],
        variants: &[(String, Option<Type>)],
        variant_name: &str,
        payload: Option<&Expr>,
        span: &sage_parser::Span,
    ) -> HashMap<String, Type> {
        let mut bindings = HashMap::new();

        if !type_args.is_empty() {
            // Explicit type arguments via turbofish syntax
            if type_args.len() != type_params.len() {
                self.errors.push(CheckError::generic(
                    format!(
                        "expected {} type argument(s), found {}",
                        type_params.len(),
                        type_args.len()
                    ),
                    span,
                ));
                return bindings;
            }

            // Convert TypeExpr to Type and create bindings
            let empty_params = HashSet::new();
            for (param, arg) in type_params.iter().zip(type_args.iter()) {
                let ty = resolve_type_with_params(arg, &empty_params);
                bindings.insert(param.clone(), ty);
            }
        } else {
            // Infer type arguments from payload
            if let Some(payload_expr) = payload {
                // Find the variant's payload type
                if let Some((_, Some(expected_ty))) =
                    variants.iter().find(|(name, _)| name == variant_name)
                {
                    let actual_ty = self.check_expr(payload_expr);
                    self.unify_types(expected_ty, &actual_ty, &mut bindings);
                }
            }

            // For enum variants, all type params must be inferrable or explicitly provided
            // since generated Rust code needs concrete types
            for param in type_params {
                if !bindings.contains_key(param) {
                    self.errors.push(CheckError::generic(
                        format!(
                            "type parameter `{}` cannot be inferred; use turbofish syntax",
                            param
                        ),
                        span,
                    ));
                    // Insert Error to avoid cascading errors
                    bindings.insert(param.clone(), Type::Error);
                }
            }
        }

        bindings
    }

    /// Resolve type parameter bindings for a generic function call.
    fn resolve_type_bindings(
        &mut self,
        type_params: &[String],
        type_args: &[sage_parser::TypeExpr],
        params: &[(String, Type)],
        args: &[Expr],
        span: &sage_parser::Span,
    ) -> HashMap<String, Type> {
        let mut bindings = HashMap::new();

        if !type_args.is_empty() {
            // Explicit type arguments via turbofish syntax
            if type_args.len() != type_params.len() {
                self.errors.push(CheckError::generic(
                    format!(
                        "expected {} type argument(s), found {}",
                        type_params.len(),
                        type_args.len()
                    ),
                    span,
                ));
                return bindings;
            }

            // Convert TypeExpr to Type and create bindings
            let empty_params = HashSet::new();
            for (param, arg) in type_params.iter().zip(type_args.iter()) {
                let ty = resolve_type_with_params(arg, &empty_params);
                bindings.insert(param.clone(), ty);
            }
        } else {
            // Infer type arguments from actual arguments
            for (arg, (_, param_ty)) in args.iter().zip(params.iter()) {
                let arg_ty = self.check_expr(arg);
                self.unify_types(param_ty, &arg_ty, &mut bindings);
            }

            // Check all type parameters were inferred
            for param in type_params {
                if !bindings.contains_key(param) {
                    self.errors.push(CheckError::generic(
                        format!("could not infer type for type parameter `{}`", param),
                        span,
                    ));
                }
            }
        }

        bindings
    }

    /// Attempt to unify a pattern type with a concrete type, extracting type parameter bindings.
    fn unify_types(&mut self, pattern: &Type, concrete: &Type, bindings: &mut HashMap<String, Type>) {
        match (pattern, concrete) {
            // Type parameter: bind to concrete type
            (Type::TypeParam(name), concrete) => {
                if let Some(existing) = bindings.get(name) {
                    // Already bound - check consistency
                    if !existing.is_compatible_with(concrete) {
                        // Conflicting types for same type parameter - handled elsewhere
                    }
                } else {
                    bindings.insert(name.clone(), concrete.clone());
                }
            }

            // Recurse into compound types
            (Type::List(p_elem), Type::List(c_elem)) => {
                self.unify_types(p_elem, c_elem, bindings);
            }
            (Type::Option(p_inner), Type::Option(c_inner)) => {
                self.unify_types(p_inner, c_inner, bindings);
            }
            (Type::Map(p_k, p_v), Type::Map(c_k, c_v)) => {
                self.unify_types(p_k, c_k, bindings);
                self.unify_types(p_v, c_v, bindings);
            }
            (Type::Tuple(p_elems), Type::Tuple(c_elems)) if p_elems.len() == c_elems.len() => {
                for (p, c) in p_elems.iter().zip(c_elems.iter()) {
                    self.unify_types(p, c, bindings);
                }
            }
            (Type::Result(p_ok, p_err), Type::Result(c_ok, c_err)) => {
                self.unify_types(p_ok, c_ok, bindings);
                self.unify_types(p_err, c_err, bindings);
            }
            (Type::Fn(p_params, p_ret), Type::Fn(c_params, c_ret))
                if p_params.len() == c_params.len() =>
            {
                for (p, c) in p_params.iter().zip(c_params.iter()) {
                    self.unify_types(p, c, bindings);
                }
                self.unify_types(p_ret, c_ret, bindings);
            }
            (Type::Generic(p_name, p_args), Type::Generic(c_name, c_args))
                if p_name == c_name && p_args.len() == c_args.len() =>
            {
                for (p, c) in p_args.iter().zip(c_args.iter()) {
                    self.unify_types(p, c, bindings);
                }
            }
            (Type::Oracle(p_inner), Type::Oracle(c_inner)) => {
                self.unify_types(p_inner, c_inner, bindings);
            }

            // Base types: no unification needed
            _ => {}
        }
    }

    fn check_builtin_call(
        &mut self,
        builtin: &crate::scope::BuiltinInfo,
        args: &[Expr],
        span: &sage_parser::Span,
    ) -> Type {
        // RFC-0012: E050 - assertion builtins only valid in test files
        if self.symbols.is_test_assertion(builtin.name) && !self.is_test_file {
            self.errors.push(CheckError::test_outside_test_file(span));
            return Type::Unit;
        }

        match builtin.name {
            "len" => {
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("len", 1, args.len(), span));
                    return Type::Error;
                }
                let arg_ty = self.check_expr(&args[0]);
                // len() accepts List<T> or Map<K, V>
                if arg_ty.list_element().is_none()
                    && arg_ty.map_key_value().is_none()
                    && !arg_ty.is_error()
                {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T> or Map<K, V>",
                        arg_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Int
            }

            "push" => {
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("push", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let elem_ty = self.check_expr(&args[1]);

                if let Some(expected_elem) = list_ty.list_element() {
                    if !elem_ty.is_compatible_with(expected_elem) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_elem.to_string(),
                            elem_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    list_ty.clone()
                } else {
                    if !list_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "List<T>",
                            list_ty.to_string(),
                            args[0].span(),
                        ));
                    }
                    Type::Error
                }
            }

            "str" => {
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("str", 1, args.len(), span));
                    return Type::Error;
                }
                self.check_expr(&args[0]);
                Type::String
            }

            "map_get" => {
                // map_get(Map<K, V>, K) -> Option<V>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map_get", 2, args.len(), span));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);
                let key_ty = self.check_expr(&args[1]);

                if let Some((expected_key, value_ty)) = map_ty.map_key_value() {
                    if !key_ty.is_compatible_with(expected_key) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_key.to_string(),
                            key_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    Type::Option(Box::new(value_ty.clone()))
                } else {
                    if !map_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Map<K, V>",
                            map_ty.to_string(),
                            args[0].span(),
                        ));
                    }
                    Type::Error
                }
            }

            "map_set" => {
                // map_set(Map<K, V>, K, V) -> Unit
                if args.len() != 3 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map_set", 3, args.len(), span));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);
                let key_ty = self.check_expr(&args[1]);
                let value_ty = self.check_expr(&args[2]);

                if let Some((expected_key, expected_value)) = map_ty.map_key_value() {
                    if !key_ty.is_compatible_with(expected_key) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_key.to_string(),
                            key_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    if !value_ty.is_compatible_with(expected_value) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_value.to_string(),
                            value_ty.to_string(),
                            args[2].span(),
                        ));
                    }
                } else if !map_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Map<K, V>",
                        map_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Unit
            }

            "map_delete" => {
                // map_delete(Map<K, V>, K) -> Unit
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "map_delete",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);
                let key_ty = self.check_expr(&args[1]);

                if let Some((expected_key, _)) = map_ty.map_key_value() {
                    if !key_ty.is_compatible_with(expected_key) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_key.to_string(),
                            key_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !map_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Map<K, V>",
                        map_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Unit
            }

            "map_has" => {
                // map_has(Map<K, V>, K) -> Bool
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map_has", 2, args.len(), span));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);
                let key_ty = self.check_expr(&args[1]);

                if let Some((expected_key, _)) = map_ty.map_key_value() {
                    if !key_ty.is_compatible_with(expected_key) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_key.to_string(),
                            key_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !map_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Map<K, V>",
                        map_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Bool
            }

            "map_keys" => {
                // map_keys(Map<K, V>) -> List<K>
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map_keys", 1, args.len(), span));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);

                if let Some((key_ty, _)) = map_ty.map_key_value() {
                    Type::List(Box::new(key_ty.clone()))
                } else {
                    if !map_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Map<K, V>",
                            map_ty.to_string(),
                            args[0].span(),
                        ));
                    }
                    Type::Error
                }
            }

            "map_values" => {
                // map_values(Map<K, V>) -> List<V>
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "map_values",
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let map_ty = self.check_expr(&args[0]);

                if let Some((_, value_ty)) = map_ty.map_key_value() {
                    Type::List(Box::new(value_ty.clone()))
                } else {
                    if !map_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            "Map<K, V>",
                            map_ty.to_string(),
                            args[0].span(),
                        ));
                    }
                    Type::Error
                }
            }

            // =========================================================================
            // RFC-0013: List Higher-Order Functions
            // =========================================================================
            "map" => {
                // map(List<A>, Fn(A) -> B) -> List<B>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("map", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1 && params[0].is_compatible_with(elem_ty) {
                            return Type::List(ret.clone());
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> B"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "filter" => {
                // filter(List<A>, Fn(A) -> Bool) -> List<A>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("filter", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1
                            && params[0].is_compatible_with(elem_ty)
                            && **ret == Type::Bool
                        {
                            return list_ty.clone();
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> Bool"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "reduce" => {
                // reduce(List<A>, B, Fn(B, A) -> B) -> B
                if args.len() != 3 {
                    self.errors
                        .push(CheckError::wrong_arg_count("reduce", 3, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let init_ty = self.check_expr(&args[1]);
                let fn_ty = self.check_expr(&args[2]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 2
                            && params[0].is_compatible_with(&init_ty)
                            && params[1].is_compatible_with(elem_ty)
                            && ret.is_compatible_with(&init_ty)
                        {
                            return init_ty;
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({init_ty}, {elem_ty}) -> {init_ty}"),
                            fn_ty.to_string(),
                            args[2].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "any" | "all" | "count_where" => {
                // any/all(List<A>, Fn(A) -> Bool) -> Bool
                // count_where(List<A>, Fn(A) -> Bool) -> Int
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1
                            && params[0].is_compatible_with(elem_ty)
                            && **ret == Type::Bool
                        {
                            return if builtin.name == "count_where" {
                                Type::Int
                            } else {
                                Type::Bool
                            };
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> Bool"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "find" => {
                // find(List<A>, Fn(A) -> Bool) -> Option<A>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("find", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1
                            && params[0].is_compatible_with(elem_ty)
                            && **ret == Type::Bool
                        {
                            return Type::Option(Box::new(elem_ty.clone()));
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> Bool"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "flat_map" => {
                // flat_map(List<A>, Fn(A) -> List<B>) -> List<B>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("flat_map", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1 && params[0].is_compatible_with(elem_ty) {
                            // Return type must be List<B>
                            if ret.list_element().is_some() {
                                return (**ret).clone();
                            }
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> List<B>"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "zip" => {
                // zip(List<A>, List<B>) -> List<(A, B)>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("zip", 2, args.len(), span));
                    return Type::Error;
                }
                let list1_ty = self.check_expr(&args[0]);
                let list2_ty = self.check_expr(&args[1]);

                if let (Some(elem1), Some(elem2)) =
                    (list1_ty.list_element(), list2_ty.list_element())
                {
                    return Type::List(Box::new(Type::Tuple(vec![elem1.clone(), elem2.clone()])));
                }
                if !list1_ty.is_error() && list1_ty.list_element().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list1_ty.to_string(),
                        args[0].span(),
                    ));
                }
                if !list2_ty.is_error() && list2_ty.list_element().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<B>",
                        list2_ty.to_string(),
                        args[1].span(),
                    ));
                }
                Type::Error
            }

            "sort_by" => {
                // sort_by(List<A>, Fn(A, A) -> Int) -> List<A>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("sort_by", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 2
                            && params[0].is_compatible_with(elem_ty)
                            && params[1].is_compatible_with(elem_ty)
                            && **ret == Type::Int
                        {
                            return list_ty.clone();
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}, {elem_ty}) -> Int"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "enumerate" => {
                // enumerate(List<A>) -> List<(Int, A)>
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "enumerate",
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if let Some(elem_ty) = list_ty.list_element() {
                    return Type::List(Box::new(Type::Tuple(vec![Type::Int, elem_ty.clone()])));
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "take" | "drop" => {
                // take/drop(List<A>, Int) -> List<A>
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let n_ty = self.check_expr(&args[1]);

                if n_ty != Type::Int && !n_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        n_ty.to_string(),
                        args[1].span(),
                    ));
                }
                if list_ty.list_element().is_some() {
                    return list_ty;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "flatten" => {
                // flatten(List<List<A>>) -> List<A>
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("flatten", 1, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if let Some(inner) = list_ty.list_element() {
                    if let Some(elem_ty) = inner.list_element() {
                        return Type::List(Box::new(elem_ty.clone()));
                    }
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<List<A>>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "reverse" | "unique" => {
                // reverse/unique(List<A>) -> List<A>
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if list_ty.list_element().is_some() {
                    return list_ty;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<A>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            // =========================================================================
            // RFC-0010: List Utilities
            // =========================================================================
            "first" | "last" | "pop" => {
                // first/last/pop(List<T>) -> Option<T>
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if let Some(elem_ty) = list_ty.list_element() {
                    return Type::Option(Box::new(elem_ty.clone()));
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "get" => {
                // get(List<T>, Int) -> Option<T>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("get", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let idx_ty = self.check_expr(&args[1]);

                if idx_ty != Type::Int && !idx_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        idx_ty.to_string(),
                        args[1].span(),
                    ));
                }
                if let Some(elem_ty) = list_ty.list_element() {
                    return Type::Option(Box::new(elem_ty.clone()));
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "list_contains" => {
                // list_contains(List<T>, T) -> Bool
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "list_contains",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let elem_ty = self.check_expr(&args[1]);

                if let Some(expected_elem) = list_ty.list_element() {
                    if !elem_ty.is_compatible_with(expected_elem) {
                        self.errors.push(CheckError::type_mismatch(
                            expected_elem.to_string(),
                            elem_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    return Type::Bool;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "sort" => {
                // sort(List<T>) -> List<T>
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("sort", 1, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);

                if list_ty.list_element().is_some() {
                    return list_ty;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "list_slice" => {
                // list_slice(List<T>, Int, Int) -> List<T>
                if args.len() != 3 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "list_slice",
                        3,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let start_ty = self.check_expr(&args[1]);
                let end_ty = self.check_expr(&args[2]);

                if start_ty != Type::Int && !start_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        start_ty.to_string(),
                        args[1].span(),
                    ));
                }
                if end_ty != Type::Int && !end_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        end_ty.to_string(),
                        args[2].span(),
                    ));
                }
                if list_ty.list_element().is_some() {
                    return list_ty;
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "chunk" => {
                // chunk(List<T>, Int) -> List<List<T>>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("chunk", 2, args.len(), span));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let size_ty = self.check_expr(&args[1]);

                if size_ty != Type::Int && !size_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Int",
                        size_ty.to_string(),
                        args[1].span(),
                    ));
                }
                if list_ty.list_element().is_some() {
                    return Type::List(Box::new(list_ty));
                }
                if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "concat" => {
                // concat(List<T>, List<T>) -> List<T>
                if args.len() != 2 {
                    self.errors
                        .push(CheckError::wrong_arg_count("concat", 2, args.len(), span));
                    return Type::Error;
                }
                let list1_ty = self.check_expr(&args[0]);
                let list2_ty = self.check_expr(&args[1]);

                if let (Some(elem1), Some(elem2)) =
                    (list1_ty.list_element(), list2_ty.list_element())
                {
                    if !elem1.is_compatible_with(elem2) {
                        self.errors.push(CheckError::type_mismatch(
                            list1_ty.to_string(),
                            list2_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    return list1_ty;
                }
                if !list1_ty.is_error() && list1_ty.list_element().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list1_ty.to_string(),
                        args[0].span(),
                    ));
                }
                if !list2_ty.is_error() && list2_ty.list_element().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list2_ty.to_string(),
                        args[1].span(),
                    ));
                }
                Type::Error
            }

            "take_while" | "drop_while" => {
                // take_while/drop_while(List<T>, Fn(T)->Bool) -> List<T>
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let list_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(elem_ty) = list_ty.list_element() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1
                            && params[0].is_compatible_with(elem_ty)
                            && **ret == Type::Bool
                        {
                            return list_ty;
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({elem_ty}) -> Bool"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !list_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
                        list_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            // =========================================================================
            // RFC-0010: Option Utilities
            // =========================================================================
            "is_some" | "is_none" => {
                // is_some/is_none(Option<T>) -> Bool
                if args.len() != 1 {
                    self.errors.push(CheckError::wrong_arg_count(
                        builtin.name,
                        1,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);

                if opt_ty.option_inner().is_some() || opt_ty.is_error() {
                    return Type::Bool;
                }
                self.errors.push(CheckError::type_mismatch(
                    "Option<T>",
                    opt_ty.to_string(),
                    args[0].span(),
                ));
                Type::Error
            }

            "unwrap" => {
                // unwrap(Option<T>) -> T (fallible)
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("unwrap", 1, args.len(), span));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);

                if let Some(inner_ty) = opt_ty.option_inner() {
                    return inner_ty.clone();
                }
                if !opt_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "unwrap_or" => {
                // unwrap_or(Option<T>, T) -> T
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "unwrap_or",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);
                let default_ty = self.check_expr(&args[1]);

                if let Some(inner_ty) = opt_ty.option_inner() {
                    if !default_ty.is_compatible_with(inner_ty) {
                        self.errors.push(CheckError::type_mismatch(
                            inner_ty.to_string(),
                            default_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    return inner_ty.clone();
                }
                if !opt_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "unwrap_or_else" => {
                // unwrap_or_else(Option<T>, Fn()->T) -> T
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "unwrap_or_else",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(inner_ty) = opt_ty.option_inner() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.is_empty() && ret.is_compatible_with(inner_ty) {
                            return inner_ty.clone();
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn() -> {inner_ty}"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !opt_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "map_option" => {
                // map_option(Option<T>, Fn(T)->U) -> Option<U>
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "map_option",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt_ty = self.check_expr(&args[0]);
                let fn_ty = self.check_expr(&args[1]);

                if let Some(inner_ty) = opt_ty.option_inner() {
                    if let Type::Fn(params, ret) = &fn_ty {
                        if params.len() == 1 && params[0].is_compatible_with(inner_ty) {
                            return Type::Option(ret.clone());
                        }
                    }
                    if !fn_ty.is_error() {
                        self.errors.push(CheckError::type_mismatch(
                            format!("Fn({inner_ty}) -> U"),
                            fn_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                } else if !opt_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt_ty.to_string(),
                        args[0].span(),
                    ));
                }
                Type::Error
            }

            "or_option" => {
                // or_option(Option<T>, Option<T>) -> Option<T>
                if args.len() != 2 {
                    self.errors.push(CheckError::wrong_arg_count(
                        "or_option",
                        2,
                        args.len(),
                        span,
                    ));
                    return Type::Error;
                }
                let opt1_ty = self.check_expr(&args[0]);
                let opt2_ty = self.check_expr(&args[1]);

                if let (Some(inner1), Some(inner2)) =
                    (opt1_ty.option_inner(), opt2_ty.option_inner())
                {
                    if !inner1.is_compatible_with(inner2) {
                        self.errors.push(CheckError::type_mismatch(
                            opt1_ty.to_string(),
                            opt2_ty.to_string(),
                            args[1].span(),
                        ));
                    }
                    return opt1_ty;
                }
                if !opt1_ty.is_error() && opt1_ty.option_inner().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt1_ty.to_string(),
                        args[0].span(),
                    ));
                }
                if !opt2_ty.is_error() && opt2_ty.option_inner().is_none() {
                    self.errors.push(CheckError::type_mismatch(
                        "Option<T>",
                        opt2_ty.to_string(),
                        args[1].span(),
                    ));
                }
                Type::Error
            }

            _ => {
                if let Some(ref params) = builtin.params {
                    if args.len() != params.len() {
                        self.errors.push(CheckError::wrong_arg_count(
                            builtin.name,
                            params.len(),
                            args.len(),
                            span,
                        ));
                        return Type::Error;
                    }

                    for (arg, param_ty) in args.iter().zip(params.iter()) {
                        let arg_ty = self.check_expr(arg);
                        if !arg_ty.is_compatible_with(param_ty) {
                            self.errors.push(CheckError::type_mismatch(
                                param_ty.to_string(),
                                arg_ty.to_string(),
                                arg.span(),
                            ));
                        }
                    }
                }

                builtin.return_type.clone()
            }
        }
    }

    fn lookup_agent(&self, name: &str) -> Option<&AgentInfo> {
        // Check imports first
        if let Some((module_path, original_name)) = self.imports.get(name) {
            for (_, agent) in self.symbols.iter_agents() {
                if &agent.module_path == module_path && agent.name == *original_name {
                    return Some(agent);
                }
            }
        }

        // Check local agents
        self.symbols
            .iter_agents()
            .map(|(_, agent)| agent)
            .find(|&agent| agent.module_path == self.module_path && agent.name == name)
            .map(|v| v as _)
    }

    fn lookup_record(&self, name: &str) -> Option<&RecordInfo> {
        // Check imports first
        if let Some((module_path, original_name)) = self.imports.get(name) {
            for (_, record) in self.symbols.iter_records() {
                if &record.module_path == module_path && record.name == *original_name {
                    return Some(record);
                }
            }
        }

        // Check local records
        self.symbols
            .iter_records()
            .map(|(_, record)| record)
            .find(|&record| record.module_path == self.module_path && record.name == name)
            .map(|v| v as _)
    }

    fn lookup_enum(&self, name: &str) -> Option<&EnumInfo> {
        // Check imports first
        if let Some((module_path, original_name)) = self.imports.get(name) {
            for (_, enum_info) in self.symbols.iter_enums() {
                if &enum_info.module_path == module_path && enum_info.name == *original_name {
                    return Some(enum_info);
                }
            }
        }

        // Check local enums
        self.symbols
            .iter_enums()
            .map(|(_, enum_info)| enum_info)
            .find(|&enum_info| enum_info.module_path == self.module_path && enum_info.name == name)
            .map(|v| v as _)
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define_var(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.define(name, ty);
        }
    }

    fn lookup_var(&mut self, name: &str, span: &sage_parser::Span) -> Type {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return ty.clone();
            }
        }

        // Check if it's a constant
        if let Some(const_info) = self.lookup_const(name) {
            return const_info.ty.clone();
        }

        // Check for builtin constants (RFC-0013)
        if name == "PI" || name == "E" {
            return Type::Float;
        }

        // Time constants (RFC-0010)
        if matches!(
            name,
            "MS_PER_SECOND" | "MS_PER_MINUTE" | "MS_PER_HOUR" | "MS_PER_DAY"
        ) {
            return Type::Int;
        }

        self.errors.push(CheckError::undefined_variable(name, span));
        Type::Error
    }

    fn lookup_const(&self, name: &str) -> Option<&ConstInfo> {
        // Check imports first
        if let Some((module_path, original_name)) = self.imports.get(name) {
            for (_, const_info) in self.symbols.iter_consts() {
                if &const_info.module_path == module_path && const_info.name == *original_name {
                    return Some(const_info);
                }
            }
        }

        // Check local consts
        self.symbols
            .iter_consts()
            .map(|(_, const_info)| const_info)
            .find(|&const_info| {
                const_info.module_path == self.module_path && const_info.name == name
            })
            .map(|v| v as _)
    }
}
