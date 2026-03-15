//! Sage source code formatter.
//!
//! Provides an opinionated, non-configurable formatter for Sage source code.
//! Similar in spirit to `gofmt` and `rustfmt`.
//!
//! # Formatting Rules
//!
//! - Indent with 4 spaces (no tabs)
//! - One blank line between top-level declarations
//! - Two blank lines before `run` statement
//! - Handler ordering: `on start` → `on message` → `on error` → `on stop`
//! - Binary operators with spaces either side
//! - Closing braces on their own line
//! - Spawn initializer fields: one per line if >2 fields, inline if ≤2

use crate::ast::*;
use crate::ty::TypeExpr;
use std::fmt::Write;

/// Formats a Sage program into canonical source code.
pub fn format(program: &Program) -> String {
    let mut formatter = Formatter::new();
    formatter.format_program(program);
    formatter.output
}

struct Formatter {
    output: String,
    indent: usize,
}

impl Formatter {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn writeln(&mut self, s: &str) {
        self.write_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    fn indent(&mut self) {
        self.indent += 1;
    }

    fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    fn format_program(&mut self, program: &Program) {
        let mut first = true;

        // Module declarations
        for m in &program.mod_decls {
            if !first {
                self.newline();
            }
            first = false;
            self.format_mod_decl(m);
        }

        // Use declarations
        for u in &program.use_decls {
            if !first {
                self.newline();
            }
            first = false;
            self.format_use_decl(u);
        }

        // Constants
        for c in &program.consts {
            if !first {
                self.newline();
            }
            first = false;
            self.format_const_decl(c);
        }

        // Records
        for r in &program.records {
            if !first {
                self.newline();
            }
            first = false;
            self.format_record_decl(r);
        }

        // Enums
        for e in &program.enums {
            if !first {
                self.newline();
            }
            first = false;
            self.format_enum_decl(e);
        }

        // Tools
        for t in &program.tools {
            if !first {
                self.newline();
            }
            first = false;
            self.format_tool_decl(t);
        }

        // Functions
        for f in &program.functions {
            if !first {
                self.newline();
            }
            first = false;
            self.format_fn_decl(f);
        }

        // Agents
        for a in &program.agents {
            if !first {
                self.newline();
            }
            first = false;
            self.format_agent_decl(a);
        }

        // Tests
        for t in &program.tests {
            if !first {
                self.newline();
            }
            first = false;
            self.format_test_decl(t);
        }

        // Run statement (two blank lines before)
        if let Some(run_agent) = &program.run_agent {
            if !first {
                self.newline();
                self.newline();
            }
            self.write("run ");
            self.write(&run_agent.name);
            self.writeln(";");
        }
    }

    fn format_mod_decl(&mut self, m: &ModDecl) {
        if m.is_pub {
            self.write("pub ");
        }
        self.write("mod ");
        self.write(&m.name.name);
        self.writeln(";");
    }

