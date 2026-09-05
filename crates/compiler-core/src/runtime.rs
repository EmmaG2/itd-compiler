use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};

use crate::bytecode::{Chunk, Instruction, compile, validate};
use crate::{
    ast::{
        BinaryOp, ClassDecl, ClassMember, Decl, Expr, ExprKind, FunctionDecl, Item, Literal,
        MethodDecl, Program, Stmt, UnaryOp,
    },
    diagnostic::{Diagnostic, Label},
    semantic::{SemanticResult, SymbolId, Type},
    source::Span,
};

const MAX_CALL_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLimits {
    pub instructions: usize,
    pub call_depth: usize,
    pub heap_objects: usize,
    pub output_bytes: usize,
    pub wall_time_ms: u64,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            instructions: 1_000_000,
            call_depth: MAX_CALL_DEPTH,
            heap_objects: 10_000,
            output_bytes: 1_048_576,
            wall_time_ms: 2_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Decimal(Decimal),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(String),
    Char(char),
    Object(usize),
    Class(String),
    Void,
}

#[derive(Clone, Debug, PartialEq)]
struct Object {
    class: String,
    fields: HashMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeResult {
    pub output: String,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn execute(program: &Program, semantic: &SemanticResult) -> RuntimeResult {
    match compile(program) {
        Ok(chunk) => execute_chunk(&chunk, semantic, RuntimeLimits::default(), None),
        Err(diagnostic) => RuntimeResult {
            output: String::new(),
            diagnostics: vec![diagnostic],
        },
    }
}

pub fn execute_reference(program: &Program, semantic: &SemanticResult) -> RuntimeResult {
    Interpreter::new(program, semantic, RuntimeLimits::default(), None).run()
}

pub fn execute_chunk(
    chunk: &Chunk,
    semantic: &SemanticResult,
    limits: RuntimeLimits,
    cancelled: Option<Arc<AtomicBool>>,
) -> RuntimeResult {
    if let Err(diagnostic) = validate(chunk) {
        return RuntimeResult {
            output: String::new(),
            diagnostics: vec![diagnostic],
        };
    }
    let mut items = Vec::new();
    for instruction in &chunk.code {
        match instruction {
            Instruction::Execute(index) => items.push(chunk.constants[*index as usize].clone()),
            Instruction::Halt => break,
        }
    }
    Interpreter::new(&Program { items }, semantic, limits, cancelled).run()
}

enum Flow {
    Next,
    Return(Value),
    Break,
    Continue,
}

struct Interpreter<'a> {
    program: &'a Program,
    semantic: &'a SemanticResult,
    scopes: Vec<HashMap<SymbolId, Value>>,
    functions: HashMap<SymbolId, &'a FunctionDecl>,
    classes: HashMap<String, &'a ClassDecl>,
    heap: Vec<Object>,
    static_fields: HashMap<(String, String), Value>,
    output: String,
    diagnostics: Vec<Diagnostic>,
    call_depth: usize,
    remaining: usize,
    limits: RuntimeLimits,
    cancelled: Option<Arc<AtomicBool>>,
    started: Instant,
}

impl<'a> Interpreter<'a> {
    fn new(
        program: &'a Program,
        semantic: &'a SemanticResult,
        limits: RuntimeLimits,
        cancelled: Option<Arc<AtomicBool>>,
    ) -> Self {
        let mut functions = HashMap::new();
        let mut classes = HashMap::new();
        for item in &program.items {
            let function = match item {
                Item::Decl(Decl::Function(function)) | Item::Export(Decl::Function(function)) => {
                    function
                }
                Item::Class(class) | Item::ExportClass(class) => {
                    classes.insert(class.name.clone(), class);
                    continue;
                }
                _ => continue,
            };
            if let Some(symbol) = semantic.symbols.iter().find(|symbol| {
                symbol.name == function.name && symbol.declaration_span == function.name_span
            }) {
                functions.insert(symbol.id, function);
            }
        }
        Self {
            program,
            semantic,
            scopes: vec![HashMap::new()],
            functions,
            classes,
            heap: Vec::new(),
            static_fields: HashMap::new(),
            output: String::new(),
            diagnostics: Vec::new(),
            call_depth: 0,
            remaining: limits.instructions,
            limits,
            cancelled,
            started: Instant::now(),
        }
    }

