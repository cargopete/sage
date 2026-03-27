//! Tree-walking interpreter for Sage programs.
//!
//! Designed for the playground — handles the core language features needed
//! for interactive examples. Does not support: divine, tool calls, agent
//! spawning, supervisors, protocols.

use sage_parser::{BinOp, Block, Expr, Literal, Pattern, Program, Stmt, StringPart, UnaryOp};
use std::collections::HashMap;

/// Runtime value.
#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    List(Vec<Value>),
    Map(Vec<(Value, Value)>),
    Record(String, HashMap<String, Value>),
    Tuple(Vec<Value>),
    Unit,
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::String(_) => "String",
            Value::List(_) => "List",
            Value::Map(_) => "Map",
            Value::Record(name, _) => name,
            Value::Tuple(_) => "Tuple",
            Value::Unit => "Unit",
        }
    }

    pub fn to_display(&self) -> String {
        match self {
            Value::Int(n) => n.to_string(),
            Value::Float(f) => format!("{f}"),
            Value::Bool(b) => b.to_string(),
            Value::String(s) => s.clone(),
            Value::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_display()).collect();
                format!("[{}]", inner.join(", "))
            }
            Value::Map(entries) => {
                let inner: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k.to_display(), v.to_display()))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Record(name, fields) => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{k}: {}", v.to_display()))
                    .collect();
                format!("{name} {{ {} }}", inner.join(", "))
            }
            Value::Tuple(items) => {
                let inner: Vec<String> = items.iter().map(|v| v.to_display()).collect();
                format!("({})", inner.join(", "))
            }
            Value::Unit => "()".to_string(),
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Int(n) => *n != 0,
            Value::String(s) => !s.is_empty(),
            Value::Unit => false,
            _ => true,
        }
    }

    fn as_int(&self) -> Result<i64, InterpError> {
        match self {
            Value::Int(n) => Ok(*n),
            other => Err(InterpError::Type(format!("expected Int, got {}", other.type_name()))),
        }
    }

    fn as_float(&self) -> Result<f64, InterpError> {
        match self {
            Value::Float(f) => Ok(*f),
            Value::Int(n) => Ok(*n as f64),
            other => Err(InterpError::Type(format!("expected Float, got {}", other.type_name()))),
        }
    }

    fn as_string(&self) -> Result<&str, InterpError> {
        match self {
            Value::String(s) => Ok(s),
            other => Err(InterpError::Type(format!(
                "expected String, got {}",
                other.type_name()
            ))),
        }
    }

    fn as_bool(&self) -> Result<bool, InterpError> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(InterpError::Type(format!("expected Bool, got {}", other.type_name()))),
        }
    }
}

/// Interpreter error.
#[derive(Debug)]
pub enum InterpError {
    /// Type mismatch at runtime.
    Type(String),
    /// Undefined variable or function.
    Undefined(String),
    /// Break statement (control flow, not a real error).
    Break,
    /// Return statement with value.
    Return(Value),
    /// Yield — agent completed with a value.
    Yield(Value),
    /// Explicit fail expression.
    Fail(String),
    /// Feature not supported in playground.
    Unsupported(String),
    /// Runtime error.
    Runtime(String),
}

impl std::fmt::Display for InterpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterpError::Type(msg) => write!(f, "Type error: {msg}"),
            InterpError::Undefined(name) => write!(f, "Undefined: {name}"),
            InterpError::Break => write!(f, "break outside loop"),
            InterpError::Return(v) => write!(f, "return {}", v.to_display()),
            InterpError::Yield(v) => write!(f, "yield {}", v.to_display()),
            InterpError::Fail(msg) => write!(f, "Error: {msg}"),
            InterpError::Unsupported(msg) => write!(f, "Not supported in playground: {msg}"),
            InterpError::Runtime(msg) => write!(f, "Runtime error: {msg}"),
        }
    }
}

type InterpResult = Result<Value, InterpError>;

/// Variable scope (lexical, with parent chain).
struct Scope {
    vars: HashMap<String, Value>,
    parent: Option<Box<Scope>>,
}

impl Scope {
    fn new() -> Self {
        Self { vars: HashMap::new(), parent: None }
    }

    fn child(parent: Scope) -> Self {
        Self { vars: HashMap::new(), parent: Some(Box::new(parent)) }
    }

    fn get(&self, name: &str) -> Option<&Value> {
        self.vars.get(name).or_else(|| self.parent.as_ref().and_then(|p| p.get(name)))
    }

    fn set(&mut self, name: &str, value: Value) -> bool {
        if self.vars.contains_key(name) {
            self.vars.insert(name.to_string(), value);
            return true;
        }
        if let Some(ref mut parent) = self.parent {
            parent.set(name, value)
        } else {
            false
        }
    }

