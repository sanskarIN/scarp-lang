//! Resolve variable names once, before any program evaluation.

use std::collections::HashMap;

use crate::{
    ast::{Expression, ExpressionKind, PrintArgument, Program, Statement},
    diagnostics::{Diagnostic, DiagnosticKind, Span},
};

/// An immutable executable program produced by name resolution.
/// Its private storage plan cannot be changed independently of its expressions.
#[derive(Debug, Clone)]
pub struct ResolvedProgram {
    pub(crate) statements: Vec<ResolvedStatement>,
    pub(crate) slots: usize,
}

impl ResolvedProgram {
    /// Number of executable statements, including binding declarations.
    pub fn statement_count(&self) -> usize {
        self.statements.len()
    }

    /// Number of distinct storage slots required by one execution.
    pub fn binding_count(&self) -> usize {
        self.slots
    }
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedStatement {
    Store {
        slot: usize,
        value: ResolvedExpression,
    },
    Println(ResolvedArgument),
}

#[derive(Debug, Clone)]
pub(crate) enum ResolvedArgument {
    String(String),
    Integer(ResolvedExpression),
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedExpression {
    pub syntax: Expression,
    pub slots: Vec<Option<usize>>,
}

struct Binding {
    slot: usize,
    declaration: Span,
}

/// Consume a syntax tree and reject unknown, duplicate, or reserved names.
pub fn resolve(program: Program) -> Result<ResolvedProgram, Diagnostic> {
    let mut bindings = HashMap::new();
    let mut statements = Vec::with_capacity(program.statements.len());
    for statement in program.statements {
        match statement {
            Statement::Let {
                name,
                name_span,
                initializer,
                ..
            } => {
                check_name(&name, name_span)?;
                if let Some(previous) = bindings.get(&name) {
                    let previous: &Binding = previous;
                    return Err(Diagnostic::new(
                        DiagnosticKind::DuplicateBinding,
                        name_span,
                        format!("binding {name:?} is already declared in this scope"),
                    )
                    .with_related(previous.declaration, "the previous declaration is here")
                    .with_help("choose a different binding name"));
                }
                // The new name is deliberately absent during its initializer.
                let value = resolve_expression(initializer, &bindings)?;
                let slot = bindings.len();
                bindings.insert(
                    name,
                    Binding {
                        slot,
                        declaration: name_span,
                    },
                );
                statements.push(ResolvedStatement::Store { slot, value });
            }
            Statement::Println { argument, .. } => {
                let argument = match argument {
                    PrintArgument::String(value) => ResolvedArgument::String(value),
                    PrintArgument::Integer(expression) => {
                        ResolvedArgument::Integer(resolve_expression(expression, &bindings)?)
                    }
                };
                statements.push(ResolvedStatement::Println(argument));
            }
        }
    }
    Ok(ResolvedProgram {
        statements,
        slots: bindings.len(),
    })
}

fn resolve_expression(
    syntax: Expression,
    bindings: &HashMap<String, Binding>,
) -> Result<ResolvedExpression, Diagnostic> {
    let mut slots = Vec::with_capacity(syntax.nodes().len());
    for node in syntax.nodes() {
        let slot = if let ExpressionKind::Name(name) = &node.kind {
            check_name(name, node.span)?;
            Some(
                bindings
                    .get(name)
                    .ok_or_else(|| unknown_name(name, node.span))?
                    .slot,
            )
        } else {
            None
        };
        slots.push(slot);
    }
    Ok(ResolvedExpression { syntax, slots })
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
    .with_help("declare the variable with let before using it")
}
