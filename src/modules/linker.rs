//! Module-local typed symbols lowered into the existing immutable resolver plan.
use super::*;
use crate::{
    ast::{Expression, ExpressionKind, PrintArgument, Statement},
    value::{Value, ValueType},
};
use std::collections::{HashMap, HashSet};

struct Symbol {
    info: SymbolInfo,
    definition: Span,
    target: Option<String>,
    constant: Option<ExpressionKind>,
}
struct Table {
    symbols: HashMap<String, Symbol>,
    ordered: Vec<SymbolInfo>,
    aliases: HashMap<String, SourceId>,
    alias_order: Vec<String>,
}
fn syntax(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(Kind::ModuleSyntax,span,message).with_help("library files contain imports, functions and immutable scalar constants; put executable statements in the entry file")
}
fn consume(remaining: &mut usize, count: usize, span: Span) -> Result<(), Diagnostic> {
    if count > *remaining {
        return Err(limit(span, "expanded symbol/string bytes"));
    }
    *remaining -= count;
    Ok(())
}
fn collect(
    graph: &ModuleGraph,
    remaining: &mut usize,
    limits: ConstantLimits,
) -> Result<(Vec<Table>, Vec<ConstantDependency>), Diagnostic> {
    let mut tables: Vec<Option<Table>> = (0..graph.modules.len()).map(|_| None).collect();
    let mut dependencies = vec![Vec::new(); graph.modules.len()];
    let mut work = constants::Work::new(limits);
    for &id in &graph.completion_order {
        let module = &graph.modules[id];
        let mut table = Table {
            symbols: HashMap::new(),
            ordered: Vec::new(),
            aliases: HashMap::new(),
            alias_order: Vec::new(),
        };
        for (import, target) in &module.dependencies {
            table.aliases.insert(import.alias.clone(), *target);
            table.alias_order.push(import.alias.clone());
        }
        work.validate(module, id == 0)?;
        for wrapped in &module.program.statements {
            let (statement, public) = match wrapped {
                Statement::Export { declaration, .. } => (declaration.as_ref(), true),
                other => (other, false),
            };
            let symbol = match statement {
                Statement::Import { .. } => continue,
                Statement::Function(function) => {
                    crate::resolver::check_name(&function.name, function.name_span)?;
                    if function.name == "repeat" {
                        return Err(Diagnostic::new(
                            Kind::FunctionDefinition,
                            function.name_span,
                            "repeat cannot name a function because it starts a loop statement",
                        ));
                    }
                    let unqualified = graph.modules.len() == 1;
                    let size = if unqualified {
                        function.name.len()
                    } else {
                        module
                            .identity
                            .len()
                            .checked_add(function.name.len())
                            .and_then(|n| n.checked_add(2))
                            .ok_or_else(|| limit(function.name_span, "symbol identity bytes"))?
                    };
                    consume(remaining, size, function.name_span)?;
                    let target = if unqualified {
                        function.name.clone()
                    } else {
                        format!("{}::{}", module.identity, function.name)
                    };
                    Symbol {
                        definition: function.span,
                        info: SymbolInfo {
                            name: function.name.clone(),
                            public,
                            span: function.name_span,
                            kind: SymbolKind::Function {
                                parameters: function.parameters.iter().map(|p| p.kind).collect(),
                                result: function.result,
                                pure: function.pure,
                            },
                        },
                        target: Some(target),
                        constant: None,
                    }
                }
                Statement::Declaration {
                    name,
                    name_span,
                    mutable,
                    initializer,
                    ..
                } if id != 0 || public => {
                    crate::resolver::check_name(name, *name_span)?;
                    if *mutable {
                        return Err(syntax(*name_span, "library variables cannot be mutable"));
                    }
                    let value = work.evaluate(initializer, |name, span| {
                        let symbol = lookup_symbol(&table, |target| tables[target].as_ref().expect("completed dependency"), name, span)?
                            .ok_or_else(|| crate::resolver::unknown_name(name, span)
                                .with_help("constant initializers may reference only earlier module constants or imported public constants"))?;
                        let value = symbol.constant.as_ref().ok_or_else(|| Diagnostic::new(Kind::ModuleSymbol, span,
                            format!("{name:?} is a function, not a scalar constant"))
                            .with_related(symbol.info.span, "function declared here")
                            .with_help("call functions at runtime; use a scalar constant in this initializer"))?;
                        if let ExpressionKind::String(text) = value { consume(remaining, text.len(), span)?; }
                        dependencies[id].push(ConstantDependency {
                            source: id, declaration: *name_span,
                            target: name.split_once('.').map_or(id, |(alias, _)| table.aliases[alias]),
                            target_declaration: symbol.info.span, span,
                        });
                        Ok(value.clone())
                    }).map_err(|error| error.with_related(*name_span, "constant initialized here"))?;
                    let kind = value.kind();
                    let value = match value {
                        Value::Integer(n) => ExpressionKind::Integer(n),
                        Value::Float(n) => ExpressionKind::Float(n),
                        Value::Boolean(b) => ExpressionKind::Boolean(b),
                        Value::String(text) => {
                            // Literal storage remains covered by source limits,
                            // as before. Reference-derived strings create new
                            // folded storage in addition to substitution text.
                            if initializer
                                .nodes()
                                .iter()
                                .any(|node| matches!(&node.kind, ExpressionKind::Name(_)))
                            {
                                consume(remaining, text.len(), *name_span)?;
                            }
                            ExpressionKind::String(text.to_string())
                        }
                        _ => unreachable!("validated scalar constant"),
                    };
                    Symbol {
                        definition: *name_span,
                        info: SymbolInfo {
                            name: name.clone(),
                            public,
                            span: *name_span,
                            kind: SymbolKind::Constant(kind),
                        },
                        target: None,
                        constant: Some(value),
                    }
                }
                _ if id != 0 => {
                    return Err(syntax(
                        statement.span(),
                        "imported modules cannot execute file-level statements",
                    ))
                }
                _ => continue,
            };
            if let Some(previous) = table.symbols.get(&symbol.info.name) {
                if symbol.target.is_some() && previous.target.is_some() {
                    return Err(Diagnostic::new(
                        Kind::FunctionDefinition,
                        symbol.info.span,
                        format!("function {:?} is already declared", symbol.info.name),
                    )
                    .with_related(previous.definition, "the first definition is here"));
                }
                return Err(duplicate(
                    &symbol.info.name,
                    symbol.info.span,
                    previous.info.span,
                ));
            }
            table.ordered.push(symbol.info.clone());
            table.symbols.insert(symbol.info.name.clone(), symbol);
        }
        // A root script binding cannot silently replace a public constant.
        if id == 0 {
            for statement in &module.program.statements {
                if let Statement::Declaration {
                    name, name_span, ..
                } = statement
                {
                    if let Some(symbol) = table.symbols.get(name).filter(|s| s.constant.is_some()) {
                        return Err(duplicate(name, *name_span, symbol.info.span));
                    }
                }
            }
        }
        tables[id] = Some(table);
    }
    Ok((
        tables
            .into_iter()
            .map(|table| table.expect("completed source graph"))
            .collect(),
        dependencies.into_iter().flatten().collect(),
    ))
}

