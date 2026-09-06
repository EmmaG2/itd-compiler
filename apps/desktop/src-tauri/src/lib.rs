use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use compiler_core::{
    AnalysisResult,
    ast::{
        ClassDecl, ClassMember, Decl, Expr, ExprKind, FunctionDecl, FunctionSignature, Item,
        Program, Stmt, TypeName, VariableDecl,
    },
    compile_source,
    diagnostic::Diagnostic,
    project::{Project, ProjectAnalysis},
    run_compiled,
    runtime::RuntimeLimits,
    source::{SourceMap, Span},
};
use serde::Serialize;
use tauri::State;

const MAX_SOURCE_BYTES: usize = 1_048_576;
const MAX_PATH_BYTES: usize = 1_024;
const MAX_DIAGNOSTICS: usize = 1_000;
const MAX_TOKENS: usize = 20_000;
const MAX_SYMBOLS: usize = 5_000;
const MAX_AST_NODES: usize = 2_000;
mod inspection;
use inspection::{BytecodeDto, bytecode_dto};

#[derive(Default)]
struct AppState {
    project: Mutex<Option<Project>>,
    runs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandError {
    code: &'static str,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocationDto {
    file: String,
    start: usize,
    end: usize,
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosticDto {
    code: String,
    severity: String,
    message: String,
    location: LocationDto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenDto {
    kind: String,
    lexeme: String,
    location: LocationDto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SymbolDto {
    name: String,
    kind: String,
    ty: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AstNodeDto {
    id: String,
    parent_id: Option<String>,
    relation: Option<String>,
    kind: String,
    label: String,
    location: Option<LocationDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AstGraphDto {
    nodes: Vec<AstNodeDto>,
    truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnalysisDto {
    diagnostics: Vec<DiagnosticDto>,
    tokens: Vec<TokenDto>,
    symbols: Vec<SymbolDto>,
    ast: AstGraphDto,
    bytecode: BytecodeDto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunDto {
    output: String,
    diagnostics: Vec<DiagnosticDto>,
    bytecode: BytecodeDto,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDto {
    modules: Vec<String>,
    diagnostics: Vec<DiagnosticDto>,
}

#[tauri::command]
async fn analyze_source(source: String, virtual_path: String) -> Result<AnalysisDto, CommandError> {
    validate_source(&source, &virtual_path)?;
    tauri::async_runtime::spawn_blocking(move || {
        analysis_dto(compiler_core::analyze(&source), &virtual_path)
    })
    .await
    .map_err(|_| internal_error())
}

#[tauri::command]
async fn run_source(
    source: String,
    virtual_path: String,
    execution_id: String,
    state: State<'_, AppState>,
) -> Result<RunDto, CommandError> {
    validate_source(&source, &virtual_path)?;
    validate_id(&execution_id)?;
    let cancelled = register_run(&execution_id, &state)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let compilation = compile_source(&source);
        let result = run_compiled(&compilation, RuntimeLimits::default(), Some(cancelled));
        RunDto {
            output: result.output,
            diagnostics: diagnostics_dto(&result.diagnostics, &compilation.sources, &virtual_path),
            bytecode: bytecode_dto(
                compilation.chunk.as_ref(),
                &compilation.sources,
                &virtual_path,
            ),
        }
    })
    .await
    .map_err(|_| internal_error());
    state
        .runs
        .lock()
        .map_err(|_| internal_error())?
        .remove(&execution_id);
    result
}

fn register_run(execution_id: &str, state: &AppState) -> Result<Arc<AtomicBool>, CommandError> {
    validate_id(execution_id)?;
    let mut runs = state.runs.lock().map_err(|_| internal_error())?;
    if runs.contains_key(execution_id) {
        return Err(CommandError {
            code: "RUN_EXISTS",
            message: "La ejecución ya está activa".into(),
        });
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    runs.insert(execution_id.into(), Arc::clone(&cancelled));
    Ok(cancelled)
}

#[tauri::command]
fn cancel_run(execution_id: String, state: State<'_, AppState>) -> Result<(), CommandError> {
    validate_id(&execution_id)?;
    let runs = state.runs.lock().map_err(|_| internal_error())?;
    let Some(cancelled) = runs.get(&execution_id) else {
        return Ok(());
    };
    cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn open_project(root: String, state: State<'_, AppState>) -> Result<ProjectDto, CommandError> {
    validate_path(&root)?;
    let project = Project::open(Path::new(&root)).map_err(|_| CommandError {
        code: "INVALID_PROJECT",
        message: "La carpeta seleccionada no es un proyecto válido".to_owned(),
    })?;
    *state.project.lock().map_err(|_| internal_error())? = Some(project);
    Ok(ProjectDto {
        modules: Vec::new(),
        diagnostics: Vec::new(),
    })
}

#[tauri::command]
async fn analyze_project(
    entry_module: String,
    state: State<'_, AppState>,
) -> Result<ProjectDto, CommandError> {
    validate_path(&entry_module)?;
    let project = state
        .project
        .lock()
        .map_err(|_| internal_error())?
        .clone()
        .ok_or_else(|| CommandError {
            code: "NO_PROJECT",
            message: "Primero selecciona una carpeta de proyecto".to_owned(),
        })?;
    tauri::async_runtime::spawn_blocking(move || project_dto(project.analyze(&entry_module)))
        .await
        .map_err(|_| internal_error())
}

#[tauri::command]
async fn run_project(
    entry_module: String,
    execution_id: String,
    state: State<'_, AppState>,
) -> Result<RunDto, CommandError> {
    validate_path(&entry_module)?;
    let project = state
        .project
        .lock()
        .map_err(|_| internal_error())?
        .clone()
        .ok_or_else(|| CommandError {
            code: "NO_PROJECT",
            message: "Primero selecciona una carpeta de proyecto".into(),
        })?;
    let cancelled = register_run(&execution_id, &state)?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        let compilation = project.compile(&entry_module);
        let result = run_compiled(&compilation, RuntimeLimits::default(), Some(cancelled));
        RunDto {
            output: result.output,
            diagnostics: diagnostics_dto(&result.diagnostics, &compilation.sources, &entry_module),
            bytecode: bytecode_dto(
                compilation.chunk.as_ref(),
                &compilation.sources,
                &entry_module,
            ),
        }
    })
    .await
    .map_err(|_| internal_error());
    state
        .runs
        .lock()
        .map_err(|_| internal_error())?
        .remove(&execution_id);
    result
}

fn analysis_dto(mut result: AnalysisResult, virtual_path: &str) -> AnalysisDto {
    let chunk = if result
        .diagnostics
        .iter()
        .any(|d| d.severity == compiler_core::diagnostic::Severity::Error)
    {
        None
    } else if let Some(semantic) = &result.semantic {
        match compiler_core::bytecode::compile(&result.program, semantic) {
            Ok(chunk) => Some(chunk),
            Err(diagnostic) => {
                result.diagnostics.push(diagnostic);
                None
            }
        }
    } else {
        None
    };
    let bytecode = bytecode_dto(chunk.as_ref(), &result.sources, virtual_path);
    let diagnostics = diagnostics_dto(&result.diagnostics, &result.sources, virtual_path);
    let tokens = result
        .tokens
        .into_iter()
        .take(MAX_TOKENS)
        .map(|token| TokenDto {
            kind: format!("{:?}", token.kind),
            lexeme: token.lexeme.chars().take(256).collect(),
            location: location(&result.sources, token.span, virtual_path),
        })
        .collect();
    let ast = ast_dto(&result.program, &result.sources, virtual_path);
    let symbols = result.semantic.map_or_else(Vec::new, |semantic| {
        semantic
            .symbols
            .into_iter()
            .take(MAX_SYMBOLS)
            .map(|symbol| SymbolDto {
                name: symbol.name,
                kind: format!("{:?}", symbol.kind),
                ty: format!("{:?}", symbol.ty),
            })
            .collect()
    });
    AnalysisDto {
        diagnostics,
        tokens,
        symbols,
        ast,
        bytecode,
    }
}

struct AstBuilder<'a> {
    nodes: Vec<AstNodeDto>,
    sources: &'a SourceMap,
    fallback: &'a str,
    truncated: bool,
}

impl<'a> AstBuilder<'a> {
    fn push(
        &mut self,
        parent_id: Option<&str>,
        relation: Option<&str>,
        kind: &str,
        label: String,
        span: Option<Span>,
    ) -> Option<String> {
        if self.nodes.len() == MAX_AST_NODES {
            self.truncated = true;
            return None;
        }
        let id = format!("ast-{}", self.nodes.len());
        self.nodes.push(AstNodeDto {
            id: id.clone(),
            parent_id: parent_id.map(str::to_owned),
            relation: relation.map(str::to_owned),
            kind: kind.to_owned(),
            label: label.chars().take(128).collect(),
            location: span.map(|value| location(self.sources, value, self.fallback)),
        });
        Some(id)
    }

    fn program(&mut self, program: &Program) {
        let Some(id) = self.push(None, None, "Program", "Program".to_owned(), None) else {
            return;
        };
        for item in &program.items {
            self.item(&id, item);
        }
    }

    fn item(&mut self, parent: &str, item: &Item) {
        match item {
            Item::Decl(value) => self.decl(parent, "item", value),
            Item::Export(value) => {
                let Some(id) = self.push(
                    Some(parent),
                    Some("item"),
                    "Export",
                    "export".to_owned(),
                    None,
                ) else {
                    return;
                };
                self.decl(&id, "declaration", value);
            }
            Item::Class(value) | Item::ExportClass(value) => self.class(parent, value),
            Item::Trait(value) | Item::ExportTrait(value) => {
                let Some(id) = self.push(
                    Some(parent),
                    Some("item"),
                    "Trait",
                    value.name.clone(),
                    Some(value.span),
                ) else {
                    return;
                };
                for method in &value.methods {
                    self.signature(&id, "method", method);
                }
            }
            Item::Module(value) => {
                self.push(
                    Some(parent),
                    Some("item"),
                    "Module",
                    value.path.join("."),
                    Some(value.span),
                );
            }
            Item::Use(value) => {
                self.push(
                    Some(parent),
                    Some("item"),
                    "Use",
                    value.alias.as_ref().map_or_else(
                        || value.path.join("."),
                        |alias| format!("{} as {alias}", value.path.join(".")),
                    ),
                    Some(value.span),
                );
            }
            Item::Stmt(value) => self.stmt(parent, "item", value),
        }
    }

    fn decl(&mut self, parent: &str, relation: &str, value: &Decl) {
        match value {
            Decl::Variable(value) => self.variable(parent, relation, value),
            Decl::Function(value) => self.function(parent, relation, value),
        }
    }
    fn class(&mut self, parent: &str, value: &ClassDecl) {
        let Some(id) = self.push(
            Some(parent),
            Some("item"),
            "Class",
            value.name.clone(),
            Some(value.span),
        ) else {
            return;
        };
        for trait_name in &value.traits {
            self.type_name(&id, "trait", trait_name);
        }
        for member in &value.members {
            match member {
                ClassMember::Field(value) => self.variable(&id, "field", &value.variable),
                ClassMember::Method(value) => self.function(&id, "method", &value.function),
                ClassMember::Constructor(value) => {
                    self.function(&id, "constructor", &value.function)
                }
            }
        }
    }
    fn variable(&mut self, parent: &str, relation: &str, value: &VariableDecl) {
        let Some(id) = self.push(
            Some(parent),
            Some(relation),
            "Variable",
            value.name.clone(),
            Some(value.span),
        ) else {
            return;
        };
        if let Some(annotation) = &value.annotation {
            self.type_name(&id, "type", annotation);
        }
        self.expr(&id, "initializer", &value.initializer);
    }
    fn function(&mut self, parent: &str, relation: &str, value: &FunctionDecl) {
        let Some(id) = self.push(
            Some(parent),
            Some(relation),
            "Function",
            value.name.clone(),
            Some(value.span),
        ) else {
            return;
        };
        for parameter in &value.parameters {
            let Some(parameter_id) = self.push(
                Some(&id),
                Some("parameter"),
                "Parameter",
                parameter.name.clone(),
                Some(parameter.span),
            ) else {
                return;
            };
            self.type_name(&parameter_id, "type", &parameter.type_name);
        }
        if let Some(return_type) = &value.return_type {
            self.type_name(&id, "returnType", return_type);
        }
        self.stmt(&id, "body", &value.body);
    }
    fn signature(&mut self, parent: &str, relation: &str, value: &FunctionSignature) {
        let Some(id) = self.push(
            Some(parent),
            Some(relation),
            "MethodSignature",
            value.name.clone(),
            Some(value.span),
        ) else {
            return;
        };
        for parameter in &value.parameters {
            let Some(parameter_id) = self.push(
                Some(&id),
                Some("parameter"),
                "Parameter",
                parameter.name.clone(),
                Some(parameter.span),
            ) else {
                return;
            };
            self.type_name(&parameter_id, "type", &parameter.type_name);
        }
        if let Some(return_type) = &value.return_type {
            self.type_name(&id, "returnType", return_type);
        }
    }
    fn type_name(&mut self, parent: &str, relation: &str, value: &TypeName) {
        self.push(
            Some(parent),
            Some(relation),
            "Type",
            value.name.clone(),
            Some(value.span),
        );
    }
    fn stmt(&mut self, parent: &str, relation: &str, value: &Stmt) {
        match value {
            Stmt::Variable(value) => self.variable(parent, relation, value),
            Stmt::Block { statements, span } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Block",
                    "block".to_owned(),
                    Some(*span),
                ) else {
                    return;
                };
                for statement in statements {
                    self.stmt(&id, "statement", statement);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "If",
                    "if".to_owned(),
                    Some(*span),
                ) else {
                    return;
                };
                self.expr(&id, "condition", condition);
                self.stmt(&id, "then", then_branch);
                if let Some(value) = else_branch {
                    self.stmt(&id, "else", value);
                }
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "While",
                    "while".to_owned(),
                    Some(*span),
                ) else {
                    return;
                };
                self.expr(&id, "condition", condition);
                self.stmt(&id, "body", body);
            }
            Stmt::Break { span } => {
                self.push(
                    Some(parent),
                    Some(relation),
                    "Break",
                    "break".to_owned(),
                    Some(*span),
                );
            }
            Stmt::Continue { span } => {
                self.push(
                    Some(parent),
                    Some(relation),
                    "Continue",
                    "continue".to_owned(),
                    Some(*span),
                );
            }
            Stmt::Return { value, span } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Return",
                    "return".to_owned(),
                    Some(*span),
                ) else {
                    return;
                };
                if let Some(value) = value {
                    self.expr(&id, "value", value);
                }
            }
            Stmt::Expression { expression, span } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "ExpressionStatement",
                    "expression".to_owned(),
                    Some(*span),
                ) else {
                    return;
                };
                self.expr(&id, "expression", expression);
            }
        }
    }
    fn expr(&mut self, parent: &str, relation: &str, value: &Expr) {
        match &value.kind {
            ExprKind::Literal(literal) => {
                self.push(
                    Some(parent),
                    Some(relation),
                    "Literal",
                    format!("{literal:?}"),
                    Some(value.span),
                );
            }
            ExprKind::Name(name) => {
                self.push(
                    Some(parent),
                    Some(relation),
                    "Name",
                    name.clone(),
                    Some(value.span),
                );
            }
            ExprKind::Unary { op, operand } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Unary",
                    format!("{op:?}"),
                    Some(value.span),
                ) else {
                    return;
                };
                self.expr(&id, "operand", operand);
            }
            ExprKind::Binary { left, op, right } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Binary",
                    format!("{op:?}"),
                    Some(value.span),
                ) else {
                    return;
                };
                self.expr(&id, "left", left);
                self.expr(&id, "right", right);
            }
            ExprKind::Assign {
                target,
                value: assigned,
            } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Assign",
                    "=".to_owned(),
                    Some(value.span),
                ) else {
                    return;
                };
                self.expr(&id, "target", target);
                self.expr(&id, "value", assigned);
            }
            ExprKind::Call { callee, arguments } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Call",
                    "call".to_owned(),
                    Some(value.span),
                ) else {
                    return;
                };
                self.expr(&id, "callee", callee);
                for argument in arguments {
                    self.expr(&id, "argument", argument);
                }
            }
            ExprKind::Member { object, name } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Member",
                    name.clone(),
                    Some(value.span),
                ) else {
                    return;
                };
                self.expr(&id, "object", object);
            }
            ExprKind::Index { object, index } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Index",
                    "[]".to_owned(),
                    Some(value.span),
                ) else {
                    return;
                };
                self.expr(&id, "object", object);
                self.expr(&id, "index", index);
            }
            ExprKind::Cast {
                expression,
                type_name,
            } => {
                let Some(id) = self.push(
                    Some(parent),
                    Some(relation),
                    "Cast",
                    "as".to_owned(),
                    Some(value.span),
                ) else {
                    return;
                };
                self.expr(&id, "expression", expression);
                self.type_name(&id, "type", type_name);
            }
        }
    }
}

