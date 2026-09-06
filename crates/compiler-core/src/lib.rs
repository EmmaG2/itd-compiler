pub mod ast;
pub mod bytecode;
mod bytecode_validate;
pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod project;
#[cfg(feature = "reference-interpreter")]
mod reference;
pub mod runtime;
pub mod semantic;
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
    let mut sources = SourceMap::new();
    let source_id = sources.add("<memory>", source);
    let lexed = lex(source, source_id);
    let parsed = parse(&lexed.tokens);
    let mut diagnostics = lexed.diagnostics;
    diagnostics.extend(parsed.diagnostics);
    let semantic = diagnostics
        .is_empty()
        .then(|| analyze_semantics(&parsed.program));
    if let Some(result) = &semantic {
        diagnostics.extend(result.diagnostics.clone());
    }
    AnalysisResult {
        sources,
        tokens: lexed.tokens,
        program: parsed.program,
        semantic,
        diagnostics,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompilationResult {
    pub sources: SourceMap,
    pub chunk: Option<bytecode::Chunk>,
    pub diagnostics: Vec<Diagnostic>,
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
