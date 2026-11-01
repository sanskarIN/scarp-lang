//! Immutable tooling views, reusing resolved signatures rather than cloning them.
use super::*;
use crate::{
    diagnostics::{Location, LocationTracker},
    resolver::FunctionInfo,
};
use std::{collections::HashSet, ops::Range};

/// A validated import declaration. All spans use the program's source map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportInfo {
    pub source: SourceId,
    pub target: SourceId,
    pub path: String,
    pub alias: String,
    pub span: Span,
    pub path_span: Span,
    pub alias_span: Span,
}

/// Validated file-local span and one-based Unicode-scalar locations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub source: SourceId,
    pub span: Span,
    pub start: Location,
    pub end: Location,
}

#[derive(Debug, Clone)]
pub(super) struct Inspection {
    imports: Vec<ImportInfo>,
    import_ranges: Vec<Range<usize>>,
    function_ranges: Vec<Range<usize>>,
    forward: Vec<Vec<SourceId>>,
    reverse: Vec<Vec<SourceId>>,
}
impl Inspection {
    pub(super) fn new(parsed: &[ParsedModule], modules: &[ModuleInfo]) -> Self {
        let mut result = Self {
            imports: Vec::new(),
            import_ranges: Vec::new(),
            function_ranges: Vec::new(),
            forward: vec![Vec::new(); modules.len()],
            reverse: vec![Vec::new(); modules.len()],
        };
        let mut function = 0;
        for (id, module) in parsed.iter().enumerate() {
            let start = result.imports.len();
            let mut seen = HashSet::new();
            for (import, target) in &module.dependencies {
                result.imports.push(ImportInfo {
                    source: id,
                    target: *target,
                    path: import.path.clone(),
                    alias: import.alias.clone(),
                    span: import.statement_span,
                    path_span: import.span,
                    alias_span: import.alias_span,
                });
                if seen.insert(*target) {
                    result.forward[id].push(*target);
                    result.reverse[*target].push(id);
                }
            }
            result.import_ranges.push(start..result.imports.len());
            let count = modules[id]
                .symbols
                .iter()
                .filter(|s| matches!(&s.kind, SymbolKind::Function { .. }))
                .count();
            result.function_ranges.push(function..function + count);
            function += count;
        }
        result
    }
    pub(super) fn function_count(&self) -> usize {
        self.function_ranges.last().map_or(0, |range| range.end)
    }
}

impl ModuleProgram {
    /// All import declarations, in module discovery then declaration order.
    pub fn imports(&self) -> &[ImportInfo] {
        &self.inspection.imports
    }
    pub fn imports_in(&self, source: SourceId) -> Option<&[ImportInfo]> {
        let range = self.inspection.import_ranges.get(source)?.clone();
        Some(&self.inspection.imports[range])
    }
    /// Unique direct targets in first-import order. Invalid IDs return None.
    pub fn dependencies(&self, source: SourceId) -> Option<&[SourceId]> {
        self.inspection.forward.get(source).map(Vec::as_slice)
    }
    /// Unique direct importers in source discovery order.
    pub fn dependents(&self, source: SourceId) -> Option<&[SourceId]> {
        self.inspection.reverse.get(source).map(Vec::as_slice)
    }
    /// Reachable targets in source discovery order, excluding the queried file.
    pub fn transitive_dependencies(&self, source: SourceId) -> Option<Vec<SourceId>> {
        closure(&self.inspection.forward, source)
    }
    /// Reachable importers in source discovery order, excluding the queried file.
    pub fn transitive_dependents(&self, source: SourceId) -> Option<Vec<SourceId>> {
        closure(&self.inspection.reverse, source)
    }
    /// Pair original module symbols with borrowed resolved function metadata.
    /// FunctionInfo.name is an internal execution identity; use SymbolInfo.name
    /// for the original declaration name. No defaults/contracts are evaluated.
    pub fn functions_in(
        &self,
        source: SourceId,
    ) -> Option<impl Iterator<Item = (&SymbolInfo, &FunctionInfo)>> {
        let module = self.modules.get(source)?;
        let range = self.inspection.function_ranges.get(source)?.clone();
        Some(
            module
                .symbols
                .iter()
                .filter(|s| matches!(&s.kind, SymbolKind::Function { .. }))
                .zip(self.program.functions()[range].iter()),
        )
    }