fn ast_dto(program: &Program, sources: &SourceMap, fallback: &str) -> AstGraphDto {
    let mut builder = AstBuilder {
        nodes: Vec::new(),
        sources,
        fallback,
        truncated: false,
    };
    builder.program(program);
    AstGraphDto {
        nodes: builder.nodes,
        truncated: builder.truncated,
    }
}

fn project_dto(result: ProjectAnalysis) -> ProjectDto {
    ProjectDto {
        modules: result
            .modules
            .into_iter()
            .map(|module| module.name)
            .collect(),
        diagnostics: diagnostics_dto(&result.diagnostics, &result.sources, "<project>"),
    }
}

fn diagnostics_dto(
    values: &[Diagnostic],
    sources: &SourceMap,
    fallback: &str,
) -> Vec<DiagnosticDto> {
    let mut values: Vec<_> = values
        .iter()
        .take(MAX_DIAGNOSTICS)
        .map(|value| {
            let span = value.labels.first().map_or(
                Span::new(compiler_core::source::SourceId(0), 0, 0),
                |label| label.span,
            );
            DiagnosticDto {
                code: value.code.clone(),
                severity: format!("{:?}", value.severity).to_lowercase(),
                message: value.message.clone(),
                location: location(sources, span, fallback),
            }
        })
        .collect();
    values.sort_by_key(|value| (value.location.file.clone(), value.location.start));
    values
}

