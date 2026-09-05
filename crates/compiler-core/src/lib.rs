pub mod ast;
pub mod bytecode;
pub mod diagnostic;
pub mod lexer;
pub mod parser;
pub mod project;
pub mod runtime;
pub mod semantic;
pub mod source;

use ast::Program;
use bytecode::compile;
use diagnostic::{Diagnostic, Severity};
use lexer::{Token, lex};
use parser::parse;
use runtime::execute_chunk;
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
    let analysis = analyze(source);
    if analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return RunResult {
            output: String::new(),
            diagnostics: analysis.diagnostics,
        };
    }
    let Some(semantic) = analysis.semantic.as_ref() else {
        return RunResult {
            output: String::new(),
            diagnostics: analysis.diagnostics,
        };
    };
    let chunk = match compile(&analysis.program) {
        Ok(chunk) => chunk,
        Err(diagnostic) => {
            return RunResult {
                output: String::new(),
                diagnostics: vec![diagnostic],
            };
        }
    };
    let runtime = execute_chunk(&chunk, semantic, limits, cancelled);
    let mut diagnostics = analysis.diagnostics;
    diagnostics.extend(runtime.diagnostics);
    RunResult {
        output: runtime.output,
        diagnostics,
    }
}
