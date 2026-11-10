use std::collections::HashMap;

use crate::{
    ast::{
        BinaryOp, ClassDecl, ClassMember, Decl, Expr, ExprKind, FunctionDecl, Item, Literal,
        Program, Stmt, UnaryOp, VariableDecl,
    },
    diagnostic::{Diagnostic, Severity},
    quadruple::{
        self, CallSiteId, CallTarget, Class, ClassId, Field, FieldId, Function, FunctionId,
        LabelId, LocalSlot, Operand, Operation, Parameter, Quadruple, QuadrupleProgram,
        RuntimeFunction, RuntimeFunctionId, Signature, Static, StaticId, Temporary, ValueType,
    },
    semantic::{SemanticResult, SymbolId, Type},
    source::Span,
};

pub fn lower(
    program: &Program,
    semantic: &SemanticResult,
) -> Result<QuadrupleProgram, Vec<Diagnostic>> {
    if semantic
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        return Err(semantic.diagnostics.clone());
    }
    let mut lowerer = Lowerer::new(semantic);
    lowerer.reserve(program);
    lowerer.lower_main(program);
    for item in &program.items {
        if let Item::Decl(Decl::Function(function)) | Item::Export(Decl::Function(function)) = item
        {
            lowerer.lower_function(function, None, None);
        } else if let Item::Class(class) | Item::ExportClass(class) = item {
            lowerer.lower_class(class);
        } else if matches!(
            item,
            Item::Trait(_) | Item::ExportTrait(_) | Item::Module(_) | Item::Use(_)
        ) {
            lowerer.error(
                "E7101",
                "esta construcción todavía no es compatible con cuádruplos",
                item_span(item),
            );
        }
    }
    if lowerer
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        return Err(lowerer.diagnostics);
    }
    let ir = lowerer.finish();
    if let Err(diagnostic) = quadruple::validate(&ir) {
        return Err(vec![diagnostic]);
    }
    Ok(ir)
}

