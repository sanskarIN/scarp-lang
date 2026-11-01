//! Explicit source loading, bounded module graphs and source-aware compilation.
//!
//! ```
//! use scarp::{modules::{self, MemorySourceLoader, ModuleLimits}, ExecutionOptions};
//! let mut loader = MemorySourceLoader::new();
//! loader.insert("main.srp", "import \"math.srp\" as math; print(math.twice(3));")?;
//! loader.insert("math.srp", "pub pure funcra twice(n: integer)->integer {return n*2;}")?;
//! let program = modules::compile_with_loader("main.srp", &loader, ModuleLimits::default())?;
//! let (mut out, mut err) = (Vec::new(), Vec::new());
//! program.execute(&mut out, &mut err, ExecutionOptions::default())?;
//! assert_eq!(out, b"6\n");
//! assert!(err.is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::{
    ast::{Program, Statement},
    diagnostics::{self, Diagnostic, DiagnosticKind as Kind, Span},
    interpreter, lexer, parser,
    resolver::ResolvedProgram,
    ExecutionOptions, RunError,
};
use std::{collections::HashMap, fmt, io, path::Path, sync::Arc};

mod inspection;
mod spelling;
use spelling::spelling_hint;
mod linker;
mod loader;
pub use inspection::{ImportInfo, SourceLocation};
pub use loader::{
    FileSourceLoader, MemorySourceLoader, SourceLoadError, SourceLoadKind, SourceLoader,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleLimits {
    pub max_files: usize,
    /// Maximum import edges on a path; the entry file has depth zero.
    pub max_depth: usize,
    pub max_file_bytes: usize,
    pub max_total_bytes: usize,
    pub max_imports: usize,
    /// New symbol/string text created during linking, separate from source bytes.
    pub max_expansion_bytes: usize,
}
impl Default for ModuleLimits {
    fn default() -> Self {
        Self {
            max_files: 128,
            max_depth: 32,
            max_file_bytes: 1024 * 1024,
            max_total_bytes: 8 * 1024 * 1024,
            max_imports: 1024,
            max_expansion_bytes: 8 * 1024 * 1024,
        }
    }
}

pub type SourceId = usize;
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    start: usize,
    length: usize,
}
impl SourceFile {
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start..self.start + self.length
    }
}
/// One immutable coordinate space keeps the established public Span layout.
#[derive(Debug, Clone)]
pub struct SourceMap {
    text: String,
    files: Vec<SourceFile>,
    entry: String,
}
impl SourceMap {
    pub fn files(&self) -> &[SourceFile] {
        &self.files
    }
    pub fn source(&self, id: SourceId) -> Option<&str> {
        self.files.get(id).map(|file| &self.text[file.range()])
    }
    /// Locate a valid span within one source, including zero-width EOF spans.
    /// Cross-source, reversed, out-of-range and non-UTF-8-boundary spans are None.
    pub fn locate_span(&self, span: Span) -> Option<SourceLocation> {
        if span.end < span.start {
            return None;
        }
        let id = self
            .files
            .partition_point(|file| file.start <= span.start)
            .checked_sub(1)?;
        let file = &self.files[id];
        if span.end > file.range().end {
            return None;
        }
        let local = Span::new(span.start - file.start, span.end - file.start);
        let source = self.source(id)?;
        if !source.is_char_boundary(local.start) || !source.is_char_boundary(local.end) {
            return None;
        }
        Some(SourceLocation {
            source: id,
            span: local,
            start: diagnostics::location(source, local.start),
            end: diagnostics::location(source, local.end),
        })
    }
    fn locate(&self, span: Span) -> Option<(&SourceFile, Span, &str)> {
        let file = self
            .files
            .iter()
            .rev()
            .find(|file| file.start <= span.start)
            .or(self.files.first())?;
        let local = Span::new(
            span.start.saturating_sub(file.start).min(file.length),
            span.end.saturating_sub(file.start).min(file.length),
        );
        Some((file, local, &self.text[file.range()]))
    }
    pub fn render(&self, error: &Diagnostic) -> String {
        let mut text = error.to_string();
        if let Some((file, span, source)) = self.locate(error.span) {
            text.push('\n');
            text.push_str(&diagnostics::excerpt(span, &file.name, source));
        } else {
            text.push_str(&format!("\n --> {}", self.entry));
        }
        for note in &error.related {
            text.push_str(&format!("\n = note: {}", note.message));
            if let Some((file, span, source)) = self.locate(note.span) {
                text.push('\n');
                text.push_str(&diagnostics::excerpt(span, &file.name, source));
            }
        }
        if let Some(help) = &error.help {
            text.push_str(&format!("\n = help: {help}"));
        }
        text
    }
    fn add(&mut self, name: String, source: &str) -> Result<usize, Diagnostic> {
        let additional = source
            .len()
            .checked_add(1)
            .ok_or_else(|| limit(Span::default(), "source coordinate size"))?;
        self.text
            .try_reserve(additional)
            .map_err(|_| limit(Span::default(), "source coordinate storage"))?;
        let start = self.text.len();
        self.files.push(SourceFile {
            name,
            start,
            length: source.len(),
        });
        self.text.push_str(source);
        self.text.push('\n');
        Ok(start)
    }
}

