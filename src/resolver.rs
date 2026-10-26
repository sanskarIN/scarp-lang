//! Resolve lexical names into an immutable, flat execution plan.

use std::collections::HashMap;

use crate::{
    ast::{
        AssignmentOperator, BinaryOperator, Expression, ExpressionKind, OutputTarget,
        PrintArgument, Program, Statement,
    },
    diagnostics::{Diagnostic, DiagnosticKind, Span},
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
}

impl ResolvedProgram {
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
    LoopControl {
        target: usize,
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
}

struct Binding {
    slot: usize,
    declaration: Span,
    mutable: bool,
    kind: ValueType,
}
type Scope = HashMap<String, Binding>;

/// Resolve a syntax tree. Inner declarations can shadow outer names, but never
/// reuse a slot, and never escape their lexical block.
pub fn resolve(program: Program) -> Result<ResolvedProgram, Diagnostic> {
    let mut resolver = Resolver {
        scopes: vec![HashMap::new()],
        statements: Vec::new(),
        slot_types: Vec::new(),
        loops: Vec::new(),
    };
    resolver.statement_list(program.statements)?;
    let statement_count = resolver
        .statements
        .iter()
        .filter(|statement| !matches!(statement.kind, ResolvedStatement::Jump { .. }))
        .count();
    Ok(ResolvedProgram {
        statements: resolver.statements,
        slot_types: resolver.slot_types,
        statement_count,
    })
}

struct Resolver {
    scopes: Vec<Scope>,
    statements: Vec<ResolvedOperation>,
    slot_types: Vec<ValueType>,
    loops: Vec<LoopContext>,
}

struct LoopContext {
    start: usize,
    breaks: Vec<usize>,
}

impl Resolver {
    fn statement_list(&mut self, statements: Vec<Statement>) -> Result<(), Diagnostic> {
        for statement in statements {
            let span = statement.span();
            match statement {
                Statement::While {
                    condition,
                    statements,
                    span,
                } => {
                    let condition = resolve_expression(condition, &self.scopes)?;
                    let condition_span = condition.syntax.nodes()[condition.syntax.root()].span;
                    require_type(condition.kind, ValueType::Boolean, condition_span)?;
                    let start = self.emit(
                        ResolvedStatement::JumpIfFalse {
                            condition,
                            target: 0,
                        },
                        condition_span,
                    );
                    self.loops.push(LoopContext {
                        start,
                        breaks: Vec::new(),
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
                    for index in context.breaks {
                        if let ResolvedStatement::LoopControl { target } =
                            &mut self.statements[index].kind
                        {
                            *target = end;
                        }
                    }
                }
                Statement::Break { span } => {
                    if self.loops.is_empty() {
                        return Err(loop_control(span));
                    }
                    let index = self.emit(ResolvedStatement::LoopControl { target: 0 }, span);
                    self.loops.last_mut().unwrap().breaks.push(index);
                }
                Statement::Continue { span } => {
                    let start = self.loops.last().ok_or_else(|| loop_control(span))?.start;
                    self.emit(ResolvedStatement::LoopControl { target: start }, span);
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
                        let condition = resolve_expression(branch.condition, &self.scopes)?;
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
                    check_name(&name, name_span)?;
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
                    let value = resolve_expression(initializer, &self.scopes)?;
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
                        if operator.is_comparison() {
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
                        .with_help("declare this binding with var to allow assignment"));
                    }
                    let slot = binding.slot;
                    let expected_type = binding.kind;
                    let declaration = binding.declaration;
                    let value = resolve_expression(value, &self.scopes)?;
                    match operator {
                        AssignmentOperator::Compound(operator)
                            if operator.is_integer_only()
                                && expected_type != ValueType::Integer =>
                        {
                            return Err(type_mismatch(
                                operator_span,
                                ValueType::Integer,
                                expected_type,
                            )
                            .with_related(declaration, "the target binding is declared here"));
                        }
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
        if self.scopes.len() > MAX_BLOCK_DEPTH {
            return Err(Diagnostic::new(
                DiagnosticKind::BlockNesting,
                Span::new(span.start, span.start.saturating_add(1)),
                format!("block nesting exceeds {MAX_BLOCK_DEPTH} levels"),
            )
            .with_help("reduce the number of nested statement blocks"));
        }
        self.scopes.push(HashMap::new());
        self.statement_list(statements)?;
        self.scopes.pop();
        Ok(())
    }

    fn output(
        &mut self,
        arguments: Vec<PrintArgument>,
        target: OutputTarget,
        newline: bool,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let arguments = arguments
            .into_iter()
            .map(|argument| match argument {
                PrintArgument::String(value) => Ok(ResolvedArgument::String(value)),
                PrintArgument::Expression(expression) => {
                    resolve_expression(expression, &self.scopes).map(ResolvedArgument::Expression)
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
) -> Result<ResolvedExpression, Diagnostic> {
    let mut slots = Vec::with_capacity(syntax.nodes().len());
    let mut types: Vec<ValueType> = Vec::with_capacity(syntax.nodes().len());
    for node in syntax.nodes() {
        let (kind, slot) = match &node.kind {
            ExpressionKind::Vector { elements } => {
                for element in elements {
                    require_type(
                        types[*element],
                        ValueType::Float,
                        syntax.nodes()[*element].span,
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
                require_type(types[*index], ValueType::Integer, *operator_span)?;
                (ValueType::Float, None)
            }
            ExpressionKind::Call {
                function,
                arguments,
                function_span,
            } => {
                let parameters = function.parameter_types();
                if arguments.len() != parameters.len() {
                    return Err(Diagnostic::new(
                        DiagnosticKind::ArgumentCount,
                        *function_span,
                        format!(
                            "expected {} arguments, found {}",
                            parameters.len(),
                            arguments.len()
                        ),
                    ));
                }
                for (argument, expected) in arguments.iter().zip(parameters) {
                    require_type(types[*argument], *expected, syntax.nodes()[*argument].span)?;
                }
                (function.result_type(), None)
            }
            ExpressionKind::Integer(_) => (ValueType::Integer, None),
            ExpressionKind::Float(_) => (ValueType::Float, None),
            ExpressionKind::Boolean(_) => (ValueType::Boolean, None),
            ExpressionKind::String(_) => (ValueType::String, None),
            ExpressionKind::Name(name) => {
                check_name(name, node.span)?;
                let binding = lookup(scopes, name).ok_or_else(|| unknown_name(name, node.span))?;
                (binding.kind, Some(binding.slot))
            }
            ExpressionKind::Group { expression } => (types[*expression], None),
            ExpressionKind::BitNot {
                operand,
                operator_span,
            } => {
                require_type(types[*operand], ValueType::Integer, *operator_span)?;
                (ValueType::Integer, None)
            }
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
                        require_type(types[*right], types[*left], *operator_span)?;
                    }
                    operator if operator.is_integer_only() => {
                        require_type(types[*left], ValueType::Integer, *operator_span)?;
                        require_type(types[*right], ValueType::Integer, *operator_span)?;
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
                let kind = if operator.is_comparison() {
                    ValueType::Boolean
                } else {
                    types[*left]
                };
                (kind, None)
            }
        };
        types.push(kind);
        slots.push(slot);
    }
    let kind = types[syntax.root()];
    Ok(ResolvedExpression {
        syntax,
        slots,
        kind,
    })
}

pub(crate) fn resolve_standalone(
    expression: &Expression,
) -> Result<ResolvedExpression, Diagnostic> {
    resolve_expression(expression.clone(), &[])
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

fn check_name(name: &str, span: Span) -> Result<(), Diagnostic> {
    if matches!(
        name,
        "bind"
            | "var"
            | "let"
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
    .with_help("declare the name with bind or var before using it in this scope")
}

fn loop_control(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::LoopControl,
        span,
        "loop control used outside a while body",
    )
    .with_help("put break or continue inside the loop it should control")
}