    fn run(mut self) -> RuntimeResult {
        for item in &self.program.items {
            let result = match item {
                Item::Decl(Decl::Variable(decl)) | Item::Export(Decl::Variable(decl)) => {
                    self.variable(decl).map(|()| Flow::Next)
                }
                Item::Class(class) | Item::ExportClass(class) => {
                    self.initialize_statics(class).map(|()| Flow::Next)
                }
                Item::Stmt(statement) => self.statement(statement),
                _ => Ok(Flow::Next),
            };
            if let Err(diagnostic) = result {
                self.diagnostics.push(diagnostic);
                break;
            }
        }
        RuntimeResult {
            output: self.output,
            diagnostics: self.diagnostics,
        }
    }

    fn statement(&mut self, statement: &Stmt) -> Result<Flow, Diagnostic> {
        self.tick(statement.span())?;
        match statement {
            Stmt::Variable(decl) => self.variable(decl).map(|()| Flow::Next),
            Stmt::Block { statements, .. } => {
                self.scopes.push(HashMap::new());
                let result = (|| {
                    for statement in statements {
                        let flow = self.statement(statement)?;
                        if !matches!(flow, Flow::Next) {
                            return Ok(flow);
                        }
                    }
                    Ok(Flow::Next)
                })();
                self.scopes.pop();
                result
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                if self.boolean(condition)? {
                    self.statement(then_branch)
                } else if let Some(branch) = else_branch {
                    self.statement(branch)
                } else {
                    Ok(Flow::Next)
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                while self.boolean(condition)? {
                    match self.statement(body)? {
                        Flow::Break => break,
                        Flow::Continue | Flow::Next => {}
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
                Ok(Flow::Next)
            }
            Stmt::Break { .. } => Ok(Flow::Break),
            Stmt::Continue { .. } => Ok(Flow::Continue),
            Stmt::Return { value, .. } => Ok(Flow::Return(
                value
                    .as_ref()
                    .map_or(Ok(Value::Void), |value| self.expression(value))?,
            )),
            Stmt::Expression { expression, .. } => {
                self.expression(expression)?;
                Ok(Flow::Next)
            }
        }
    }

    fn variable(&mut self, declaration: &crate::ast::VariableDecl) -> Result<(), Diagnostic> {
        let value = self.expression(&declaration.initializer)?;
        let Some(symbol) = self.semantic.symbols.iter().find(|symbol| {
            symbol.name == declaration.name && symbol.declaration_span == declaration.name_span
        }) else {
            return Err(error(
                "E4000",
                "variable sin resolución semántica",
                declaration.span,
            ));
        };
        self.scopes
            .last_mut()
            .expect("runtime always has a scope")
            .insert(symbol.id, value);
        Ok(())
    }

    fn boolean(&mut self, expression: &Expr) -> Result<bool, Diagnostic> {
        match self.expression(expression)? {
            Value::Bool(value) => Ok(value),
            _ => Err(error("E4000", "la condición no es bool", expression.span)),
        }
    }

    fn expression(&mut self, expression: &Expr) -> Result<Value, Diagnostic> {
        self.tick(expression.span)?;
        let value = match &expression.kind {
            ExprKind::Literal(literal) => self.literal(literal, expression)?,
            ExprKind::Name(_) => self.value(expression)?,
            ExprKind::Unary { op, operand } => {
                let value = self.expression(operand)?;
                self.unary(*op, value, expression.span)?
            }
            ExprKind::Binary { left, op, right } => {
                if *op == BinaryOp::And && !self.boolean(left)? {
                    Value::Bool(false)
                } else if *op == BinaryOp::Or && self.boolean(left)? {
                    Value::Bool(true)
                } else {
                    let left = self.expression(left)?;
                    let right = self.expression(right)?;
                    self.binary(left, *op, right, expression.span)?
                }
            }
            ExprKind::Assign { target, value } => {
                let value = self.expression(value)?;
                if let ExprKind::Member { object, name } = &target.kind {
                    let receiver = self.expression(object)?;
                    self.set_member(receiver, name, value.clone(), target.span)?;
                    return Ok(value);
                }
                let id = self.resolution(target)?;
                let Some(slot) = self
                    .scopes
                    .iter_mut()
                    .rev()
                    .find_map(|scope| scope.get_mut(&id))
                else {
                    return Err(error("E4000", "variable no inicializada", target.span));
                };
                *slot = value.clone();
                value
            }
            ExprKind::Call { callee, arguments } => {
                self.call(callee, arguments, expression.span)?
            }
            ExprKind::Cast {
                expression: value, ..
            } => {
                let value = self.expression(value)?;
                let target = self
                    .semantic
                    .types
                    .get(&expression.id)
                    .unwrap_or(&Type::Error);
                cast(value, target, expression.span)?
            }
            ExprKind::Member { object, name } => {
                let receiver = self.expression(object)?;
                self.get_member(receiver, name, expression.span)?
            }
            ExprKind::Index { .. } => {
                return Err(error(
                    "E4000",
                    "expresión no ejecutable todavía",
                    expression.span,
                ));
            }
        };
        if let Some(target) = self.semantic.coercions.get(&expression.id) {
            return cast(value, target, expression.span);
        }
        if let Some(target @ (Type::Byte | Type::Short | Type::Int | Type::Long)) =
            self.semantic.types.get(&expression.id)
        {
            return cast(value, target, expression.span);
        }
        Ok(value)
    }

    fn literal(&self, literal: &Literal, expression: &Expr) -> Result<Value, Diagnostic> {
        match literal {
            Literal::Bool(value) => Ok(Value::Bool(*value)),
            Literal::String(value) => Ok(Value::Str(unquote(value))),
            Literal::Char(value) => Ok(Value::Char(unquote(value).chars().next().unwrap_or('\0'))),
            Literal::Number(value) => match self.semantic.types.get(&expression.id) {
                Some(Type::Decimal) => Decimal::from_str_exact(value.trim_end_matches("dec"))
                    .map(Value::Decimal)
                    .map_err(|_| {
                        error(
                            "E4001",
                            "decimal fuera del rango representable",
                            expression.span,
                        )
                    }),
                Some(Type::Float) => value
                    .trim_end_matches("f32")
                    .parse()
                    .map(Value::Float)
                    .map_err(|_| error("E4001", "float inválido", expression.span)),
                Some(Type::Double) => value
                    .trim_end_matches("f64")
                    .parse()
                    .map(Value::Double)
                    .map_err(|_| error("E4001", "double inválido", expression.span)),
                ty => {
                    let value = value
                        .trim_end_matches("i8")
                        .trim_end_matches("i16")
                        .trim_end_matches("i32")
                        .trim_end_matches("i64")
                        .parse()
                        .map(Value::Int)
                        .map_err(|_| error("E4001", "entero fuera de rango", expression.span))?;
                    cast(value, ty.unwrap_or(&Type::Int), expression.span)
                }
            },
        }
    }

    fn value(&self, expression: &Expr) -> Result<Value, Diagnostic> {
        let id = self.resolution(expression)?;
        if let Type::ClassObject(name) = &self.semantic.symbols[id.0].ty {
            return Ok(Value::Class(name.clone()));
        }
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&id).cloned())
            .ok_or_else(|| error("E4000", "valor no disponible", expression.span))
    }

    fn resolution(&self, expression: &Expr) -> Result<SymbolId, Diagnostic> {
        self.semantic
            .resolutions
            .get(&expression.id)
            .copied()
            .ok_or_else(|| error("E4000", "nombre sin resolución semántica", expression.span))
    }

    fn call(&mut self, callee: &Expr, arguments: &[Expr], span: Span) -> Result<Value, Diagnostic> {
        if let ExprKind::Member { object, name } = &callee.kind {
            let receiver = self.expression(object)?;
            let values = arguments
                .iter()
                .map(|value| self.expression(value))
                .collect::<Result<Vec<_>, _>>()?;
            return self.call_method(receiver, name, values, span);
        }
        let id = self.resolution(callee)?;
        let name = self.semantic.symbols[id.0].name.clone();
        let values = arguments
            .iter()
            .map(|argument| self.expression(argument))
            .collect::<Result<Vec<_>, _>>()?;
        if let Type::ClassObject(class) = &self.semantic.symbols[id.0].ty {
            return self.construct(class.clone(), values, span);
        }
        if !self.functions.contains_key(&id) {
            return self.builtin(&name, &values, span);
        }
        let Some(function) = self.functions.get(&id).copied() else {
            return Err(error("E4000", "función no disponible", span));
        };
        if self.call_depth >= self.limits.call_depth {
            return Err(error("E4004", "límite de recursión excedido", span));
        }
        self.call_depth += 1;
        let mut frame = HashMap::new();
        for (parameter, value) in function.parameters.iter().zip(values) {
            let Some(symbol) = self.semantic.symbols.iter().find(|symbol| {
                symbol.name == parameter.name && symbol.declaration_span == parameter.span
            }) else {
                self.call_depth -= 1;
                return Err(error(
                    "E4000",
                    "parámetro sin resolución semántica",
                    parameter.span,
                ));
            };
            frame.insert(symbol.id, value);
        }
        self.scopes.push(frame);
        let result = self.statement(&function.body).map_err(|mut diagnostic| {
            diagnostic.labels.push(Label {
                span: function.name_span,
                message: format!("en llamada a {}", function.name),
            });
            diagnostic
        });
        self.scopes.pop();
        self.call_depth -= 1;
        match result? {
            Flow::Return(value) => Ok(value),
            _ => Ok(Value::Void),
        }
    }

    fn initialize_statics(&mut self, class: &ClassDecl) -> Result<(), Diagnostic> {
        for member in &class.members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            if field.is_static {
                let value = self.expression(&field.variable.initializer)?;
                self.static_fields
                    .insert((class.name.clone(), field.variable.name.clone()), value);
            }
        }
        Ok(())
    }

