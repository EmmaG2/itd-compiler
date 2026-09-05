use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    ast::{Decl, Item, Program, UseDecl},
    diagnostic::{Diagnostic, Severity},
    lexer::lex,
    parser::parse,
    semantic::{ImportedSymbol, SemanticResult, analyze_with_imports},
    source::{SourceId, SourceMap, Span},
};

const MAX_MODULES: usize = 256;
const MAX_IMPORT_DEPTH: usize = 64;
const MAX_SOURCE_BYTES: u64 = 1_048_576;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleAnalysis {
    pub name: String,
    pub path: PathBuf,
    pub program: Program,
    pub semantic: Option<SemanticResult>,
    pub exports: Vec<ImportedSymbol>,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectAnalysis {
    pub sources: SourceMap,
    pub modules: Vec<ModuleAnalysis>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Project {
    root: PathBuf,
}

impl Project {
    pub fn open(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().canonicalize()?;
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "project root is not a directory",
            ));
        }
        Ok(Self { root })
    }

    #[must_use]
    pub fn analyze(&self, entry: &str) -> ProjectAnalysis {
        Loader::new(&self.root).analyze(entry)
    }
}

#[must_use]
pub fn analyze_project(root: impl AsRef<Path>, entry: &str) -> ProjectAnalysis {
    Loader::new(root.as_ref()).analyze(entry)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Visiting,
    Done,
}

struct Loader {
    root: Option<PathBuf>,
    sources: SourceMap,
    modules: HashMap<String, ModuleAnalysis>,
    states: HashMap<String, State>,
    physical_modules: HashMap<PathBuf, String>,
    diagnostics: Vec<Diagnostic>,
}

impl Loader {
    fn new(root: &Path) -> Self {
        let root = root.canonicalize().ok().filter(|path| path.is_dir());
        Self {
            root,
            sources: SourceMap::new(),
            modules: HashMap::new(),
            states: HashMap::new(),
            physical_modules: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    fn analyze(mut self, entry: &str) -> ProjectAnalysis {
        let span = Span::new(self.sources.add("<project>", ""), 0, 0);
        if self.root.is_none() {
            self.diagnostics.push(Diagnostic::error(
                "E3000",
                "la raíz del proyecto no existe o no es un directorio",
                span,
            ));
        } else if let Some(path) = parse_module_name(entry) {
            self.load(path, span, Vec::new());
        } else {
            self.diagnostics.push(Diagnostic::error(
                "E3000",
                "nombre de módulo de entrada inválido",
                span,
            ));
        }
        let mut modules: Vec<_> = self.modules.into_values().collect();
        modules.sort_by(|left, right| left.name.cmp(&right.name));
        ProjectAnalysis {
            sources: self.sources,
            modules,
            diagnostics: self.diagnostics,
        }
    }

    fn load(&mut self, path: Vec<String>, import_span: Span, mut chain: Vec<String>) {
        if self.states.len() >= MAX_MODULES {
            self.diagnostics.push(Diagnostic::error(
                "E3006",
                "límite de módulos excedido",
                import_span,
            ));
            return;
        }
        if chain.len() >= MAX_IMPORT_DEPTH {
            self.diagnostics.push(Diagnostic::error(
                "E3006",
                "profundidad de imports excedida",
                import_span,
            ));
            return;
        }
        let name = path.join(".");
        match self.states.get(&name) {
            Some(State::Done) => return,
            Some(State::Visiting) => {
                chain.push(name);
                self.diagnostics.push(Diagnostic::error(
                    "E3003",
                    format!("ciclo de módulos: {}", chain.join(" -> ")),
                    import_span,
                ));
                return;
            }
            None => {}
        }
        let Some(file) = self.module_file(&path, import_span, &chain) else {
            return;
        };
        if fs::metadata(&file).is_ok_and(|value| value.len() > MAX_SOURCE_BYTES) {
            self.diagnostics.push(Diagnostic::error(
                "E3006",
                "módulo demasiado grande",
                import_span,
            ));
            return;
        }
        if let Some(existing) = self.physical_modules.get(&file).cloned() {
            if existing != name {
                self.diagnostics.push(Diagnostic::error(
                    "E3004",
                    format!("`{name}` ya fue cargado como `{existing}`"),
                    import_span,
                ));
            }
            return;
        }
        let Ok(source) = fs::read_to_string(&file) else {
            self.diagnostics.push(Diagnostic::error(
                "E3001",
                format!("no se pudo leer el módulo `{name}`"),
                import_span,
            ));
            return;
        };
        let source_id = self.sources.add(file.to_string_lossy(), &source);
        let lexed = lex(&source, source_id);
        let parsed = parse(&lexed.tokens);
        let has_syntax_errors = lexed
            .diagnostics
            .iter()
            .chain(&parsed.diagnostics)
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        self.diagnostics.extend(lexed.diagnostics);
        self.diagnostics.extend(parsed.diagnostics);
        self.validate_module_name(&parsed.program, &path, source_id);

        self.states.insert(name.clone(), State::Visiting);
        self.physical_modules.insert(file.clone(), name.clone());
        chain.push(name.clone());
        let uses: Vec<_> = parsed
            .program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Use(declaration) => Some(declaration.clone()),
                _ => None,
            })
            .collect();
        let mut seen = HashSet::new();
        let mut unique_uses = Vec::new();
        for declaration in &uses {
            if !seen.insert(declaration.path.clone()) {
                self.diagnostics.push(Diagnostic::error(
                    "E3002",
                    "import duplicado",
                    declaration.span,
                ));
                continue;
            }
            unique_uses.push(declaration);
            self.load(declaration.path.clone(), declaration.span, chain.clone());
        }

        let imports = unique_uses
            .iter()
            .flat_map(|declaration| self.imports(declaration))
            .collect::<Vec<_>>();
        let semantic =
            (!has_syntax_errors).then(|| analyze_with_imports(&parsed.program, &imports));
        if let Some(result) = &semantic {
            self.diagnostics.extend(result.diagnostics.clone());
        }
        let exports = semantic
            .as_ref()
            .map_or_else(Vec::new, |semantic| exports(&parsed.program, semantic));
        let dependencies = unique_uses
            .iter()
            .map(|declaration| declaration.path.join("."))
            .collect();
        self.modules.insert(
            name.clone(),
            ModuleAnalysis {
                name: name.clone(),
                path: file,
                program: parsed.program,
                semantic,
                exports,
                dependencies,
            },
        );
        self.states.insert(name, State::Done);
    }