/// One visibility/spelling policy for constant compilation and runtime lowering.
fn lookup_symbol<'a>(
    local: &'a Table,
    table: impl Fn(SourceId) -> &'a Table,
    name: &str,
    span: Span,
) -> Result<Option<&'a Symbol>, Diagnostic> {
    if let Some((alias, member)) = name.split_once('.') {
        let target = local.aliases.get(alias).ok_or_else(|| {
            Diagnostic::new(
                Kind::ModuleSymbol,
                span,
                format!("unknown module alias {alias:?}"),
            )
            .with_help(
                match spelling_hint(alias, local.alias_order.iter().map(String::as_str)) {
                    Some(hint) => format!(
                        "did you mean module alias {hint:?}? use an import declared in this file"
                    ),
                    None => {
                        "declare an import with this alias before using qualified members".into()
                    }
                },
            )
        })?;
        let target = table(*target);
        let symbol = target.symbols.get(member).ok_or_else(|| {
            Diagnostic::new(Kind::ModuleSymbol, span, format!("module {alias:?} has no symbol {member:?}"))
                .with_help(match spelling_hint(member, target.ordered.iter().filter(|s| s.public).map(|s| s.name.as_str())) {
                    Some(hint) => format!("did you mean {alias}.{hint}? check the imported module's public declarations"),
                    None => "check the imported module's public declarations".into(),
                })
        })?;
        if !symbol.info.public {
            return Err(Diagnostic::new(
                Kind::ModuleVisibility,
                span,
                format!("{name} is private"),
            )
            .with_related(symbol.info.span, "the private declaration is here")
            .with_help("mark the declaration pub in its defining module"));
        }
        Ok(Some(symbol))
    } else {
        Ok(local.symbols.get(name))
    }
}