    fn construct(
        &mut self,
        class_name: String,
        arguments: Vec<Value>,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        if self.heap.len() >= self.limits.heap_objects {
            return Err(error("E5005", "límite de objetos excedido", span));
        }
        let Some(class) = self.classes.get(&class_name).copied() else {
            return Err(error("E4000", "clase no disponible", span));
        };
        let mut fields = HashMap::new();
        for member in &class.members {
            if let ClassMember::Field(field) = member
                && !field.is_static
            {
                fields.insert(
                    field.variable.name.clone(),
                    self.expression(&field.variable.initializer)?,
                );
            }
        }
        let handle = self.heap.len();
        self.heap.push(Object {
            class: class_name,
            fields,
        });
        if let Some(method) = class.members.iter().find_map(|value| match value {
            ClassMember::Constructor(value) => Some(value),
            _ => None,
        }) {
            self.invoke(method, Some(handle), arguments, span)?;
        } else if !arguments.is_empty() {
            return Err(error("E4000", "la clase no acepta argumentos", span));
        }
        Ok(Value::Object(handle))
    }

    fn call_method(
        &mut self,
        receiver: Value,
        name: &str,
        arguments: Vec<Value>,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        let (class_name, object) = match receiver {
            Value::Object(handle) => (
                self.heap
                    .get(handle)
                    .ok_or_else(|| error("E4000", "objeto inválido", span))?
                    .class
                    .clone(),
                Some(handle),
            ),
            Value::Class(name) => (name, None),
            _ => return Err(error("E4000", "el valor no tiene métodos", span)),
        };
        let class = self
            .classes
            .get(&class_name)
            .copied()
            .ok_or_else(|| error("E4000", "clase no disponible", span))?;
        let method = class
            .members
            .iter()
            .find_map(|value| match value {
                ClassMember::Method(value)
                    if value.function.name == name && value.is_static == object.is_none() =>
                {
                    Some(value)
                }
                _ => None,
            })
            .ok_or_else(|| error("E4000", "método no disponible", span))?;
        self.invoke(method, object, arguments, span)
    }