    fn module_file(&mut self, path: &[String], span: Span, chain: &[String]) -> Option<PathBuf> {
        let root = self.root.clone()?;
        let mut candidate = root.clone();
        for segment in &path[..path.len().saturating_sub(1)] {
            candidate.push(segment);
        }
        candidate.push(format!("{}.itd", path.last()?));
        let Ok(candidate) = candidate.canonicalize() else {
            self.diagnostics.push(Diagnostic::error(
                "E3001",
                import_message(format!("módulo inexistente: {}", path.join(".")), chain),
                span,
            ));
            return None;
        };
        if !candidate.starts_with(&root) || !candidate.is_file() {
            self.diagnostics.push(Diagnostic::error(
                "E3005",
                import_message(
                    "el import intenta salir de la raíz del proyecto".to_owned(),
                    chain,
                ),
                span,
            ));
            return None;
        }
        Some(candidate)
    }

    fn validate_module_name(&mut self, program: &Program, expected: &[String], source: SourceId) {
        let modules: Vec<_> = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Module(module) => Some(module),
                _ => None,
            })
            .collect();
        if modules.len() == 1 && modules[0].path == expected {
            return;
        }
        let span = modules
            .first()
            .map_or(Span::new(source, 0, 0), |module| module.span);
        self.diagnostics.push(Diagnostic::error(
            "E3002",
            format!("el archivo debe declarar `module {};`", expected.join(".")),
            span,
        ));
    }

    fn imports(&self, declaration: &UseDecl) -> Vec<ImportedSymbol> {
        let Some(module) = self.modules.get(&declaration.path.join(".")) else {
            return Vec::new();
        };
        module
            .exports
            .iter()
            .map(|symbol| ImportedSymbol {
                name: declaration.alias.as_ref().map_or_else(
                    || symbol.name.clone(),
                    |alias| format!("{alias}.{}", symbol.name),
                ),
                ty: symbol.ty.clone(),
                declaration_span: declaration.span,
                class: symbol.class.clone(),
                trait_info: symbol.trait_info.clone(),
            })
            .collect()
    }
}

fn exports(program: &Program, semantic: &SemanticResult) -> Vec<ImportedSymbol> {
    program
        .items
        .iter()
        .filter_map(|item| {
            let declaration = match item {
                Item::Export(declaration) => declaration,
                Item::ExportClass(declaration) => {
                    let symbol = semantic.symbols.iter().find(|symbol| {
                        symbol.name == declaration.name
                            && symbol.declaration_span == declaration.name_span
                    })?;
                    return Some(ImportedSymbol {
                        name: declaration.name.clone(),
                        ty: symbol.ty.clone(),
                        declaration_span: declaration.name_span,
                        class: semantic
                            .classes
                            .get(&declaration.name)
                            .cloned()
                            .map(|value| (declaration.name.clone(), value)),
                        trait_info: None,
                    });
                }
                Item::ExportTrait(declaration) => {
                    let symbol = semantic.symbols.iter().find(|symbol| {
                        symbol.name == declaration.name
                            && symbol.declaration_span == declaration.name_span
                    })?;
                    return Some(ImportedSymbol {
                        name: declaration.name.clone(),
                        ty: symbol.ty.clone(),
                        declaration_span: declaration.name_span,
                        class: None,
                        trait_info: semantic
                            .traits
                            .get(&declaration.name)
                            .cloned()
                            .map(|value| (declaration.name.clone(), value)),
                    });
                }
                _ => return None,
            };
            let (name, span) = match declaration {
                Decl::Variable(declaration) => (&declaration.name, declaration.name_span),
                Decl::Function(declaration) => (&declaration.name, declaration.name_span),
            };
            let symbol = semantic
                .symbols
                .iter()
                .find(|symbol| symbol.name == *name && symbol.declaration_span == span)?;
            Some(ImportedSymbol {
                name: name.clone(),
                ty: symbol.ty.clone(),
                declaration_span: span,
                class: None,
                trait_info: None,
            })
        })
        .collect()
}

fn parse_module_name(name: &str) -> Option<Vec<String>> {
    let path: Vec<_> = name.split('.').map(str::to_owned).collect();
    (!path.is_empty()
        && path.iter().all(|segment| {
            let mut chars = segment.chars();
            chars
                .next()
                .is_some_and(|first| first == '_' || first.is_alphabetic())
                && chars.all(|ch| ch == '_' || ch.is_alphanumeric())
        }))
    .then_some(path)
}

fn import_message(message: String, chain: &[String]) -> String {
    if chain.is_empty() {
        message
    } else {
        format!("{message} (cadena: {})", chain.join(" -> "))
    }
}
