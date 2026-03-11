//! Type checker and name resolver for Sage programs.

use crate::error::CheckError;
use crate::scope::{resolve_type, AgentInfo, FunctionInfo, Scope, SymbolTable};
use crate::types::Type;
use sage_parser::{
    AgentDecl, BinOp, Block, EventKind, Expr, FnDecl, Literal, Program, Stmt, UnaryOp,
};
use std::collections::HashMap;

/// Result of type checking a program.
pub struct CheckResult {
    /// The symbol table with all resolved declarations.
    pub symbols: SymbolTable,
    /// Any errors encountered during checking.
    pub errors: Vec<CheckError>,
}

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

        for func in &program.functions {
            self.check_function(func);
        }

        // Validate the entry agent
        self.validate_entry_agent(program);

        CheckResult {
            symbols: self.symbols,
            errors: self.errors,
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

            let params: Vec<(String, Type)> = func
                .params
                .iter()
                .map(|p| (p.name.name.clone(), resolve_type(&p.ty)))
                .collect();

            let return_type = resolve_type(&func.return_ty);

            self.symbols.define_function(FunctionInfo {
                name: func.name.name.clone(),
                params,
                return_type,
            });
        }
    }

    // =========================================================================
    // Second pass: type checking
    // =========================================================================

    fn check_agent(&mut self, agent: &AgentDecl) {
        self.current_agent = Some(agent.name.name.clone());

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

            self.check_block(&handler.body);
            self.pop_scope();
        }

        self.current_agent = None;
    }

    fn check_function(&mut self, func: &FnDecl) {
        self.in_function = true;
        self.expected_return = Some(resolve_type(&func.return_ty));

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
                var,
                iter,
                body,
                span,
            } => {
                let iter_ty = self.check_expr(iter);

                let elem_ty = if let Some(elem) = iter_ty.list_element() {
                    elem.clone()
                } else {
                    if !iter_ty.is_error() {
                        self.errors
                            .push(CheckError::not_iterable(iter_ty.to_string(), span));
                    }
                    Type::Error
                };

                self.push_scope();
                self.define_var(&var.name, elem_ty);
                self.check_block(body);
                self.pop_scope();
            }

            Stmt::Expr { expr, .. } => {
                self.check_expr(expr);
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

            Expr::Call { name, args, span } => self.check_call(&name.name, args, span),

            Expr::SelfField { field, span } => {
                let Some(agent_name) = &self.current_agent else {
                    self.errors.push(CheckError::self_outside_agent(span));
                    return Type::Error;
                };

                let Some(agent) = self.symbols.get_agent(agent_name) else {
                    return Type::Error; // Agent should exist
                };

                if let Some(ty) = agent.beliefs.get(&field.name) {
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

            Expr::Infer { result_ty, .. } => {
                // infer returns Inferred<T>, default to Inferred<String>
                let inner = result_ty.as_ref().map_or(Type::String, resolve_type);
                Type::Inferred(Box::new(inner))
            }

            Expr::Spawn {
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

            Expr::Await { handle, span } => {
                let handle_ty = self.check_expr(handle);

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

            Expr::Emit { value, .. } => {
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
        }
    }

    fn check_binary_op(
        &mut self,
        op: BinOp,
        left: &Type,
        right: &Type,
        span: &sage_types::Span,
    ) -> Type {
        // Handle error propagation
        if left.is_error() || right.is_error() {
            return Type::Error;
        }

        // Unwrap inferred types for comparison
        let left = left.unwrap_inferred();
        let right = right.unwrap_inferred();

        match op {
            // Arithmetic: Int/Float
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
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

    fn check_unary_op(&mut self, op: UnaryOp, operand: &Type, span: &sage_types::Span) -> Type {
        if operand.is_error() {
            return Type::Error;
        }

        let operand = operand.unwrap_inferred();

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

    fn check_call(&mut self, name: &str, args: &[Expr], span: &sage_types::Span) -> Type {
        // Check for user-defined function
        if let Some(func) = self.symbols.get_function(name).cloned() {
            if args.len() != func.params.len() {
                self.errors.push(CheckError::wrong_arg_count(
                    name,
                    func.params.len(),
                    args.len(),
                    span,
                ));
                return Type::Error;
            }

            for (arg, (_, param_ty)) in args.iter().zip(func.params.iter()) {
                let arg_ty = self.check_expr(arg);
                if !arg_ty.is_compatible_with(param_ty) {
                    self.errors.push(CheckError::type_mismatch(
                        param_ty.to_string(),
                        arg_ty.to_string(),
                        arg.span(),
                    ));
                }
            }

            return func.return_type.clone();
        }

        // Check for built-in function
        if let Some(builtin) = self.symbols.get_builtin(name).cloned() {
            return self.check_builtin_call(&builtin, args, span);
        }

        self.errors.push(CheckError::undefined_function(name, span));
        Type::Error
    }

    fn check_builtin_call(
        &mut self,
        builtin: &crate::scope::BuiltinInfo,
        args: &[Expr],
        span: &sage_types::Span,
    ) -> Type {
        match builtin.name {
            "len" => {
                if args.len() != 1 {
                    self.errors
                        .push(CheckError::wrong_arg_count("len", 1, args.len(), span));
                    return Type::Error;
                }
                let arg_ty = self.check_expr(&args[0]);
                if arg_ty.list_element().is_none() && !arg_ty.is_error() {
                    self.errors.push(CheckError::type_mismatch(
                        "List<T>",
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
        let entry_name = &program.run_agent.name;

        let Some(agent) = self.symbols.get_agent(entry_name).cloned() else {
            self.errors.push(CheckError::undefined_agent(
                entry_name,
                &program.run_agent.span,
            ));
            return;
        };

        // Entry agent must have no beliefs
        if !agent.beliefs.is_empty() {
            self.errors.push(CheckError::entry_agent_has_beliefs(
                entry_name,
                &program.run_agent.span,
            ));
        }

        // Entry agent must have on start handler
        if !agent.has_start_handler {
            self.errors.push(CheckError::entry_agent_no_start(
                entry_name,
                &program.run_agent.span,
            ));
        }
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

    fn lookup_var(&mut self, name: &str, span: &sage_types::Span) -> Type {
        // Search from innermost to outermost scope
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return ty.clone();
            }
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