    fn invoke(
        &mut self,
        method: &MethodDecl,
        object: Option<usize>,
        arguments: Vec<Value>,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        if self.call_depth >= self.limits.call_depth {
            return Err(error("E4004", "límite de recursión excedido", span));
        }
        let mut frame = HashMap::new();
        if let Some(handle) = object {
            let symbol = self
                .semantic
                .symbols
                .iter()
                .find(|value| {
                    value.name == "this" && value.declaration_span == method.function.name_span
                })
                .ok_or_else(|| error("E4000", "`this` sin resolución semántica", span))?;
            frame.insert(symbol.id, Value::Object(handle));
        }
        for (parameter, value) in method.function.parameters.iter().zip(arguments) {
            let symbol = self
                .semantic
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.name == parameter.name && symbol.declaration_span == parameter.span
                })
                .ok_or_else(|| error("E4000", "parámetro sin resolución semántica", span))?;
            frame.insert(symbol.id, value);
        }
        self.call_depth += 1;
        self.scopes.push(frame);
        let result = self
            .statement(&method.function.body)
            .map_err(|mut diagnostic| {
                diagnostic.labels.push(Label {
                    span: method.function.name_span,
                    message: format!("en llamada a {}", method.function.name),
                });
                diagnostic
            });
        self.scopes.pop();
        self.call_depth -= 1;
        match result? {
            Flow::Return(value) => Ok(value),
            _ => Ok(Value::Void),
        }
    }

    fn get_member(&self, receiver: Value, name: &str, span: Span) -> Result<Value, Diagnostic> {
        match receiver {
            Value::Object(handle) => self
                .heap
                .get(handle)
                .and_then(|value| value.fields.get(name))
                .cloned()
                .ok_or_else(|| error("E4000", "campo inválido", span)),
            Value::Class(class) => self
                .static_fields
                .get(&(class, name.to_owned()))
                .cloned()
                .ok_or_else(|| error("E4000", "campo estático inválido", span)),
            _ => Err(error("E4000", "el valor no tiene campos", span)),
        }
    }

    fn set_member(
        &mut self,
        receiver: Value,
        name: &str,
        value: Value,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match receiver {
            Value::Object(handle) => {
                let field = self
                    .heap
                    .get_mut(handle)
                    .and_then(|object| object.fields.get_mut(name))
                    .ok_or_else(|| error("E4000", "campo inválido", span))?;
                *field = value;
                Ok(())
            }
            Value::Class(class) => {
                let field = self
                    .static_fields
                    .get_mut(&(class, name.to_owned()))
                    .ok_or_else(|| error("E4000", "campo estático inválido", span))?;
                *field = value;
                Ok(())
            }
            _ => Err(error("E4000", "el valor no tiene campos", span)),
        }
    }

    fn tick(&mut self, span: Span) -> Result<(), Diagnostic> {
        if self
            .cancelled
            .as_ref()
            .is_some_and(|value| value.load(Ordering::Relaxed))
        {
            return Err(error("E5004", "ejecución cancelada", span));
        }
        if self.started.elapsed().as_millis() > u128::from(self.limits.wall_time_ms) {
            return Err(error("E5007", "límite de tiempo excedido", span));
        }
        let Some(remaining) = self.remaining.checked_sub(1) else {
            return Err(error("E5003", "límite de instrucciones excedido", span));
        };
        self.remaining = remaining;
        Ok(())
    }

    fn builtin(&mut self, name: &str, values: &[Value], span: Span) -> Result<Value, Diagnostic> {
        match (name, values) {
            ("print", [Value::Str(value)]) => {
                if self.output.len().saturating_add(value.len() + 1) > self.limits.output_bytes {
                    return Err(error("E5006", "límite de salida excedido", span));
                }
                self.output.push_str(value);
                self.output.push('\n');
                Ok(Value::Void)
            }
            ("round", [Value::Decimal(value), Value::Int(scale)]) => {
                let scale = u32::try_from(*scale)
                    .ok()
                    .filter(|scale| *scale <= Decimal::MAX_SCALE)
                    .ok_or_else(|| error("E4005", "escala decimal inválida", span))?;
                Ok(Value::Decimal(value.round_dp_with_strategy(
                    scale,
                    RoundingStrategy::MidpointNearestEven,
                )))
            }
            ("scale", [Value::Decimal(value)]) => Ok(Value::Int(i64::from(value.scale()))),
            ("to_string", [Value::Decimal(value)]) => Ok(Value::Str(value.to_string())),
            _ => Err(error("E4000", "llamada nativa inválida", span)),
        }
    }

    fn unary(&self, op: UnaryOp, value: Value, span: Span) -> Result<Value, Diagnostic> {
        match (op, value) {
            (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
            (
                UnaryOp::Plus,
                value @ (Value::Int(_) | Value::Decimal(_) | Value::Float(_) | Value::Double(_)),
            ) => Ok(value),
            (UnaryOp::Minus, Value::Int(value)) => value
                .checked_neg()
                .map(Value::Int)
                .ok_or_else(|| overflow(span)),
            (UnaryOp::Minus, Value::Decimal(value)) => Decimal::ZERO
                .checked_sub(value)
                .map(Value::Decimal)
                .ok_or_else(|| overflow(span)),
            (UnaryOp::Minus, Value::Float(value)) => Ok(Value::Float(-value)),
            (UnaryOp::Minus, Value::Double(value)) => Ok(Value::Double(-value)),
            _ => Err(error("E4000", "operador unario inválido", span)),
        }
    }

    fn binary(
        &self,
        left: Value,
        op: BinaryOp,
        right: Value,
        span: Span,
    ) -> Result<Value, Diagnostic> {
        match (left, right) {
            (Value::Int(left), Value::Int(right)) => integer(left, op, right, span),
            (Value::Decimal(left), Value::Decimal(right)) => decimal(left, op, right, span),
            (Value::Float(left), Value::Float(right)) if comparison(op) => {
                compare(left, op, right, span)
            }
            (Value::Float(_), Value::Float(0.0))
                if matches!(op, BinaryOp::Divide | BinaryOp::Remainder) =>
            {
                Err(error("E4003", "división por cero", span))
            }
            (Value::Float(left), Value::Float(right)) if op == BinaryOp::Power => {
                let result = left.powf(right);
                result
                    .is_finite()
                    .then_some(Value::Float(result))
                    .ok_or_else(|| overflow(span))
            }
            (Value::Float(left), Value::Float(right)) => {
                let result = float(left, op, right, span)?;
                result
                    .is_finite()
                    .then_some(Value::Float(result))
                    .ok_or_else(|| overflow(span))
            }
            (Value::Double(left), Value::Double(right)) if comparison(op) => {
                compare(left, op, right, span)
            }
            (Value::Double(_), Value::Double(0.0))
                if matches!(op, BinaryOp::Divide | BinaryOp::Remainder) =>
            {
                Err(error("E4003", "división por cero", span))
            }
            (Value::Double(left), Value::Double(right)) if op == BinaryOp::Power => {
                let result = left.powf(right);
                result
                    .is_finite()
                    .then_some(Value::Double(result))
                    .ok_or_else(|| overflow(span))
            }
            (Value::Double(left), Value::Double(right)) => {
                let result = float(left, op, right, span)?;
                result
                    .is_finite()
                    .then_some(Value::Double(result))
                    .ok_or_else(|| overflow(span))
            }
            (Value::Bool(left), Value::Bool(right)) => compare(left, op, right, span),
            (Value::Str(left), Value::Str(right)) if op == BinaryOp::Add => {
                Ok(Value::Str(left + &right))
            }
            (Value::Str(left), Value::Str(right)) => compare(left, op, right, span),
            (Value::Char(left), Value::Char(right)) => compare(left, op, right, span),
            (Value::Object(left), Value::Object(right))
                if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) =>
            {
                compare(left, op, right, span)
            }
            _ => Err(error("E4000", "operandos incompatibles", span)),
        }
    }
}