fn location(sources: &SourceMap, span: Span, fallback: &str) -> LocationDto {
    let (line, column) = sources
        .line_column(span.source, span.start)
        .unwrap_or((1, 1));
    LocationDto {
        file: display_name(sources.name(span.source).unwrap_or(fallback), fallback),
        start: span.start,
        end: span.end,
        line,
        column,
    }
}

fn display_name(value: &str, fallback: &str) -> String {
    if value.starts_with('<') {
        return fallback.to_owned();
    }
    Path::new(value).file_name().map_or_else(
        || fallback.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn validate_source(source: &str, virtual_path: &str) -> Result<(), CommandError> {
    validate_path(virtual_path)?;
    if source.len() > MAX_SOURCE_BYTES {
        return Err(CommandError {
            code: "SOURCE_TOO_LARGE",
            message: "El archivo supera 1 MiB".to_owned(),
        });
    }
    Ok(())
}

fn validate_path(value: &str) -> Result<(), CommandError> {
    if value.is_empty() || value.len() > MAX_PATH_BYTES || value.contains('\0') {
        return Err(CommandError {
            code: "INVALID_PATH",
            message: "La ruta no es válida".to_owned(),
        });
    }
    Ok(())
}

fn validate_id(value: &str) -> Result<(), CommandError> {
    if value.len() > 128 || value.is_empty() {
        return Err(CommandError {
            code: "INVALID_EXECUTION",
            message: "El identificador de ejecución no es válido".to_owned(),
        });
    }
    Ok(())
}

fn internal_error() -> CommandError {
    CommandError {
        code: "INTERNAL",
        message: "No se pudo completar la operación".to_owned(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            analyze_source,
            run_source,
            cancel_run,
            open_project,
            analyze_project,
            run_project
        ])
        .run(tauri::generate_context!())
        .expect("failed to run desktop application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_boundaries_and_maps_precise_diagnostics() {
        assert_eq!(
            validate_source(&"x".repeat(MAX_SOURCE_BYTES + 1), "main.itd")
                .unwrap_err()
                .code,
            "SOURCE_TOO_LARGE"
        );
        let dto = analysis_dto(compiler_core::analyze("missing;"), "main.itd");
        assert_eq!(dto.diagnostics[0].location.file, "main.itd");
        assert_eq!(dto.diagnostics[0].location.line, 1);
    }

    #[test]
    fn exposes_a_stable_bounded_ast_graph() {
        let analysis = compiler_core::analyze("let value := 1 + 2;");
        let graph = ast_dto(&analysis.program, &analysis.sources, "main.itd");
        assert_eq!(graph.nodes[0].id, "ast-0");
        assert_eq!(graph.nodes[0].kind, "Program");
        assert!(graph.nodes.iter().any(|node| {
            node.parent_id.as_deref() == Some("ast-0") && node.relation.as_deref() == Some("item")
        }));
        assert!(
            graph
                .nodes
                .iter()
                .filter_map(|node| node.location.as_ref())
                .any(|location| location.start < location.end)
        );
        assert!(!graph.truncated);
    }

    #[test]
    fn marks_graphs_that_exceed_the_node_limit() {
        let source = "let value := 1;\n".repeat(MAX_AST_NODES);
        let analysis = compiler_core::analyze(&source);
        let graph = ast_dto(&analysis.program, &analysis.sources, "main.itd");
        assert_eq!(graph.nodes.len(), MAX_AST_NODES);
        assert!(graph.truncated);
    }
}
