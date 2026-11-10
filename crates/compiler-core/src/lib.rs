pub mod arm64;
pub mod ast;
pub mod bytecode;
mod bytecode_validate;
pub mod diagnostic;
pub mod lexer;
mod linker;
pub mod parser;
pub mod project;
pub mod quadruple;
pub mod quadruple_lowering;
pub mod quadruple_optimize;
#[cfg(feature = "reference-interpreter")]
mod reference;
pub mod runtime;
pub mod semantic;
mod semantic_stack;
pub mod source;
mod vm;

use ast::Program;
use bytecode::compile;
use diagnostic::{Diagnostic, Severity};
use lexer::{Token, lex};
use parser::parse;
use semantic::{SemanticResult, analyze as analyze_semantics};
use source::SourceMap;
use std::sync::{Arc, atomic::AtomicBool};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisResult {
    pub sources: SourceMap,
    pub tokens: Vec<Token>,
    pub program: Program,
    pub semantic: Option<SemanticResult>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunResult {
    pub output: String,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn analyze(source: &str) -> AnalysisResult {
    analyze_internal(source, None, false).0
}

/// Capture semantic stack operations; syntax errors have no semantic trace.
#[must_use]
pub fn analyze_with_trace(
    source: &str,
    event_limit: usize,
) -> (AnalysisResult, Option<semantic::SemanticTrace>) {
    analyze_internal(source, Some(event_limit), false)
}

fn analyze_internal(
    source: &str,
    event_limit: Option<usize>,
    quadruple_backend: bool,
) -> (AnalysisResult, Option<semantic::SemanticTrace>) {
    let mut sources = SourceMap::new();
    let source_id = sources.add("<memory>", source);
    let lexed = lex(source, source_id);
    let parsed = parse(&lexed.tokens);
    let mut diagnostics = lexed.diagnostics;
    diagnostics.extend(parsed.diagnostics);
    let mut trace = None;
    let semantic = diagnostics.is_empty().then(|| {
        if let Some(limit) = event_limit {
            let (result, captured) =
                semantic::analyze_with_imports_and_trace(&parsed.program, &[], limit);
            trace = Some(captured);
            result
        } else if quadruple_backend {
            semantic::analyze_for_quadruples(&parsed.program)
        } else {
            analyze_semantics(&parsed.program)
        }
    });
    if let Some(result) = &semantic {
        diagnostics.extend(result.diagnostics.clone());
    }
    (
        AnalysisResult {
            sources,
            tokens: lexed.tokens,
            program: parsed.program,
            semantic,
            diagnostics,
        },
        trace,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompilationResult {
    pub sources: SourceMap,
    pub chunk: Option<bytecode::Chunk>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuadrupleCompilationResult {
    pub sources: SourceMap,
    pub program: Option<quadruple::QuadrupleProgram>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arm64CompilationResult {
    pub assembly: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Lower source to the independent ARM64-oriented quadruple IR without using the VM compiler.
#[must_use]
pub fn compile_quadruples(source: &str) -> QuadrupleCompilationResult {
    let analysis = analyze_internal(source, None, true).0;
    let mut diagnostics = analysis.diagnostics.clone();
    let program = if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        None
    } else if let Some(semantic) = analysis.semantic.as_ref() {
        match quadruple_lowering::lower(&analysis.program, semantic) {
            Ok(program) => Some(program),
            Err(mut errors) => {
                diagnostics.append(&mut errors);
                None
            }
        }
    } else {
        None
    };
    QuadrupleCompilationResult {
        sources: analysis.sources,
        program,
        diagnostics,
    }
}

/// Emit Darwin ARM64 assembly only from validated quadruples.
pub fn emit_arm64(program: &quadruple::QuadrupleProgram) -> Result<String, Diagnostic> {
    arm64::emit(program)
}

/// Source convenience entrypoint; lowering remains the only source-to-IR path.
pub fn compile_arm64(source: &str) -> Arm64CompilationResult {
    let quadruples = compile_quadruples(source);
    let mut diagnostics = quadruples.diagnostics;
    let assembly = quadruples
        .program
        .as_ref()
        .and_then(|program| match arm64::emit(program) {
            Ok(assembly) => Some(assembly),
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                None
            }
        });
    Arm64CompilationResult {
        assembly,
        diagnostics,
    }
}

#[must_use]
pub fn compile_source(source: &str) -> CompilationResult {
    compile_analysis(analyze(source))
}

#[must_use]
pub fn compile_analysis(analysis: AnalysisResult) -> CompilationResult {
    let mut diagnostics = analysis.diagnostics;
    let chunk = if diagnostics.iter().any(|d| d.severity == Severity::Error) {
        None
    } else if let Some(semantic) = analysis.semantic.as_ref() {
        match compile(&analysis.program, semantic) {
            Ok(chunk) => Some(chunk),
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                None
            }
        }
    } else {
        None
    };
    CompilationResult {
        sources: analysis.sources,
        chunk,
        diagnostics,
    }
}

#[must_use]
pub fn run_compiled(
    compilation: &CompilationResult,
    limits: runtime::RuntimeLimits,
    cancelled: Option<Arc<AtomicBool>>,
) -> RunResult {
    let mut diagnostics = compilation.diagnostics.clone();
    let Some(chunk) = compilation
        .chunk
        .as_ref()
        .filter(|_| !diagnostics.iter().any(|d| d.severity == Severity::Error))
    else {
        return RunResult {
            output: String::new(),
            diagnostics,
        };
    };
    let result = vm::execute(chunk, limits, cancelled);
    diagnostics.extend(result.diagnostics);
    RunResult {
        output: result.output,
        diagnostics,
    }
}

#[must_use]
pub fn run(source: &str) -> RunResult {
    run_with_options(source, runtime::RuntimeLimits::default(), None)
}

#[must_use]
pub fn run_with_options(
    source: &str,
    limits: runtime::RuntimeLimits,
    cancelled: Option<Arc<AtomicBool>>,
) -> RunResult {
    run_compiled(&compile_source(source), limits, cancelled)
}