fn integer(left: i64, op: BinaryOp, right: i64, span: Span) -> Result<Value, Diagnostic> {
    let value = match op {
        BinaryOp::Add => left.checked_add(right),
        BinaryOp::Subtract => left.checked_sub(right),
        BinaryOp::Multiply => left.checked_mul(right),
        BinaryOp::Divide => left.checked_div(right),
        BinaryOp::Remainder => left.checked_rem(right),
        BinaryOp::Power => u32::try_from(right)
            .ok()
            .and_then(|right| left.checked_pow(right)),
        _ => return compare(left, op, right, span),
    };
    value
        .map(Value::Int)
        .ok_or_else(|| arithmetic_error(right, span))
}

fn decimal(left: Decimal, op: BinaryOp, right: Decimal, span: Span) -> Result<Value, Diagnostic> {
    let value = match op {
        BinaryOp::Add => left.checked_add(right),
        BinaryOp::Subtract => left.checked_sub(right),
        BinaryOp::Multiply => left.checked_mul(right),
        BinaryOp::Divide => left.checked_div(right),
        BinaryOp::Remainder => left.checked_rem(right),
        BinaryOp::Power => return decimal_power(left, right, span).map(Value::Decimal),
        _ => return compare(left, op, right, span),
    };
    value.map(Value::Decimal).ok_or_else(|| {
        if right.is_zero() && matches!(op, BinaryOp::Divide | BinaryOp::Remainder) {
            error("E4003", "división por cero", span)
        } else {
            overflow(span)
        }
    })
}