#[derive(Debug, Clone)]
pub struct ModuleDiagnostic {
    pub diagnostic: Diagnostic,
    pub sources: Arc<SourceMap>,
}
impl ModuleDiagnostic {
    pub fn render(&self) -> String {
        self.sources.render(&self.diagnostic)
    }
}
impl fmt::Display for ModuleDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}
impl std::error::Error for ModuleDiagnostic {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.diagnostic)
    }
}

#[derive(Debug)]
pub enum ModuleRunError {
    Diagnostic(ModuleDiagnostic),
    Output(io::Error),
}
impl fmt::Display for ModuleRunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Diagnostic(error) => error.fmt(f),
            Self::Output(error) => write!(f, "could not write program output: {error}"),
        }
    }
}
impl std::error::Error for ModuleRunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Diagnostic(error) => Some(error),
            Self::Output(error) => Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Function {
        parameters: Vec<crate::value::ValueType>,
        result: crate::value::ValueType,
        pure: bool,
    },
    Constant(crate::value::ValueType),
}
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub public: bool,
    pub span: Span,
    pub kind: SymbolKind,
}
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub source: SourceId,
    pub identity: String,
    pub imports: Vec<(String, SourceId)>,
    pub symbols: Vec<SymbolInfo>,
}

#[derive(Debug, Clone)]
pub struct ModuleProgram {
    program: ResolvedProgram,
    sources: Arc<SourceMap>,
    modules: Vec<ModuleInfo>,
    inspection: inspection::Inspection,
}
impl ModuleProgram {
    pub fn program(&self) -> &ResolvedProgram {
        &self.program
    }
    pub fn sources(&self) -> &Arc<SourceMap> {
        &self.sources
    }
    pub fn modules(&self) -> &[ModuleInfo] {
        &self.modules
    }
    pub fn check(&self, options: ExecutionOptions) -> Result<(), ModuleDiagnostic> {
        interpreter::validate_with_options(&self.program, options).map_err(|diagnostic| {
            ModuleDiagnostic {
                diagnostic,
                sources: self.sources.clone(),
            }
        })
    }
    pub fn execute(
        &self,
        stdout: &mut impl io::Write,
        stderr: &mut impl io::Write,
        options: ExecutionOptions,
    ) -> Result<(), ModuleRunError> {
        interpreter::execute_with_io_and_options(&self.program, stdout, stderr, options).map_err(
            |error| match error {
                RunError::Diagnostic(diagnostic) => ModuleRunError::Diagnostic(ModuleDiagnostic {
                    diagnostic,
                    sources: self.sources.clone(),
                }),
                RunError::Output(error) => ModuleRunError::Output(error),
            },
        )
    }
}

#[derive(Debug, Clone)]
struct Import {
    path: String,
    alias: String,
    span: Span,
    alias_span: Span,
    statement_span: Span,
}
#[derive(Debug, Clone)]
struct ParsedModule {
    identity: String,
    program: Program,
    imports: Vec<Import>,
    dependencies: Vec<(Import, SourceId)>,
}

