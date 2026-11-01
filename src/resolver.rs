//! Resolve lexical names into an immutable, flat execution plan.

use std::collections::HashMap;

mod functions;
#[cfg(test)]
mod retirement_audit;
use functions::{build_call_graph, resolve_function};

use crate::{
    ast::{
        AssignmentOperator, BinaryOperator, Expression, ExpressionKind, FunctionDeclaration,
        OutputTarget, PrintArgument, Program, Statement,
    },
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    numeric::Builtin,
    parser::MAX_BLOCK_DEPTH,
    value::{type_mismatch, ValueType},
};

/// An immutable executable program produced by name resolution.
/// Its private storage plan cannot be changed independently of its expressions.
#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub(crate) statements: Vec<ResolvedOperation>,
    slot_types: Vec<ValueType>,
    statement_count: usize,
    pub(crate) functions: Vec<ResolvedFunction>,
    metadata: Vec<FunctionInfo>,
}

impl ResolvedProgram {
    /// Source-ordered function metadata, produced without executing user code.
    pub fn functions(&self) -> &[FunctionInfo] {
        &self.metadata
    }
    /// Number of executable operations, including branch guards and declarations,
    /// excluding block containers and unconditional bookkeeping jumps.
    pub fn statement_count(&self) -> usize {
        self.statement_count
    }

    /// Number of distinct storage slots required by one execution.
    pub fn binding_count(&self) -> usize {
        self.slot_types.len()
    }