fn decimal_power(base: Decimal, exponent: Decimal, span: Span) -> Result<Decimal, Diagnostic> {
    let exponent = exponent
        .to_i32()
        .filter(|value| Decimal::from(*value) == exponent)
        .ok_or_else(|| error("E4000", "el exponente decimal debe ser entero", span))?;
    let mut power = exponent.unsigned_abs();
    let mut factor = base;
    let mut result = Decimal::ONE;
    while power > 0 {
        if power % 2 == 1 {
            result = result.checked_mul(factor).ok_or_else(|| overflow(span))?;
        }
        power /= 2;
        if power > 0 {
            factor = factor.checked_mul(factor).ok_or_else(|| overflow(span))?;
        }
    }
    if exponent < 0 {
        Decimal::ONE
            .checked_div(result)
            .ok_or_else(|| arithmetic_error_decimal(result, span))
    } else {
        Ok(result)
    }
}

fn float<T>(left: T, op: BinaryOp, right: T, span: Span) -> Result<T, Diagnostic>
where
    T: Copy
        + PartialEq
        + PartialOrd
        + std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>
        + std::ops::Rem<Output = T>,
{
    match op {
        BinaryOp::Add => Ok(left + right),
        BinaryOp::Subtract => Ok(left - right),
        BinaryOp::Multiply => Ok(left * right),
        BinaryOp::Divide => Ok(left / right),
        BinaryOp::Remainder => Ok(left % right),
        _ => Err(error("E4000", "operación float inválida", span)),
    }
}