    fn define(&mut self, name: String, value: Value) {
        self.vars.insert(name, value);
    }

    fn into_parent(self) -> Scope {
        *self.parent.unwrap()
    }
}

/// The interpreter.
pub struct Interpreter {
    /// Collected output lines.
    pub output: Vec<String>,
    /// Function declarations.
    functions: HashMap<String, sage_parser::FnDecl>,
    /// Record declarations (for construction).
    records: HashMap<String, Vec<String>>,
    /// Enum declarations.
    enums: HashMap<String, Vec<(String, bool)>>,
    /// Constants.
    constants: HashMap<String, Value>,
    /// Step counter for infinite loop protection.
    steps: u64,
    max_steps: u64,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            output: Vec::new(),
            functions: HashMap::new(),
            records: HashMap::new(),
            enums: HashMap::new(),
            constants: HashMap::new(),
            steps: 0,
            max_steps: 100_000,
        }
    }

    /// Run a parsed program. Returns the yield value (or Unit).
    pub fn run(&mut self, program: &Program) -> Result<Value, InterpError> {
        // Register constants
        for c in &program.consts {
            let val = self.eval_const_literal(&c.value)?;
            self.constants.insert(c.name.name.clone(), val);
        }

        // Register records
        for r in &program.records {
            let fields: Vec<String> = r.fields.iter().map(|f| f.name.name.clone()).collect();
            self.records.insert(r.name.name.clone(), fields);
        }

        // Register enums
        for e in &program.enums {
            let variants: Vec<(String, bool)> = e
                .variants
                .iter()
                .map(|v| (v.name.name.clone(), v.payload.is_some()))
                .collect();
            self.enums.insert(e.name.name.clone(), variants);
        }

        // Register functions
        for f in &program.functions {
            self.functions.insert(f.name.name.clone(), f.clone());
        }

        // Find entry agent
        let entry_name = program
            .run_agent
            .as_ref()
            .ok_or_else(|| InterpError::Runtime("No 'run' entry point".to_string()))?;

        let agent = program
            .agents
            .iter()
            .find(|a| a.name.name == entry_name.name)
            .ok_or_else(|| InterpError::Undefined(format!("agent {}", entry_name.name)))?;

        // Find on_start handler
        let start_handler = agent
            .handlers
            .iter()
            .find(|h| matches!(h.event, sage_parser::EventKind::Start))
            .ok_or_else(|| {
                InterpError::Runtime(format!("Agent {} has no on start handler", agent.name.name))
            })?;

        // Execute on_start
        let mut scope = Scope::new();
        match self.exec_block(&start_handler.body, &mut scope) {
            Ok(val) => Ok(val),
            Err(InterpError::Yield(val)) => Ok(val),
            Err(InterpError::Return(val)) => Ok(val),
            Err(e) => Err(e),
        }
    }

    fn tick(&mut self) -> Result<(), InterpError> {
        self.steps += 1;
        if self.steps > self.max_steps {
            Err(InterpError::Runtime(
                "Execution limit exceeded (possible infinite loop)".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn exec_block(&mut self, block: &Block, scope: &mut Scope) -> InterpResult {
        let mut result = Value::Unit;
        for stmt in &block.stmts {
            result = self.exec_stmt(stmt, scope)?;
        }
        Ok(result)
    }

    fn exec_stmt(&mut self, stmt: &Stmt, scope: &mut Scope) -> InterpResult {
        self.tick()?;
        match stmt {
            Stmt::Let { name, value, .. } => {
                let val = self.eval_expr(value, scope)?;
                scope.define(name.name.clone(), val);
                Ok(Value::Unit)
            }

            Stmt::LetTuple { names, value, .. } => {
                let val = self.eval_expr(value, scope)?;
                if let Value::Tuple(items) = val {
                    for (i, name) in names.iter().enumerate() {
                        let v = items.get(i).cloned().unwrap_or(Value::Unit);
                        scope.define(name.name.clone(), v);
                    }
                } else {
                    return Err(InterpError::Type("let tuple requires tuple value".into()));
                }
                Ok(Value::Unit)
            }

            Stmt::Assign { name, value, .. } => {
                let val = self.eval_expr(value, scope)?;
                if !scope.set(&name.name, val.clone()) {
                    scope.define(name.name.clone(), val);
                }
                Ok(Value::Unit)
            }

            Stmt::If { condition, then_block, else_block, .. } => {
                let cond = self.eval_expr(condition, scope)?;
                if cond.is_truthy() {
                    self.exec_block(then_block, scope)
                } else if let Some(else_branch) = else_block {
                    match else_branch {
                        sage_parser::ElseBranch::Block(block) => self.exec_block(block, scope),
                        sage_parser::ElseBranch::ElseIf(stmt) => self.exec_stmt(stmt, scope),
                    }
                } else {
                    Ok(Value::Unit)
                }
            }

            Stmt::While { condition, body, .. } => {
                loop {
                    let cond = self.eval_expr(condition, scope)?;
                    if !cond.is_truthy() {
                        break;
                    }
                    match self.exec_block(body, scope) {
                        Ok(_) => {}
                        Err(InterpError::Break) => break,
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::Unit)
            }

            Stmt::Loop { body, .. } => {
                loop {
                    match self.exec_block(body, scope) {
                        Ok(_) => {}
                        Err(InterpError::Break) => break,
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::Unit)
            }

            Stmt::For { pattern, iter, body, .. } => {
                let iterable = self.eval_expr(iter, scope)?;
                let items = match iterable {
                    Value::List(items) => items,
                    Value::Map(entries) => entries
                        .into_iter()
                        .map(|(k, v)| Value::Tuple(vec![k, v]))
                        .collect(),
                    other => {
                        return Err(InterpError::Type(format!(
                            "cannot iterate over {}",
                            other.type_name()
                        )))
                    }
                };
                for item in items {
                    self.bind_pattern(pattern, &item, scope)?;
                    match self.exec_block(body, scope) {
                        Ok(_) => {}
                        Err(InterpError::Break) => break,
                        Err(e) => return Err(e),
                    }
                }
                Ok(Value::Unit)
            }

            Stmt::Break { .. } => Err(InterpError::Break),

            Stmt::Return { value, .. } => {
                let val = if let Some(expr) = value {
                    self.eval_expr(expr, scope)?
                } else {
                    Value::Unit
                };
                Err(InterpError::Return(val))
            }

            Stmt::Expr { expr, .. } => self.eval_expr(expr, scope),

            Stmt::SpanBlock { body, .. } => self.exec_block(body, scope),

            Stmt::Checkpoint { .. } => Ok(Value::Unit),

            Stmt::MockDivine { .. } | Stmt::MockTool { .. } => Ok(Value::Unit),
        }
    }

    fn eval_expr(&mut self, expr: &Expr, scope: &mut Scope) -> InterpResult {
        self.tick()?;
        match expr {
            Expr::Literal { value, .. } => Ok(self.eval_literal(value)),

            Expr::Var { name, .. } => {
                if let Some(val) = scope.get(&name.name) {
                    Ok(val.clone())
                } else if let Some(val) = self.constants.get(&name.name) {
                    Ok(val.clone())
                } else {
                    Err(InterpError::Undefined(name.name.clone()))
                }
            }

            Expr::Binary { op, left, right, .. } => {
                let l = self.eval_expr(left, scope)?;
                // Short-circuit for && and ||
                match op {
                    BinOp::And => {
                        if !l.is_truthy() {
                            return Ok(Value::Bool(false));
                        }
                        let r = self.eval_expr(right, scope)?;
                        return Ok(Value::Bool(r.is_truthy()));
                    }
                    BinOp::Or => {
                        if l.is_truthy() {
                            return Ok(Value::Bool(true));
                        }
                        let r = self.eval_expr(right, scope)?;
                        return Ok(Value::Bool(r.is_truthy()));
                    }
                    _ => {}
                }
                let r = self.eval_expr(right, scope)?;
                self.eval_binop(op, &l, &r)
            }

            Expr::Unary { op, operand, .. } => {
                let val = self.eval_expr(operand, scope)?;
                match op {
                    UnaryOp::Neg => match val {
                        Value::Int(n) => Ok(Value::Int(-n)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        other => Err(InterpError::Type(format!("cannot negate {}", other.type_name()))),
                    },
                    UnaryOp::Not => Ok(Value::Bool(!val.is_truthy())),
                }
            }

            Expr::Paren { inner, .. } => self.eval_expr(inner, scope),

            Expr::Call { name, args, .. } => {
                let arg_vals: Vec<Value> = args
                    .iter()
                    .map(|a| self.eval_expr(a, scope))
                    .collect::<Result<_, _>>()?;
                self.call_function(&name.name, arg_vals, scope)
            }

            Expr::Yield { value, .. } => {
                let val = self.eval_expr(value, scope)?;
                Err(InterpError::Yield(val))
            }

            Expr::List { elements, .. } => {
                let items: Vec<Value> = elements
                    .iter()
                    .map(|e| self.eval_expr(e, scope))
                    .collect::<Result<_, _>>()?;
                Ok(Value::List(items))
            }

            Expr::Map { entries, .. } => {
                let pairs: Vec<(Value, Value)> = entries
                    .iter()
                    .map(|e| {
                        let k = self.eval_expr(&e.key, scope)?;
                        let v = self.eval_expr(&e.value, scope)?;
                        Ok((k, v))
                    })
                    .collect::<Result<_, InterpError>>()?;
                Ok(Value::Map(pairs))
            }

            Expr::Tuple { elements, .. } => {
                let items: Vec<Value> = elements
                    .iter()
                    .map(|e| self.eval_expr(e, scope))
                    .collect::<Result<_, _>>()?;
                Ok(Value::Tuple(items))
            }

            Expr::TupleIndex { tuple, index, .. } => {
                let val = self.eval_expr(tuple, scope)?;
                if let Value::Tuple(items) = val {
                    items
                        .get(*index)
                        .cloned()
                        .ok_or_else(|| InterpError::Runtime(format!("tuple index {index} out of bounds")))
                } else {
                    Err(InterpError::Type("tuple index on non-tuple".into()))
                }
            }

            Expr::StringInterp { template, .. } => {
                let mut result = String::new();
                for part in &template.parts {
                    match part {
                        StringPart::Literal(s) => result.push_str(s),
                        StringPart::Interpolation(expr) => {
                            let val = self.eval_expr(expr, scope)?;
                            result.push_str(&val.to_display());
                        }
                    }
                }
                Ok(Value::String(result))
            }

            Expr::RecordConstruct { name, fields, .. } => {
                let mut field_map = HashMap::new();
                for f in fields {
                    let val = self.eval_expr(&f.value, scope)?;
                    field_map.insert(f.name.name.clone(), val);
                }
                Ok(Value::Record(name.name.clone(), field_map))
            }

            Expr::FieldAccess { object, field, .. } => {
                let val = self.eval_expr(object, scope)?;
                match val {
                    Value::Record(_, fields) => fields
                        .get(&field.name)
                        .cloned()
                        .ok_or_else(|| InterpError::Undefined(format!("field {}", field.name))),
                    Value::Map(entries) => {
                        let key = Value::String(field.name.clone());
                        for (k, v) in &entries {
                            if matches!(k, Value::String(s) if s == &field.name) {
                                return Ok(v.clone());
                            }
                        }
                        Err(InterpError::Undefined(format!("key {}", key.to_display())))
                    }
                    other => Err(InterpError::Type(format!(
                        "field access on {}",
                        other.type_name()
                    ))),
                }
            }

            Expr::SelfField { field, .. } => {
                // In playground, self fields are just scoped variables
                if let Some(val) = scope.get(&field.name) {
                    Ok(val.clone())
                } else {
                    Err(InterpError::Undefined(format!("self.{}", field.name)))
                }
            }

            Expr::Match { scrutinee, arms, .. } => {
                let val = self.eval_expr(scrutinee, scope)?;
                for arm in arms {
                    let mut arm_scope = Scope::child(std::mem::replace(scope, Scope::new()));
                    if self.match_pattern(&arm.pattern, &val, &mut arm_scope).is_ok() {
                        let result = self.eval_expr(&arm.body, &mut arm_scope)?;
                        *scope = arm_scope.into_parent();
                        return Ok(result);
                    }
                    *scope = arm_scope.into_parent();
                }
                Err(InterpError::Runtime("no match arm matched".into()))
            }

            Expr::VariantConstruct { variant, payload, .. } => {
                let val = if let Some(p) = payload {
                    self.eval_expr(p, scope)?
                } else {
                    Value::Unit
                };
                // Represent as a record with special __variant field
                let mut fields = HashMap::new();
                fields.insert("__variant".to_string(), Value::String(variant.name.clone()));
                if !matches!(val, Value::Unit) {
                    fields.insert("__payload".to_string(), val);
                }
                Ok(Value::Record(variant.name.clone(), fields))
            }

            Expr::Try { expr, .. } => {
                // In playground, try just evaluates the expression
                // If it fails, propagate the error
                match self.eval_expr(expr, scope) {
                    Ok(val) => Ok(val),
                    Err(InterpError::Fail(msg)) => Err(InterpError::Fail(msg)),
                    Err(e) => Err(e),
                }
            }

            Expr::Catch { expr, error_bind, recovery, .. } => {
                match self.eval_expr(expr, scope) {
                    Ok(val) => Ok(val),
                    Err(InterpError::Fail(msg)) => {
                        if let Some(bind) = error_bind {
                            scope.define(bind.name.clone(), Value::String(msg));
                        }
                        self.eval_expr(recovery, scope)
                    }
                    Err(e) => Err(e),
                }
            }

            Expr::Fail { error, .. } => {
                let val = self.eval_expr(error, scope)?;
                Err(InterpError::Fail(val.to_display()))
            }

            Expr::Trace { message, .. } => {
                let val = self.eval_expr(message, scope)?;
                self.output.push(format!("[trace] {}", val.to_display()));
                Ok(Value::Unit)
            }

            Expr::Closure { params, body, .. } => {
                // Closures are not fully supported but we handle simple cases
                // For map/filter stdlib calls
                let _ = (params, body);
                Err(InterpError::Unsupported("closures".into()))
            }

            Expr::Apply { callee, args, .. } => {
                // Method-style calls: expr.method(args)
                // Check if callee is a field access (like list.push())
                if let Expr::FieldAccess { object, field, .. } = callee.as_ref() {
                    let obj = self.eval_expr(object, scope)?;
                    let mut arg_vals: Vec<Value> = args
                        .iter()
                        .map(|a| self.eval_expr(a, scope))
                        .collect::<Result<_, _>>()?;
                    arg_vals.insert(0, obj);
                    return self.call_method(&field.name, arg_vals);
                }
                Err(InterpError::Unsupported("apply expression".into()))
            }

            Expr::Divine { .. } => {
                Err(InterpError::Unsupported("divine() — LLM calls require a server".into()))
            }

            Expr::Summon { .. } => {
                Err(InterpError::Unsupported("summon — agent spawning".into()))
            }

            Expr::Await { .. } => Err(InterpError::Unsupported("await".into())),
            Expr::Send { .. } => Err(InterpError::Unsupported("send".into())),
            Expr::Receive { .. } => Err(InterpError::Unsupported("receive".into())),
            Expr::Reply { .. } => Err(InterpError::Unsupported("reply".into())),

            Expr::ToolCall { tool, function, .. } => {
                Err(InterpError::Unsupported(format!(
                    "{}.{}() — tool calls require the full runtime",
                    tool.name, function.name
                )))
            }

            Expr::SelfMethodCall { method, .. } => {
                Err(InterpError::Unsupported(format!("self.{}()", method.name)))
            }

            Expr::Retry { body, count, .. } => {
                let max = self.eval_expr(count, scope)?.as_int()?;
                let mut last_err = None;
                for _ in 0..max {
                    match self.eval_expr(body, scope) {
                        Ok(val) => return Ok(val),
                        Err(e) => last_err = Some(e),
                    }
                }
                Err(last_err.unwrap_or(InterpError::Runtime("retry exhausted".into())))
            }
        }
    }

    fn eval_literal(&self, lit: &Literal) -> Value {
        match lit {
            Literal::Int(n) => Value::Int(*n),
            Literal::Float(f) => Value::Float(*f),
            Literal::Bool(b) => Value::Bool(*b),
            Literal::String(s) => Value::String(s.clone()),
        }
    }

    fn eval_const_literal(&self, expr: &Expr) -> InterpResult {
        match expr {
            Expr::Literal { value, .. } => Ok(self.eval_literal(value)),
            Expr::Unary { op: UnaryOp::Neg, operand, .. } => {
                let val = self.eval_const_literal(operand)?;
                match val {
                    Value::Int(n) => Ok(Value::Int(-n)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    _ => Err(InterpError::Runtime("invalid constant".into())),
                }
            }
            _ => Err(InterpError::Runtime("non-literal constant".into())),
        }
    }

    fn eval_binop(&self, op: &BinOp, left: &Value, right: &Value) -> InterpResult {
        match op {
            BinOp::Add => match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 + b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + *b as f64)),
                _ => Err(InterpError::Type(format!(
                    "cannot add {} and {}",
                    left.type_name(),
                    right.type_name()
                ))),
            },
            BinOp::Sub => match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 - b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - *b as f64)),
                _ => Err(InterpError::Type("cannot subtract".into())),
            },
            BinOp::Mul => match (left, right) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 * b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * *b as f64)),
                _ => Err(InterpError::Type("cannot multiply".into())),
            },
            BinOp::Div => match (left, right) {
                (Value::Int(a), Value::Int(b)) => {
                    if *b == 0 {
                        Err(InterpError::Runtime("division by zero".into()))
                    } else {
                        Ok(Value::Int(a / b))
                    }
                }
                (Value::Float(a), Value::Float(b)) => Ok(Value::Float(a / b)),
                (Value::Int(a), Value::Float(b)) => Ok(Value::Float(*a as f64 / b)),
                (Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / *b as f64)),
                _ => Err(InterpError::Type("cannot divide".into())),
            },
            BinOp::Rem => match (left, right) {
                (Value::Int(a), Value::Int(b)) => {
                    if *b == 0 {
                        Err(InterpError::Runtime("modulo by zero".into()))
                    } else {
                        Ok(Value::Int(a % b))
                    }
                }
                _ => Err(InterpError::Type("modulo requires Int".into())),
            },
            BinOp::Eq => Ok(Value::Bool(self.values_equal(left, right))),
            BinOp::Ne => Ok(Value::Bool(!self.values_equal(left, right))),
            BinOp::Lt => self.compare(left, right, |o| o == std::cmp::Ordering::Less),
            BinOp::Gt => self.compare(left, right, |o| o == std::cmp::Ordering::Greater),
            BinOp::Le => self.compare(left, right, |o| o != std::cmp::Ordering::Greater),
            BinOp::Ge => self.compare(left, right, |o| o != std::cmp::Ordering::Less),
            BinOp::Concat => {
                let l = left.to_display();
                let r = right.to_display();
                Ok(Value::String(format!("{l}{r}")))
            }
            BinOp::And | BinOp::Or => {
                // Handled in eval_expr for short-circuit
                unreachable!()
            }
        }
    }

    fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            _ => false,
        }
    }

    fn compare(&self, a: &Value, b: &Value, pred: impl Fn(std::cmp::Ordering) -> bool) -> InterpResult {
        let ord = match (a, b) {
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
            (Value::String(a), Value::String(b)) => a.cmp(b),
            _ => {
                return Err(InterpError::Type(format!(
                    "cannot compare {} and {}",
                    a.type_name(),
                    b.type_name()
                )))
            }
        };
        Ok(Value::Bool(pred(ord)))
    }

    fn call_function(&mut self, name: &str, args: Vec<Value>, _scope: &mut Scope) -> InterpResult {
        // Check stdlib first
        if let Some(result) = self.call_stdlib(name, &args)? {
            return Ok(result);
        }

        // User-defined functions
        if let Some(func) = self.functions.get(name).cloned() {
            let mut fn_scope = Scope::new();
            for (param, val) in func.params.iter().zip(args) {
                fn_scope.define(param.name.name.clone(), val);
            }
            // Copy constants into function scope
            for (k, v) in &self.constants {
                fn_scope.define(k.clone(), v.clone());
            }
            match self.exec_block(&func.body, &mut fn_scope) {
                Ok(val) => Ok(val),
                Err(InterpError::Return(val)) => Ok(val),
                Err(e) => Err(e),
            }
        } else {
            Err(InterpError::Undefined(format!("function {name}")))
        }
    }

    fn call_method(&mut self, name: &str, args: Vec<Value>) -> InterpResult {
        // Only handle common methods
        self.call_stdlib(name, &args)?
            .ok_or_else(|| InterpError::Undefined(format!("method {name}")))
    }

    /// Evaluate a standard library function. Returns Ok(None) if not a stdlib fn.
    fn call_stdlib(&mut self, name: &str, args: &[Value]) -> Result<Option<Value>, InterpError> {
        match name {
            "print" | "println" => {
                let msg = args.first().map(|v| v.to_display()).unwrap_or_default();
                self.output.push(msg);
                Ok(Some(Value::Unit))
            }
            "trace" => {
                let msg = args.first().map(|v| v.to_display()).unwrap_or_default();
                self.output.push(format!("[trace] {msg}"));
                Ok(Some(Value::Unit))
            }
            "int_to_str" => {
                let n = args.first().ok_or(InterpError::Runtime("int_to_str needs 1 arg".into()))?.as_int()?;
                Ok(Some(Value::String(n.to_string())))
            }
            "float_to_str" => {
                let f = args.first().ok_or(InterpError::Runtime("float_to_str needs 1 arg".into()))?.as_float()?;
                Ok(Some(Value::String(format!("{f}"))))
            }
            "str_to_int" => {
                let s = args.first().ok_or(InterpError::Runtime("str_to_int needs 1 arg".into()))?.as_string()?;
                let n = s.parse::<i64>().map_err(|e| InterpError::Fail(format!("str_to_int: {e}")))?;
                Ok(Some(Value::Int(n)))
            }
            "str_to_float" => {
                let s = args.first().ok_or(InterpError::Runtime("str_to_float needs 1 arg".into()))?.as_string()?;
                let f = s.parse::<f64>().map_err(|e| InterpError::Fail(format!("str_to_float: {e}")))?;
                Ok(Some(Value::Float(f)))
            }
            "len" => {
                let val = args.first().ok_or(InterpError::Runtime("len needs 1 arg".into()))?;
                let n = match val {
                    Value::String(s) => s.chars().count() as i64,
                    Value::List(l) => l.len() as i64,
                    Value::Map(m) => m.len() as i64,
                    other => return Err(InterpError::Type(format!("len() on {}", other.type_name()))),
                };
                Ok(Some(Value::Int(n)))
            }
            "contains" => {
                let first = args.get(0).ok_or(InterpError::Runtime("contains needs 2 args".into()))?;
                let second = args.get(1).ok_or(InterpError::Runtime("contains needs 2 args".into()))?;
                match first {
                    Value::String(s) => {
                        let sub = second.as_string()?;
                        Ok(Some(Value::Bool(s.contains(sub))))
                    }
                    Value::List(items) => {
                        let found = items.iter().any(|item| self.values_equal(item, second));
                        Ok(Some(Value::Bool(found)))
                    }
                    other => Err(InterpError::Type(format!(
                        "contains expects String or List, got {}",
                        other.type_name()
                    ))),
                }
            }
            "split" => {
                let s = args.get(0).ok_or(InterpError::Runtime("split needs 2 args".into()))?.as_string()?;
                let sep = args.get(1).ok_or(InterpError::Runtime("split needs 2 args".into()))?.as_string()?;
                let parts: Vec<Value> = if sep.is_empty() {
                    // Split into individual characters (match compiled Sage behaviour)
                    s.chars().map(|c| Value::String(c.to_string())).collect()
                } else {
                    s.split(sep).map(|p| Value::String(p.to_string())).collect()
                };
                Ok(Some(Value::List(parts)))
            }
            "trim" => {
                let s = args.first().ok_or(InterpError::Runtime("trim needs 1 arg".into()))?.as_string()?;
                Ok(Some(Value::String(s.trim().to_string())))
            }
            "to_upper" => {
                let s = args.first().ok_or(InterpError::Runtime("to_upper needs 1 arg".into()))?.as_string()?;
                Ok(Some(Value::String(s.to_uppercase())))
            }
            "to_lower" => {
                let s = args.first().ok_or(InterpError::Runtime("to_lower needs 1 arg".into()))?.as_string()?;
                Ok(Some(Value::String(s.to_lowercase())))
            }
            "push" => {
                if let (Some(Value::List(items)), Some(val)) = (args.first(), args.get(1)) {
                    let mut new_list = items.clone();
                    new_list.push(val.clone());
                    Ok(Some(Value::List(new_list)))
                } else {
                    Err(InterpError::Type("push(list, item)".into()))
                }
            }
            "get" => {
                let collection = args.first().ok_or(InterpError::Runtime("get needs 2 args".into()))?;
                let key = args.get(1).ok_or(InterpError::Runtime("get needs 2 args".into()))?;
                match (collection, key) {
                    (Value::List(items), Value::Int(i)) => {
                        let idx = *i as usize;
                        Ok(Some(items.get(idx).cloned().unwrap_or(Value::Unit)))
                    }
                    (Value::Map(entries), _) => {
                        for (k, v) in entries {
                            if self.values_equal(k, key) {
                                return Ok(Some(v.clone()));
                            }
                        }
                        Ok(Some(Value::Unit))
                    }
                    _ => Err(InterpError::Type("get() requires list/map".into())),
                }
            }
            "slice" => {
                let s = args.get(0).ok_or(InterpError::Runtime("slice needs 3 args".into()))?;
                let start = args.get(1).ok_or(InterpError::Runtime("slice needs 3 args".into()))?.as_int()? as usize;
                let end = args.get(2).ok_or(InterpError::Runtime("slice needs 3 args".into()))?.as_int()? as usize;
                match s {
                    Value::String(s) => {
                        let sliced: String = s.chars().skip(start).take(end - start).collect();
                        Ok(Some(Value::String(sliced)))
                    }
                    Value::List(items) => {
                        let sliced = items[start..end.min(items.len())].to_vec();
                        Ok(Some(Value::List(sliced)))
                    }
                    _ => Err(InterpError::Type("slice() requires String or List".into())),
                }
            }
            "join" => {
                let list = args.get(0).ok_or(InterpError::Runtime("join needs 2 args".into()))?;
                let sep = args.get(1).ok_or(InterpError::Runtime("join needs 2 args".into()))?.as_string()?;
                if let Value::List(items) = list {
                    let parts: Vec<String> = items.iter().map(|v| v.to_display()).collect();
                    Ok(Some(Value::String(parts.join(sep))))
                } else {
                    Err(InterpError::Type("join() requires List".into()))
                }
            }
            "abs" => {
                let val = args.first().ok_or(InterpError::Runtime("abs needs 1 arg".into()))?;
                match val {
                    Value::Int(n) => Ok(Some(Value::Int(n.abs()))),
                    Value::Float(f) => Ok(Some(Value::Float(f.abs()))),
                    _ => Err(InterpError::Type("abs() requires Int or Float".into())),
                }
            }
            "min" => {
                let a = args.get(0).ok_or(InterpError::Runtime("min needs 2 args".into()))?.as_int()?;
                let b = args.get(1).ok_or(InterpError::Runtime("min needs 2 args".into()))?.as_int()?;
                Ok(Some(Value::Int(a.min(b))))
            }
            "max" => {
                let a = args.get(0).ok_or(InterpError::Runtime("max needs 2 args".into()))?.as_int()?;
                let b = args.get(1).ok_or(InterpError::Runtime("max needs 2 args".into()))?.as_int()?;
                Ok(Some(Value::Int(a.max(b))))
            }
            "range" => {
                let end = args.first().ok_or(InterpError::Runtime("range needs 1 arg".into()))?.as_int()?;
                let items: Vec<Value> = (0..end).map(Value::Int).collect();
                Ok(Some(Value::List(items)))
            }
            "chr" => {
                let n = args.first().ok_or(InterpError::Runtime("chr needs 1 arg".into()))?.as_int()?;
                let c = char::from_u32(n as u32).unwrap_or('\u{FFFD}');
                Ok(Some(Value::String(c.to_string())))
            }
            "json_escape" => {
                let s = args.first().ok_or(InterpError::Runtime("json_escape needs 1 arg".into()))?.as_string()?;
                let mut out = String::with_capacity(s.len() + 16);
                for c in s.chars() {
                    match c {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        c if (c as u32) < 0x20 => {
                            out.push_str(&format!("\\u{:04x}", c as u32));
                        }
                        c => out.push(c),
                    }
                }
                Ok(Some(Value::String(out)))
            }
            "str_truncate" => {
                let s = args.get(0).ok_or(InterpError::Runtime("str_truncate needs 2 args".into()))?.as_string()?;
                let max_len = args.get(1).ok_or(InterpError::Runtime("str_truncate needs 2 args".into()))?.as_int()?;
                let max = max_len.max(0) as usize;
                let char_count = s.chars().count();
                let result = if char_count <= max {
                    s.to_string()
                } else {
                    let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
                    format!("{}...", truncated)
                };
                Ok(Some(Value::String(result)))
            }
            "env" => {
                // In WASM playground, env vars are not available — always returns None/Unit
                Ok(Some(Value::Unit))
            }
            "env_or" => {
                // In WASM playground, env vars are not available — always returns default
                let default = args.get(1).ok_or(InterpError::Runtime("env_or needs 2 args".into()))?.as_string()?;
                Ok(Some(Value::String(default.to_string())))
            }
            _ => Ok(None),
        }
    }

    fn bind_pattern(&self, pattern: &Pattern, value: &Value, scope: &mut Scope) -> Result<(), InterpError> {
        match pattern {
            Pattern::Binding { name, .. } => {
                scope.define(name.name.clone(), value.clone());
                Ok(())
            }
            Pattern::Wildcard { .. } => Ok(()),
            Pattern::Tuple { elements, .. } => {
                if let Value::Tuple(items) = value {
                    for (pat, val) in elements.iter().zip(items) {
                        self.bind_pattern(pat, val, scope)?;
                    }
                    Ok(())
                } else {
                    Err(InterpError::Type("tuple pattern on non-tuple".into()))
                }
            }
            _ => Ok(()),
        }
    }

    fn match_pattern(&self, pattern: &Pattern, value: &Value, scope: &mut Scope) -> Result<(), InterpError> {
        match pattern {
            Pattern::Wildcard { .. } => Ok(()),
            Pattern::Binding { name, .. } => {
                scope.define(name.name.clone(), value.clone());
                Ok(())
            }
            Pattern::Literal { value: lit, .. } => {
                let lit_val = self.eval_literal(lit);
                if self.values_equal(&lit_val, value) {
                    Ok(())
                } else {
                    Err(InterpError::Runtime("pattern mismatch".into()))
                }
            }
            Pattern::Variant { variant, payload, .. } => {
                if let Value::Record(name, fields) = value {
                    if let Some(Value::String(v)) = fields.get("__variant") {
                        if v == &variant.name {
                            if let Some(pat) = payload {
                                if let Some(pl) = fields.get("__payload") {
                                    return self.match_pattern(pat, pl, scope);
                                }
                            }
                            return Ok(());
                        }
                    }
                    // Also match by record name for simple enum variants
                    if name == &variant.name {
                        return Ok(());
                    }
                }
                Err(InterpError::Runtime("variant mismatch".into()))
            }
            Pattern::Tuple { elements, .. } => {
                if let Value::Tuple(items) = value {
                    if elements.len() != items.len() {
                        return Err(InterpError::Runtime("tuple size mismatch".into()));
                    }
                    for (pat, val) in elements.iter().zip(items) {
                        self.match_pattern(pat, val, scope)?;
                    }
                    Ok(())
                } else {
                    Err(InterpError::Runtime("tuple pattern on non-tuple".into()))
                }
            }
        }
    }
}