/// Parsed, nonexecuted source graph. Link/type-check it with `resolve`.
#[derive(Debug)]
pub struct ModuleGraph {
    sources: SourceMap,
    modules: Vec<ParsedModule>,
    limits: ModuleLimits,
    source_bytes: usize,
    imports: usize,
}
impl ModuleGraph {
    pub fn sources(&self) -> &SourceMap {
        &self.sources
    }
    pub fn file_count(&self) -> usize {
        self.modules.len()
    }
    fn load_one(
        &mut self,
        loader: &dyn SourceLoader,
        identity: String,
        label: String,
        span: Span,
    ) -> Result<SourceId, Diagnostic> {
        if self.modules.len() >= self.limits.max_files {
            return Err(limit(span, "unique module file count"));
        }
        let allowance = self
            .limits
            .max_total_bytes
            .saturating_sub(self.source_bytes)
            .min(self.limits.max_file_bytes);
        let source = loader
            .load(&identity, allowance)
            .map_err(|error| load_error(error, span))?;
        if source.len() > allowance {
            return Err(limit(span, "module source bytes"));
        }
        self.source_bytes += source.len();
        let base = self.sources.add(label, &source)?;
        let mut tokens = lexer::lex(&source).map_err(|mut error| {
            shift(&mut error.span, base);
            error
        })?;
        for token in &mut tokens {
            shift(&mut token.span, base);
        }
        let program = parser::parse(&tokens)?;
        let imports = validate_imports(
            &program,
            self.limits.max_imports.saturating_sub(self.imports),
        )?;
        self.imports += imports.len();
        let id = self.modules.len();
        self.modules.push(ParsedModule {
            identity,
            program,
            imports,
            dependencies: Vec::new(),
        });
        Ok(id)
    }
    fn discover(
        &mut self,
        entry: &str,
        loader: &dyn SourceLoader,
        identity: Option<String>,
    ) -> Result<(), Diagnostic> {
        let identity = match identity {
            Some(identity) => identity,
            None => loader
                .resolve(None, entry)
                .map_err(|e| load_error(e, Span::default()))?,
        };
        self.load_one(loader, identity.clone(), entry.to_string(), Span::default())?;
        let mut known = HashMap::from([(identity, 0)]);
        let mut active = vec![true];
        let mut stack = vec![(0usize, 0usize)];
        let mut ordered = Vec::new();
        while let Some(&(id, index)) = stack.last() {
            if index == self.modules[id].imports.len() {
                ordered.push(id);
                active[id] = false;
                stack.pop();
                continue;
            }
            stack.last_mut().unwrap().1 += 1;
            let import = self.modules[id].imports[index].clone();
            let identity = loader
                .resolve(Some(&self.modules[id].identity), &import.path)
                .map_err(|e| load_error(e, import.span))?;
            let child = if let Some(&child) = known.get(&identity) {
                if active[child] {
                    let mut error = Diagnostic::new(
                        Kind::ModuleCycle,
                        import.span,
                        format!(
                            "circular import reaches {:?}",
                            self.sources.files[child].name
                        ),
                    )
                    .with_help(
                        "remove an import edge; mutually recursive functions belong in one module",
                    );
                    let cycle_start = stack.iter().position(|&(node, _)| node == child).unwrap();
                    // The closing edge is primary. Related spans are only the
                    // remaining actual edges in this cycle, not its prefix.
                    for &(node, next) in &stack[cycle_start..stack.len() - 1] {
                        error = error.with_related(
                            self.modules[node].imports[next - 1].span,
                            "this import continues the cycle",
                        );
                    }
                    return Err(error);
                }
                child
            } else {
                if stack.len() > self.limits.max_depth {
                    return Err(limit(import.span, "import depth"));
                }
                let child = self.load_one(
                    loader,
                    identity.clone(),
                    loader.display_name(&identity),
                    import.span,
                )?;
                known.insert(identity, child);
                active.push(true);
                stack.push((child, 0));
                child
            };
            self.modules[id].dependencies.push((import, child));
        }
        // Cached diamonds must obey the longest graph path, not only the path
        // on which each source happened to be discovered first.
        let mut heights = vec![0usize; self.modules.len()];
        for id in ordered {
            for (import, child) in &self.modules[id].dependencies {
                let height = heights[*child]
                    .checked_add(1)
                    .ok_or_else(|| limit(import.span, "import depth"))?;
                if height > self.limits.max_depth {
                    return Err(limit(import.span, "import depth"));
                }
                heights[id] = heights[id].max(height);
            }
        }
        Ok(())
    }
}

fn duplicate(name: &str, span: Span, previous: Span) -> Diagnostic {
    Diagnostic::new(
        Kind::DuplicateBinding,
        span,
        format!("module symbol {name:?} is already declared"),
    )
    .with_related(previous, "the conflicting declaration is here")
    .with_help("choose distinct import aliases and exported declaration names")
}