fn compare<T: PartialEq + PartialOrd>(
    left: T,
    op: BinaryOp,
    right: T,
    span: Span,
) -> Result<Value, Diagnostic> {
    Ok(Value::Bool(match op {
        BinaryOp::Equal => left == right,
        BinaryOp::NotEqual => left != right,
        BinaryOp::Less => left < right,
        BinaryOp::LessEqual => left <= right,
        BinaryOp::Greater => left > right,
        BinaryOp::GreaterEqual => left >= right,
        BinaryOp::And | BinaryOp::Or => {
            return Err(error("E4000", "operación lógica inválida", span));
        }
        _ => return Err(error("E4000", "comparación inválida", span)),
    }))
}

fn cast(value: Value, target: &Type, span: Span) -> Result<Value, Diagnostic> {
    match (value, target) {
        (value, Type::Error) => Ok(value),
        (Value::Int(value), Type::Byte) => i8::try_from(value)
            .map(|value| Value::Int(i64::from(value)))
            .map_err(|_| precision(span)),
        (Value::Int(value), Type::Short) => i16::try_from(value)
            .map(|value| Value::Int(i64::from(value)))
            .map_err(|_| precision(span)),
        (Value::Int(value), Type::Int) => i32::try_from(value)
            .map(|value| Value::Int(i64::from(value)))
            .map_err(|_| precision(span)),
        (value @ Value::Int(_), Type::Long) => Ok(value),
        (Value::Int(value), Type::Decimal) => Ok(Value::Decimal(Decimal::from(value))),
        (Value::Int(value), Type::Float) if value.unsigned_abs() <= (1_u64 << 24) => {
            Ok(Value::Float(value as f32))
        }
        (Value::Int(value), Type::Double) if value.unsigned_abs() <= (1_u64 << 53) => {
            Ok(Value::Double(value as f64))
        }
        (Value::Decimal(value), Type::Byte | Type::Short | Type::Int | Type::Long) => value
            .to_i64()
            .filter(|integer| Decimal::from(*integer) == value)
            .map(Value::Int)
            .and_then(|value| cast(value, target, span).ok())
            .ok_or_else(|| precision(span)),
        (Value::Decimal(value), Type::Float) => {
            let converted = value
                .to_string()
                .parse::<f32>()
                .map_err(|_| precision(span))?;
            (Decimal::from_f32_retain(converted) == Some(value))
                .then_some(Value::Float(converted))
                .ok_or_else(|| precision(span))
        }
        (Value::Decimal(value), Type::Double) => {
            let converted = value
                .to_string()
                .parse::<f64>()
                .map_err(|_| precision(span))?;
            (Decimal::from_f64_retain(converted) == Some(value))
                .then_some(Value::Double(converted))
                .ok_or_else(|| precision(span))
        }
        (Value::Float(value), Type::Decimal) => Decimal::from_f32_retain(value)
            .map(Value::Decimal)
            .ok_or_else(|| precision(span)),
        (Value::Double(value), Type::Decimal) => Decimal::from_f64_retain(value)
            .map(Value::Decimal)
            .ok_or_else(|| precision(span)),
        (Value::Float(value), Type::Double) => Ok(Value::Double(f64::from(value))),
        (Value::Double(value), Type::Float) if f64::from(value as f32) == value => {
            Ok(Value::Float(value as f32))
        }
        (Value::Float(value), Type::Byte | Type::Short | Type::Int | Type::Long)
            if value.is_finite() && value.fract() == 0.0 =>
        {
            let integer = value as i64;
            (integer as f32 == value)
                .then(|| cast(Value::Int(integer), target, span))
                .transpose()?
                .ok_or_else(|| precision(span))
        }
        (Value::Double(value), Type::Byte | Type::Short | Type::Int | Type::Long)
            if value.is_finite() && value.fract() == 0.0 =>
        {
            let integer = value as i64;
            (integer as f64 == value)
                .then(|| cast(Value::Int(integer), target, span))
                .transpose()?
                .ok_or_else(|| precision(span))
        }
        (value, ty) if value_type(&value) == *ty => Ok(value),
        _ => Err(error("E4005", "cast fuera de rango o con pérdida", span)),
    }
}

