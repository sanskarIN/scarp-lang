//! Resolve lexical names into an immutable, flat execution plan.

use std::collections::HashMap;

use crate::{
    ast::{
        AssignmentOperator, BinaryOperator, Expression, ExpressionKind, PrintArgument, Program,
        Statement,
    },
    diagnostics::{Diagnostic, DiagnosticKind, Span},
    parser::MAX_BLOCK_DEPTH,
    value::{type_mismatch, ValueType},
};

/// An immutable executable program produced by name resolution.
/// Its private storage plan cannot be changed independently of its expressions.
#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub(crate) statements: Vec<ResolvedStatement>,
    slot_types: Vec<ValueType>,
}

impl ResolvedProgram {
    /// Number of executable statements, including declarations, excluding block containers.
    pub fn statement_count(&self) -> usize {
        self.statements.len()
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
pub(crate) enum ResolvedStatement {
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
    Println(ResolvedArgument),
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
    };
    resolver.statement_list(program.statements)?;
    Ok(ResolvedProgram {
        statements: resolver.statements,
        slot_types: resolver.slot_types,
    })
}

struct Resolver {
    scopes: Vec<Scope>,
    statements: Vec<ResolvedStatement>,
    slot_types: Vec<ValueType>,
}

impl Resolver {
    fn statement_list(&mut self, statements: Vec<Statement>) -> Result<(), Diagnostic> {
        for statement in statements {
            match statement {
                Statement::Block { statements, span } => {
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
                }
                Statement::Let {
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
                    self.statements
                        .push(ResolvedStatement::Store { slot, value });
                }
                Statement::Println { argument, .. } => {
                    let argument = match argument {
                        PrintArgument::String(value) => ResolvedArgument::String(value),
                        PrintArgument::Expression(expression) => ResolvedArgument::Expression(
                            resolve_expression(expression, &self.scopes)?,
                        ),
                    };
                    self.statements.push(ResolvedStatement::Println(argument));
                }
                Statement::Assign {
                    name,
                    name_span,
                    operator,
                    operator_span,
                    value,
                    ..
                } => {
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
                        .with_help("declare this binding with let mut to allow assignment"));
                    }
                    let slot = binding.slot;
                    let expected_type = binding.kind;
                    let declaration = binding.declaration;
                    let value = resolve_expression(value, &self.scopes)?;
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
                            if expected_type != ValueType::Integer
                                || value.kind != ValueType::Integer =>
                        {
                            let found = if expected_type != ValueType::Integer {
                                expected_type
                            } else {
                                value.kind
                            };
                            return Err(type_mismatch(operator_span, ValueType::Integer, found)
                                .with_related(declaration, "the target binding is declared here"));
                        }
                        _ => {}
                    }
                    self.statements.push(match operator {
                        AssignmentOperator::Set => ResolvedStatement::Store { slot, value },
                        AssignmentOperator::Compound(operator) => ResolvedStatement::Update {
                            slot,
                            operator,
                            value,
                            operator_span,
                        },
                    });
                }
            }
        }
        Ok(())
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
            ExpressionKind::Integer(_) => (ValueType::Integer, None),
            ExpressionKind::String(_) => (ValueType::String, None),
            ExpressionKind::Name(name) => {
                check_name(name, node.span)?;
                let binding = lookup(scopes, name).ok_or_else(|| unknown_name(name, node.span))?;
                (binding.kind, Some(binding.slot))
            }
            ExpressionKind::Group { expression } => (types[*expression], None),
            ExpressionKind::Negate {
                operand,
                operator_span,
            } => {
                if types[*operand] != ValueType::Integer {
                    return Err(type_mismatch(
                        *operator_span,
                        ValueType::Integer,
                        types[*operand],
                    ));
                }
                (ValueType::Integer, None)
            }
            ExpressionKind::Binary {
                left,
                right,
                operator_span,
                ..
            } => {
                for operand in [*left, *right] {
                    if types[operand] != ValueType::Integer {
                        return Err(type_mismatch(
                            *operator_span,
                            ValueType::Integer,
                            types[operand],
                        ));
                    }
                }
                (ValueType::Integer, None)
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

fn check_name(name: &str, span: Span) -> Result<(), Diagnostic> {
    if matches!(name, "let" | "mut" | "println") {
        Err(Diagnostic::new(
            DiagnosticKind::ReservedName,
            span,
            format!("{name:?} is reserved and cannot be used as a variable"),
        )
        .with_help("choose a name other than let, mut, or println"))
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
    .with_help("declare the variable with let before using it in this scope")
}
