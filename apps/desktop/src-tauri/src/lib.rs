use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, atomic::AtomicBool},
};

use compiler_core::{
    AnalysisResult,
    diagnostic::Diagnostic,
    project::{Project, ProjectAnalysis},
    run_with_options,
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
struct AnalysisDto {
    diagnostics: Vec<DiagnosticDto>,
    tokens: Vec<TokenDto>,
    symbols: Vec<SymbolDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunDto {
    output: String,
    diagnostics: Vec<DiagnosticDto>,
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
    let cancelled = Arc::new(AtomicBool::new(false));
    state
        .runs
        .lock()
        .map_err(|_| internal_error())?
        .insert(execution_id.clone(), Arc::clone(&cancelled));
    let execution_source = source.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        run_with_options(&execution_source, RuntimeLimits::default(), Some(cancelled))
    })
    .await
    .map_err(|_| internal_error());
    state
        .runs
        .lock()
        .map_err(|_| internal_error())?
        .remove(&execution_id);
    let result = result?;
    let analysis = compiler_core::analyze(&source);
    Ok(RunDto {
        output: result.output,
        diagnostics: diagnostics_dto(&result.diagnostics, &analysis.sources, &virtual_path),
    })
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
async fn run_project(
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

fn analysis_dto(result: AnalysisResult, virtual_path: &str) -> AnalysisDto {
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
}