    /// Inferred types in declaration/slot order, including declarations in blocks.
    pub fn binding_types(&self) -> &[ValueType] {
        &self.slot_types
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedOperation {
    pub kind: ResolvedStatement,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedStatement {
    Return {
        value: Option<ResolvedExpression>,
    },
    Call {
        expression: ResolvedExpression,
    },
    RepeatInit {
        count: ResolvedExpression,
        count_slot: usize,
        index_slot: usize,
    },
    RepeatGuard {
        count_slot: usize,
        index_slot: usize,
        target: usize,
    },
    RepeatNext {
        index_slot: usize,
    },
    LoopControl {
        target: usize,
        // Slots belonging to scopes exited by this control transfer.
        retire: Vec<usize>,
    },
    // Compiler-planned retirement: no source syntax or runtime graph traversal.
    Retire {
        slots: Vec<usize>,
    },
    JumpIfFalse {
        condition: ResolvedExpression,
        target: usize,
    },
    Jump {
        target: usize,
    },
    Store {
        slot: usize,
        value: ResolvedExpression,
    },
    Update {
        slot: usize,
        operator: BinaryOperator,
        value: ResolvedExpression,
        operator_span: Span,
    },
    Output {
        arguments: Vec<ResolvedArgument>,
        target: OutputTarget,
        newline: bool,
    },
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedArgument {
    String(String),
    Expression(ResolvedExpression),
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedExpression {
    pub syntax: Expression,
    pub slots: Vec<Option<usize>>,
    pub kind: ValueType,
    pub calls: Vec<Option<ResolvedCall>>,
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedCall {
    User {
        function: usize,
        /// Parameter indices in the original explicit-argument order.
        parameters: Vec<usize>,
    },
    Builtin(Builtin),
}

/// Static function signature and call-graph information. Spans are source bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionInfo {
    pub name: String,
    pub span: Span,
    pub parameters: Vec<ParameterInfo>,
    pub result: ValueType,
    pub pure: bool,
    pub contracts: Vec<ContractInfo>,
    pub callees: Vec<String>,
    pub recursive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterInfo {
    pub name: String,
    pub kind: ValueType,
    pub span: Span,
    pub default: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractInfo {
    pub postcondition: bool,
    pub span: Span,
}

/// Deterministic human-readable static function information. Does not execute.
pub fn write_function_insight(
    program: &ResolvedProgram,
    output: &mut impl std::io::Write,
) -> std::io::Result<()> {
    writeln!(output, "Scarp function insight 1")?;
    for function in program.functions() {
        write!(
            output,
            "{} funcra {}(",
            if function.pure { "pure" } else { "impure" },
            function.name
        )?;
        for (index, parameter) in function.parameters.iter().enumerate() {
            if index > 0 {
                write!(output, ", ")?;
            }
            write!(output, "{}: {}", parameter.name, parameter.kind)?;
            if let Some(span) = parameter.default {
                write!(output, " = default@{}..{}", span.start, span.end)?;
            }
        }
        writeln!(
            output,
            ") -> {} [{}..{}]",
            function.result, function.span.start, function.span.end
        )?;
        for parameter in &function.parameters {
            writeln!(
                output,
                "  parameter {} [{}..{}]",
                parameter.name, parameter.span.start, parameter.span.end
            )?;
        }
        for contract in &function.contracts {
            writeln!(
                output,
                "  {} [{}..{}]",
                if contract.postcondition {
                    "ensures"
                } else {
                    "needs"
                },
                contract.span.start,
                contract.span.end
            )?;
        }
        writeln!(
            output,
            "  callees: {}",
            if function.callees.is_empty() {
                "-".into()
            } else {
                function.callees.join(", ")
            }
        )?;
        writeln!(output, "  recursive: {}", function.recursive)?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedFunction {
    pub statements: Vec<ResolvedOperation>,
    pub slots: usize,
    pub defaults: Vec<Option<ResolvedExpression>>,
    pub contracts: Vec<ResolvedContract>,
    pub result_slot: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedContract {
    pub postcondition: bool,
    pub expression: ResolvedExpression,
    pub span: Span,
}

struct Binding {
    slot: usize,
    declaration: Span,
    mutable: bool,
    kind: ValueType,
    repeat_index: bool,
    parameter: bool,
}
type Scope = HashMap<String, Binding>;

/// Resolve a syntax tree. Inner declarations can shadow outer names, but never
/// reuse a slot, and never escape their lexical block.
pub fn resolve(program: Program) -> Result<ResolvedProgram, Diagnostic> {
    let mut declarations = Vec::new();
    let mut statements = Vec::new();
    let mut metadata: Vec<FunctionInfo> = Vec::new();
    let mut names: HashMap<String, usize> = HashMap::new();
    for statement in program.statements {
        if matches!(
            statement,
            Statement::Import { .. } | Statement::Export { .. }
        ) {
            return Err(Diagnostic::new(
                DiagnosticKind::ModuleSyntax,
                statement.span(),
                "module declarations require an explicit source loader",
            )
            .with_help("use modules::compile_with_loader or modules::compile_file"));
        }
        if let Statement::Function(function) = statement {
            check_name(&function.name, function.name_span)?;
            if function.name == "repeat" {
                return Err(Diagnostic::new(
                    DiagnosticKind::FunctionDefinition,
                    function.name_span,
                    "repeat cannot name a function because it starts a loop statement",
                ));
            }
            if let Some(&previous) = names.get(&function.name) {
                return Err(Diagnostic::new(
                    DiagnosticKind::FunctionDefinition,
                    function.name_span,
                    format!("function {:?} is already declared", function.name),
                )
                .with_related(metadata[previous].span, "the first definition is here"));
            }
            if metadata.len() >= 4096 || function.parameters.len() > 1024 {
                return Err(Diagnostic::new(
                    DiagnosticKind::FunctionDefinition,
                    function.span,
                    "function declarations exceed 4096 functions or 1024 parameters per function",
                ));
            }
            names.insert(function.name.clone(), metadata.len());
            metadata.push(FunctionInfo {
                name: function.name.clone(),
                span: function.span,
                parameters: function
                    .parameters
                    .iter()
                    .map(|p| ParameterInfo {
                        name: p.name.clone(),
                        kind: p.kind,
                        span: p.span,
                        default: p.default.as_ref().map(|e| e.nodes()[e.root()].span),
                    })
                    .collect(),
                result: function.result,
                pure: function.pure,
                contracts: function
                    .contracts
                    .iter()
                    .map(|c| ContractInfo {
                        postcondition: c.postcondition,
                        span: c.span,
                    })
                    .collect(),
                callees: Vec::new(),
                recursive: false,
            });
            declarations.push(function);
        } else {
            statements.push(statement);
        }
    }
    let functions = declarations
        .into_iter()
        .map(|f| resolve_function(f, &metadata, &names))
        .collect::<Result<Vec<_>, _>>()?;
    let mut resolver = Resolver::new(&metadata, &names);
    resolver.statement_list(statements)?;
    let statement_count = resolver
        .statements
        .iter()
        .filter(|statement| {
            !matches!(
                statement.kind,
                ResolvedStatement::Jump { .. } | ResolvedStatement::Retire { .. }
            )
        })
        .count();
    let mut result = ResolvedProgram {
        statements: resolver.statements,
        slot_types: resolver.slot_types,
        statement_count,
        functions,
        metadata,
    };
    build_call_graph(&mut result);
    #[cfg(test)]
    if let Err(error) = retirement_audit::audit(&result) {
        panic!("resolved storage plan is invalid: {error}");
    }
    Ok(result)
}

struct Resolver<'a> {
    scopes: Vec<Scope>,
    statements: Vec<ResolvedOperation>,
    slot_types: Vec<ValueType>,
    loops: Vec<LoopContext>,
    functions: &'a [FunctionInfo],
    names: &'a HashMap<String, usize>,
    result: Option<ValueType>,
    pure: bool,
}

struct LoopContext {
    breaks: Vec<usize>,
    continues: Vec<usize>,
    // Scopes created inside this loop must retire before break/continue.
    scope_depth: usize,
}

impl<'a> Resolver<'a> {
    fn new(functions: &'a [FunctionInfo], names: &'a HashMap<String, usize>) -> Self {
        Self {
            scopes: vec![HashMap::new()],
            statements: Vec::new(),
            slot_types: Vec::new(),
            loops: Vec::new(),
            functions,
            names,
            result: None,
            pure: false,
        }
    }

    fn expression(&self, syntax: Expression) -> Result<ResolvedExpression, Diagnostic> {
        resolve_expression(syntax, &self.scopes, self.functions, self.names, self.pure)
    }

    fn binding_name(&self, name: &str, span: Span) -> Result<(), Diagnostic> {
        check_name(name, span)?;
        if let Some(&function) = self.names.get(name) {
            return Err(Diagnostic::new(
                DiagnosticKind::FunctionDefinition,
                span,
                format!("binding {name:?} collides with a function"),
            )
            .with_related(self.functions[function].span, "function declared here"));
        }
        Ok(())
    }
    fn statement_list(&mut self, statements: Vec<Statement>) -> Result<(), Diagnostic> {
        for statement in statements {
            let span = statement.span();
            match statement {
                Statement::Import { .. } | Statement::Export { .. } => {
                    return Err(Diagnostic::new(
                        DiagnosticKind::ModuleSyntax,
                        span,
                        "module declarations require an explicit source loader",
                    )
                    .with_help("use modules::compile_with_loader or modules::compile_file"));
                }
                Statement::Function(function) => {
                    return Err(Diagnostic::new(
                        DiagnosticKind::FunctionUnsupported,
                        function.span,
                        "function declarations are allowed only at file scope",
                    ))
                }
                Statement::Return { value, span } => {
                    let expected = self.result.ok_or_else(|| {
                        Diagnostic::new(
                            DiagnosticKind::InvalidReturn,
                            span,
                            "return is only valid inside funcra",
                        )
                    })?;
                    let value = value.map(|v| self.expression(v)).transpose()?;
                    match (&value, expected) {
                        (None, ValueType::Unit) => {}
                        (Some(value), kind) if kind != ValueType::Unit && value.kind == kind => {}
                        _ => {
                            return Err(Diagnostic::new(
                                DiagnosticKind::InvalidReturn,
                                span,
                                format!(
                                    "return must {}",
                                    if expected == ValueType::Unit {
                                        "be bare for a unit function".into()
                                    } else {
                                        format!("produce {expected}")
                                    }
                                ),
                            ))
                        }
                    }
                    self.emit(ResolvedStatement::Return { value }, span);
                }
                Statement::Call { expression, span } => {
                    if !matches!(
                        expression.nodes()[expression.root()].kind,
                        ExpressionKind::UserCall { .. }
                    ) {
                        return Err(Diagnostic::new(
                            DiagnosticKind::CallArguments,
                            span,
                            "a call statement requires a call expression",
                        ));
                    }
                    let expression = self.expression(expression)?;
                    self.emit(ResolvedStatement::Call { expression }, span);
                }
                Statement::Repeat {
                    count,
                    index,
                    index_span,
                    statements,
                    span,
                } => {
                    let count = self.expression(count)?;
                    let count_span = count.syntax.nodes()[count.syntax.root()].span;
                    require_type(count.kind, ValueType::Integer, count_span).map_err(|error| {
                        error.with_help(
                            "repeat needs an integer count: repeat 5 as index { statements }",
                        )
                    })?;
                    self.binding_name(&index, index_span)?;
                    let count_slot = self.slot_types.len();
                    let index_slot = count_slot + 1;
                    self.slot_types
                        .extend([ValueType::Integer, ValueType::Integer]);
                    self.emit(
                        ResolvedStatement::RepeatInit {
                            count,
                            count_slot,
                            index_slot,
                        },
                        count_span,
                    );
                    let start = self.emit(
                        ResolvedStatement::RepeatGuard {
                            count_slot,
                            index_slot,
                            target: 0,
                        },
                        count_span,
                    );
                    self.loops.push(LoopContext {
                        breaks: Vec::new(),
                        continues: Vec::new(),
                        scope_depth: self.scopes.len(),
                    });
                    self.enter_scope(span)?;
                    self.scopes.last_mut().unwrap().insert(
                        index,
                        Binding {
                            slot: index_slot,
                            declaration: index_span,
                            mutable: false,
                            kind: ValueType::Integer,
                            repeat_index: true,
                            parameter: false,
                        },
                    );
                    self.statement_list(statements)?;
                    self.retire_scope(span);
                    self.scopes.pop();
                    let next = self.emit(ResolvedStatement::RepeatNext { index_slot }, index_span);
                    self.emit(ResolvedStatement::Jump { target: start }, count_span);
                    let end = self.statements.len();
                    if let ResolvedStatement::RepeatGuard { target, .. } =
                        &mut self.statements[start].kind
                    {
                        *target = end;
                    }
                    let context = self.loops.pop().unwrap();
                    self.finish_loop(context, next, end);
                }
                Statement::While {
                    condition,
                    statements,
                    span,
                } => {
                    let condition = self.expression(condition)?;
                    let condition_span = condition.syntax.nodes()[condition.syntax.root()].span;
                    require_type(condition.kind, ValueType::Boolean, condition_span).map_err(|error|
                        error.with_help("while needs a boolean guard; compare values, for example while index < count { statements }"))?;
                    let start = self.emit(
                        ResolvedStatement::JumpIfFalse {
                            condition,
                            target: 0,
                        },
                        condition_span,
                    );
                    self.loops.push(LoopContext {
                        breaks: Vec::new(),
                        continues: Vec::new(),
                        scope_depth: self.scopes.len(),
                    });
                    self.scoped_statements(statements, span)?;
                    let context = self.loops.pop().unwrap();
                    self.emit(ResolvedStatement::Jump { target: start }, span);
                    let end = self.statements.len();
                    if let ResolvedStatement::JumpIfFalse { target, .. } =
                        &mut self.statements[start].kind
                    {
                        *target = end;
                    }
                    self.finish_loop(context, start, end);
                }
                Statement::Break { span } => {
                    if self.loops.is_empty() {
                        return Err(loop_control(span));
                    }
                    let depth = self.loops.last().unwrap().scope_depth;
                    let retire = self.retirement_slots(depth);
                    let index =
                        self.emit(ResolvedStatement::LoopControl { target: 0, retire }, span);
                    self.loops.last_mut().unwrap().breaks.push(index);
                }
                Statement::Continue { span } => {
                    if self.loops.is_empty() {
                        return Err(loop_control(span));
                    }
                    let depth = self.loops.last().unwrap().scope_depth;
                    let retire = self.retirement_slots(depth);
                    let index =
                        self.emit(ResolvedStatement::LoopControl { target: 0, retire }, span);
                    self.loops.last_mut().unwrap().continues.push(index);
                }
                Statement::If {
                    branches,
                    otherwise,
                    span,
                } => {
                    if branches.is_empty() {
                        return Err(Diagnostic::new(
                            DiagnosticKind::UnexpectedToken,
                            span,
                            "an if statement requires a condition and body",
                        ));
                    }
                    let mut exits = Vec::new();
                    for branch in branches {
                        let condition = self.expression(branch.condition)?;
                        let condition_span = condition.syntax.nodes()[condition.syntax.root()].span;
                        require_type(condition.kind, ValueType::Boolean, condition_span)?;
                        let guard = self.emit(
                            ResolvedStatement::JumpIfFalse {
                                condition,
                                target: 0,
                            },
                            condition_span,
                        );
                        self.scoped_statements(branch.statements, branch.span)?;
                        exits.push(self.emit(ResolvedStatement::Jump { target: 0 }, span));
                        let next = self.statements.len();
                        if let ResolvedStatement::JumpIfFalse { target, .. } =
                            &mut self.statements[guard].kind
                        {
                            *target = next;
                        }
                    }
                    self.scoped_statements(otherwise, span)?;
                    let end = self.statements.len();
                    for exit in exits {
                        if let ResolvedStatement::Jump { target } = &mut self.statements[exit].kind
                        {
                            *target = end;
                        }
                    }
                }
                Statement::Block { statements, span } => {
                    self.scoped_statements(statements, span)?;
                }
                Statement::Declaration {
                    name,
                    name_span,
                    initializer,
                    mutable,
                    ..
                } => {
                    self.binding_name(&name, name_span)?;
                    let current = self.scopes.last().expect("a file scope always exists");
                    if let Some(previous) = current.get(&name) {
                        return Err(Diagnostic::new(
                            DiagnosticKind::DuplicateBinding,
                            name_span,
                            format!("binding {name:?} is already declared in this scope"),
                        )
                        .with_related(previous.declaration, "the previous declaration is here")
                        .with_help("choose a different name or introduce a nested scope"));
                    }
                    // Resolve before insertion: a shadowing initializer sees the outer name.
                    let value = self.expression(initializer)?;
                    require_value(value.kind, span)?;
                    let slot = self.slot_types.len();
                    let kind = value.kind;
                    self.slot_types.push(kind);
                    self.scopes
                        .last_mut()
                        .expect("a file scope always exists")
                        .insert(
                            name,
                            Binding {
                                slot,
                                declaration: name_span,
                                mutable,
                                kind,
                                repeat_index: false,
                                parameter: false,
                            },
                        );
                    self.emit(ResolvedStatement::Store { slot, value }, span);
                }
                Statement::Println { argument, .. } => {
                    self.output(vec![argument], OutputTarget::Stdout, true, span)?;
                }
                Statement::Output {
                    arguments,
                    target,
                    newline,
                    ..
                } => {
                    self.output(arguments, target, newline, span)?;
                }
                Statement::Assign {
                    name,
                    name_span,
                    operator,
                    operator_span,
                    value,
                    ..
                } => {
                    if let AssignmentOperator::Compound(operator) = operator {
                        if !matches!(
                            operator,
                            BinaryOperator::Add
                                | BinaryOperator::Subtract
                                | BinaryOperator::Multiply
                                | BinaryOperator::Divide
                        ) {
                            return Err(Diagnostic::new(
                                DiagnosticKind::UnexpectedToken,
                                operator_span,
                                "compound assignment requires an arithmetic operator",
                            ));
                        }
                    }
                    check_name(&name, name_span)?;
                    let binding = lookup(&self.scopes, &name)
                        .ok_or_else(|| unknown_name(&name, name_span))?;
                    if !binding.mutable {
                        return Err(Diagnostic::new(
                            DiagnosticKind::ImmutableAssignment,
                            name_span,
                            format!("cannot assign to immutable binding {name:?}"),
                        )
                        .with_related(
                            binding.declaration,
                            "the immutable binding is declared here",
                        )
                        .with_help(if binding.repeat_index {
                            "repeat indices are read-only; copy the index into a separate var variable to change a local value"
                        } else if binding.parameter { "parameters are immutable; copy the parameter into a local var to change a local value" } else { "declare this binding with var to allow assignment" }));
                    }
                    let slot = binding.slot;
                    let expected_type = binding.kind;
                    let declaration = binding.declaration;
                    let value = self.expression(value)?;
                    match operator {
                        AssignmentOperator::Set if value.kind != expected_type => {
                            let span = value.syntax.nodes()[value.syntax.root()].span;
                            return Err(type_mismatch(span, expected_type, value.kind)
                                .with_related(
                                    declaration,
                                    "the binding's type is established here",
                                ));
                        }
                        AssignmentOperator::Compound(_)
                            if !expected_type.is_numeric() || value.kind != expected_type =>
                        {
                            let (expected, found) = if expected_type.is_numeric() {
                                (expected_type, value.kind)
                            } else {
                                (ValueType::Integer, expected_type)
                            };
                            return Err(type_mismatch(operator_span, expected, found)
                                .with_related(declaration, "the target binding is declared here"));
                        }
                        _ => {}
                    }
                    self.emit(
                        match operator {
                            AssignmentOperator::Set => ResolvedStatement::Store { slot, value },
                            AssignmentOperator::Compound(operator) => ResolvedStatement::Update {
                                slot,
                                operator,
                                value,
                                operator_span,
                            },
                        },
                        span,
                    );
                }
            }
        }
        Ok(())
    }

    fn scoped_statements(
        &mut self,
        statements: Vec<Statement>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.enter_scope(span)?;
        self.statement_list(statements)?;
        self.retire_scope(span);
        self.scopes.pop();
        Ok(())
    }

    // Every declaration has a unique resolver-assigned slot. Lexical retirement
    // can therefore be planned once and executed without tracing live objects.
    // Repeat indices are read after body exit by RepeatNext and stay live.
    fn retirement_slots(&self, from_depth: usize) -> Vec<usize> {
        let mut slots: Vec<usize> = self.scopes[from_depth..]
            .iter()
            .flat_map(|scope| scope.values())
            .filter(|binding| !binding.repeat_index && !binding.parameter)
            .map(|binding| binding.slot)
            .collect();
        slots.sort_unstable();
        slots
    }

    fn retire_scope(&mut self, span: Span) {
        let slots = self.retirement_slots(self.scopes.len() - 1);
        if !slots.is_empty() {
            self.emit(ResolvedStatement::Retire { slots }, span);
        }
    }

    fn enter_scope(&mut self, span: Span) -> Result<(), Diagnostic> {
        if self.scopes.len() > MAX_BLOCK_DEPTH {
            return Err(Diagnostic::new(
                DiagnosticKind::BlockNesting,
                Span::new(span.start, span.start.saturating_add(1)),
                format!("block nesting exceeds {MAX_BLOCK_DEPTH} levels"),
            )
            .with_help("reduce the number of nested statement blocks"));
        }
        self.scopes.push(HashMap::new());
        Ok(())
    }

    fn finish_loop(&mut self, context: LoopContext, next: usize, end: usize) {
        for (indices, destination) in [(context.breaks, end), (context.continues, next)] {
            for index in indices {
                if let ResolvedStatement::LoopControl { target, .. } =
                    &mut self.statements[index].kind
                {
                    *target = destination;
                }
            }
        }
    }

    fn output(
        &mut self,
        arguments: Vec<PrintArgument>,
        target: OutputTarget,
        newline: bool,
        span: Span,
    ) -> Result<(), Diagnostic> {
        if self.pure {
            return Err(Diagnostic::new(
                DiagnosticKind::ImpureFunction,
                span,
                "pure functions cannot perform output",
            ));
        }
        let arguments = arguments
            .into_iter()
            .map(|argument| match argument {
                PrintArgument::String(value) => Ok(ResolvedArgument::String(value)),
                PrintArgument::Expression(expression) => {
                    let expression = self.expression(expression)?;
                    require_value(
                        expression.kind,
                        expression.syntax.nodes()[expression.syntax.root()].span,
                    )?;
                    Ok(ResolvedArgument::Expression(expression))
                }
            })
            .collect::<Result<_, _>>()?;
        self.emit(
            ResolvedStatement::Output {
                arguments,
                target,
                newline,
            },
            span,
        );
        Ok(())
    }

    fn emit(&mut self, kind: ResolvedStatement, span: Span) -> usize {
        let index = self.statements.len();
        self.statements.push(ResolvedOperation { kind, span });
        index
    }
}

fn lookup<'a>(scopes: &'a [Scope], name: &str) -> Option<&'a Binding> {
    scopes.iter().rev().find_map(|scope| scope.get(name))
}

fn resolve_expression(
    syntax: Expression,
    scopes: &[Scope],
    functions: &[FunctionInfo],
    names: &HashMap<String, usize>,
    pure: bool,
) -> Result<ResolvedExpression, Diagnostic> {
    let mut slots = Vec::with_capacity(syntax.nodes().len());
    let mut calls = Vec::with_capacity(syntax.nodes().len());
    let mut types: Vec<ValueType> = Vec::with_capacity(syntax.nodes().len());
    for node in syntax.nodes() {
        let mut call = None;
        let (kind, slot) = match &node.kind {
            ExpressionKind::Vector { elements } => {
                for &element in elements {
                    require_type(
                        types[element],
                        ValueType::Float,
                        syntax.nodes()[element].span,
                    )?;
                }
                (ValueType::Vector, None)
            }
            ExpressionKind::Index {
                vector,
                index,
                operator_span,
            } => {
                require_type(types[*vector], ValueType::Vector, *operator_span)?;
                require_type(
                    types[*index],
                    ValueType::Integer,
                    syntax.nodes()[*index].span,
                )?;
                (ValueType::Float, None)
            }
            ExpressionKind::UserCall {
                name,
                arguments,
                function_span,
            } => {
                // Preserve existing user function names; kernels occupy call
                // position only when no user declaration has this name.
                if let Some(builtin) =
                    Builtin::from_name(name).filter(|_| !names.contains_key(name))
                {
                    let parameters = builtin.parameter_types();
                    if arguments.len() != parameters.len()
                        || arguments.iter().any(|a| a.label.is_some())
                    {
                        return Err(Diagnostic::new(
                            DiagnosticKind::CallArguments,
                            *function_span,
                            format!("{name} requires {} positional arguments", parameters.len()),
                        ));
                    }
                    for (position, (argument, &expected)) in
                        arguments.iter().zip(parameters).enumerate()
                    {
                        require_type(
                            types[argument.expression],
                            expected,
                            syntax.nodes()[argument.expression].span,
                        )
                        .map_err(|error| {
                            if builtin.is_tile() {
                                error.with_related(
                                    *function_span,
                                    format!(
                                        "argument {} of {name} requires {expected}",
                                        position + 1
                                    ),
                                )
                            } else {
                                error
                            }
                        })?;
                    }
                    slots.push(None);
                    calls.push(Some(ResolvedCall::Builtin(builtin)));
                    types.push(builtin.result_type());
                    continue;
                }
                let &id = names.get(name).ok_or_else(|| {
                    Diagnostic::new(
                        DiagnosticKind::UnknownFunction,
                        *function_span,
                        if lookup(scopes, name).is_some() {
                            format!("binding {name:?} is not callable")
                        } else {
                            format!("unknown function {name:?}")
                        },
                    )
                    .with_help("declare a funcra; output forms such as print are statements")
                })?;
                let function = &functions[id];
                if pure && !function.pure {
                    return Err(Diagnostic::new(
                        DiagnosticKind::ImpureFunction,
                        *function_span,
                        format!("pure expressions cannot call impure function {name:?}"),
                    ));
                }
                let mut parameters = Vec::new();
                let mut supplied = vec![false; function.parameters.len()];
                let mut named = false;
                for (position, argument) in arguments.iter().enumerate() {
                    let parameter = if let Some((label, span)) = &argument.label {
                        named = true;
                        function
                            .parameters
                            .iter()
                            .position(|p| &p.name == label)
                            .ok_or_else(|| {
                                Diagnostic::new(
                                    DiagnosticKind::CallArguments,
                                    *span,
                                    format!("unknown parameter {label:?} in {name:?}"),
                                )
                            })?
                    } else {
                        if named || position >= supplied.len() {
                            return Err(Diagnostic::new(DiagnosticKind::CallArguments,
                                syntax.nodes()[argument.expression].span,
                                "positional arguments must precede named arguments and fit the signature"));
                        }
                        position
                    };
                    if supplied[parameter] {
                        return Err(Diagnostic::new(
                            DiagnosticKind::CallArguments,
                            argument
                                .label
                                .as_ref()
                                .map_or(syntax.nodes()[argument.expression].span, |a| a.1),
                            format!(
                                "parameter {:?} was supplied twice",
                                function.parameters[parameter].name
                            ),
                        ));
                    }
                    require_type(
                        types[argument.expression],
                        function.parameters[parameter].kind,
                        syntax.nodes()[argument.expression].span,
                    )
                    .map_err(|error| {
                        if function.name.contains("::") {
                            error.with_related(
                                function.parameters[parameter].span,
                                format!(
                                    "parameter {:?} is declared here",
                                    function.parameters[parameter].name
                                ),
                            )
                        } else {
                            error
                        }
                    })?;
                    supplied[parameter] = true;
                    parameters.push(parameter);
                }
                for (parameter, supplied) in function.parameters.iter().zip(supplied) {
                    if !supplied && parameter.default.is_none() {
                        return Err(Diagnostic::new(
                            DiagnosticKind::CallArguments,
                            *function_span,
                            format!("missing required argument {:?}", parameter.name),
                        ));
                    }
                }
                call = Some(ResolvedCall::User {
                    function: id,
                    parameters,
                });
                (function.result, None)
            }

            ExpressionKind::Integer(_) => (ValueType::Integer, None),
            ExpressionKind::Float(_) => (ValueType::Float, None),
            ExpressionKind::Boolean(_) => (ValueType::Boolean, None),
            ExpressionKind::String(_) => (ValueType::String, None),
            ExpressionKind::Name(name) => {
                if names.contains_key(name) && lookup(scopes, name).is_none() {
                    return Err(Diagnostic::new(
                        DiagnosticKind::FunctionUnsupported,
                        node.span,
                        "function values are not supported; call the function with arguments",
                    ));
                }
                check_name(name, node.span)?;
                let binding = lookup(scopes, name).ok_or_else(|| unknown_name(name, node.span))?;
                (binding.kind, Some(binding.slot))
            }
            ExpressionKind::Group { expression } => (types[*expression], None),
            ExpressionKind::Negate {
                operand,
                operator_span,
            } => {
                if !types[*operand].is_numeric() {
                    return Err(type_mismatch(
                        *operator_span,
                        ValueType::Integer,
                        types[*operand],
                    ));
                }
                (types[*operand], None)
            }
            ExpressionKind::Not {
                operand,
                operator_span,
            } => {
                require_type(types[*operand], ValueType::Boolean, *operator_span)?;
                (ValueType::Boolean, None)
            }
            ExpressionKind::Logical {
                left,
                right,
                operator_span,
                ..
            } => {
                require_type(types[*left], ValueType::Boolean, *operator_span)?;
                require_type(types[*right], ValueType::Boolean, *operator_span)?;
                (ValueType::Boolean, None)
            }
            ExpressionKind::Binary {
                left,
                right,
                operator,
                operator_span,
            } => {
                match operator {
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        require_value(types[*left], *operator_span)?;
                        require_type(types[*right], types[*left], *operator_span)?;
                    }
                    _ => {
                        if !types[*left].is_numeric() {
                            return Err(type_mismatch(
                                *operator_span,
                                ValueType::Integer,
                                types[*left],
                            ));
                        }
                        require_type(types[*right], types[*left], *operator_span)?;
                    }
                }
                let kind = match operator {
                    BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide => types[*left],
                    _ => ValueType::Boolean,
                };
                (kind, None)
            }
        };
        types.push(kind);
        slots.push(slot);
        calls.push(call);
    }
    let kind = types[syntax.root()];
    Ok(ResolvedExpression {
        syntax,
        slots,
        kind,
        calls,
    })
}

pub(crate) fn resolve_standalone(
    expression: &Expression,
) -> Result<ResolvedExpression, Diagnostic> {
    resolve_expression(expression.clone(), &[], &[], &HashMap::new(), false)
}

fn require_value(kind: ValueType, span: Span) -> Result<(), Diagnostic> {
    if kind == ValueType::Unit {
        Err(Diagnostic::new(
            DiagnosticKind::InvalidReturn,
            span,
            "a unit call is allowed only as a standalone statement",
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn require_type(
    found: ValueType,
    expected: ValueType,
    span: Span,
) -> Result<(), Diagnostic> {
    if found != expected {
        Err(type_mismatch(span, expected, found))
    } else {
        Ok(())
    }
}

pub(crate) fn check_name(name: &str, span: Span) -> Result<(), Diagnostic> {
    if matches!(
        name,
        "let"
            | "bind"
            | "var"
            | "mut"
            | "println"
            | "print"
            | "write"
            | "eprint"
            | "ewrite"
            | "true"
            | "false"
            | "and"
            | "or"
            | "not"
            | "if"
            | "elif"
            | "else"
            | "while"
            | "break"
            | "continue"
            | "funcra"
            | "pure"
            | "return"
            | "needs"
            | "ensures"
    ) {
        Err(Diagnostic::new(
            DiagnosticKind::ReservedName,
            span,
            format!("{name:?} is reserved and cannot be used as a variable"),
        )
        .with_help(
            "choose a name that is not a statement word, boolean literal, or logical operator",
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn unknown_name(name: &str, span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::UnknownName,
        span,
        format!("unknown variable {name:?}"),
    )
    .with_help("declare the variable with bind or var before using it in this scope")
}

fn loop_control(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::LoopControl,
        span,
        "loop control used outside a while or repeat body",
    )
    .with_help("put break; or continue; inside the while or repeat loop it should control")
}

#[cfg(test)]
mod retirement_plan_tests {
    use super::{ResolvedProgram, ResolvedStatement};

    fn plan(source: &str) -> ResolvedProgram {
        crate::compile(source).unwrap()
    }

    #[test]
    fn nested_scopes_retire_only_their_own_binding_slots() {
        let program = plan(
            "bind root = [1.0]; { bind outer = [2.0]; { bind inner = [3.0]; } print(outer); } print(root);",
        );
        let retirements: Vec<Vec<usize>> = program
            .statements
            .iter()
            .filter_map(|operation| match &operation.kind {
                ResolvedStatement::Retire { slots } => Some(slots.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(retirements, vec![vec![2], vec![1]]);
        // Internal bookkeeping is excluded from observable statement counts.
        assert_eq!(program.statement_count(), 5);
    }

    #[test]
    fn loop_transfers_retire_exited_scopes_but_preserve_repeat_index() {
        let program = plan(
            "bind carried = [1.0]; repeat 2 as i { bind body = [2.0]; if i == 0 { bind leaf = [3.0]; continue; } if i == 1 { bind leaf2 = [4.0]; break; } print(i); } print(carried);",
        );
        let transfers: Vec<(usize, Vec<usize>)> = program
            .statements
            .iter()
            .filter_map(|operation| match &operation.kind {
                ResolvedStatement::LoopControl { target, retire } => {
                    Some((*target, retire.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(transfers.len(), 2);
        assert_eq!(transfers[0].1, vec![3, 4]);
        assert_eq!(transfers[1].1, vec![3, 5]);
        assert!(transfers[0].0 != transfers[1].0);
        // Index slot 2 and outer binding slot 0 must stay live.
        assert!(transfers
            .iter()
            .all(|(_, slots)| !slots.contains(&0) && !slots.contains(&2)));
    }
}
