use crate::{
    AnalysisResult, CompilationResult,
    ast::*,
    diagnostic::Severity,
    project::{ModuleAnalysis, ProjectAnalysis},
    semantic::{SemanticResult, SymbolId, Type},
    source::SourceId,
};
use std::collections::{HashMap, HashSet};

pub(crate) fn compile(project: ProjectAnalysis, entry: &str) -> CompilationResult {
    if project
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        return CompilationResult {
            sources: project.sources,
            chunk: None,
            diagnostics: project.diagnostics,
        };
    }
    let modules: HashMap<_, _> = project
        .modules
        .iter()
        .map(|m| (m.name.as_str(), m))
        .collect();
    let sources: HashMap<_, _> = project
        .modules
        .iter()
        .filter_map(|m| source(m).map(|s| (s, m.name.as_str())))
        .collect();
    let mut order = Vec::new();
    fn visit<'a>(
        name: &str,
        modules: &HashMap<&str, &'a ModuleAnalysis>,
        seen: &mut HashSet<String>,
        order: &mut Vec<&'a ModuleAnalysis>,
    ) {
        if !seen.insert(name.into()) {
            return;
        }
        if let Some(module) = modules.get(name) {
            for dependency in &module.dependencies {
                visit(dependency, modules, seen, order);
            }
            order.push(module);
        }
    }
    visit(entry, &modules, &mut HashSet::new(), &mut order);
    let mut program = Program { items: Vec::new() };
    let mut next_id = 0;
    for module in order {
        let Some(semantic) = &module.semantic else {
            continue;
        };
        let mut names = HashMap::new();
        for symbol in &semantic.symbols {
            if let Some(origin) = sources.get(&symbol.declaration_span.source) {
                if Some(symbol.declaration_span.source) != source(module) {
                    names.insert(
                        symbol.id,
                        format!(
                            "{origin}::{}",
                            symbol.name.rsplit('.').next().unwrap_or(&symbol.name)
                        ),
                    );
                } else if module.program.items.iter().any(|item| match item {
                    Item::Decl(Decl::Variable(v)) | Item::Export(Decl::Variable(v)) => {
                        v.name_span == symbol.declaration_span
                    }
                    Item::Decl(Decl::Function(f)) | Item::Export(Decl::Function(f)) => {
                        f.name_span == symbol.declaration_span
                    }
                    Item::Class(c) | Item::ExportClass(c) => c.name_span == symbol.declaration_span,
                    Item::Trait(t) | Item::ExportTrait(t) => t.name_span == symbol.declaration_span,
                    _ => false,
                }) {
                    names.insert(symbol.id, format!("{}::{}", module.name, symbol.name));
                }
            }
        }
        let mut rewrite = Rewrite {
            module: &module.name,
            semantic,
            names,
            next_id: &mut next_id,
        };
        for mut item in module.program.items.clone() {
            if matches!(item, Item::Module(_) | Item::Use(_)) {
                continue;
            }
            rewrite.item(&mut item);
            program.items.push(item);
        }
    }
    let semantic = crate::semantic::analyze(&program);
    let mut diagnostics = project.diagnostics;
    // Each module was already checked in isolation; linking can introduce identity errors.
    for diagnostic in &semantic.diagnostics {
        if !diagnostics.contains(diagnostic) {
            diagnostics.push(diagnostic.clone());
        }
    }
    crate::compile_analysis(AnalysisResult {
        sources: project.sources,
        tokens: Vec::new(),
        program,
        semantic: Some(semantic),
        diagnostics,
    })
}
fn source(module: &ModuleAnalysis) -> Option<SourceId> {
    module.program.items.iter().find_map(|i| {
        if let Item::Module(m) = i {
            Some(m.span.source)
        } else {
            None
        }
    })
}
struct Rewrite<'a> {
    module: &'a str,
    semantic: &'a SemanticResult,
    names: HashMap<SymbolId, String>,
    next_id: &'a mut usize,
}
impl Rewrite<'_> {
    fn qualified(&self, name: &str) -> String {
        format!("{}::{name}", self.module)
    }
    fn ty(&self, ty: &mut TypeName) {
        if self.semantic.classes.contains_key(&ty.name)
            || self.semantic.traits.contains_key(&ty.name)
        {
            if !ty.name.contains("::") {
                ty.name = self.qualified(&ty.name);
            }
            return;
        }
        if let Some(symbol) = self.semantic.symbols.iter().find(|s| s.name == ty.name)
            && let Type::ClassObject(name) | Type::Trait(name) = &symbol.ty
        {
            ty.name = name.clone();
        }
    }
    fn function(&mut self, f: &mut FunctionDecl) {
        for param in &mut f.parameters {
            self.ty(&mut param.type_name);
        }
        if let Some(ty) = &mut f.return_type {
            self.ty(ty);
        }
        self.statement(&mut f.body);
    }
    fn variable(&mut self, v: &mut VariableDecl) {
        if let Some(ty) = &mut v.annotation {
            self.ty(ty);
        }
        self.expr(&mut v.initializer);
    }
    fn item(&mut self, item: &mut Item) {
        match item {
            Item::Decl(decl) | Item::Export(decl) => match decl {
                Decl::Variable(v) => {
                    v.name = self.qualified(&v.name);
                    self.variable(v);
                }
                Decl::Function(f) => {
                    f.name = self.qualified(&f.name);
                    self.function(f);
                }
            },
            Item::Class(c) | Item::ExportClass(c) => {
                c.name = self.qualified(&c.name);
                for ty in &mut c.traits {
                    self.ty(ty);
                }
                for member in &mut c.members {
                    match member {
                        ClassMember::Field(f) => self.variable(&mut f.variable),
                        ClassMember::Method(m) | ClassMember::Constructor(m) => {
                            self.function(&mut m.function)
                        }
                    }
                }
            }
            Item::Trait(t) | Item::ExportTrait(t) => {
                t.name = self.qualified(&t.name);
                for f in &mut t.methods {
                    for p in &mut f.parameters {
                        self.ty(&mut p.type_name);
                    }
                    if let Some(ty) = &mut f.return_type {
                        self.ty(ty);
                    }
                }
            }
            Item::Stmt(stmt) => self.statement(stmt),
            _ => {}
        }
    }
    fn statement(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Variable(v) => self.variable(v),
            Stmt::Block { statements, .. } => {
                for s in statements {
                    self.statement(s);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expr(condition);
                self.statement(then_branch);
                if let Some(b) = else_branch {
                    self.statement(b);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expr(condition);
                self.statement(body);
            }
            Stmt::Return { value: Some(v), .. } => self.expr(v),
            Stmt::Expression { expression, .. } => self.expr(expression),
            _ => {}
        }
    }
    fn expr(&mut self, expr: &mut Expr) {
        if let Some(name) = self
            .semantic
            .resolutions
            .get(&expr.id)
            .and_then(|id| self.names.get(id))
        {
            expr.kind = ExprKind::Name(name.clone());
        } else {
            match &mut expr.kind {
                ExprKind::Unary { operand, .. } => self.expr(operand),
                ExprKind::Binary { left, right, .. } => {
                    self.expr(left);
                    self.expr(right);
                }
                ExprKind::Assign { target, value } => {
                    self.expr(target);
                    self.expr(value);
                }
                ExprKind::Call { callee, arguments } => {
                    self.expr(callee);
                    for a in arguments {
                        self.expr(a);
                    }
                }
                ExprKind::Member { object, .. } => self.expr(object),
                ExprKind::Index { object, index } => {
                    self.expr(object);
                    self.expr(index);
                }
                ExprKind::Cast {
                    expression,
                    type_name,
                } => {
                    self.expr(expression);
                    self.ty(type_name);
                }
                _ => {}
            }
        }
        expr.id = ExprId(*self.next_id);
        *self.next_id += 1;
    }
}
