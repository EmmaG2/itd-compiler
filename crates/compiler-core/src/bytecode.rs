use std::collections::HashMap;

pub use crate::bytecode_validate::validate;
use crate::{
    ast::{
        BinaryOp, ClassDecl, ClassMember, Decl, Expr, ExprKind, FunctionDecl, Item, Program, Stmt,
        UnaryOp, VariableDecl,
    },
    diagnostic::{Diagnostic, Severity},
    runtime::{Value, error, literal_value},
    semantic::{SemanticResult, SymbolId, Type},
    source::{SourceId, Span},
};

#[derive(Clone, Debug, PartialEq)]
pub enum Instruction {
    Constant(usize),
    LoadLocal(usize),
    StoreLocal(usize),
    LoadGlobal(usize),
    StoreGlobal(usize),
    Pop,
    Dup,
    Unary(UnaryOp),
    Binary(BinaryOp),
    Cast(Type),
    Jump(usize),
    JumpIfFalse(usize),
    JumpIfTrue(usize),
    Call(usize),
    BindMethod(usize),
    GetField(usize),
    SetField(usize),
    GetStatic(usize, usize),
    SetStatic(usize, usize),
    Trap(Diagnostic),
    Return,
    Halt,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub span: Span,
    pub code: Vec<Instruction>,
    pub spans: Vec<Span>,
    pub parameters: usize,
    pub locals: usize,
    pub receiver: bool,
    pub constructor: Option<usize>,
}
impl Function {
    fn new(name: String, span: Span) -> Self {
        Self {
            name,
            span,
            code: Vec::new(),
            spans: Vec::new(),
            parameters: 0,
            locals: 0,
            receiver: false,
            constructor: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Class {
    pub name: String,
    pub fields: usize,
    pub statics: usize,
    pub constructor: usize,
    pub methods: HashMap<usize, usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chunk {
    pub functions: Vec<Function>,
    pub constants: Vec<Value>,
    pub classes: Vec<Class>,
    pub globals: usize,
    pub method_names: Vec<String>,
}

pub fn compile(program: &Program, semantic: &SemanticResult) -> Result<Chunk, Diagnostic> {
    if let Some(diagnostic) = semantic
        .diagnostics
        .iter()
        .find(|d| d.severity == Severity::Error)
    {
        return Err(diagnostic.clone());
    }
    Compiler::new(semantic).compile(program)
}

struct Loop {
    start: usize,
    breaks: Vec<usize>,
}
struct Compiler<'a> {
    semantic: &'a SemanticResult,
    chunk: Chunk,
    globals: HashMap<SymbolId, usize>,
    functions: HashMap<SymbolId, usize>,
    classes: HashMap<String, usize>,
    fields: HashMap<(String, String), usize>,
    methods: HashMap<(String, String), usize>,
    locals: HashMap<SymbolId, usize>,
    current: usize,
    loops: Vec<Loop>,
}
impl<'a> Compiler<'a> {
    fn new(semantic: &'a SemanticResult) -> Self {
        Self {
            semantic,
            chunk: Chunk {
                functions: vec![Function::new("<main>".into(), Span::new(SourceId(0), 0, 0))],
                constants: Vec::new(),
                classes: Vec::new(),
                globals: 0,
                method_names: Vec::new(),
            },
            globals: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            fields: HashMap::new(),
            methods: HashMap::new(),
            locals: HashMap::new(),
            current: 0,
            loops: Vec::new(),
        }
    }
    fn symbol(&self, name: &str, span: Span) -> Result<SymbolId, Diagnostic> {
        self.semantic
            .symbols
            .iter()
            .find(|s| s.name == name && s.declaration_span == span)
            .map(|s| s.id)
            .ok_or_else(|| error("E5001", "declaración sin resolución", span))
    }
    fn resolution(&self, expr: &Expr) -> Result<SymbolId, Diagnostic> {
        self.semantic
            .resolutions
            .get(&expr.id)
            .copied()
            .ok_or_else(|| error("E5001", "expresión sin resolución", expr.span))
    }
    fn reserve(&mut self, name: String, span: Span) -> usize {
        let id = self.chunk.functions.len();
        self.chunk.functions.push(Function::new(name, span));
        id
    }
    fn method_id(&mut self, name: &str) -> usize {
        if let Some(id) = self.chunk.method_names.iter().position(|s| s == name) {
            return id;
        }
        let id = self.chunk.method_names.len();
        self.chunk.method_names.push(name.into());
        id
    }
    fn compile(mut self, program: &Program) -> Result<Chunk, Diagnostic> {
        for item in &program.items {
            match item {
                Item::Decl(Decl::Variable(v)) | Item::Export(Decl::Variable(v)) => {
                    let id = self.symbol(&v.name, v.name_span)?;
                    self.globals.insert(id, self.globals.len());
                }
                Item::Decl(Decl::Function(f)) | Item::Export(Decl::Function(f)) => {
                    let symbol = self.symbol(&f.name, f.name_span)?;
                    let id = self.reserve(f.name.clone(), f.name_span);
                    self.functions.insert(symbol, id);
                }
                Item::Class(c) | Item::ExportClass(c) => {
                    let id = self.chunk.classes.len();
                    self.classes.insert(c.name.clone(), id);
                    let constructor = self.reserve(format!("{}.this", c.name), c.name_span);
                    let mut class = Class {
                        name: c.name.clone(),
                        fields: 0,
                        statics: 0,
                        constructor,
                        methods: HashMap::new(),
                    };
                    for member in &c.members {
                        match member {
                            ClassMember::Field(f) => {
                                let count = if f.is_static {
                                    &mut class.statics
                                } else {
                                    &mut class.fields
                                };
                                self.fields
                                    .insert((c.name.clone(), f.variable.name.clone()), *count);
                                *count += 1;
                            }
                            ClassMember::Method(m) => {
                                let function = self.reserve(
                                    format!("{}.{}", c.name, m.function.name),
                                    m.function.name_span,
                                );
                                self.methods
                                    .insert((c.name.clone(), m.function.name.clone()), function);
                                if !m.is_static {
                                    let method = self.method_id(&m.function.name);
                                    class.methods.insert(method, function);
                                }
                            }
                            ClassMember::Constructor(_) => {}
                        }
                    }
                    self.chunk.classes.push(class);
                }
                _ => {}
            }
        }
        self.chunk.globals = self.globals.len();
        for item in &program.items {
            match item {
                Item::Decl(Decl::Variable(v)) | Item::Export(Decl::Variable(v)) => {
                    self.variable(v)?
                }
                Item::Stmt(s) => self.statement(s)?,
                Item::Class(c) | Item::ExportClass(c) => {
                    let id = self.classes[&c.name];
                    for member in &c.members {
                        if let ClassMember::Field(f) = member
                            && f.is_static
                        {
                            self.expression(&f.variable.initializer)?;
                            self.emit(
                                Instruction::SetStatic(
                                    id,
                                    self.fields[&(c.name.clone(), f.variable.name.clone())],
                                ),
                                f.variable.span,
                            );
                            self.emit(Instruction::Pop, f.variable.span);
                        }
                    }
                }
                _ => {}
            }
        }
        self.emit(
            Instruction::Halt,
            self.chunk.functions[0]
                .spans
                .last()
                .copied()
                .unwrap_or(self.chunk.functions[0].span),
        );
        for item in &program.items {
            match item {
                Item::Decl(Decl::Function(f)) | Item::Export(Decl::Function(f)) => {
                    let id = self.functions[&self.symbol(&f.name, f.name_span)?];
                    self.function(id, f, false, None)?;
                }
                Item::Class(c) | Item::ExportClass(c) => {
                    let id = self.classes[&c.name];
                    let constructor = self.chunk.classes[id].constructor;
                    let explicit = c.members.iter().find_map(|m| {
                        if let ClassMember::Constructor(m) = m {
                            Some(&m.function)
                        } else {
                            None
                        }
                    });
                    let implicit = FunctionDecl {
                        name: "this".into(),
                        name_span: c.name_span,
                        parameters: Vec::new(),
                        return_type: None,
                        body: Stmt::Block {
                            statements: Vec::new(),
                            span: c.span,
                        },
                        span: c.span,
                    };
                    self.function(
                        constructor,
                        explicit.unwrap_or(&implicit),
                        explicit.is_some(),
                        Some(c),
                    )?;
                    for member in &c.members {
                        if let ClassMember::Method(m) = member {
                            self.function(
                                self.methods[&(c.name.clone(), m.function.name.clone())],
                                &m.function,
                                !m.is_static,
                                None,
                            )?;
                        }
                    }
                }
                _ => {}
            }
        }
        validate(&self.chunk)?;
        Ok(self.chunk)
    }
    fn function(
        &mut self,
        id: usize,
        f: &FunctionDecl,
        receiver: bool,
        class: Option<&ClassDecl>,
    ) -> Result<(), Diagnostic> {
        self.current = id;
        self.locals.clear();
        self.loops.clear();
        self.chunk.functions[id].parameters = f.parameters.len();
        self.chunk.functions[id].receiver = receiver || class.is_some();
        self.chunk.functions[id].constructor = class.map(|c| self.classes[&c.name]);
        if receiver {
            let symbol = self.symbol("this", f.name_span)?;
            self.locals.insert(symbol, 0);
        }
        self.chunk.functions[id].locals = usize::from(receiver || class.is_some());
        for param in &f.parameters {
            let symbol = self.symbol(&param.name, param.span)?;
            self.local(symbol);
        }
        if let Some(class) = class {
            for member in &class.members {
                if let ClassMember::Field(field) = member
                    && !field.is_static
                {
                    self.expression(&field.variable.initializer)?;
                    self.emit(Instruction::LoadLocal(0), field.variable.span);
                    self.emit(
                        Instruction::SetField(
                            self.fields[&(class.name.clone(), field.variable.name.clone())],
                        ),
                        field.variable.span,
                    );
                    self.emit(Instruction::Pop, field.variable.span);
                }
            }
        }
        self.statement(&f.body)?;
        self.return_value(None, f.span)?;
        Ok(())
    }
    fn local(&mut self, id: SymbolId) -> usize {
        if let Some(slot) = self.locals.get(&id) {
            return *slot;
        }
        let slot = self.chunk.functions[self.current].locals;
        self.chunk.functions[self.current].locals += 1;
        self.locals.insert(id, slot);
        slot
    }
    fn emit(&mut self, instruction: Instruction, span: Span) -> usize {
        let f = &mut self.chunk.functions[self.current];
        let offset = f.code.len();
        f.code.push(instruction);
        f.spans.push(span);
        offset
    }
    fn constant(&mut self, value: Value, span: Span) {
        let id = self.chunk.constants.len();
        self.chunk.constants.push(value);
        self.emit(Instruction::Constant(id), span);
    }
    fn offset(&self) -> usize {
        self.chunk.functions[self.current].code.len()
    }
    fn patch(&mut self, offset: usize) {
        let target = self.offset();
        match &mut self.chunk.functions[self.current].code[offset] {
            Instruction::Jump(t) | Instruction::JumpIfFalse(t) | Instruction::JumpIfTrue(t) => {
                *t = target
            }
            _ => unreachable!("only generated jumps are patched"),
        }
    }
    fn variable(&mut self, v: &VariableDecl) -> Result<(), Diagnostic> {
        self.expression(&v.initializer)?;
        let symbol = self.symbol(&v.name, v.name_span)?;
        let instruction = if let Some(slot) = self.globals.get(&symbol) {
            Instruction::StoreGlobal(*slot)
        } else {
            Instruction::StoreLocal(self.local(symbol))
        };
        self.emit(instruction, v.span);
        Ok(())
    }
    fn return_value(&mut self, expr: Option<&Expr>, span: Span) -> Result<(), Diagnostic> {
        if self.chunk.functions[self.current].constructor.is_some() {
            if let Some(expr) = expr {
                self.expression(expr)?;
                self.emit(Instruction::Pop, span);
            }
            self.emit(Instruction::LoadLocal(0), span);
        } else if let Some(expr) = expr {
            self.expression(expr)?;
        } else {
            self.constant(Value::Void, span);
        }
        self.emit(Instruction::Return, span);
        Ok(())
    }
    fn statement(&mut self, stmt: &Stmt) -> Result<(), Diagnostic> {
        let span = stmt.span();
        match stmt {
            Stmt::Variable(v) => self.variable(v)?,
            Stmt::Block { statements, .. } => {
                for stmt in statements {
                    self.statement(stmt)?;
                }
            }
            Stmt::Expression { expression, .. } => {
                self.expression(expression)?;
                self.emit(Instruction::Pop, span);
            }
            Stmt::Return { value, .. } => self.return_value(value.as_ref(), span)?,
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(condition)?;
                let otherwise = self.emit(Instruction::JumpIfFalse(0), condition.span);
                self.statement(then_branch)?;
                let end = self.emit(Instruction::Jump(0), span);
                self.patch(otherwise);
                if let Some(branch) = else_branch {
                    self.statement(branch)?;
                }
                self.patch(end);
            }
            Stmt::While {
                condition, body, ..
            } => {
                let start = self.offset();
                self.expression(condition)?;
                let end = self.emit(Instruction::JumpIfFalse(0), condition.span);
                self.loops.push(Loop {
                    start,
                    breaks: Vec::new(),
                });
                self.statement(body)?;
                self.emit(Instruction::Jump(start), span);
                self.patch(end);
                for jump in self.loops.pop().expect("compiler loop").breaks {
                    self.patch(jump);
                }
            }
            Stmt::Break { .. } => {
                let jump = self.emit(Instruction::Jump(0), span);
                self.loops
                    .last_mut()
                    .ok_or_else(|| error("E5001", "break sin ciclo", span))?
                    .breaks
                    .push(jump);
            }
            Stmt::Continue { .. } => {
                let start = self
                    .loops
                    .last()
                    .ok_or_else(|| error("E5001", "continue sin ciclo", span))?
                    .start;
                self.emit(Instruction::Jump(start), span);
            }
        }
        Ok(())
    }
    fn load(&mut self, expr: &Expr) -> Result<(), Diagnostic> {
        let id = self.resolution(expr)?;
        if let Some(function) = self.functions.get(&id) {
            self.constant(Value::Function(*function), expr.span);
        } else if let Some(slot) = self.globals.get(&id) {
            self.emit(Instruction::LoadGlobal(*slot), expr.span);
        } else if let Some(slot) = self.locals.get(&id) {
            self.emit(Instruction::LoadLocal(*slot), expr.span);
        } else {
            let symbol = &self.semantic.symbols[id.0];
            match &symbol.ty {
                Type::ClassObject(name) => {
                    let class = *self
                        .classes
                        .get(name)
                        .ok_or_else(|| error("E5001", "clase sin enlace", expr.span))?;
                    self.constant(Value::Constructor(class), expr.span);
                }
                Type::Function(_, _)
                    if ["print", "round", "scale", "to_string"].contains(&symbol.name.as_str()) =>
                {
                    self.constant(Value::Native(symbol.name.clone()), expr.span)
                }
                _ => return Err(error("E5001", "símbolo sin almacenamiento", expr.span)),
            }
        }
        Ok(())
    }
    fn member(
        &mut self,
        object: &Expr,
        name: &str,
        span: Span,
        store: bool,
    ) -> Result<(), Diagnostic> {
        let ty = self
            .semantic
            .types
            .get(&object.id)
            .ok_or_else(|| error("E5001", "miembro sin tipo", span))?;
        if let Type::ClassObject(class) = ty {
            let class = class.clone();
            let id = self.classes[&class];
            if let Some(field) = self.fields.get(&(class.clone(), name.into())).copied() {
                self.emit(
                    if store {
                        Instruction::SetStatic(id, field)
                    } else {
                        Instruction::GetStatic(id, field)
                    },
                    span,
                );
            } else if !store {
                let f = self
                    .methods
                    .get(&(class, name.into()))
                    .copied()
                    .ok_or_else(|| error("E5001", "método sin enlace", span))?;
                self.constant(Value::Function(f), span);
            } else {
                return Err(error("E5001", "destino inválido", span));
            }
            return Ok(());
        }
        let field = if let Type::Class(class) = ty {
            self.fields.get(&(class.clone(), name.into())).copied()
        } else {
            None
        };
        self.expression(object)?;
        if let Some(field) = field {
            self.emit(
                if store {
                    Instruction::SetField(field)
                } else {
                    Instruction::GetField(field)
                },
                span,
            );
        } else if !store {
            let method = self.method_id(name);
            self.emit(Instruction::BindMethod(method), span);
        } else {
            return Err(error("E5001", "destino inválido", span));
        }
        Ok(())
    }
    fn expression(&mut self, expr: &Expr) -> Result<(), Diagnostic> {
        let span = expr.span;
        match &expr.kind {
            ExprKind::Literal(value) => match literal_value(
                value,
                self.semantic.types.get(&expr.id).unwrap_or(&Type::Int),
                span,
            ) {
                Ok(value) => self.constant(value, span),
                Err(diagnostic) => {
                    self.emit(Instruction::Trap(diagnostic), span);
                }
            },
            ExprKind::Name(_) => self.load(expr)?,
            ExprKind::Member { object, name } => {
                if self.semantic.resolutions.contains_key(&expr.id) {
                    self.load(expr)?;
                } else {
                    self.member(object, name, span, false)?;
                }
            }
            ExprKind::Unary { op, operand } => {
                self.expression(operand)?;
                self.emit(Instruction::Unary(*op), span);
            }
            ExprKind::Binary { left, op, right } => {
                self.expression(left)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or) {
                    self.emit(Instruction::Dup, left.span);
                    let jump = self.emit(
                        if *op == BinaryOp::And {
                            Instruction::JumpIfFalse(0)
                        } else {
                            Instruction::JumpIfTrue(0)
                        },
                        span,
                    );
                    self.emit(Instruction::Pop, span);
                    self.expression(right)?;
                    self.patch(jump);
                } else {
                    self.expression(right)?;
                    self.emit(Instruction::Binary(*op), span);
                }
            }
            ExprKind::Assign { target, value } => {
                self.expression(value)?;
                if let ExprKind::Member { object, name } = &target.kind {
                    self.member(object, name, target.span, true)?;
                } else {
                    let id = self.resolution(target)?;
                    self.emit(Instruction::Dup, span);
                    let instruction =
                        if let Some(slot) = self.globals.get(&id) {
                            Instruction::StoreGlobal(*slot)
                        } else {
                            Instruction::StoreLocal(*self.locals.get(&id).ok_or_else(|| {
                                error("E5001", "destino sin almacenamiento", span)
                            })?)
                        };
                    self.emit(instruction, span);
                }
            }
            ExprKind::Call { callee, arguments } => {
                self.expression(callee)?;
                for arg in arguments {
                    self.expression(arg)?;
                }
                self.emit(Instruction::Call(arguments.len()), span);
            }
            ExprKind::Cast { expression, .. } => {
                self.expression(expression)?;
                self.emit(
                    Instruction::Cast(self.semantic.types[&expr.id].clone()),
                    span,
                );
            }
            ExprKind::Index { .. } => {
                return Err(error(
                    "E2015",
                    "la indexación todavía no está disponible",
                    span,
                ));
            }
        }
        if let Some(ty) = self.semantic.coercions.get(&expr.id) {
            self.emit(Instruction::Cast(ty.clone()), span);
        } else if matches!(expr.kind, ExprKind::Binary { .. } | ExprKind::Unary { .. })
            && let Some(ty @ (Type::Byte | Type::Short | Type::Int | Type::Long)) =
                self.semantic.types.get(&expr.id)
        {
            self.emit(Instruction::Cast(ty.clone()), span);
        }
        Ok(())
    }
}