    fn format_use_decl(&mut self, u: &UseDecl) {
        if u.is_pub {
            self.write("pub ");
        }
        self.write("use ");
        self.write(
            &u.path
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>()
                .join("::"),
        );
        match &u.kind {
            UseKind::Simple(alias) => {
                if let Some(a) = alias {
                    self.write(" as ");
                    self.write(&a.name);
                }
            }
            UseKind::Glob => {
                self.write("::*");
            }
            UseKind::Group(names) => {
                self.write("::{");
                for (i, (name, alias)) in names.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&name.name);
                    if let Some(a) = alias {
                        self.write(" as ");
                        self.write(&a.name);
                    }
                }
                self.write("}");
            }
        }
        self.writeln(";");
    }

    fn format_const_decl(&mut self, c: &ConstDecl) {
        if c.is_pub {
            self.write("pub ");
        }
        self.write("const ");
        self.write(&c.name.name);
        self.write(": ");
        self.format_type(&c.ty);
        self.write(" = ");
        self.format_expr(&c.value);
        self.writeln(";");
    }

    fn format_record_decl(&mut self, r: &RecordDecl) {
        if r.is_pub {
            self.write("pub ");
        }
        self.write("record ");
        self.write(&r.name.name);
        self.write(" {\n");
        self.indent();
        for field in &r.fields {
            self.write_indent();
            self.write(&field.name.name);
            self.write(": ");
            self.format_type(&field.ty);
            self.write("\n");
        }
        self.dedent();
        self.writeln("}");
    }

    fn format_enum_decl(&mut self, e: &EnumDecl) {
        if e.is_pub {
            self.write("pub ");
        }
        self.write("enum ");
        self.write(&e.name.name);
        self.write(" {\n");
        self.indent();
        for variant in &e.variants {
            self.write_indent();
            self.write(&variant.name.name);
            if let Some(payload) = &variant.payload {
                self.write("(");
                self.format_type(payload);
                self.write(")");
            }
            self.write("\n");
        }
        self.dedent();
        self.writeln("}");
    }

    fn format_tool_decl(&mut self, t: &ToolDecl) {
        if t.is_pub {
            self.write("pub ");
        }
        self.write("tool ");
        self.write(&t.name.name);
        self.write(" {\n");
        self.indent();
        for func in &t.functions {
            self.write_indent();
            self.write("fn ");
            self.write(&func.name.name);
            self.write("(");
            for (i, param) in func.params.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&param.name.name);
                self.write(": ");
                self.format_type(&param.ty);
            }
            self.write(") -> ");
            self.format_type(&func.return_ty);
            self.write("\n");
        }
        self.dedent();
        self.writeln("}");
    }

    fn format_fn_decl(&mut self, f: &FnDecl) {
        if f.is_pub {
            self.write("pub ");
        }
        self.write("fn ");
        self.write(&f.name.name);
        self.write("(");
        for (i, param) in f.params.iter().enumerate() {
            if i > 0 {
                self.write(", ");
            }
            self.write(&param.name.name);
            self.write(": ");
            self.format_type(&param.ty);
        }
        self.write(") -> ");
        self.format_type(&f.return_ty);
        if f.is_fallible {
            self.write(" fails");
        }
        self.write(" {\n");
        self.indent();
        self.format_block(&f.body);
        self.dedent();
        self.writeln("}");
    }

    fn format_agent_decl(&mut self, agent: &AgentDecl) {
        if agent.is_pub {
            self.write("pub ");
        }
        self.write("agent ");
        self.write(&agent.name.name);

        // Receives clause
        if let Some(recv) = &agent.receives {
            self.write(" receives ");
            self.format_type(recv);
        }

        // Tool uses
        if !agent.tool_uses.is_empty() {
            self.write(" uses ");
            for (i, tool) in agent.tool_uses.iter().enumerate() {
                if i > 0 {
                    self.write(", ");
                }
                self.write(&tool.name);
            }
        }

        self.write(" {\n");
        self.indent();

        // Beliefs (fields)
        for belief in &agent.beliefs {
            self.write_indent();
            self.write(&belief.name.name);
            self.write(": ");
            self.format_type(&belief.ty);
            self.write("\n");
        }

        // Blank line between fields and handlers if there are both
        if !agent.beliefs.is_empty() && !agent.handlers.is_empty() {
            self.newline();
        }

        // Sort handlers: on start, on message, on error, on stop
        let mut sorted_handlers: Vec<&HandlerDecl> = agent.handlers.iter().collect();
        sorted_handlers.sort_by_key(|h| match &h.event {
            EventKind::Start => 0,
            EventKind::Message { .. } => 1,
            EventKind::Error { .. } => 2,
            EventKind::Stop => 3,
        });

        for (i, handler) in sorted_handlers.iter().enumerate() {
            if i > 0 {
                self.newline();
            }
            self.format_handler(handler);
        }

        self.dedent();
        self.writeln("}");
    }

    fn format_handler(&mut self, handler: &HandlerDecl) {
        self.write_indent();
        match &handler.event {
            EventKind::Start => {
                self.write("on start {\n");
            }
            EventKind::Message { param_name, param_ty } => {
                self.write("on message(");
                self.write(&param_name.name);
                self.write(": ");
                self.format_type(param_ty);
                self.write(") {\n");
            }
            EventKind::Error { param_name } => {
                self.write("on error(");
                self.write(&param_name.name);
                self.write(": Error) {\n");
            }
            EventKind::Stop => {
                self.write("on stop {\n");
            }
        }
        self.indent();
        self.format_block(&handler.body);
        self.dedent();
        self.writeln("}");
    }

    fn format_test_decl(&mut self, t: &TestDecl) {
        self.write("test \"");
        for c in t.name.chars() {
            match c {
                '\\' => self.write("\\\\"),
                '"' => self.write("\\\""),
                '\n' => self.write("\\n"),
                _ => self.output.push(c),
            }
        }
        self.write("\" {\n");
        self.indent();
        self.format_block(&t.body);
        self.dedent();
        self.writeln("}");
    }

    fn format_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.format_stmt(stmt);
        }
    }

    fn format_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, ty, value, .. } => {
                self.write_indent();
                self.write("let ");
                self.write(&name.name);
                if let Some(t) = ty {
                    self.write(": ");
                    self.format_type(t);
                }
                self.write(" = ");
                self.format_expr(value);
                self.write(";\n");
            }
            Stmt::LetTuple { names, ty, value, .. } => {
                self.write_indent();
                self.write("let (");
                for (i, name) in names.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&name.name);
                }
                self.write(")");
                if let Some(t) = ty {
                    self.write(": ");
                    self.format_type(t);
                }
                self.write(" = ");
                self.format_expr(value);
                self.write(";\n");
            }
            Stmt::Assign { name, value, .. } => {
                self.write_indent();
                self.write(&name.name);
                self.write(" = ");
                self.format_expr(value);
                self.write(";\n");
            }
            Stmt::Expr { expr, .. } => {
                self.write_indent();
                self.format_expr(expr);
                self.write(";\n");
            }
            Stmt::Return { value, .. } => {
                self.write_indent();
                self.write("return");
                if let Some(v) = value {
                    self.write(" ");
                    self.format_expr(v);
                }
                self.write(";\n");
            }
            Stmt::While { condition, body, .. } => {
                self.write_indent();
                self.write("while ");
                self.format_expr(condition);
                self.write(" {\n");
                self.indent();
                self.format_block(body);
                self.dedent();
                self.writeln("}");
            }
            Stmt::For { pattern, iter, body, .. } => {
                self.write_indent();
                self.write("for ");
                self.format_pattern(pattern);
                self.write(" in ");
                self.format_expr(iter);
                self.write(" {\n");
                self.indent();
                self.format_block(body);
                self.dedent();
                self.writeln("}");
            }
            Stmt::Loop { body, .. } => {
                self.writeln("loop {");
                self.indent();
                self.format_block(body);
                self.dedent();
                self.writeln("}");
            }
            Stmt::Break { .. } => {
                self.writeln("break;");
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.write_indent();
                self.write("if ");
                self.format_expr(condition);
                self.write(" {\n");
                self.indent();
                self.format_block(then_block);
                self.dedent();
                self.write_indent();
                self.write("}");
                if let Some(else_b) = else_block {
                    self.write(" else ");
                    match else_b {
                        ElseBranch::ElseIf(if_stmt) => {
                            // Format the nested if without leading indent
                            if let Stmt::If {
                                condition,
                                then_block,
                                else_block,
                                ..
                            } = if_stmt.as_ref()
                            {
                                self.write("if ");
                                self.format_expr(condition);
                                self.write(" {\n");
                                self.indent();
                                self.format_block(then_block);
                                self.dedent();
                                self.write_indent();
                                self.write("}");
                                if let Some(eb) = else_block {
                                    self.format_else_branch(eb);
                                }
                            }
                        }
                        ElseBranch::Block(block) => {
                            self.write("{\n");
                            self.indent();
                            self.format_block(block);
                            self.dedent();
                            self.write_indent();
                            self.write("}");
                        }
                    }
                }
                self.write("\n");
            }
            Stmt::MockInfer { value, .. } => {
                self.write_indent();
                self.write("mock infer -> ");
                self.format_mock_value(value);
                self.write(";\n");
            }
        }
    }

    fn format_else_branch(&mut self, else_branch: &ElseBranch) {
        self.write(" else ");
        match else_branch {
            ElseBranch::ElseIf(if_stmt) => {
                if let Stmt::If {
                    condition,
                    then_block,
                    else_block,
                    ..
                } = if_stmt.as_ref()
                {
                    self.write("if ");
                    self.format_expr(condition);
                    self.write(" {\n");
                    self.indent();
                    self.format_block(then_block);
                    self.dedent();
                    self.write_indent();
                    self.write("}");
                    if let Some(eb) = else_block {
                        self.format_else_branch(eb);
                    }
                }
            }
            ElseBranch::Block(block) => {
                self.write("{\n");
                self.indent();
                self.format_block(block);
                self.dedent();
                self.write_indent();
                self.write("}");
            }
        }
    }

    fn format_mock_value(&mut self, value: &MockValue) {
        match value {
            MockValue::Value(expr) => self.format_expr(expr),
            MockValue::Fail(expr) => {
                self.write("fail(");
                self.format_expr(expr);
                self.write(")");
            }
        }
    }

    fn format_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Wildcard { .. } => self.write("_"),
            Pattern::Binding { name, .. } => self.write(&name.name),
            Pattern::Variant {
                enum_name,
                variant,
                payload,
                ..
            } => {
                if let Some(en) = enum_name {
                    self.write(&en.name);
                    self.write("::");
                }
                self.write(&variant.name);
                if let Some(p) = payload {
                    self.write("(");
                    self.format_pattern(p);
                    self.write(")");
                }
            }
            Pattern::Literal { value, .. } => self.format_literal(value),
            Pattern::Tuple { elements, .. } => {
                self.write("(");
                for (i, p) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_pattern(p);
                }
                self.write(")");
            }
        }
    }

    fn format_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal { value, .. } => self.format_literal(value),
            Expr::Var { name, .. } => self.write(&name.name),
            Expr::Binary { left, op, right, .. } => {
                self.format_expr(left);
                self.write(" ");
                self.write(&op.to_string());
                self.write(" ");
                self.format_expr(right);
            }
            Expr::Unary { op, operand, .. } => {
                self.write(&op.to_string());
                self.format_expr(operand);
            }
            Expr::Call { name, args, .. } => {
                self.write(&name.name);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(arg);
                }
                self.write(")");
            }
            Expr::SelfMethodCall { method, args, .. } => {
                self.write("self.");
                self.write(&method.name);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(arg);
                }
                self.write(")");
            }
            Expr::SelfField { field, .. } => {
                self.write("self.");
                self.write(&field.name);
            }
            Expr::FieldAccess { object, field, .. } => {
                self.format_expr(object);
                self.write(".");
                self.write(&field.name);
            }
            Expr::TupleIndex { tuple, index, .. } => {
                self.format_expr(tuple);
                self.write(".");
                let _ = write!(self.output, "{}", index);
            }
            Expr::List { elements, .. } => {
                self.write("[");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(elem);
                }
                self.write("]");
            }
            Expr::Tuple { elements, .. } => {
                self.write("(");
                for (i, elem) in elements.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(elem);
                }
                if elements.len() == 1 {
                    self.write(",");
                }
                self.write(")");
            }
            Expr::RecordConstruct { name, fields, .. } => {
                self.write(&name.name);
                self.write(" { ");
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&field.name.name);
                    self.write(": ");
                    self.format_expr(&field.value);
                }
                self.write(" }");
            }
            Expr::Map { entries, .. } => {
                self.write("{");
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(&entry.key);
                    self.write(": ");
                    self.format_expr(&entry.value);
                }
                self.write("}");
            }
            Expr::VariantConstruct {
                enum_name,
                variant,
                payload,
                ..
            } => {
                self.write(&enum_name.name);
                self.write(".");
                self.write(&variant.name);
                if let Some(p) = payload {
                    self.write("(");
                    self.format_expr(p);
                    self.write(")");
                }
            }
            Expr::Match { scrutinee, arms, .. } => {
                self.write("match ");
                self.format_expr(scrutinee);
                self.write(" {\n");
                self.indent();
                for arm in arms {
                    self.format_match_arm(arm);
                }
                self.dedent();
                self.write_indent();
                self.write("}");
            }
            Expr::Closure { params, body, .. } => {
                self.write("|");
                for (i, param) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.write(&param.name.name);
                    if let Some(ty) = &param.ty {
                        self.write(": ");
                        self.format_type(ty);
                    }
                }
                self.write("| ");
                self.format_expr(body);
            }
            Expr::Paren { inner, .. } => {
                self.write("(");
                self.format_expr(inner);
                self.write(")");
            }
            Expr::StringInterp { template, .. } => {
                self.format_string_template(template);
            }
            Expr::Spawn { agent, fields, .. } => {
                self.write("spawn ");
                self.write(&agent.name);
                if fields.is_empty() {
                    self.write(" {}");
                } else if fields.len() <= 2 {
                    self.write(" { ");
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        self.write(&field.name.name);
                        self.write(": ");
                        self.format_expr(&field.value);
                    }
                    self.write(" }");
                } else {
                    self.write(" {\n");
                    self.indent();
                    for field in fields {
                        self.write_indent();
                        self.write(&field.name.name);
                        self.write(": ");
                        self.format_expr(&field.value);
                        self.write(",\n");
                    }
                    self.dedent();
                    self.write_indent();
                    self.write("}");
                }
            }
            Expr::Await { handle, timeout, .. } => {
                self.write("await ");
                self.format_expr(handle);
                if let Some(t) = timeout {
                    self.write(" timeout(");
                    self.format_expr(t);
                    self.write(")");
                }
            }
            Expr::Emit { value, .. } => {
                self.write("emit(");
                self.format_expr(value);
                self.write(")");
            }
            Expr::Send { handle, message, .. } => {
                self.write("send(");
                self.format_expr(handle);
                self.write(", ");
                self.format_expr(message);
                self.write(")");
            }
            Expr::Receive { .. } => {
                self.write("receive()");
            }
            Expr::Infer {
                template,
                result_ty,
                ..
            } => {
                self.write("infer(");
                self.format_string_template(template);
                if let Some(ty) = result_ty {
                    self.write(" -> ");
                    self.format_type(ty);
                }
                self.write(")");
            }
            Expr::Try { expr, .. } => {
                self.write("try ");
                self.format_expr(expr);
            }
            Expr::Catch {
                expr,
                error_bind,
                recovery,
                ..
            } => {
                self.format_expr(expr);
                self.write(" catch");
                if let Some(name) = error_bind {
                    self.write("(");
                    self.write(&name.name);
                    self.write(")");
                }
                self.write(" { ");
                self.format_expr(recovery);
                self.write(" }");
            }
            Expr::Fail { error, .. } => {
                self.write("fail ");
                self.format_expr(error);
            }
            Expr::Retry {
                count, delay, body, ..
            } => {
                self.write("retry(");
                self.format_expr(count);
                if let Some(d) = delay {
                    self.write(", delay: ");
                    self.format_expr(d);
                }
                self.write(") {\n");
                self.indent();
                self.write_indent();
                self.format_expr(body);
                self.write("\n");
                self.dedent();
                self.write_indent();
                self.write("}");
            }
            Expr::Trace { message, .. } => {
                self.write("trace(");
                self.format_expr(message);
                self.write(")");
            }
            Expr::ToolCall {
                tool,
                function,
                args,
                ..
            } => {
                self.write(&tool.name);
                self.write(".");
                self.write(&function.name);
                self.write("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_expr(arg);
                }
                self.write(")");
            }
        }
    }

    fn format_string_template(&mut self, template: &StringTemplate) {
        self.write("\"");
        for part in &template.parts {
            match part {
                StringPart::Literal(s) => {
                    for c in s.chars() {
                        match c {
                            '\\' => self.write("\\\\"),
                            '"' => self.write("\\\""),
                            '\n' => self.write("\\n"),
                            '\r' => self.write("\\r"),
                            '\t' => self.write("\\t"),
                            '{' => self.write("{{"),
                            '}' => self.write("}}"),
                            _ => self.output.push(c),
                        }
                    }
                }
                StringPart::Interpolation(interp) => {
                    self.write("{");
                    self.format_interp_expr(interp);
                    self.write("}");
                }
            }
        }
        self.write("\"");
    }

    fn format_interp_expr(&mut self, interp: &InterpExpr) {
        match interp {
            InterpExpr::Ident(ident) => {
                self.write(&ident.name);
            }
            InterpExpr::FieldAccess { base, field, .. } => {
                self.format_interp_expr(base);
                self.write(".");
                self.write(&field.name);
            }
            InterpExpr::TupleIndex { base, index, .. } => {
                self.format_interp_expr(base);
                self.write(".");
                let _ = write!(self.output, "{}", index);
            }
        }
    }

    fn format_match_arm(&mut self, arm: &MatchArm) {
        self.write_indent();
        self.format_pattern(&arm.pattern);
        self.write(" => ");
        self.format_expr(&arm.body);
        self.write(",\n");
    }

    fn format_literal(&mut self, lit: &Literal) {
        match lit {
            Literal::Int(n) => {
                let _ = write!(self.output, "{}", n);
            }
            Literal::Float(f) => {
                let _ = write!(self.output, "{}", f);
            }
            Literal::String(s) => {
                self.write("\"");
                for c in s.chars() {
                    match c {
                        '\\' => self.write("\\\\"),
                        '"' => self.write("\\\""),
                        '\n' => self.write("\\n"),
                        '\r' => self.write("\\r"),
                        '\t' => self.write("\\t"),
                        _ => self.output.push(c),
                    }
                }
                self.write("\"");
            }
            Literal::Bool(b) => {
                self.write(if *b { "true" } else { "false" });
            }
        }
    }

    fn format_type(&mut self, ty: &TypeExpr) {
        match ty {
            TypeExpr::Int => self.write("Int"),
            TypeExpr::Float => self.write("Float"),
            TypeExpr::Bool => self.write("Bool"),
            TypeExpr::String => self.write("String"),
            TypeExpr::Unit => self.write("Unit"),
            TypeExpr::Error => self.write("Error"),
            TypeExpr::Named(name) => self.write(&name.name),
            TypeExpr::List(inner) => {
                self.write("List<");
                self.format_type(inner);
                self.write(">");
            }
            TypeExpr::Option(inner) => {
                self.write("Option<");
                self.format_type(inner);
                self.write(">");
            }
            TypeExpr::Result(ok, err) => {
                self.write("Result<");
                self.format_type(ok);
                self.write(", ");
                self.format_type(err);
                self.write(">");
            }
            TypeExpr::Tuple(types) => {
                self.write("(");
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_type(t);
                }
                self.write(")");
            }
            TypeExpr::Inferred(inner) => {
                self.write("Inferred<");
                self.format_type(inner);
                self.write(">");
            }
            TypeExpr::Fn(params, ret) => {
                self.write("Fn(");
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    self.format_type(p);
                }
                self.write(") -> ");
                self.format_type(ret);
            }
            TypeExpr::Map(key, value) => {
                self.write("Map<");
                self.format_type(key);
                self.write(", ");
                self.format_type(value);
                self.write(">");
            }
            TypeExpr::Agent(name) => {
                self.write("Agent<");
                self.write(&name.name);
                self.write(">");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lex, parse};
    use std::sync::Arc;

    fn format_source(source: &str) -> String {
        let lex_result = lex(source).unwrap();
        let source_arc: Arc<str> = Arc::from(source);
        let (program, errors) = parse(lex_result.tokens(), source_arc);
        assert!(errors.is_empty(), "Parse errors: {:?}", errors);
        let program = program.unwrap();
        format(&program)
    }

    #[test]
    fn test_format_simple_agent() {
        let source = r#"agent Greeter{name:String on start{print("Hello");emit(0);}}run Greeter;"#;
        let formatted = format_source(source);
        assert!(formatted.contains("agent Greeter {"));
        assert!(formatted.contains("    name: String"));
        assert!(formatted.contains("    on start {"));
    }

    #[test]
    fn test_format_binary_operators() {
        let source = r#"agent Test{on start{let x=1+2*3;emit(x);}}run Test;"#;
        let formatted = format_source(source);
        assert!(formatted.contains("1 + 2 * 3"));
    }
}