fn value_type(value: &Value) -> Type {
    match value {
        Value::Int(_) => Type::Long,
        Value::Decimal(_) => Type::Decimal,
        Value::Float(_) => Type::Float,
        Value::Double(_) => Type::Double,
        Value::Bool(_) => Type::Bool,
        Value::Str(_) => Type::Str,
        Value::Char(_) => Type::Char,
        Value::Object(_) | Value::Class(_) => Type::Error,
        Value::Void => Type::Void,
    }
}

fn comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
    )
}

fn unquote(value: &str) -> String {
    value[1..value.len().saturating_sub(1)]
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\t", "\t")
        .replace("\\0", "\0")
        .replace("\\\"", "\"")
        .replace("\\'", "'")
        .replace("\\\\", "\\")
}

fn arithmetic_error(divisor: i64, span: Span) -> Diagnostic {
    if divisor == 0 {
        error("E4003", "división por cero", span)
    } else {
        overflow(span)
    }
}

fn arithmetic_error_decimal(divisor: Decimal, span: Span) -> Diagnostic {
    if divisor.is_zero() {
        error("E4003", "división por cero", span)
    } else {
        overflow(span)
    }
}

fn overflow(span: Span) -> Diagnostic {
    error("E4002", "resultado numérico fuera de rango", span)
}

fn precision(span: Span) -> Diagnostic {
    error("E4005", "conversión con pérdida de precisión", span)
}

fn error(code: &str, message: &str, span: Span) -> Diagnostic {
    Diagnostic::error(code, message, span)
}
