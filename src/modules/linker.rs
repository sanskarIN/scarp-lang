//! Module-local typed symbols lowered into the existing immutable resolver plan.
use super::*;
use crate::{
    ast::{Expression, ExpressionKind, PrintArgument, Statement},
    value::{FiniteFloat, ValueType},
};
use std::collections::{HashMap, HashSet};

struct Symbol {
    info: SymbolInfo,
    target: Option<String>,
    constant: Option<ExpressionKind>,
}
struct Table {
    symbols: HashMap<String, Symbol>,
    ordered: Vec<SymbolInfo>,
    aliases: HashMap<String, SourceId>,
}
fn syntax(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(Kind::ModuleSyntax,span,message).with_help("library files contain imports, functions and immutable scalar literals; put executable statements in the entry file")
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
fn consume(remaining: &mut usize, count: usize, span: Span) -> Result<(), Diagnostic> {
    if count > *remaining {
        return Err(limit(span, "expanded symbol/string bytes"));
    }
    *remaining -= count;
    Ok(())
}
fn scalar(expression: &Expression) -> Result<ExpressionKind, Diagnostic> {
    let mut id = expression.root();
    let mut signs = Vec::new();
    let mut value = loop {
        match &expression.nodes()[id].kind {
            ExpressionKind::Group { expression } => id = *expression,
            ExpressionKind::Negate {
                operand,
                operator_span,
            } => {
                signs.push(*operator_span);
                id = *operand;
            }
            kind @ (ExpressionKind::Integer(_)
            | ExpressionKind::Float(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::String(_)) => break kind.clone(),
            _ => {
                return Err(syntax(
                    expression.nodes()[id].span,
                    "module constants require a scalar literal, not an executable initializer",
                ))
            }
        }
    };
    for span in signs.into_iter().rev() {
        value = match value {
            ExpressionKind::Integer(n) => {
                ExpressionKind::Integer(n.checked_neg().ok_or_else(|| {
                    Diagnostic::new(
                        Kind::ArithmeticOverflow,
                        span,
                        "integer literal negation overflows",
                    )
                })?)
            }
            ExpressionKind::Float(n) => {
                ExpressionKind::Float(FiniteFloat::new(-n.get()).expect("finite negation"))
            }
            _ => return Err(syntax(span, "only numeric module literals can be negated")),
        };
    }
    Ok(value)
}
fn scalar_type(kind: &ExpressionKind) -> ValueType {
    match kind {
        ExpressionKind::Integer(_) => ValueType::Integer,
        ExpressionKind::Float(_) => ValueType::Float,
        ExpressionKind::Boolean(_) => ValueType::Boolean,
        ExpressionKind::String(_) => ValueType::String,
        _ => unreachable!("checked scalar literal"),
    }
}
fn collect(graph: &ModuleGraph, remaining: &mut usize) -> Result<Vec<Table>, Diagnostic> {
    let mut tables = Vec::new();
    for (id, module) in graph.modules.iter().enumerate() {
        let mut table = Table {
            symbols: HashMap::new(),
            ordered: Vec::new(),
            aliases: HashMap::new(),
        };
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
                        return Err(syntax(function.name_span, "repeat cannot name a function"));
                    }
                    let size = if id == 0 {
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
                    let target = if id == 0 {
                        function.name.clone()
                    } else {
                        format!("{}::{}", module.identity, function.name)
                    };
                    Symbol {
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
                    let value = scalar(initializer)?;
                    Symbol {
                        info: SymbolInfo {
                            name: name.clone(),
                            public,
                            span: *name_span,
                            kind: SymbolKind::Constant(scalar_type(&value)),
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
                return Err(duplicate(
                    &symbol.info.name,
                    symbol.info.span,
                    previous.info.span,
                ));
            }
            table.ordered.push(symbol.info.clone());
            table.symbols.insert(symbol.info.name.clone(), symbol);
        }
        for (import, target) in &module.dependencies {
            crate::resolver::check_name(&import.alias, import.alias_span)?;
            if let Some(previous) = module
                .dependencies
                .iter()
                .take_while(|(earlier, _)| earlier.alias_span != import.alias_span)
                .find(|(earlier, _)| earlier.alias == import.alias)
            {
                return Err(duplicate(
                    &import.alias,
                    import.alias_span,
                    previous.0.alias_span,
                ));
            }
            for wrapped in &module.program.statements {
                let statement = match wrapped {
                    Statement::Export { declaration, .. } => declaration.as_ref(),
                    s => s,
                };
                let declaration = match statement {
                    Statement::Function(f) => Some((&f.name, f.name_span)),
                    Statement::Declaration {
                        name, name_span, ..
                    } => Some((name, *name_span)),
                    _ => None,
                };
                if let Some((name, span)) = declaration {
                    if name == &import.alias {
                        return Err(duplicate(name, import.alias_span, span));
                    }
                }
            }
            table.aliases.insert(import.alias.clone(), *target);
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
        tables.push(table);
    }
    Ok(tables)
}

struct Rewriter<'a> {
    id: usize,
    tables: &'a [Table],
    remaining: &'a mut usize,
    scopes: Vec<HashSet<String>>,
}
impl Rewriter<'_> {
    fn symbol(&self, name: &str, span: Span) -> Result<Option<&Symbol>, Diagnostic> {
        if let Some((alias, member)) = name.split_once('.') {
            let target = self.tables[self.id].aliases.get(alias).ok_or_else(|| {
                Diagnostic::new(
                    Kind::ModuleSymbol,
                    span,
                    format!("unknown module alias {alias:?}"),
                )
                .with_help("declare an import with this alias before using qualified members")
            })?;
            let symbol = self.tables[*target].symbols.get(member).ok_or_else(|| {
                Diagnostic::new(
                    Kind::ModuleSymbol,
                    span,
                    format!("module {alias:?} has no symbol {member:?}"),
                )
                .with_help("check the imported module's public declarations")
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
            Ok(self.tables[self.id].symbols.get(name))
        }
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
                        if let Some(default) = &mut parameter.default {
                            self.expression(default)?;
                        }
                        self.scopes[0].insert(parameter.name.clone());
                    }
                    for contract in &mut function.contracts {
                        if contract.postcondition {
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
                    name, initializer, ..
                } => {
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
                    statements,
                    ..
                } => {
                    self.expression(count)?;
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

pub(super) fn resolve(mut graph: ModuleGraph) -> Result<ModuleProgram, ModuleDiagnostic> {
    let result = (|| -> Result<(ResolvedProgram, Vec<ModuleInfo>), Diagnostic> {
        let mut remaining = graph.limits.max_expansion_bytes;
        let tables = collect(&graph, &mut remaining)?;
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
        Ok((crate::resolver::resolve(Program { statements })?, info))
    })();
    let sources = Arc::new(graph.sources);
    match result {
        Ok((program, modules)) => Ok(ModuleProgram {
            program,
            sources,
            modules,
        }),
        Err(diagnostic) => Err(ModuleDiagnostic {
            diagnostic,
            sources,
        }),
    }
}