/// Validate the whole list before any dependency-loader callback. Borrowed name
/// maps avoid the previous repeated scans over imports and declarations.
fn validate_imports(program: &Program, allowance: usize) -> Result<Vec<Import>, Diagnostic> {
    if !program
        .statements
        .iter()
        .any(|s| matches!(s, Statement::Import { .. }))
    {
        return Ok(Vec::new());
    }
    let mut declarations = HashMap::new();
    for wrapped in &program.statements {
        let statement = match wrapped {
            Statement::Export { declaration, .. } => declaration.as_ref(),
            other => other,
        };
        let declared = match statement {
            Statement::Function(f) => Some((f.name.as_str(), f.name_span)),
            Statement::Declaration {
                name, name_span, ..
            } => Some((name.as_str(), *name_span)),
            _ => None,
        };
        if let Some((name, span)) = declared {
            declarations.entry(name).or_insert(span);
        }
    }
    let mut aliases = HashMap::new();
    let mut imports = Vec::new();
    for statement in &program.statements {
        if let Statement::Import {
            path,
            alias,
            path_span,
            alias_span,
            span,
        } = statement
        {
            if imports.len() >= allowance {
                return Err(limit(*path_span, "import declaration count"));
            }
            loader::validate_path(path, true).map_err(|e| load_error(e, *path_span))?;
            crate::resolver::check_name(alias, *alias_span)?;
            if let Some(previous) = aliases.insert(alias.as_str(), *alias_span) {
                return Err(duplicate(alias, *alias_span, previous));
            }
            if let Some(previous) = declarations.get(alias.as_str()) {
                return Err(duplicate(alias, *alias_span, *previous));
            }
            imports.push(Import {
                path: path.clone(),
                alias: alias.clone(),
                span: *path_span,
                alias_span: *alias_span,
                statement_span: *span,
            });
        }
    }
    Ok(imports)
}

pub fn load(
    entry: &str,
    loader: &dyn SourceLoader,
    limits: ModuleLimits,
) -> Result<ModuleGraph, ModuleDiagnostic> {
    load_identity(entry, loader, limits, None)
}
fn load_identity(
    entry: &str,
    loader: &dyn SourceLoader,
    limits: ModuleLimits,
    identity: Option<String>,
) -> Result<ModuleGraph, ModuleDiagnostic> {
    let mut graph = ModuleGraph {
        sources: SourceMap {
            text: String::new(),
            files: Vec::new(),
            entry: entry.to_string(),
        },
        modules: Vec::new(),
        limits,
        source_bytes: 0,
        imports: 0,
    };
    match graph.discover(entry, loader, identity) {
        Ok(()) => Ok(graph),
        Err(diagnostic) => Err(ModuleDiagnostic {
            diagnostic,
            sources: Arc::new(graph.sources),
        }),
    }
}
pub fn resolve(graph: ModuleGraph) -> Result<ModuleProgram, ModuleDiagnostic> {
    linker::resolve(graph)
}
pub fn compile_with_loader(
    entry: &str,
    loader: &dyn SourceLoader,
    limits: ModuleLimits,
) -> Result<ModuleProgram, ModuleDiagnostic> {
    resolve(load(entry, loader, limits)?)
}
pub fn compile_file(
    path: impl AsRef<Path>,
    limits: ModuleLimits,
) -> Result<ModuleProgram, ModuleDiagnostic> {
    let path = path.as_ref();
    let label = path.to_string_lossy().into_owned();
    let make_error = |message: String| ModuleDiagnostic {
        diagnostic: Diagnostic::new(Kind::ModuleLoad, Span::default(), message),
        sources: Arc::new(SourceMap {
            text: String::new(),
            files: Vec::new(),
            entry: label.clone(),
        }),
    };
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let loader = FileSourceLoader::new(parent)
        .map_err(|e| make_error(format!("could not read '{}': {e}", path.display())))?;
    let identity = loader.identify(path).map_err(|error| ModuleDiagnostic {
        diagnostic: load_error(error, Span::default()),
        sources: Arc::new(SourceMap {
            text: String::new(),
            files: Vec::new(),
            entry: label.clone(),
        }),
    })?;
    resolve(load_identity(&label, &loader, limits, Some(identity))?)
}
fn shift(span: &mut Span, base: usize) {
    span.start += base;
    span.end += base;
}
fn limit(span: Span, what: &str) -> Diagnostic {
    Diagnostic::new(
        Kind::ModuleLimit,
        span,
        format!("module {what} limit exceeded"),
    )
    .with_help("reduce the source graph or explicitly adjust ModuleLimits in the embedding host")
}
fn load_error(error: SourceLoadError, span: Span) -> Diagnostic {
    let kind = match error.kind {
        SourceLoadKind::InvalidPath => Kind::ModulePath,
        SourceLoadKind::Limit => Kind::ModuleLimit,
        _ => Kind::ModuleLoad,
    };
    Diagnostic::new(kind, span, error.message).with_help(
        "check the importing path, trusted source root, UTF-8 encoding and module limits",
    )
}