struct Lowerer<'a> {
    semantic: &'a SemanticResult,
    ir: QuadrupleProgram,
    functions: HashMap<SymbolId, FunctionId>,
    classes: HashMap<String, ClassId>,
    fields: HashMap<(ClassId, String), FieldId>,
    statics: HashMap<(ClassId, String), StaticId>,
    methods: HashMap<(ClassId, String), FunctionId>,
    runtime: HashMap<(String, ValueType), RuntimeFunctionId>,
    slots: HashMap<SymbolId, LocalSlot>,
    current: FunctionId,
    next_temp: usize,
    next_label: usize,
    next_call: usize,
    next_slot: usize,
    loops: Vec<(LabelId, LabelId)>,
    diagnostics: Vec<Diagnostic>,
}
impl<'a> Lowerer<'a> {
    fn new(semantic: &'a SemanticResult) -> Self {
        let span = Span::new(crate::source::SourceId(0), 0, 0);
        Self {
            semantic,
            ir: QuadrupleProgram {
                main: FunctionId(0),
                functions: vec![Function {
                    name: "<main>".into(),
                    signature: Signature {
                        parameters: vec![],
                        return_type: ValueType::Void,
                        receiver: None,
                    },
                    locals: vec![],
                    code: vec![],
                    span,
                }],
                runtime_functions: vec![],
                strings: vec![],
                classes: vec![],
                fields: vec![],
                statics: vec![],
            },
            functions: HashMap::new(),
            classes: HashMap::new(),
            fields: HashMap::new(),
            statics: HashMap::new(),
            methods: HashMap::new(),
            runtime: HashMap::new(),
            slots: HashMap::new(),
            current: FunctionId(0),
            next_temp: 0,
            next_label: 0,
            next_call: 0,
            next_slot: 0,
            loops: vec![],
            diagnostics: vec![],
        }
    }
    fn reserve(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::Class(class) | Item::ExportClass(class) = item {
                let id = ClassId(self.ir.classes.len());
                // The constructor ID is filled during the second deterministic pass.
                self.ir.classes.push(Class {
                    fields: vec![],
                    statics: vec![],
                    constructor: None,
                    span: class.span,
                });
                self.classes.insert(class.name.clone(), id);
            }
        }
        for item in &program.items {
            match item {
                Item::Decl(Decl::Function(function)) | Item::Export(Decl::Function(function)) => {
                    self.reserve_function(&function.name, function.name_span, function, None);
                }
                Item::Class(class) | Item::ExportClass(class) => self.reserve_class(class),
                _ => {}
            }
        }
    }
    fn reserve_function(
        &mut self,
        name: &str,
        name_span: Span,
        function: &FunctionDecl,
        receiver: Option<ClassId>,
    ) -> Option<FunctionId> {
        let signature = self.signature(function, receiver)?;
        let id = FunctionId(self.ir.functions.len());
        self.ir.functions.push(Function {
            name: name.into(),
            signature,
            locals: vec![],
            code: vec![],
            span: function.span,
        });
        if receiver.is_none()
            && let Some(symbol) = self.symbol(name, name_span)
        {
            self.functions.insert(symbol, id);
        }
        Some(id)
    }
    fn reserve_class(&mut self, class: &ClassDecl) {
        let Some(&id) = self.classes.get(&class.name) else {
            return;
        };
        for member in &class.members {
            match member {
                ClassMember::Field(field) => {
                    let Some(ty) = self.value_type(
                        self.semantic
                            .types
                            .get(&field.variable.initializer.id)
                            .unwrap_or(&Type::Error),
                        field.variable.span,
                    ) else {
                        continue;
                    };
                    if field.is_static {
                        let field_id = StaticId(self.ir.statics.len());
                        self.ir.statics.push(Static {
                            owner: id,
                            ty,
                            span: field.variable.span,
                        });
                        self.ir.classes[id.0].statics.push(field_id);
                        self.statics
                            .insert((id, field.variable.name.clone()), field_id);
                    } else {
                        let field_id = FieldId(self.ir.fields.len());
                        self.ir.fields.push(Field {
                            owner: id,
                            ty,
                            span: field.variable.span,
                        });
                        self.ir.classes[id.0].fields.push(field_id);
                        self.fields
                            .insert((id, field.variable.name.clone()), field_id);
                    }
                }
                ClassMember::Constructor(method) => {
                    if let Some(constructor) = self.reserve_function(
                        &format!("{}.this", class.name),
                        method.function.name_span,
                        &method.function,
                        Some(id),
                    ) {
                        self.ir.classes[id.0].constructor = Some(constructor);
                    }
                }
                ClassMember::Method(method) => {
                    let receiver = (!method.is_static).then_some(id);
                    if let Some(function) = self.reserve_function(
                        &format!("{}.{}", class.name, method.function.name),
                        method.function.name_span,
                        &method.function,
                        receiver,
                    ) {
                        self.methods
                            .insert((id, method.function.name.clone()), function);
                    }
                }
            }
        }
        if self.ir.classes[id.0].constructor.is_none() {
            let implicit = FunctionDecl {
                name: "this".into(),
                name_span: class.name_span,
                parameters: vec![],
                return_type: None,
                body: Stmt::Block {
                    statements: vec![],
                    span: class.span,
                },
                span: class.span,
            };
            if let Some(constructor) = self.reserve_function(
                &format!("{}.this", class.name),
                class.name_span,
                &implicit,
                Some(id),
            ) {
                self.ir.classes[id.0].constructor = Some(constructor);
            }
        }
    }
    fn lower_main(&mut self, program: &Program) {
        self.reset(FunctionId(0));
        for item in &program.items {
            match item {
                Item::Decl(Decl::Variable(v)) | Item::Export(Decl::Variable(v)) => self.variable(v),
                Item::Stmt(stmt) => self.statement(stmt),
                Item::Class(class) | Item::ExportClass(class) => {
                    self.lower_static_initializers(class)
                }
                _ => {}
            }
        }
        self.emit(Operation::Halt, self.ir.functions[0].span);
    }
    fn lower_static_initializers(&mut self, class: &ClassDecl) {
        let id = self.classes[&class.name];
        for member in &class.members {
            if let ClassMember::Field(field) = member
                && field.is_static
                && let (Some(static_), Some(value)) = (
                    self.statics
                        .get(&(id, field.variable.name.clone()))
                        .copied(),
                    self.expr(&field.variable.initializer),
                )
            {
                self.emit(Operation::SetStatic { static_, value }, field.variable.span);
            }
        }
    }
    fn lower_class(&mut self, class: &ClassDecl) {
        let id = self.classes[&class.name];
        for member in &class.members {
            match member {
                ClassMember::Constructor(method) => self.lower_function(
                    &method.function,
                    self.ir.classes[id.0].constructor,
                    Some(class),
                ),
                ClassMember::Method(method) => {
                    if let Some(function) = self
                        .methods
                        .get(&(id, method.function.name.clone()))
                        .copied()
                    {
                        self.lower_function(&method.function, Some(function), None);
                    }
                }
                ClassMember::Field(_) => {}
            }
        }
        if !class
            .members
            .iter()
            .any(|member| matches!(member, ClassMember::Constructor(_)))
        {
            let implicit = FunctionDecl {
                name: "this".into(),
                name_span: class.name_span,
                parameters: vec![],
                return_type: None,
                body: Stmt::Block {
                    statements: vec![],
                    span: class.span,
                },
                span: class.span,
            };
            self.lower_function(&implicit, self.ir.classes[id.0].constructor, Some(class));
        }
    }
    fn lower_function(
        &mut self,
        function: &FunctionDecl,
        id: Option<FunctionId>,
        class: Option<&ClassDecl>,
    ) {
        let id = id.or_else(|| {
            self.symbol(&function.name, function.name_span)
                .and_then(|symbol| self.functions.get(&symbol).copied())
        });
        let Some(id) = id else {
            return;
        };
        self.reset(id);
        if let Some(class) = class {
            let class_id = self.classes[&class.name];
            for member in &class.members {
                if let ClassMember::Field(field) = member
                    && !field.is_static
                {
                    let Some(value) = self.expr(&field.variable.initializer) else {
                        continue;
                    };
                    let Some(field_id) = self
                        .fields
                        .get(&(class_id, field.variable.name.clone()))
                        .copied()
                    else {
                        continue;
                    };
                    self.emit(
                        Operation::SetField {
                            owner: Operand::Receiver { class: class_id },
                            field: field_id,
                            value,
                        },
                        field.variable.span,
                    );
                }
            }
        }
        for (index, parameter) in function.parameters.iter().enumerate() {
            if let Some(symbol) = self.symbol(&parameter.name, parameter.span)
                && let Some(ty) = self.ty(&parameter.type_name.name, parameter.span)
            {
                let slot = LocalSlot { index, ty };
                self.declare_slot(slot);
                self.slots.insert(symbol, slot);
                self.next_slot = index + 1;
            }
        }
        self.statement(&function.body);
        if !matches!(
            self.ir.functions[id.0].code.last().map(|q| &q.operation),
            Some(Operation::Return(_))
        ) && self.ir.functions[id.0].signature.return_type == ValueType::Void
        {
            self.emit(Operation::Return(None), function.span);
        }
    }
    fn reset(&mut self, id: FunctionId) {
        self.current = id;
        self.slots.clear();
        self.next_temp = 0;
        self.next_label = 0;
        self.next_call = 0;
        self.next_slot = 0;
        self.loops.clear();
    }
    fn finish(self) -> QuadrupleProgram {
        self.ir
    }
    fn signature(
        &mut self,
        function: &FunctionDecl,
        receiver: Option<ClassId>,
    ) -> Option<Signature> {
        let mut parameters = Vec::new();
        for (index, parameter) in function.parameters.iter().enumerate() {
            parameters.push(Parameter {
                index,
                ty: self.ty(&parameter.type_name.name, parameter.span)?,
                span: parameter.span,
            });
        }
        Some(Signature {
            parameters,
            return_type: function
                .return_type
                .as_ref()
                .map_or(Some(ValueType::Void), |t| self.ty(&t.name, t.span))?,
            receiver,
        })
    }
    fn ty(&mut self, name: &str, span: Span) -> Option<ValueType> {
        match name {
            "int" => Some(ValueType::I32),
            "decimal" => Some(ValueType::F64),
            "bool" => Some(ValueType::Bool),
            "str" => Some(ValueType::Str),
            "void" => Some(ValueType::Void),
            name if self.classes.contains_key(name) => Some(ValueType::Object(self.classes[name])),
            _ => {
                self.error(
                    "E7101",
                    format!("tipo no compatible con cuádruplos: {name}"),
                    span,
                );
                None
            }
        }
    }
    fn variable(&mut self, variable: &VariableDecl) {
        let Some(symbol) = self.symbol(&variable.name, variable.name_span) else {
            return;
        };
        let value = self.expr(&variable.initializer);
        let ty = match &self.semantic.symbols[symbol.0].ty {
            Type::Function(_, _) => {
                Some(value.as_ref().map_or(ValueType::Void, |value| value.ty()))
            }
            ty => self.value_type(ty, variable.span),
        };
        if let (Some(value), Some(ty)) = (value, ty) {
            let slot = LocalSlot {
                index: self.next_slot,
                ty,
            };
            self.next_slot += 1;
            self.declare_slot(slot);
            self.slots.insert(symbol, slot);
            if let Some(value) = self.coerce(value, ty, variable.initializer.span) {
                self.emit(Operation::StoreLocal { local: slot, value }, variable.span);
            }
        }
    }
    fn statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Variable(v) => self.variable(v),
            Stmt::Block { statements, .. } => {
                for statement in statements {
                    self.statement(statement);
                }
            }
            Stmt::Expression { expression, .. } => {
                self.expr(expression);
            }
            Stmt::Return { value, span } => {
                let return_type = self.ir.functions[self.current.0].signature.return_type;
                let value = value
                    .as_ref()
                    .and_then(|expression| self.expr(expression))
                    .and_then(|value| self.coerce(value, return_type, *span));
                if return_type == ValueType::Void && value.is_some() {
                    self.error(
                        "E7101",
                        "una función void no puede devolver un valor",
                        *span,
                    );
                    return;
                }
                if return_type != ValueType::Void && value.is_none() {
                    self.error("E7101", "falta el valor de retorno", *span);
                    return;
                }
                self.emit(Operation::Return(value), *span);
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let Some(condition) = self.expr(condition) else {
                    return;
                };
                let otherwise = self.label();
                let end = self.label();
                self.emit(
                    Operation::JumpIfFalse {
                        condition,
                        target: otherwise,
                    },
                    *span,
                );
                self.statement(then_branch);
                self.emit(Operation::Jump(end), *span);
                self.emit(Operation::Label(otherwise), *span);
                if let Some(branch) = else_branch {
                    self.statement(branch);
                }
                self.emit(Operation::Label(end), *span);
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let start = self.label();
                let end = self.label();
                self.emit(Operation::Label(start), *span);
                let Some(condition) = self.expr(condition) else {
                    return;
                };
                self.emit(
                    Operation::JumpIfFalse {
                        condition,
                        target: end,
                    },
                    *span,
                );
                self.loops.push((start, end));
                self.statement(body);
                self.loops.pop();
                self.emit(Operation::Jump(start), *span);
                self.emit(Operation::Label(end), *span);
            }
            Stmt::Break { span } => {
                if let Some((_, end)) = self.loops.last() {
                    self.emit(Operation::Jump(*end), *span);
                }
            }
            Stmt::Continue { span } => {
                if let Some((start, _)) = self.loops.last() {
                    self.emit(Operation::Jump(*start), *span);
                }
            }
        }
    }
    fn expr(&mut self, expr: &Expr) -> Option<Operand> {
        match &expr.kind {
            ExprKind::Literal(literal) => self.literal(literal, expr),
            ExprKind::Name(_) => self.load(expr),
            ExprKind::Unary { op, operand } => {
                if *op == UnaryOp::Minus
                    && let ExprKind::Literal(Literal::Number(text)) = &operand.kind
                {
                    return self.negative_literal(text, expr.span);
                }
                let value = self.expr(operand)?;
                match op {
                    UnaryOp::Plus => Some(value),
                    UnaryOp::Minus => {
                        let result = self.temp(value.ty());
                        Some(self.unary(
                            Operation::Neg {
                                operand: value,
                                result,
                            },
                            expr.span,
                        ))
                    }
                    UnaryOp::Not => {
                        let result = self.temp(ValueType::Bool);
                        Some(self.unary(
                            Operation::Not {
                                operand: value,
                                result,
                            },
                            expr.span,
                        ))
                    }
                }
            }
            ExprKind::Binary { left, op, right } if matches!(op, BinaryOp::And | BinaryOp::Or) => {
                self.short_circuit(left, *op, right, expr.span)
            }
            ExprKind::Binary { left, op, right } => {
                let left = self.expr(left)?;
                let right = self.expr(right)?;
                self.binary(*op, left, right, expr.span)
            }
            ExprKind::Assign { target, value } => {
                // Source semantics evaluate the value before the member receiver.
                let value = self.expr(value)?;
                if let ExprKind::Member { object, name } = &target.kind {
                    return self.store_member(object, name, value, target.span);
                }
                let symbol = self.resolution(target)?;
                let Some(slot) = self.slots.get(&symbol).copied() else {
                    self.error("E7101", "destino sin almacenamiento", target.span);
                    return None;
                };
                let value = self.coerce(value, slot.ty, value_span(value, expr.span))?;
                self.emit(Operation::StoreLocal { local: slot, value }, expr.span);
                Some(value)
            }
            ExprKind::Cast {
                expression,
                type_name,
            } => {
                let value = self.expr(expression)?;
                match (value.ty(), self.ty(&type_name.name, type_name.span)?) {
                    (ValueType::I32, ValueType::F64) => {
                        let result = self.temp(ValueType::F64);
                        self.emit(
                            Operation::I32ToF64 {
                                source: value,
                                result,
                            },
                            expr.span,
                        );
                        Some(Operand::Temporary(result))
                    }
                    (from, to) if from == to => Some(value),
                    _ => {
                        self.error(
                            "E7101",
                            "conversión no compatible con cuádruplos",
                            expr.span,
                        );
                        None
                    }
                }
            }
            ExprKind::Member { object, name } => self.member(object, name, expr.span),
            ExprKind::Call { callee, arguments } => self.call(callee, arguments, expr.span),
            _ => {
                self.error(
                    "E7101",
                    "expresión todavía no compatible con cuádruplos",
                    expr.span,
                );
                None
            }
        }
    }
    fn literal(&mut self, literal: &Literal, expr: &Expr) -> Option<Operand> {
        match literal {
            Literal::Bool(value) => Some(Operand::Bool(*value)),
            Literal::String(value) => {
                let id = quadruple::StringId(self.ir.strings.len());
                self.ir.strings.push(unquote(value));
                Some(Operand::String(id))
            }
            Literal::Number(value) => self.signed_literal(value, expr.span),
            Literal::Char(_) => {
                self.error("E7101", "char no es compatible con cuádruplos", expr.span);
                None
            }
        }
    }
    fn signed_literal(&mut self, text: &str, span: Span) -> Option<Operand> {
        self.number_literal(text, false, span)
    }
    fn negative_literal(&mut self, text: &str, span: Span) -> Option<Operand> {
        self.number_literal(text, true, span)
    }
    fn number_literal(&mut self, text: &str, negative: bool, span: Span) -> Option<Operand> {
        let (number, suffix) = text
            .char_indices()
            .find(|(_, ch)| ch.is_ascii_alphabetic() || *ch == '_')
            .map_or((text, ""), |(index, _)| text.split_at(index));
        if !suffix.is_empty() && suffix != "dec" && suffix != "i32" {
            self.error(
                "E7101",
                format!("sufijo numérico no compatible con cuádruplos: {suffix}"),
                span,
            );
            return None;
        }
        let type_ = if suffix == "dec" || number.contains(['.', 'e', 'E']) {
            ValueType::F64
        } else {
            ValueType::I32
        };
        match type_ {
            ValueType::I32 => match number
                .parse::<i64>()
                .ok()
                .and_then(|v| if negative { v.checked_neg() } else { Some(v) })
                .and_then(|v| i32::try_from(v).ok())
            {
                Some(value) => Some(Operand::I32(value)),
                None => {
                    self.error("E7102", "literal int fuera del rango i32", span);
                    None
                }
            },
            ValueType::F64 => match number.parse::<f64>() {
                Ok(value) if value.is_finite() => {
                    Some(Operand::F64(if negative { -value } else { value }))
                }
                _ => {
                    self.error("E7102", "literal decimal inválido", span);
                    None
                }
            },
            _ => None,
        }
    }
    fn coerce(&mut self, value: Operand, target: ValueType, span: Span) -> Option<Operand> {
        match (value.ty(), target) {
            (ValueType::I32, ValueType::F64) => {
                let result = self.temp(ValueType::F64);
                self.emit(
                    Operation::I32ToF64 {
                        source: value,
                        result,
                    },
                    span,
                );
                Some(Operand::Temporary(result))
            }
            (from, to) if from == to => Some(value),
            _ => {
                self.error("E7101", "conversión no compatible con cuádruplos", span);
                None
            }
        }
    }
    fn load(&mut self, expr: &Expr) -> Option<Operand> {
        let symbol = self.resolution(expr)?;
        if let Some(slot) = self.slots.get(&symbol).copied() {
            let result = self.temp(slot.ty);
            self.emit(
                Operation::LoadLocal {
                    local: slot,
                    result,
                },
                expr.span,
            );
            return Some(Operand::Temporary(result));
        }
        if let Some(&function) = self.functions.get(&symbol) {
            self.error(
                "E7101",
                format!("referencia a función {function:?} fuera de una llamada"),
                expr.span,
            );
            return None;
        }
        if let Some(class) = self.ir.functions[self.current.0].signature.receiver
            && self
                .semantic
                .symbols
                .get(symbol.0)
                .is_some_and(|entry| entry.name == "this")
        {
            return Some(Operand::Receiver { class });
        }
        self.error("E7101", "nombre no compatible con cuádruplos", expr.span);
        None
    }
    fn member(&mut self, object: &Expr, name: &str, span: Span) -> Option<Operand> {
        let (class, static_access) = self.member_class(object)?;
        if static_access {
            let static_ = self.statics.get(&(class, name.into())).copied()?;
            let result = self.temp(self.ir.statics[static_.0].ty);
            self.emit(Operation::GetStatic { static_, result }, span);
            return Some(Operand::Temporary(result));
        }
        let receiver = self.expr(object)?;
        if let Some(field) = self.fields.get(&(class, name.into())).copied() {
            let result = self.temp(self.ir.fields[field.0].ty);
            self.emit(
                Operation::GetField {
                    owner: receiver,
                    field,
                    result,
                },
                span,
            );
            Some(Operand::Temporary(result))
        } else if let Some(method) = self.methods.get(&(class, name.into())).copied() {
            let result = self.temp(ValueType::BoundMethod(method));
            self.emit(
                Operation::BindMethod {
                    receiver,
                    method,
                    result,
                },
                span,
            );
            Some(Operand::Temporary(result))
        } else {
            self.error("E7101", "miembro no compatible con cuádruplos", span);
            None
        }
    }
    fn store_member(
        &mut self,
        object: &Expr,
        name: &str,
        value: Operand,
        span: Span,
    ) -> Option<Operand> {
        let (class, static_access) = self.member_class(object)?;
        if static_access {
            let static_ = self.statics.get(&(class, name.into())).copied()?;
            let value = self.coerce(value, self.ir.statics[static_.0].ty, span)?;
            self.emit(Operation::SetStatic { static_, value }, span);
            return Some(value);
        }
        let field = self.fields.get(&(class, name.into())).copied()?;
        let value = self.coerce(value, self.ir.fields[field.0].ty, span)?;
        let owner = self.expr(object)?;
        self.emit(
            Operation::SetField {
                owner,
                field,
                value,
            },
            span,
        );
        Some(value)
    }
    fn member_class(&mut self, object: &Expr) -> Option<(ClassId, bool)> {
        match self.semantic.types.get(&object.id) {
            Some(Type::Class(name)) => self.classes.get(name).copied().map(|class| (class, false)),
            Some(Type::ClassObject(name)) => {
                self.classes.get(name).copied().map(|class| (class, true))
            }
            _ => {
                self.error("E7101", "miembro no compatible con cuádruplos", object.span);
                None
            }
        }
    }
    fn call(&mut self, callee: &Expr, arguments: &[Expr], span: Span) -> Option<Operand> {
        let (target, receiver, signature, return_type) = match &callee.kind {
            ExprKind::Name(_)
                if matches!(
                    self.semantic.types.get(&callee.id),
                    Some(Type::ClassObject(_))
                ) =>
            {
                let Type::ClassObject(name) = self.semantic.types[&callee.id].clone() else {
                    unreachable!()
                };
                let class = *self.classes.get(&name)?;
                let constructor = self.ir.classes[class.0].constructor?;
                let signature = self.ir.functions[constructor.0].signature.clone();
                (
                    CallTarget::Constructor(class),
                    None,
                    signature,
                    ValueType::Object(class),
                )
            }
            ExprKind::Member { object, name } => {
                let (class, static_access) = self.member_class(object)?;
                if static_access {
                    self.error(
                        "E7101",
                        "las llamadas estáticas todavía no son compatibles",
                        span,
                    );
                    return None;
                }
                let method = *self.methods.get(&(class, name.clone()))?;
                let receiver = self.expr(object)?;
                let signature = self.ir.functions[method.0].signature.clone();
                (
                    CallTarget::Function(method),
                    Some(receiver),
                    signature.clone(),
                    signature.return_type,
                )
            }
            _ => {
                let symbol = self.resolution(callee)?;
                if self.slots.contains_key(&symbol) {
                    let Operand::Temporary(bound) = self.load(callee)? else {
                        self.error("E7101", "llamada no compatible con cuádruplos", span);
                        return None;
                    };
                    let ValueType::BoundMethod(method) = bound.ty else {
                        self.error("E7101", "llamada no compatible con cuádruplos", span);
                        return None;
                    };
                    let signature = self.ir.functions[method.0].signature.clone();
                    (
                        CallTarget::BoundMethod(bound),
                        None,
                        signature.clone(),
                        signature.return_type,
                    )
                } else if let Some(id) = self.functions.get(&symbol) {
                    let signature = self.ir.functions[id.0].signature.clone();
                    (
                        CallTarget::Function(*id),
                        None,
                        signature.clone(),
                        signature.return_type,
                    )
                } else {
                    let mut values = Vec::with_capacity(arguments.len());
                    for argument in arguments {
                        values.push(self.expr(argument)?);
                    }
                    let runtime = self.runtime_function(symbol, &values, callee.span)?;
                    let signature = self.ir.runtime_functions[runtime.0].signature.clone();
                    return self.emit_call(
                        CallTarget::Runtime(runtime),
                        None,
                        signature.clone(),
                        signature.return_type,
                        values,
                        span,
                    );
                }
            }
        };
        let mut values = Vec::with_capacity(arguments.len());
        for argument in arguments {
            values.push(self.expr(argument)?);
        }
        self.emit_call(target, receiver, signature, return_type, values, span)
    }
    fn emit_call(
        &mut self,
        target: CallTarget,
        receiver: Option<Operand>,
        signature: Signature,
        return_type: ValueType,
        values: Vec<Operand>,
        span: Span,
    ) -> Option<Operand> {
        if signature.parameters.len() != values.len() {
            self.error("E7101", "cantidad de argumentos no compatible", span);
            return None;
        }
        let values = values
            .into_iter()
            .zip(&signature.parameters)
            .map(|(value, parameter)| self.coerce(value, parameter.ty, parameter.span))
            .collect::<Option<Vec<_>>>()?;
        let call = CallSiteId(self.next_call);
        self.next_call += 1;
        let mut index = 0;
        if let Some(receiver) = receiver {
            self.emit(
                Operation::Param {
                    call,
                    index,
                    value: receiver,
                },
                span,
            );
            index += 1;
        }
        for value in values {
            self.emit(Operation::Param { call, index, value }, span);
            index += 1;
        }
        let result = (return_type != ValueType::Void).then(|| self.temp(return_type));
        self.emit(
            Operation::Call {
                call,
                target,
                result,
            },
            span,
        );
        result.map(Operand::Temporary)
    }
    fn runtime_function(
        &mut self,
        symbol: SymbolId,
        values: &[Operand],
        span: Span,
    ) -> Option<RuntimeFunctionId> {
        let name = self.semantic.symbols[symbol.0].name.clone();
        let Some(&argument) = values.first() else {
            self.error(
                "E7101",
                format!("runtime no compatible con cuádruplos: {name}"),
                span,
            );
            return None;
        };
        if values.len() != 1
            || !matches!(
                argument.ty(),
                ValueType::I32 | ValueType::F64 | ValueType::Bool | ValueType::Str
            )
        {
            self.error(
                "E7101",
                format!("firma runtime no compatible con cuádruplos: {name}"),
                span,
            );
            return None;
        }
        let key = (name.clone(), argument.ty());
        if let Some(id) = self.runtime.get(&key) {
            return Some(*id);
        }
        let return_type = match name.as_str() {
            "print" => ValueType::Void,
            "to_string" => ValueType::Str,
            _ => {
                self.error(
                    "E7101",
                    format!("runtime no compatible con cuádruplos: {name}"),
                    span,
                );
                return None;
            }
        };
        let id = RuntimeFunctionId(self.ir.runtime_functions.len());
        self.ir.runtime_functions.push(RuntimeFunction {
            name: name.clone(),
            signature: Signature {
                parameters: vec![Parameter {
                    index: 0,
                    ty: argument.ty(),
                    span,
                }],
                return_type,
                receiver: None,
            },
            span,
        });
        self.runtime.insert(key, id);
        Some(id)
    }
    fn binary(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        span: Span,
    ) -> Option<Operand> {
        let (left, right, ty) = self.coerce_binary_operands(op, left, right, span)?;
        if op == BinaryOp::Power && matches!(right, Operand::I32(value) if value < 0) {
            self.error("E7101", "el exponente debe ser un int no negativo", span);
            return None;
        }
        let result = self.temp(ty);
        let operation = match op {
            BinaryOp::Add => Operation::Add {
                left,
                right,
                result,
            },
            BinaryOp::Subtract => Operation::Sub {
                left,
                right,
                result,
            },
            BinaryOp::Multiply => Operation::Mul {
                left,
                right,
                result,
            },
            BinaryOp::Divide => Operation::Div {
                left,
                right,
                result,
            },
            BinaryOp::Remainder => Operation::Mod {
                left,
                right,
                result,
            },
            BinaryOp::Power => Operation::Pow {
                left,
                right,
                result,
            },
            BinaryOp::Equal => Operation::Eq {
                left,
                right,
                result,
            },
            BinaryOp::NotEqual => Operation::Ne {
                left,
                right,
                result,
            },
            BinaryOp::Less => Operation::Lt {
                left,
                right,
                result,
            },
            BinaryOp::LessEqual => Operation::Le {
                left,
                right,
                result,
            },
            BinaryOp::Greater => Operation::Gt {
                left,
                right,
                result,
            },
            BinaryOp::GreaterEqual => Operation::Le {
                left: right,
                right: left,
                result,
            },
            _ => unreachable!(),
        };
        self.emit(operation, span);
        Some(Operand::Temporary(result))
    }
    fn short_circuit(
        &mut self,
        left: &Expr,
        op: BinaryOp,
        right: &Expr,
        span: Span,
    ) -> Option<Operand> {
        let left = self.expr(left)?;
        let slot = LocalSlot {
            index: self.next_slot,
            ty: ValueType::Bool,
        };
        self.next_slot += 1;
        self.declare_slot(slot);
        let skip = self.label();
        let end = self.label();
        self.emit(
            Operation::StoreLocal {
                local: slot,
                value: left,
            },
            span,
        );
        if op == BinaryOp::And {
            self.emit(
                Operation::JumpIfFalse {
                    condition: left,
                    target: skip,
                },
                span,
            );
        } else {
            let not = self.temp(ValueType::Bool);
            self.emit(
                Operation::Not {
                    operand: left,
                    result: not,
                },
                span,
            );
            self.emit(
                Operation::JumpIfFalse {
                    condition: Operand::Temporary(not),
                    target: skip,
                },
                span,
            );
        }
        let right = self.expr(right)?;
        self.emit(
            Operation::StoreLocal {
                local: slot,
                value: right,
            },
            span,
        );
        self.emit(Operation::Jump(end), span);
        self.emit(Operation::Label(skip), span);
        self.emit(Operation::Label(end), span);
        let result = self.temp(ValueType::Bool);
        self.emit(
            Operation::LoadLocal {
                local: slot,
                result,
            },
            span,
        );
        Some(Operand::Temporary(result))
    }
    fn value_type(&mut self, ty: &Type, span: Span) -> Option<ValueType> {
        match ty {
            Type::Int => Some(ValueType::I32),
            Type::Decimal => Some(ValueType::F64),
            Type::Bool => Some(ValueType::Bool),
            Type::Str => Some(ValueType::Str),
            Type::Class(name) => self
                .classes
                .get(name)
                .copied()
                .map(ValueType::Object)
                .or_else(|| {
                    self.error(
                        "E7101",
                        format!("tipo no compatible con cuádruplos: {name}"),
                        span,
                    );
                    None
                }),
            other => {
                self.error(
                    "E7101",
                    format!("tipo no compatible con cuádruplos: {other:?}"),
                    span,
                );
                None
            }
        }
    }
    fn coerce_binary_operands(
        &mut self,
        op: BinaryOp,
        left: Operand,
        right: Operand,
        span: Span,
    ) -> Option<(Operand, Operand, ValueType)> {
        let comparison = matches!(
            op,
            BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
        );
        let arithmetic = matches!(
            op,
            BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Remainder
                | BinaryOp::Power
        );
        let numeric = matches!(left.ty(), ValueType::I32 | ValueType::F64)
            && matches!(right.ty(), ValueType::I32 | ValueType::F64);
        if numeric {
            let target = if left.ty() == ValueType::F64 || right.ty() == ValueType::F64 {
                ValueType::F64
            } else {
                ValueType::I32
            };
            let left = self.coerce(left, target, span)?;
            let right = self.coerce(right, target, span)?;
            return Some((
                left,
                right,
                if comparison { ValueType::Bool } else { target },
            ));
        }
        if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) && left.ty() == right.ty() {
            return Some((left, right, ValueType::Bool));
        }
        let message = if arithmetic || comparison {
            "operación con tipos incompatibles"
        } else {
            "operación no compatible con cuádruplos"
        };
        self.error("E7101", message, span);
        None
    }
    fn unary(&mut self, operation: Operation, span: Span) -> Operand {
        let result = match &operation {
            Operation::Neg { result, .. } | Operation::Not { result, .. } => *result,
            _ => unreachable!(),
        };
        self.emit(operation, span);
        Operand::Temporary(result)
    }
    fn temp(&mut self, ty: ValueType) -> Temporary {
        let value = Temporary {
            index: self.next_temp,
            ty,
        };
        self.next_temp += 1;
        value
    }
    fn label(&mut self) -> LabelId {
        let value = LabelId(self.next_label);
        self.next_label += 1;
        value
    }
    fn declare_slot(&mut self, slot: LocalSlot) {
        self.ir.functions[self.current.0].locals.push(slot);
    }
    fn emit(&mut self, operation: Operation, span: Span) {
        self.ir.functions[self.current.0]
            .code
            .push(Quadruple { operation, span });
    }
    fn symbol(&self, name: &str, span: Span) -> Option<SymbolId> {
        self.semantic
            .symbols
            .iter()
            .find(|s| s.name == name && s.declaration_span == span)
            .map(|s| s.id)
    }
    fn resolution(&self, expr: &Expr) -> Option<SymbolId> {
        self.semantic.resolutions.get(&expr.id).copied()
    }
    fn error(&mut self, code: &str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }
}
fn unquote(value: &str) -> String {
    let content = value
        .get(1..value.len().saturating_sub(1))
        .unwrap_or_default();
    let mut decoded = String::with_capacity(content.len());
    let mut escaped = false;
    for character in content.chars() {
        if escaped {
            decoded.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '0' => '\0',
                '"' => '"',
                '\\' => '\\',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            decoded.push(character);
        }
    }
    if escaped {
        decoded.push('\\');
    }
    decoded
}

fn value_span(_: Operand, fallback: Span) -> Span {
    fallback
}
fn item_span(item: &Item) -> Span {
    match item {
        Item::Class(v) | Item::ExportClass(v) => v.span,
        Item::Trait(v) | Item::ExportTrait(v) => v.span,
        Item::Module(v) => v.span,
        Item::Use(v) => v.span,
        Item::Decl(Decl::Variable(v)) | Item::Export(Decl::Variable(v)) => v.span,
        Item::Decl(Decl::Function(v)) | Item::Export(Decl::Function(v)) => v.span,
        Item::Stmt(v) => v.span(),
    }
}
