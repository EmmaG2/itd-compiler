use std::collections::{HashMap, HashSet};

use crate::{
    ast::{
        BinaryOp, ClassDecl, ClassMember, Decl, Expr, ExprId, ExprKind, FunctionDecl, Item,
        Literal, MethodDecl, Program, Stmt, TraitDecl, TypeName, UnaryOp, VariableDecl, Visibility,
    },
    diagnostic::{Diagnostic, Label},
    source::Span,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SymbolId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Byte,
    Short,
    Int,
    Long,
    Decimal,
    IntegerOrDecimal,
    Float,
    Double,
    Bool,
    Str,
    Char,
    Void,
    Function(Vec<Type>, Box<Type>),
    Class(String),
    Trait(String),
    ClassObject(String),
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Variable,
    Parameter,
    Function,
    Class,
    Trait,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Member {
    pub ty: Type,
    pub visibility: Visibility,
    pub is_static: bool,
    pub mutable: bool,
    pub declaration_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassInfo {
    pub fields: HashMap<String, Member>,
    pub methods: HashMap<String, Member>,
    pub constructor: Option<Member>,
    pub traits: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitInfo {
    pub methods: HashMap<String, Type>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub ty: Type,
    pub mutable: bool,
    pub initialized: bool,
    pub declaration_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticResult {
    pub symbols: Vec<Symbol>,
    pub resolutions: HashMap<ExprId, SymbolId>,
    pub types: HashMap<ExprId, Type>,
    pub coercions: HashMap<ExprId, Type>,
    pub classes: HashMap<String, ClassInfo>,
    pub traits: HashMap<String, TraitInfo>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportedSymbol {
    pub name: String,
    pub ty: Type,
    pub declaration_span: Span,
    pub class: Option<(String, ClassInfo)>,
    pub trait_info: Option<(String, TraitInfo)>,
}

#[must_use]
pub fn analyze(program: &Program) -> SemanticResult {
    analyze_with_imports(program, &[])
}

#[must_use]
pub fn analyze_with_imports(program: &Program, imports: &[ImportedSymbol]) -> SemanticResult {
    let mut analyzer = Analyzer {
        scopes: vec![HashMap::new()],
        symbols: Vec::new(),
        resolutions: HashMap::new(),
        types: HashMap::new(),
        coercions: HashMap::new(),
        diagnostics: Vec::new(),
        classes: HashMap::new(),
        traits: HashMap::new(),
        expected_return: None,
        loop_depth: 0,
        current_class: None,
    };
    analyzer.declare(
        "print",
        SymbolKind::Function,
        Type::Function(vec![Type::Str], Box::new(Type::Void)),
        false,
        true,
        Span::new(crate::source::SourceId(0), 0, 0),
    );
    for (name, parameters, result) in [
        ("round", vec![Type::Decimal, Type::Int], Type::Decimal),
        ("scale", vec![Type::Decimal], Type::Int),
        ("to_string", vec![Type::IntegerOrDecimal], Type::Str),
    ] {
        analyzer.declare(
            name,
            SymbolKind::Function,
            Type::Function(parameters, Box::new(result)),
            false,
            true,
            Span::new(crate::source::SourceId(0), 0, 0),
        );
    }
    for import in imports {
        if let Some((name, class)) = &import.class {
            analyzer.classes.insert(name.clone(), class.clone());
        }
        if let Some((name, trait_info)) = &import.trait_info {
            analyzer.traits.insert(name.clone(), trait_info.clone());
        }
        analyzer.declare(
            &import.name,
            SymbolKind::Variable,
            import.ty.clone(),
            false,
            true,
            import.declaration_span,
        );
    }
    analyzer.program(program)
}

struct Analyzer {
    scopes: Vec<HashMap<String, SymbolId>>,
    symbols: Vec<Symbol>,
    resolutions: HashMap<ExprId, SymbolId>,
    types: HashMap<ExprId, Type>,
    coercions: HashMap<ExprId, Type>,
    diagnostics: Vec<Diagnostic>,
    classes: HashMap<String, ClassInfo>,
    traits: HashMap<String, TraitInfo>,
    expected_return: Option<Type>,
    loop_depth: usize,
    current_class: Option<String>,
}

impl Analyzer {
    fn program(mut self, program: &Program) -> SemanticResult {
        for item in &program.items {
            match item {
                Item::Class(class) | Item::ExportClass(class) => {
                    self.declare(
                        &class.name,
                        SymbolKind::Class,
                        Type::ClassObject(class.name.clone()),
                        false,
                        true,
                        class.name_span,
                    );
                }
                Item::Trait(trait_decl) | Item::ExportTrait(trait_decl) => {
                    self.declare(
                        &trait_decl.name,
                        SymbolKind::Trait,
                        Type::Trait(trait_decl.name.clone()),
                        false,
                        true,
                        trait_decl.name_span,
                    );
                }
                _ => {}
            }
        }
        for item in &program.items {
            match item {
                Item::Trait(value) | Item::ExportTrait(value) => self.declare_trait(value),
                _ => {}
            }
        }
        for item in &program.items {
            match item {
                Item::Class(value) | Item::ExportClass(value) => self.declare_class(value),
                _ => {}
            }
        }
        self.detect_composition_cycles();
        for item in &program.items {
            if let Item::Decl(Decl::Function(function)) | Item::Export(Decl::Function(function)) =
                item
            {
                self.declare_function(function);
            }
        }
        for item in &program.items {
            match item {
                Item::Decl(decl) => self.declaration(decl),
                Item::Export(decl) => self.declaration(decl),
                Item::Class(class) | Item::ExportClass(class) => self.class_bodies(class),
                Item::Trait(_) | Item::ExportTrait(_) | Item::Module(_) | Item::Use(_) => {}
                Item::Stmt(stmt) => self.statement(stmt),
            }
        }
        SemanticResult {
            symbols: self.symbols,
            resolutions: self.resolutions,
            types: self.types,
            coercions: self.coercions,
            classes: self.classes,
            traits: self.traits,
            diagnostics: self.diagnostics,
        }
    }

    fn signature(
        &mut self,
        parameters: &[crate::ast::Parameter],
        result: &Option<TypeName>,
    ) -> Type {
        Type::Function(
            parameters
                .iter()
                .map(|value| self.resolve_type(&value.type_name))
                .collect(),
            Box::new(
                result
                    .as_ref()
                    .map_or(Type::Void, |value| self.resolve_type(value)),
            ),
        )
    }

    fn declare_trait(&mut self, declaration: &TraitDecl) {
        let mut methods = HashMap::new();
        for method in &declaration.methods {
            let ty = self.signature(&method.parameters, &method.return_type);
            if methods.insert(method.name.clone(), ty).is_some() {
                self.type_error(
                    "E2010",
                    format!("miembro duplicado: {}", method.name),
                    method.name_span,
                );
            }
        }
        self.traits
            .insert(declaration.name.clone(), TraitInfo { methods });
    }

    fn declare_class(&mut self, declaration: &ClassDecl) {
        let mut fields = HashMap::new();
        let mut methods = HashMap::new();
        let mut constructor = None;
        let mut member_names = HashSet::new();
        for member in &declaration.members {
            let (name, value, target) = match member {
                ClassMember::Field(field) => {
                    let ty = field
                        .variable
                        .annotation
                        .as_ref()
                        .map_or(Type::Error, |value| self.resolve_type(value));
                    (
                        &field.variable.name,
                        Member {
                            ty,
                            visibility: field.visibility,
                            is_static: field.is_static,
                            mutable: field.variable.mutable,
                            declaration_span: field.variable.name_span,
                        },
                        &mut fields,
                    )
                }
                ClassMember::Method(method) => {
                    let ty =
                        self.signature(&method.function.parameters, &method.function.return_type);
                    (
                        &method.function.name,
                        Member {
                            ty,
                            visibility: method.visibility,
                            is_static: method.is_static,
                            mutable: false,
                            declaration_span: method.function.name_span,
                        },
                        &mut methods,
                    )
                }
                ClassMember::Constructor(method) => {
                    let value = Member {
                        ty: self.signature(&method.function.parameters, &None),
                        visibility: method.visibility,
                        is_static: false,
                        mutable: false,
                        declaration_span: method.function.name_span,
                    };
                    if constructor.replace(value).is_some() {
                        self.type_error(
                            "E2010",
                            "constructor duplicado",
                            method.function.name_span,
                        );
                    }
                    continue;
                }
            };
            if !member_names.insert(name.clone()) {
                self.type_error(
                    "E2010",
                    format!("miembro duplicado: {name}"),
                    declaration.name_span,
                );
                continue;
            }
            if target.insert(name.clone(), value).is_some() {
                self.type_error(
                    "E2010",
                    format!("miembro duplicado: {name}"),
                    declaration.name_span,
                );
            }
        }
        let traits: Vec<_> = declaration
            .traits
            .iter()
            .map(|value| value.name.clone())
            .collect();
        for trait_name in &traits {
            let Some(required) = self.traits.get(trait_name).cloned() else {
                self.type_error(
                    "E2007",
                    format!("trait desconocido: {trait_name}"),
                    declaration.name_span,
                );
                continue;
            };
            for (name, signature) in &required.methods {
                if methods.get(name).map(|value| &value.ty) != Some(signature) {
                    self.type_error(
                        "E2011",
                        format!("{} no implementa {trait_name}.{name}", declaration.name),
                        declaration.name_span,
                    );
                }
            }
        }
        self.classes.insert(
            declaration.name.clone(),
            ClassInfo {
                fields,
                methods,
                constructor,
                traits,
            },
        );
    }

    fn class_bodies(&mut self, declaration: &ClassDecl) {
        self.current_class = Some(declaration.name.clone());
        for member in &declaration.members {
            match member {
                ClassMember::Field(field) => {
                    let found = self.expression(&field.variable.initializer);
                    let expected = self.classes[&declaration.name].fields[&field.variable.name]
                        .ty
                        .clone();
                    if !self.assignable(&expected, &found, field.variable.initializer.id) {
                        self.type_error(
                            "E2005",
                            format!("no se puede asignar {found:?} a {expected:?}"),
                            field.variable.initializer.span,
                        );
                    }
                }
                ClassMember::Method(method) => self.method_body(method, &declaration.name),
                ClassMember::Constructor(method) => self.method_body(method, &declaration.name),
            };
        }
        self.current_class = None;
    }

    fn detect_composition_cycles(&mut self) {
        for name in self.classes.keys().cloned().collect::<Vec<_>>() {
            if self.composes(&name, &name, &mut Vec::new()) {
                let span = self
                    .lookup(&name)
                    .map_or(Span::new(crate::source::SourceId(0), 0, 0), |id| {
                        self.symbols[id.0].declaration_span
                    });
                self.type_error(
                    "E2014",
                    format!("ciclo de composición por valor: {name}"),
                    span,
                );
            }
        }
    }

    fn composes(&self, current: &str, target: &str, visited: &mut Vec<String>) -> bool {
        if visited.iter().any(|value| value == current) {
            return false;
        }
        visited.push(current.to_owned());
        let found = self.classes.get(current).is_some_and(|class| {
            class.fields.values().any(|field| match &field.ty {
                Type::Class(next) => next == target || self.composes(next, target, visited),
                _ => false,
            })
        });
        visited.pop();
        found
    }

    fn method_body(&mut self, method: &MethodDecl, class: &str) {
        let parameters: Vec<_> = method
            .function
            .parameters
            .iter()
            .map(|value| self.resolve_type(&value.type_name))
            .collect();
        let result = method
            .function
            .return_type
            .as_ref()
            .map_or(Type::Void, |value| self.resolve_type(value));
        self.scopes.push(HashMap::new());
        if !method.is_static {
            self.declare(
                "this",
                SymbolKind::Parameter,
                Type::Class(class.to_owned()),
                false,
                true,
                method.function.name_span,
            );
        }
        for (parameter, ty) in method.function.parameters.iter().zip(parameters) {
            self.declare(
                &parameter.name,
                SymbolKind::Parameter,
                ty,
                false,
                true,
                parameter.span,
            );
        }
        let previous = self.expected_return.replace(result);
        self.statement(&method.function.body);
        self.expected_return = previous;
        self.scopes.pop();
        if method
            .function
            .return_type
            .as_ref()
            .is_some_and(|ty| ty.name != "void")
            && !always_returns(&method.function.body)
        {
            self.type_error(
                "E2009",
                "no todas las rutas retornan un valor",
                method.function.span,
            );
        }
    }

    fn declaration(&mut self, declaration: &Decl) {
        match declaration {
            Decl::Variable(decl) => self.variable(decl),
            Decl::Function(decl) => self.function_body(decl),
        }
    }

    fn variable(&mut self, declaration: &VariableDecl) {
        let annotation = declaration
            .annotation
            .as_ref()
            .map(|name| self.resolve_type(name));
        let id = self.declare(
            &declaration.name,
            SymbolKind::Variable,
            annotation.clone().unwrap_or(Type::Error),
            declaration.mutable,
            false,
            declaration.name_span,
        );
        let initializer = self.expression(&declaration.initializer);
        let ty = annotation.unwrap_or_else(|| initializer.clone());
        if !self.assignable(&ty, &initializer, declaration.initializer.id) {
            self.type_error(
                "E2005",
                format!("no se puede asignar {initializer:?} a {ty:?}"),
                declaration.initializer.span,
            );
        }
        if let Some(id) = id {
            self.symbols[id.0].ty = ty;
            self.symbols[id.0].initialized = true;
        }
    }

    fn declare_function(&mut self, function: &FunctionDecl) {
        let parameters: Vec<_> = function
            .parameters
            .iter()
            .map(|parameter| self.resolve_type(&parameter.type_name))
            .collect();
        let return_type = function
            .return_type
            .as_ref()
            .map_or(Type::Void, |name| self.resolve_type(name));
        self.declare(
            &function.name,
            SymbolKind::Function,
            Type::Function(parameters.clone(), Box::new(return_type.clone())),
            false,
            true,
            function.name_span,
        );
    }

    fn function_body(&mut self, function: &FunctionDecl) {
        let parameters: Vec<_> = function
            .parameters
            .iter()
            .map(|parameter| self.resolve_type(&parameter.type_name))
            .collect();
        let return_type = function
            .return_type
            .as_ref()
            .map_or(Type::Void, |name| self.resolve_type(name));
        self.scopes.push(HashMap::new());
        for (parameter, ty) in function.parameters.iter().zip(parameters) {
            self.declare(
                &parameter.name,
                SymbolKind::Parameter,
                ty,
                false,
                true,
                parameter.span,
            );
        }
        let previous = self.expected_return.replace(return_type.clone());
        self.statement(&function.body);
        self.expected_return = previous;
        self.scopes.pop();
        if return_type != Type::Void
            && return_type != Type::Error
            && !always_returns(&function.body)
        {
            self.type_error(
                "E2009",
                "no todas las rutas retornan un valor",
                function.span,
            );
        }
    }

    fn statement(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Variable(decl) => self.variable(decl),
            Stmt::Block { statements, .. } => {
                self.scopes.push(HashMap::new());
                let mut unreachable = false;
                for statement in statements {
                    if unreachable {
                        self.diagnostics.push(Diagnostic::warning(
                            "W2001",
                            "sentencia inalcanzable",
                            statement.span(),
                        ));
                    }
                    self.statement(statement);
                    unreachable |= terminates(statement);
                }
                self.scopes.pop();
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.condition(condition);
                self.statement(then_branch);
                if let Some(branch) = else_branch {
                    self.statement(branch);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.condition(condition);
                self.loop_depth += 1;
                self.statement(body);
                self.loop_depth -= 1;
            }
            Stmt::Break { span } | Stmt::Continue { span } => {
                if self.loop_depth == 0 {
                    self.type_error(
                        "E2008",
                        "el control de ciclo solo es válido dentro de un ciclo",
                        *span,
                    );
                }
            }
            Stmt::Return { value, span } => {
                let found = value
                    .as_ref()
                    .map_or(Type::Void, |value| self.expression(value));
                let expected = self.expected_return.clone().unwrap_or(Type::Error);
                if self.expected_return.is_none() {
                    self.type_error(
                        "E2008",
                        "`return` solo es válido dentro de una función",
                        *span,
                    );
                } else if !self.assignable(
                    &expected,
                    &found,
                    value.as_ref().map_or(ExprId(usize::MAX), |value| value.id),
                ) {
                    self.type_error(
                        "E2005",
                        format!("se esperaba retorno {expected:?}; se encontró {found:?}"),
                        *span,
                    );
                }
            }
            Stmt::Expression { expression, .. } => {
                self.expression(expression);
            }
        }
    }

    fn condition(&mut self, expression: &Expr) {
        let ty = self.expression(expression);
        if ty != Type::Bool && ty != Type::Error {
            self.type_error("E2006", "la condición debe ser bool", expression.span);
        }
    }

    fn expression(&mut self, expression: &Expr) -> Type {
        let ty = match &expression.kind {
            ExprKind::Literal(literal) => literal_type(literal),
            ExprKind::Name(name) => self.name(expression.id, name, expression.span),
            ExprKind::Unary { op, operand } => {
                let operand = self.expression(operand);
                match op {
                    UnaryOp::Not if operand == Type::Bool => Type::Bool,
                    UnaryOp::Plus | UnaryOp::Minus if numeric(&operand) => operand,
                    _ if operand == Type::Error => Type::Error,
                    _ => {
                        self.type_error(
                            "E2004",
                            format!("operador unario inválido para {operand:?}"),
                            expression.span,
                        );
                        Type::Error
                    }
                }
            }
            ExprKind::Binary { left, op, right } => {
                let left_type = self.expression(left);
                let right_type = self.expression(right);
                self.binary(
                    *op,
                    (left.id, left_type),
                    (right.id, right_type),
                    expression.span,
                )
            }
            ExprKind::Assign { target, value } => self.assignment(target, value, expression.span),
            ExprKind::Cast {
                expression: value,
                type_name,
            } => {
                let from = self.expression(value);
                let to = self.resolve_type(type_name);
                if from == to || numeric(&from) && numeric(&to) {
                    to
                } else if from == Type::Error || to == Type::Error {
                    Type::Error
                } else {
                    self.type_error(
                        "E2007",
                        format!("cast inválido de {from:?} a {to:?}"),
                        expression.span,
                    );
                    Type::Error
                }
            }
            ExprKind::Call { callee, arguments } => {
                let callee_type = self.expression(callee);
                let argument_types: Vec<_> = arguments
                    .iter()
                    .map(|argument| self.expression(argument))
                    .collect();
                if let Type::ClassObject(name) = callee_type {
                    let constructor = self
                        .classes
                        .get(&name)
                        .and_then(|class| class.constructor.clone());
                    if constructor.as_ref().is_some_and(|value| {
                        value.visibility == Visibility::Priv
                            && self.current_class.as_deref() != Some(&name)
                    }) {
                        self.type_error("E2013", "constructor privado", expression.span);
                    }
                    let parameters = constructor
                        .and_then(|value| match value.ty {
                            Type::Function(parameters, _) => Some(parameters),
                            _ => None,
                        })
                        .unwrap_or_default();
                    self.check_arguments(&parameters, &argument_types, arguments, expression.span);
                    Type::Class(name)
                } else if let Type::Function(parameters, result) = callee_type {
                    self.check_arguments(&parameters, &argument_types, arguments, expression.span);
                    *result
                } else if callee_type == Type::Error {
                    Type::Error
                } else {
                    self.type_error("E2004", "solo se pueden llamar funciones", callee.span);
                    Type::Error
                }
            }
            ExprKind::Member { object, name } => {
                if let ExprKind::Name(module) = &object.kind
                    && self.lookup(&format!("{module}.{name}")).is_some()
                {
                    let ty = self.name(expression.id, &format!("{module}.{name}"), expression.span);
                    return self.record_type(expression, ty);
                }
                let object_type = self.expression(object);
                self.member(&object_type, name, expression.span)
            }
            ExprKind::Index { object, index } => {
                self.expression(object);
                self.expression(index);
                self.type_error(
                    "E2015",
                    "la indexación todavía no está disponible",
                    expression.span,
                );
                Type::Error
            }
        };
        self.record_type(expression, ty)
    }

    fn record_type(&mut self, expression: &Expr, ty: Type) -> Type {
        self.types.insert(expression.id, ty.clone());
        ty
    }

    fn check_arguments(
        &mut self,
        parameters: &[Type],
        found: &[Type],
        arguments: &[Expr],
        span: Span,
    ) {
        if parameters.len() != found.len() {
            self.type_error("E2004", "cantidad de argumentos incorrecta", span);
        }
        for ((expected, actual), argument) in parameters.iter().zip(found).zip(arguments) {
            if !self.assignable(expected, actual, argument.id) {
                self.type_error(
                    "E2005",
                    format!("se esperaba {expected:?}; se encontró {actual:?}"),
                    argument.span,
                );
            }
        }
    }

    fn member(&mut self, object: &Type, name: &str, span: Span) -> Type {
        let (class_name, static_access) = match object {
            Type::Class(name) => (name, false),
            Type::ClassObject(name) => (name, true),
            Type::Trait(trait_name) => {
                return self
                    .traits
                    .get(trait_name)
                    .and_then(|value| value.methods.get(name))
                    .cloned()
                    .unwrap_or_else(|| {
                        self.type_error("E2012", format!("miembro inexistente: {name}"), span);
                        Type::Error
                    });
            }
            Type::Error => return Type::Error,
            _ => {
                self.type_error("E2012", "el valor no tiene miembros", span);
                return Type::Error;
            }
        };
        let Some(member) = self
            .classes
            .get(class_name)
            .and_then(|class| class.fields.get(name).or_else(|| class.methods.get(name)))
            .cloned()
        else {
            self.type_error("E2012", format!("miembro inexistente: {name}"), span);
            return Type::Error;
        };
        if member.is_static != static_access {
            self.type_error("E2012", "acceso estático/de instancia inválido", span);
        }
        if member.visibility == Visibility::Priv
            && self.current_class.as_deref() != Some(class_name)
        {
            self.type_error("E2013", format!("miembro privado: {name}"), span);
        }
        member.ty
    }

    fn name(&mut self, expression: ExprId, name: &str, span: Span) -> Type {
        let Some(id) = self.lookup(name) else {
            self.type_error("E2002", format!("nombre no definido: {name}"), span);
            return Type::Error;
        };
        self.resolutions.insert(expression, id);
        let symbol = &self.symbols[id.0];
        if !symbol.initialized {
            self.type_error(
                "E2003",
                format!("`{name}` se usa antes de inicializarse"),
                span,
            );
            return Type::Error;
        }
        symbol.ty.clone()
    }

    fn assignment(&mut self, target: &Expr, value: &Expr, span: Span) -> Type {
        let value_type = self.expression(value);
        if let ExprKind::Member { object, name } = &target.kind {
            let object_type = self.expression(object);
            let target_type = self.member(&object_type, name, target.span);
            let class_name = match &object_type {
                Type::Class(name) | Type::ClassObject(name) => Some(name),
                _ => None,
            };
            if class_name
                .and_then(|class| self.classes.get(class))
                .and_then(|class| class.fields.get(name))
                .is_some_and(|field| !field.mutable)
            {
                self.type_error("E2003", format!("`{name}` es inmutable"), target.span);
            }
            if !self.assignable(&target_type, &value_type, value.id) {
                self.type_error(
                    "E2005",
                    format!("no se puede asignar {value_type:?} a {target_type:?}"),
                    span,
                );
            }
            self.types.insert(target.id, target_type.clone());
            return target_type;
        }
        let ExprKind::Name(name) = &target.kind else {
            self.type_error(
                "E2004",
                "el destino de una asignación debe ser una variable",
                target.span,
            );
            return Type::Error;
        };
        let Some(id) = self.lookup(name) else {
            self.type_error("E2002", format!("nombre no definido: {name}"), target.span);
            return Type::Error;
        };
        self.resolutions.insert(target.id, id);
        let symbol = self.symbols[id.0].clone();
        self.types.insert(target.id, symbol.ty.clone());
        if !symbol.mutable {
            self.related_error(
                "E2003",
                format!("`{name}` es inmutable"),
                target.span,
                symbol.declaration_span,
            );
        }
        if !self.assignable(&symbol.ty, &value_type, value.id) {
            self.type_error(
                "E2005",
                format!("no se puede asignar {value_type:?} a {:?}", symbol.ty),
                span,
            );
        }
        symbol.ty
    }

    fn binary(
        &mut self,
        op: BinaryOp,
        left: (ExprId, Type),
        right: (ExprId, Type),
        span: Span,
    ) -> Type {
        let (left_id, left) = left;
        let (right_id, right) = right;
        if left == Type::Error || right == Type::Error {
            return Type::Error;
        }
        if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual)
            && matches!(left, Type::Class(_) | Type::Trait(_))
            && matches!(right, Type::Class(_) | Type::Trait(_))
            && (self.assignable(&left, &right, right_id) || self.assignable(&right, &left, left_id))
        {
            return Type::Bool;
        }
        match op {
            BinaryOp::And | BinaryOp::Or if left == Type::Bool && right == Type::Bool => Type::Bool,
            BinaryOp::Equal | BinaryOp::NotEqual if left == right => Type::Bool,
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual
                if left == right && numeric(&left) =>
            {
                Type::Bool
            }
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
            | BinaryOp::Power
                if left == right && numeric(&left) =>
            {
                left
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual
                if integer_rank(&left).is_some() && integer_rank(&right).is_some() =>
            {
                self.widen_operands(left_id, &left, right_id, &right);
                Type::Bool
            }
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
            | BinaryOp::Power
                if integer_rank(&left).is_some() && integer_rank(&right).is_some() =>
            {
                self.widen_operands(left_id, &left, right_id, &right)
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual
                if decimal_and_integer(&left, &right) =>
            {
                self.promote_decimal(left_id, &left, right_id, &right);
                Type::Bool
            }
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder
                if decimal_and_integer(&left, &right) =>
            {
                self.promote_decimal(left_id, &left, right_id, &right);
                Type::Decimal
            }
            _ => {
                self.type_error(
                    "E2004",
                    format!("operación inválida entre {left:?} y {right:?}"),
                    span,
                );
                Type::Error
            }
        }
    }

    fn widen_operands(
        &mut self,
        left_id: ExprId,
        left: &Type,
        right_id: ExprId,
        right: &Type,
    ) -> Type {
        let result = if integer_rank(left) >= integer_rank(right) {
            left
        } else {
            right
        }
        .clone();
        if left != &result {
            self.coercions.insert(left_id, result.clone());
        }
        if right != &result {
            self.coercions.insert(right_id, result.clone());
        }
        result
    }

    fn promote_decimal(&mut self, left_id: ExprId, left: &Type, right_id: ExprId, right: &Type) {
        if left != &Type::Decimal {
            self.coercions.insert(left_id, Type::Decimal);
        }
        if right != &Type::Decimal {
            self.coercions.insert(right_id, Type::Decimal);
        }
    }

    fn declare(
        &mut self,
        name: &str,
        kind: SymbolKind,
        ty: Type,
        mutable: bool,
        initialized: bool,
        span: Span,
    ) -> Option<SymbolId> {
        if let Some(previous) = self
            .scopes
            .last()
            .and_then(|scope| scope.get(name))
            .copied()
        {
            self.related_error(
                "E2001",
                format!("declaración duplicada: {name}"),
                span,
                self.symbols[previous.0].declaration_span,
            );
            return None;
        }
        let id = SymbolId(self.symbols.len());
        self.symbols.push(Symbol {
            id,
            name: name.to_owned(),
            kind,
            ty,
            mutable,
            initialized,
            declaration_span: span,
        });
        self.scopes
            .last_mut()
            .expect("always has global scope")
            .insert(name.to_owned(), id);
        Some(id)
    }

    fn lookup(&self, name: &str) -> Option<SymbolId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
    fn assignable(&mut self, expected: &Type, found: &Type, expression: ExprId) -> bool {
        if expected == found || *expected == Type::Error || *found == Type::Error {
            return true;
        }
        if *expected == Type::IntegerOrDecimal {
            return integer_rank(found).is_some() || *found == Type::Decimal;
        }
        if integer_rank(found)
            .is_some_and(|from| integer_rank(expected).is_some_and(|to| from <= to))
        {
            self.coercions.insert(expression, expected.clone());
            return true;
        }
        if let Type::Trait(name) = expected
            && let Type::Class(class) = found
            && self
                .classes
                .get(class)
                .is_some_and(|value| value.traits.contains(name))
        {
            return true;
        }
        false
    }
    fn resolve_type(&mut self, name: &TypeName) -> Type {
        match name.name.as_str() {
            "byte" => Type::Byte,
            "short" => Type::Short,
            "int" => Type::Int,
            "long" => Type::Long,
            "decimal" => Type::Decimal,
            "float" => Type::Float,
            "double" => Type::Double,
            "bool" => Type::Bool,
            "str" => Type::Str,
            "char" => Type::Char,
            "void" => Type::Void,
            value if self.classes.contains_key(value) => Type::Class(value.to_owned()),
            value if self.traits.contains_key(value) => Type::Trait(value.to_owned()),
            value => match self.lookup(value).map(|id| self.symbols[id.0].ty.clone()) {
                Some(Type::ClassObject(class)) => Type::Class(class),
                Some(Type::Trait(trait_name)) => Type::Trait(trait_name),
                _ => {
                    self.type_error(
                        "E2007",
                        format!("tipo desconocido: {}", name.name),
                        name.span,
                    );
                    Type::Error
                }
            },
        }
    }
    fn type_error(&mut self, code: &str, message: impl Into<String>, span: Span) {
        self.diagnostics
            .push(Diagnostic::error(code, message, span));
    }
    fn related_error(
        &mut self,
        code: &str,
        message: impl Into<String>,
        span: Span,
        declaration: Span,
    ) {
        let mut diagnostic = Diagnostic::error(code, message, span);
        diagnostic.labels.push(Label {
            span: declaration,
            message: "declarado aquí".to_owned(),
        });
        self.diagnostics.push(diagnostic);
    }
}

fn numeric(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Byte
            | Type::Short
            | Type::Int
            | Type::Long
            | Type::Decimal
            | Type::Float
            | Type::Double
    )
}
fn integer_rank(ty: &Type) -> Option<u8> {
    Some(match ty {
        Type::Byte => 0,
        Type::Short => 1,
        Type::Int => 2,
        Type::Long => 3,
        _ => return None,
    })
}
fn decimal_and_integer(left: &Type, right: &Type) -> bool {
    (*left == Type::Decimal && integer_rank(right).is_some())
        || (*right == Type::Decimal && integer_rank(left).is_some())
}
fn literal_type(literal: &Literal) -> Type {
    match literal {
        Literal::Bool(_) => Type::Bool,
        Literal::String(_) => Type::Str,
        Literal::Char(_) => Type::Char,
        Literal::Number(value) if value.ends_with("dec") => Type::Decimal,
        Literal::Number(value) if value.ends_with("f32") => Type::Float,
        Literal::Number(value) if value.ends_with("f64") || value.contains(['.', 'e', 'E']) => {
            Type::Double
        }
        Literal::Number(value) if value.ends_with("i8") => Type::Byte,
        Literal::Number(value) if value.ends_with("i16") => Type::Short,
        Literal::Number(value) if value.ends_with("i64") => Type::Long,
        Literal::Number(_) => Type::Int,
    }
}

fn terminates(statement: &Stmt) -> bool {
    matches!(
        statement,
        Stmt::Return { .. } | Stmt::Break { .. } | Stmt::Continue { .. }
    ) || always_returns(statement)
}

fn always_returns(statement: &Stmt) -> bool {
    match statement {
        Stmt::Return { .. } => true,
        Stmt::Block { statements, .. } => statements.iter().any(always_returns),
        Stmt::If {
            then_branch,
            else_branch: Some(else_branch),
            ..
        } => always_returns(then_branch) && always_returns(else_branch),
        _ => false,
    }
}
