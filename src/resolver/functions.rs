use super::*;

pub(super) fn resolve_function(
    function: FunctionDeclaration,
    signatures: &[FunctionInfo],
    names: &HashMap<String, usize>,
) -> Result<ResolvedFunction, Diagnostic> {
    let mut resolver = Resolver::new(signatures, names);
    resolver.result = Some(function.result);
    resolver.pure = true;
    let mut defaults = Vec::new();
    let mut optional = false;
    for parameter in function.parameters {
        resolver.binding_name(&parameter.name, parameter.span)?;
        if parameter.name == "result" || parameter.kind == ValueType::Unit {
            return Err(Diagnostic::new(
                DiagnosticKind::FunctionDefinition,
                parameter.span,
                "parameters cannot be named result or have type unit",
            ));
        }
        if let Some(previous) = resolver.scopes[0].get(&parameter.name) {
            return Err(Diagnostic::new(
                DiagnosticKind::FunctionDefinition,
                parameter.span,
                format!("duplicate parameter {:?}", parameter.name),
            )
            .with_related(previous.declaration, "first parameter here"));
        }
        let default = if let Some(expression) = parameter.default {
            optional = true;
            let expression = resolver.expression(expression)?;
            let initializer_span = expression.syntax.nodes()[expression.syntax.root()].span;
            require_type(expression.kind, parameter.kind, initializer_span).map_err(|error| {
                error.with_related(parameter.span, "the parameter's type is declared here")
            })?;
            Some(expression)
        } else {
            if optional {
                return Err(Diagnostic::new(
                    DiagnosticKind::FunctionDefinition,
                    parameter.span,
                    "required parameters cannot follow a default parameter",
                ));
            }
            None
        };
        let slot = resolver.slot_types.len();
        resolver.slot_types.push(parameter.kind);
        resolver.scopes[0].insert(
            parameter.name,
            Binding {
                slot,
                kind: parameter.kind,
                declaration: parameter.span,
                mutable: false,
                repeat_index: false,
                parameter: true,
            },
        );
        defaults.push(default);
    }
    let result_slot = (function.result != ValueType::Unit).then_some(resolver.slot_types.len());
    if result_slot.is_some() {
        resolver.slot_types.push(function.result);
    }
    let mut contracts = Vec::new();
    for contract in function.contracts {
        if contract.postcondition {
            if let Some(slot) = result_slot {
                resolver.scopes[0].insert(
                    "result".into(),
                    Binding {
                        slot,
                        kind: function.result,
                        mutable: false,
                        repeat_index: false,
                        parameter: false,
                        declaration: contract.span,
                    },
                );
            }
        }
        let expression = resolver.expression(contract.expression)?;
        require_type(expression.kind, ValueType::Boolean, contract.span)?;
        resolver.scopes[0].remove("result");
        contracts.push(ResolvedContract {
            expression,
            postcondition: contract.postcondition,
            span: contract.span,
        });
    }
    if function.result != ValueType::Unit && !definitely_returns(&function.statements, 0) {
        return Err(Diagnostic::new(
            DiagnosticKind::MissingReturn,
            function.span,
            format!(
                "function {:?} must return {} on every path",
                function.name, function.result
            ),
        )
        .with_help("provide a return after loops and cover every conditional branch"));
    }
    resolver.pure = function.pure;
    resolver.scoped_statements(function.statements, function.span)?;
    Ok(ResolvedFunction {
        statements: resolver.statements,
        slots: resolver.slot_types.len(),
        defaults,
        contracts,
        result_slot,
    })
}

// Loops deliberately do not establish return, even for a literal true guard.
fn definitely_returns(statements: &[Statement], depth: usize) -> bool {
    // Also bound manually constructed public ASTs before descending into them.
    if depth >= MAX_BLOCK_DEPTH {
        return false;
    }
    for statement in statements {
        match statement {
            Statement::Return { .. } => return true,
            Statement::Block { statements, .. } if definitely_returns(statements, depth + 1) => {
                return true
            }
            Statement::If {
                branches,
                otherwise,
                ..
            } if !branches.is_empty()
                && !otherwise.is_empty()
                && branches
                    .iter()
                    .all(|b| definitely_returns(&b.statements, depth + 1))
                && definitely_returns(otherwise, depth + 1) =>
            {
                return true
            }
            // A break/continue makes subsequent statements unreachable in this list.
            Statement::Break { .. } | Statement::Continue { .. } => return false,
            _ => {}
        }
    }
    false
}

pub(super) fn build_call_graph(program: &mut ResolvedProgram) {
    let mut graph = vec![Vec::new(); program.functions.len()];
    for (id, function) in program.functions.iter().enumerate() {
        let mut expressions = Vec::new();
        expressions.extend(function.defaults.iter().flatten());
        expressions.extend(function.contracts.iter().map(|c| &c.expression));
        for operation in &function.statements {
            match &operation.kind {
                ResolvedStatement::Store { value, .. }
                | ResolvedStatement::Update { value, .. }
                | ResolvedStatement::Return { value: Some(value) } => expressions.push(value),
                ResolvedStatement::Call { expression } => expressions.push(expression),
                ResolvedStatement::JumpIfFalse { condition, .. } => expressions.push(condition),
                ResolvedStatement::RepeatInit { count, .. } => expressions.push(count),
                ResolvedStatement::Output { arguments, .. } => {
                    expressions.extend(arguments.iter().filter_map(|a| match a {
                        ResolvedArgument::Expression(e) => Some(e),
                        _ => None,
                    }));
                }
                _ => {}
            }
        }
        let mut callees = Vec::new();
        for expression in expressions {
            for call in expression.calls.iter().flatten() {
                graph[id].push(call.function);
                callees.push(program.metadata[call.function].name.clone());
            }
        }
        graph[id].sort_unstable();
        graph[id].dedup();
        callees.sort();
        callees.dedup();
        program.metadata[id].callees = callees;
    }
    // Iterative strongly-connected components (Kosaraju), O(V + E).
    let mut visited = vec![false; graph.len()];
    let mut order = Vec::new();
    for root in 0..graph.len() {
        let mut stack = vec![(root, false)];
        while let Some((node, exiting)) = stack.pop() {
            if exiting {
                order.push(node);
                continue;
            }
            if visited[node] {
                continue;
            }
            visited[node] = true;
            stack.push((node, true));
            stack.extend(graph[node].iter().rev().map(|&next| (next, false)));
        }
    }
    let mut reverse = vec![Vec::new(); graph.len()];
    for (node, edges) in graph.iter().enumerate() {
        for &edge in edges {
            reverse[edge].push(node);
        }
    }
    visited.fill(false);
    for root in order.into_iter().rev() {
        if visited[root] {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if visited[node] {
                continue;
            }
            visited[node] = true;
            component.push(node);
            stack.extend(&reverse[node]);
        }
        let recursive = component.len() > 1 || graph[root].contains(&root);
        for node in component {
            program.metadata[node].recursive = recursive;
        }
    }
}
