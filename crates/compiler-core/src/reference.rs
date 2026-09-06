use crate::runtime::*;
use crate::{
    ast::{
        BinaryOp, ClassDecl, ClassMember, Decl, Expr, ExprKind, FunctionDecl, Item, MethodDecl,
        Program, Stmt,
    },
    diagnostic::{Diagnostic, Label},
    semantic::{SemanticResult, SymbolId, Type},
    source::Span,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

pub fn execute_reference(program: &Program, semantic: &SemanticResult) -> RuntimeResult {
    execute_reference_with_options(program, semantic, RuntimeLimits::default(), None)
}
pub fn execute_reference_with_options(
    program: &Program,
    semantic: &SemanticResult,
    limits: RuntimeLimits,
    cancelled: Option<Arc<AtomicBool>>,
) -> RuntimeResult {
    Interpreter::new(program, semantic, limits, cancelled).run()
}
#[derive(Clone, Debug, PartialEq)]
struct Object {
    class: String,
    fields: HashMap<String, Value>,
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
            ExprKind::Literal(literal) => literal_value(
                literal,
                self.semantic
                    .types
                    .get(&expression.id)
                    .unwrap_or(&Type::Int),
                expression.span,
            )?,
            ExprKind::Name(_) => self.value(expression)?,
            ExprKind::Unary { op, operand } => {
                let value = self.expression(operand)?;
                unary(*op, value, expression.span)?
            }
            ExprKind::Binary { left, op, right } => {
                let left = self.expression(left)?;
                match (op, &left) {
                    (BinaryOp::And, Value::Bool(false)) => Value::Bool(false),
                    (BinaryOp::Or, Value::Bool(true)) => Value::Bool(true),
                    (BinaryOp::And | BinaryOp::Or, Value::Bool(_)) => {
                        Value::Bool(self.boolean(right)?)
                    }
                    _ => {
                        let right = self.expression(right)?;
                        binary(left, *op, right, expression.span)?
                    }
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
            return builtin(
                &name,
                &values,
                &mut self.output,
                self.limits.output_bytes,
                span,
            );
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
}
