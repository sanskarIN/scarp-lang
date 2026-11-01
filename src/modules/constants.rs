//! Bounded scalar constant compilation; no user frames or I/O are reachable.
use super::*;
use crate::{
    ast::{Expression, ExpressionKind},
    evaluator::Workspace,
    limits::Budget,
    value::Value,
};

/// Cumulative compilation allowances, independent of runtime execution limits.
/// Existing compile entry points use these defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstantLimits {
    /// All initializer nodes, including operands skipped by short-circuiting.
    pub max_nodes: usize,
    /// Selected scalar evaluator tasks across all module constants.
    pub max_steps: u64,
}
impl Default for ConstantLimits {
    fn default() -> Self {
        Self {
            max_nodes: 1_000_000,
            max_steps: 2_000_000,
        }
    }
}

/// A statically checked direct reference from one constant initializer to another.
/// Spans are global source-map coordinates. Skipped operands are included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstantDependency {
    pub source: SourceId,
    pub declaration: Span,
    pub target: SourceId,
    pub target_declaration: Span,
    pub span: Span,
}

pub(super) struct Work {
    nodes: usize,
    limits: ConstantLimits,
    budget: Budget,
    workspace: Workspace,
}
struct Declaration<'a> {
    name: &'a str,
    span: Span,
    initializer: &'a Expression,
}
fn exceeded(span: Span, what: &str, limit: impl fmt::Display) -> Diagnostic {
    Diagnostic::new(
        Kind::ModuleLimit,
        span,
        format!("constant {what} limit of {limit} exceeded"),
    )
    .with_help("reduce constant expressions or adjust ConstantLimits in the embedding host")
}
impl Work {
    pub(super) fn new(limits: ConstantLimits) -> Self {
        Self {
            nodes: limits.max_nodes,
            limits,
            budget: Budget::new(limits.max_steps),
            workspace: Workspace::default(),
        }
    }

    /// Bound/check every syntax node before cloning, typing or selected evaluation.
    /// Local dependency traversal is iterative and cannot consume the Rust stack.
    pub(super) fn validate(&mut self, module: &ParsedModule, root: bool) -> Result<(), Diagnostic> {
        let declarations: Vec<_> = module
            .program
            .statements
            .iter()
            .filter_map(|wrapped| {
                let (statement, public) = match wrapped {
                    Statement::Export { declaration, .. } => (declaration.as_ref(), true),
                    other => (other, false),
                };
                match statement {
                    Statement::Declaration {
                        name,
                        name_span,
                        mutable: false,
                        initializer,
                        ..
                    } if !root || public => Some(Declaration {
                        name,
                        span: *name_span,
                        initializer,
                    }),
                    _ => None,
                }
            })
            .collect();
        let mut names = HashMap::new();
        for (id, declaration) in declarations.iter().enumerate() {
            if let Some(previous) = names.insert(declaration.name, id) {
                return Err(duplicate(
                    declaration.name,
                    declaration.span,
                    declarations[previous].span,
                ));
            }
        }
        let mut edges = Vec::with_capacity(declarations.len());
        for declaration in &declarations {
            let mut uses = Vec::new();
            for node in declaration.initializer.nodes() {
                if self.nodes == 0 {
                    return Err(
                        exceeded(node.span, "initializer node", self.limits.max_nodes)
                            .with_related(declaration.span, "constant initialized here"),
                    );
                }
                self.nodes -= 1;
                match &node.kind {
                    ExpressionKind::UserCall { function_span, .. } => return Err(
                        Diagnostic::new(Kind::ModuleSyntax, *function_span, "calls are not allowed in module constant expressions")
                            .with_related(declaration.span, "constant initialized here")
                            .with_help("use scalar operators and earlier constants; call functions at runtime")),
                    ExpressionKind::Vector { .. } | ExpressionKind::Index { .. } => return Err(
                        Diagnostic::new(Kind::ModuleSyntax, node.span, "module constant expressions require scalar values")
                            .with_related(declaration.span, "constant initialized here")
                            .with_help("construct or index vectors and tiles inside a function or entry script")),
                    ExpressionKind::Name(name) => {
                        if let Some(&target) = names.get(name.as_str()) { uses.push((target, node.span)); }
                    }
                    ExpressionKind::Integer(_)
                    | ExpressionKind::Float(_)
                    | ExpressionKind::Boolean(_)
                    | ExpressionKind::String(_)
                    | ExpressionKind::Negate { .. }
                    | ExpressionKind::Not { .. }
                    | ExpressionKind::Logical { .. }
                    | ExpressionKind::Binary { .. }
                    | ExpressionKind::Group { .. } => {}
                }
            }
            edges.push(uses);
        }
        let mut colors = vec![0u8; declarations.len()];
        for start in 0..declarations.len() {
            if colors[start] != 0 {
                continue;
            }
            colors[start] = 1;
            let mut stack = vec![(start, 0)];
            while let Some(&(id, next)) = stack.last() {
                if next == edges[id].len() {
                    colors[id] = 2;
                    stack.pop();
                    continue;
                }
                stack.last_mut().unwrap().1 += 1;
                let (target, span) = edges[id][next];
                if colors[target] == 1 {
                    let mut error = Diagnostic::new(Kind::ModuleCycle, span,
                        format!("constant dependency cycle reaches {:?}", declarations[target].name))
                        .with_related(declarations[target].span, "constant in this cycle declared here")
                        .with_help("break the cycle; constants may reference only earlier constants in the same file");
                    let cycle = stack.iter().position(|&(node, _)| node == target).unwrap();
                    for &(node, next) in stack[cycle..stack.len() - 1].iter().take(15) {
                        error = error.with_related(
                            edges[node][next - 1].1,
                            "this constant reference continues the cycle",
                        );
                    }
                    return Err(error);
                }
                if colors[target] == 0 {
                    colors[target] = 1;
                    stack.push((target, 0));
                }
            }
        }
        for (id, uses) in edges.iter().enumerate() {
            for &(target, span) in uses {
                if target > id {
                    return Err(Diagnostic::new(
                        Kind::ModuleSymbol,
                        span,
                        format!(
                            "constant {:?} is declared after this initializer",
                            declarations[target].name
                        ),
                    )
                    .with_related(
                        declarations[target].span,
                        "the later constant is declared here",
                    )
                    .with_help("move the referenced constant before this declaration"));
                }
            }
        }
        Ok(())
    }

    pub(super) fn evaluate(
        &mut self,
        expression: &Expression,
        mut reference: impl FnMut(&str, Span) -> Result<ExpressionKind, Diagnostic>,
    ) -> Result<Value, Diagnostic> {
        let mut syntax = expression.clone();
        for node in &mut syntax.nodes {
            if let ExpressionKind::Name(name) = &node.kind {
                node.kind = reference(name, node.span)?;
            }
        }
        let resolved = crate::resolver::resolve_closed_expression(syntax)?;
        self.workspace
            .evaluate(&resolved, &[], &mut self.budget)
            .map_err(|error| {
                if error.kind == Kind::ExecutionLimit {
                    exceeded(error.span, "evaluation task", self.limits.max_steps)
                } else {
                    error
                }
            })
    }
}