    /// Write a deterministic static inventory without evaluating Scarp code.
    /// Loader-provided filenames are escaped. I/O errors preserve their kind;
    /// earlier inventory bytes may remain after a writer failure.
    ///
    /// ```
    /// use scarp::modules::{self, MemorySourceLoader, ModuleLimits};
    /// let mut loader = MemorySourceLoader::new();
    /// loader.insert("main.srp", "import \"lib.srp\" as lib; print(1/0);")?;
    /// loader.insert("lib.srp", "pub bind value=3;")?;
    /// let program = modules::compile_with_loader("main.srp", &loader, ModuleLimits::default())?;
    /// assert_eq!(program.dependencies(0), Some([1].as_slice()));
    /// let mut inventory = Vec::new();
    /// program.write_inspection(&mut inventory)?;
    /// assert!(String::from_utf8(inventory)?.contains("pub bind value: integer"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn write_inspection(&self, out: &mut impl io::Write) -> io::Result<()> {
        writeln!(
            out,
            "modules: {}, imports: {}",
            self.modules.len(),
            self.imports().len()
        )?;
        for module in &self.modules {
            let id = module.source;
            let file = &self.sources.files[id];
            let source = self.sources.source(id).unwrap();
            writeln!(out, "module [{id}] {:?}", file.name)?;
            let imports = self.imports_in(id).unwrap();
            if imports.is_empty() {
                writeln!(out, "  imports: none")?;
            }
            let mut positions = LocationTracker::new(source);
            for import in imports {
                let at = positions.at(import.path_span.start - file.start);
                writeln!(
                    out,
                    "  import {} = {:?} -> [{}] @ {}:{}",
                    import.alias, import.path, import.target, at.line, at.column
                )?;
            }
            write_ids(out, "dependencies", self.dependencies(id).unwrap())?;
            write_ids(out, "dependents", self.dependents(id).unwrap())?;
            if module.symbols.is_empty() {
                writeln!(out, "  symbols: none")?;
            }
            let mut functions = self.functions_in(id).unwrap();
            let mut positions = LocationTracker::new(source);
            for symbol in &module.symbols {
                let visibility = if symbol.public { "pub" } else { "private" };
                let at = positions.at(symbol.span.start - file.start);
                match &symbol.kind {
                    SymbolKind::Constant(kind) => {
                        write!(out, "  {visibility} bind {}: {kind}", symbol.name)?
                    }
                    SymbolKind::Function { result, pure, .. } => {
                        let (_, function) = functions.next().unwrap();
                        write!(
                            out,
                            "  {visibility} {}funcra {}(",
                            if *pure { "pure " } else { "" },
                            symbol.name
                        )?;
                        for (index, parameter) in function.parameters.iter().enumerate() {
                            if index != 0 {
                                write!(out, ", ")?;
                            }
                            write!(
                                out,
                                "{}: {}{}",
                                parameter.name,
                                parameter.kind,
                                if parameter.default.is_some() {
                                    " = <default>"
                                } else {
                                    ""
                                }
                            )?;
                        }
                        write!(out, ") -> {result}")?;
                    }
                }
                writeln!(out, " @ {}:{}", at.line, at.column)?;
            }
        }
        Ok(())
    }
}

fn closure(links: &[Vec<SourceId>], source: SourceId) -> Option<Vec<SourceId>> {
    let mut stack = links.get(source)?.clone();
    let mut seen = vec![false; links.len()];
    seen[source] = true;
    while let Some(id) = stack.pop() {
        if seen[id] {
            continue;
        }
        seen[id] = true;
        stack.extend(links[id].iter().copied().filter(|&next| !seen[next]));
    }
    Some(
        seen.into_iter()
            .enumerate()
            .filter_map(|(id, yes)| (yes && id != source).then_some(id))
            .collect(),
    )
}
fn write_ids(out: &mut impl io::Write, label: &str, ids: &[SourceId]) -> io::Result<()> {
    write!(out, "  {label}: ")?;
    if ids.is_empty() {
        write!(out, "none")?;
    }
    for (index, id) in ids.iter().enumerate() {
        if index != 0 {
            write!(out, ", ")?;
        }
        write!(out, "[{id}]")?;
    }
    writeln!(out)
}