struct Rewriter<'a> {
    id: usize,
    tables: &'a [Table],
    remaining: &'a mut usize,
    scopes: Vec<HashSet<String>>,
}
impl Rewriter<'_> {
    fn binding_name(&self, name: &str, span: Span) -> Result<(), Diagnostic> {
        if let Some(symbol) = self.tables[self.id]
            .symbols
            .get(name)
            .filter(|symbol| symbol.target.is_some())
        {
            return Err(Diagnostic::new(
                Kind::FunctionDefinition,
                span,
                format!("binding {name:?} collides with a function"),
            )
            .with_related(symbol.definition, "function declared here"));
        }
        Ok(())
    }
    fn symbol(&self, name: &str, span: Span) -> Result<Option<&Symbol>, Diagnostic> {
        lookup_symbol(
            &self.tables[self.id],
            |target| &self.tables[target],
            name,
            span,
        )
    }
    fn expression(&mut self, expression: &mut Expression) -> Result<(), Diagnostic> {
        for node in &mut expression.nodes {
            match &mut node.kind {
                ExpressionKind::UserCall {
                    name,
                    function_span,
                    ..
                } => {
                    if let Some(symbol) = self.symbol(name, *function_span)? {
                        if let Some(target) = &symbol.target {
                            let size = target.len();
                            consume(self.remaining, size, *function_span)?;
                            // Reborrow after charging so no symbol data is cloned
                            // before its bounded expansion allowance is checked.
                            *name = self
                                .symbol(name, *function_span)?
                                .unwrap()
                                .target
                                .as_ref()
                                .unwrap()
                                .clone();
                        } else if name.contains('.')
                            || crate::numeric::Builtin::from_name(name).is_none()
                        {
                            return Err(Diagnostic::new(
                                Kind::ModuleSymbol,
                                *function_span,
                                format!("constant {name:?} is not callable"),
                            )
                            .with_help("read the constant without parentheses"));
                        }
                    }
                }
                ExpressionKind::Name(name) => {
                    if !name.contains('.')
                        && self.scopes.iter().rev().any(|scope| scope.contains(name))
                    {
                        continue;
                    }
                    if let Some(symbol) = self.symbol(name, node.span)? {
                        let size = match &symbol.constant {
                            Some(ExpressionKind::String(text)) => text.len(),
                            _ => symbol.target.as_ref().map_or(0, String::len),
                        };
                        consume(self.remaining, size, node.span)?;
                        let symbol = self.symbol(name, node.span)?.unwrap();
                        node.kind = if let Some(value) = &symbol.constant {
                            value.clone()
                        } else {
                            ExpressionKind::Name(symbol.target.as_ref().unwrap().clone())
                        };
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn list(&mut self, statements: &mut [Statement], nested: bool) -> Result<(), Diagnostic> {
        if nested {
            self.scopes.push(HashSet::new());
        }
        for statement in statements {
            match statement {
                Statement::Function(function) => {
                    let old = std::mem::take(&mut self.scopes);
                    self.scopes.push(HashSet::new());
                    for parameter in &mut function.parameters {
                        self.binding_name(&parameter.name, parameter.span)?;
                        if let Some(default) = &mut parameter.default {
                            self.expression(default)?;
                        }
                        self.scopes[0].insert(parameter.name.clone());
                    }
                    for contract in &mut function.contracts {
                        if contract.postcondition && function.result != ValueType::Unit {
                            self.scopes[0].insert("result".into());
                        }
                        self.expression(&mut contract.expression)?;
                        self.scopes[0].remove("result");
                    }
                    self.list(&mut function.statements, true)?;
                    self.scopes = old;
                    function.name = self.tables[self.id].symbols[&function.name]
                        .target
                        .as_ref()
                        .unwrap()
                        .clone();
                }
                Statement::Declaration {
                    name,
                    name_span,
                    initializer,
                    ..
                } => {
                    self.binding_name(name, *name_span)?;
                    self.expression(initializer)?;
                    self.scopes.last_mut().unwrap().insert(name.clone());
                }
                Statement::Assign {
                    name,
                    name_span,
                    value,
                    ..
                } => {
                    if !self.scopes.iter().any(|scope| scope.contains(name))
                        && self.tables[self.id]
                            .symbols
                            .get(name)
                            .is_some_and(|s| s.constant.is_some())
                    {
                        return Err(Diagnostic::new(
                            Kind::ImmutableAssignment,
                            *name_span,
                            format!("module constant {name:?} cannot be assigned"),
                        ));
                    }
                    self.expression(value)?;
                }
                Statement::Call { expression, .. } => self.expression(expression)?,
                Statement::Return { value, .. } => {
                    if let Some(value) = value {
                        self.expression(value)?;
                    }
                }
                Statement::Repeat {
                    count,
                    index,
                    index_span,
                    statements,
                    ..
                } => {
                    self.expression(count)?;
                    self.binding_name(index, *index_span)?;
                    self.scopes.push(HashSet::from([index.clone()]));
                    self.list(statements, false)?;
                    self.scopes.pop();
                }
                Statement::While {
                    condition,
                    statements,
                    ..
                } => {
                    self.expression(condition)?;
                    self.list(statements, true)?;
                }
                Statement::Block { statements, .. } => self.list(statements, true)?,
                Statement::If {
                    branches,
                    otherwise,
                    ..
                } => {
                    for branch in branches {
                        self.expression(&mut branch.condition)?;
                        self.list(&mut branch.statements, true)?;
                    }
                    self.list(otherwise, true)?;
                }
                Statement::Println { argument, .. } => {
                    if let PrintArgument::Expression(e) = argument {
                        self.expression(e)?;
                    }
                }
                Statement::Output { arguments, .. } => {
                    for argument in arguments {
                        if let PrintArgument::Expression(e) = argument {
                            self.expression(e)?;
                        }
                    }
                }
                Statement::Break { .. } | Statement::Continue { .. } => {}
                Statement::Import { span, .. } | Statement::Export { span, .. } => {
                    return Err(syntax(*span, "module declarations cannot be nested"))
                }
            }
        }
        if nested {
            self.scopes.pop();
        }
        Ok(())
    }
}

pub(super) fn resolve(
    mut graph: ModuleGraph,
    constants: ConstantLimits,
) -> Result<ModuleProgram, ModuleDiagnostic> {
    let result = (|| -> Result<_, Diagnostic> {
        let mut remaining = graph.limits.max_expansion_bytes;
        let (tables, dependencies) = collect(&graph, &mut remaining, constants)?;
        let mut statements = Vec::new();
        let mut info = Vec::new();
        for (id, module) in graph.modules.iter_mut().enumerate() {
            let mut body = Vec::new();
            for wrapped in std::mem::take(&mut module.program.statements) {
                let (statement, public) = match wrapped {
                    Statement::Export { declaration, .. } => (*declaration, true),
                    other => (other, false),
                };
                match &statement {
                    Statement::Import { .. } => {}
                    Statement::Declaration { .. } if id != 0 || public => {}
                    _ => body.push(statement),
                }
            }
            Rewriter {
                id,
                tables: &tables,
                remaining: &mut remaining,
                scopes: vec![HashSet::new()],
            }
            .list(&mut body, false)?;
            statements.extend(body);
            info.push(ModuleInfo {
                source: id,
                identity: module.identity.clone(),
                imports: module
                    .dependencies
                    .iter()
                    .map(|(i, t)| (i.alias.clone(), *t))
                    .collect(),
                symbols: tables[id].ordered.clone(),
            });
        }
        Ok((
            crate::resolver::resolve(Program { statements })?,
            info,
            dependencies,
        ))
    })();
    let sources = Arc::new(graph.sources);
    match result {
        Ok((program, modules, constant_dependencies)) => {
            let inspection = inspection::Inspection::new(&graph.modules, &modules);
            debug_assert_eq!(inspection.function_count(), program.functions().len());
            Ok(ModuleProgram {
                program,
                sources,
                modules,
                inspection,
                constant_dependencies,
            })
        }
        Err(diagnostic) => Err(ModuleDiagnostic {
            diagnostic,
            sources,
        }),
    }
}
