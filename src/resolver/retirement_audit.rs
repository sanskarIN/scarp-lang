//! Compile-time audit of the private storage-retirement schedule.
//!
//! The analysis computes which slots are definitely initialized at every
//! reachable operation. Successors join by intersection: a read is permitted
//! only if the binding survived *every* incoming control-flow path.
//!
//! This is deliberately a test/debug audit rather than runtime tracing, and
//! never influences the program's memory semantics or execution budget.

use super::{
    ResolvedArgument, ResolvedExpression, ResolvedOperation, ResolvedProgram, ResolvedStatement,
};
use std::collections::VecDeque;

fn read_slots(expression: &ResolvedExpression, initialized: &[bool]) -> Result<(), String> {
    for slot in expression.slots.iter().flatten() {
        if !initialized.get(*slot).copied().unwrap_or(false) {
            return Err(format!("read from uninitialized or retired slot {slot}"));
        }
    }
    Ok(())
}

/// Verify abstract storage safety of all reachable operations in a frame.
/// Frame arguments/defaults are checked before the body executes, so all
/// declared parameters are initialized when the first body operation starts.
fn audit_frame(
    operations: &[ResolvedOperation],
    slot_count: usize,
    parameter_count: usize,
) -> Result<(), String> {
    if parameter_count > slot_count {
        return Err("parameter slots exceed frame slot count".into());
    }
    let mut entry = vec![false; slot_count];
    for alive in entry.iter_mut().take(parameter_count) {
        *alive = true;
    }
    let mut at: Vec<Option<Vec<bool>>> = vec![None; operations.len() + 1];
    at[0] = Some(entry);
    let mut pending = VecDeque::from([0_usize]);

    while let Some(pc) = pending.pop_front() {
        if pc == operations.len() {
            continue;
        }
        let state = at[pc].as_ref().unwrap().clone();
        let op = &operations[pc].kind;
        let mut state_out = state.clone();
        let mut successors: Vec<usize> = Vec::with_capacity(2);

        match op {
            ResolvedStatement::Return { value } => {
                if let Some(value) = value {
                    read_slots(value, &state)?;
                }
            }
            ResolvedStatement::Call { expression } => {
                read_slots(expression, &state)?;
                successors.push(pc + 1);
            }
            ResolvedStatement::RepeatInit {
                count,
                count_slot,
                index_slot,
            } => {
                read_slots(count, &state)?;
                for slot in [*count_slot, *index_slot] {
                    let target = state_out
                        .get_mut(slot)
                        .ok_or_else(|| format!("repeat initializes out-of-range slot {slot}"))?;
                    *target = true;
                }
                successors.push(pc + 1);
            }
            ResolvedStatement::RepeatGuard {
                count_slot,
                index_slot,
                target,
            } => {
                for slot in [*count_slot, *index_slot] {
                    if !state.get(slot).copied().unwrap_or(false) {
                        return Err(format!("repeat reads uninitialized slot {slot}"));
                    }
                }
                successors.extend([*target, pc + 1]);
            }
            ResolvedStatement::RepeatNext { index_slot } => {
                if !state.get(*index_slot).copied().unwrap_or(false) {
                    return Err(format!("repeat advances retired index {index_slot}"));
                }
                successors.push(pc + 1);
            }
            ResolvedStatement::LoopControl { target, retire } => {
                for slot in retire {
                    *state_out
                        .get_mut(*slot)
                        .ok_or_else(|| format!("control retires out-of-range slot {slot}"))? =
                        false;
                }
                successors.push(*target);
            }
            ResolvedStatement::Retire { slots } => {
                for slot in slots {
                    *state_out
                        .get_mut(*slot)
                        .ok_or_else(|| format!("retirement targets out-of-range slot {slot}"))? =
                        false;
                }
                successors.push(pc + 1);
            }
            ResolvedStatement::Jump { target } => successors.push(*target),
            ResolvedStatement::JumpIfFalse { condition, target } => {
                read_slots(condition, &state)?;
                successors.extend([*target, pc + 1]);
            }
            ResolvedStatement::Store { slot, value } => {
                read_slots(value, &state)?;
                *state_out
                    .get_mut(*slot)
                    .ok_or_else(|| format!("store targets out-of-range slot {slot}"))? = true;
                successors.push(pc + 1);
            }
            ResolvedStatement::Update { slot, value, .. } => {
                if !state.get(*slot).copied().unwrap_or(false) {
                    return Err(format!("update reads uninitialized or retired slot {slot}"));
                }
                read_slots(value, &state)?;
                successors.push(pc + 1);
            }
            ResolvedStatement::Output { arguments, .. } => {
                for argument in arguments {
                    if let ResolvedArgument::Expression(expression) = argument {
                        read_slots(expression, &state)?;
                    }
                }
                successors.push(pc + 1);
            }
        }

        for next in successors {
            let previous = at
                .get_mut(next)
                .ok_or_else(|| format!("control flow jumps outside operation stream to {next}"))?;
            let changed = match previous {
                Some(existing) => {
                    let mut changed = false;
                    for (bit, next_bit) in existing.iter_mut().zip(&state_out) {
                        if *bit && !*next_bit {
                            *bit = false;
                            changed = true;
                        }
                    }
                    changed
                }
                None => {
                    *previous = Some(state_out.clone());
                    true
                }
            };
            if changed {
                pending.push_back(next);
            }
        }
    }
    Ok(())
}

/// Test-only checker for the resolver's entire compiled storage plan.
/// Rejects malformed control-flow and stale reads before running any source.
pub(super) fn audit(program: &ResolvedProgram) -> Result<(), String> {
    audit_frame(&program.statements, program.binding_count(), 0)
        .map_err(|error| format!("main: {error}"))?;
    for (index, function) in program.functions.iter().enumerate() {
        let parameter_count = program.metadata[index].parameters.len();
        let function_name = &program.metadata[index].name;
        let mut available = vec![false; function.slots];
        // Defaults run in order. A default can only see earlier parameters;
        // required or supplied parameters are initialized before proceeding.
        for (slot, default) in function.defaults.iter().enumerate() {
            if let Some(expression) = default {
                read_slots(expression, &available)
                    .map_err(|error| format!("function {function_name} default {slot}: {error}"))?;
            }
            available[slot] = true;
        }
        // Preconditions can inspect all parameters, and postconditions may
        // inspect the returned result in its dedicated slot.
        for contract in &function.contracts {
            let mut contract_state = available.clone();
            if contract.postcondition {
                if let Some(slot) = function.result_slot {
                    contract_state[slot] = true;
                }
            }
            read_slots(&contract.expression, &contract_state)
                .map_err(|error| format!("function {function_name} contract: {error}"))?;
        }
        audit_frame(&function.statements, function.slots, parameter_count)
            .map_err(|error| format!("function {function_name}: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_valid_branch_loops_and_early_returns() {
        let cases = [
            "var a = [1.0]; if true { bind b = [2.0]; print(b); } else { bind c = [3.0]; print(c); } print(a);",
            "var i = 0; while i < 3 { var local = [2.0]; if i == 1 { i += 1; continue; } if i == 2 { break; } i += len(local); }",
            "repeat 3 as i { bind outer = [1.0]; repeat 4 as j { if j == 1 { continue; } if j == 2 { break; } print(outer); } print(i); }",
            "pure funcra f(v: vector) -> vector ensures len(result) == len(v); { bind x = scale(v, 2.0); if len(v) == 0 { return v; } return x; } print(f([1.0]));",
            "funcra recur(n: integer) -> integer { if n <= 0 { return 0; } return recur(n - 1) + 1; } print(recur(3));",
        ];
        for source in cases {
            let program = crate::compile(source).unwrap();
            assert!(audit(&program).is_ok(), "{source}: {:?}", audit(&program));
        }
    }

    #[test]
    fn audits_default_parameters_and_result_contract_dependencies() {
        let program = crate::compile(
            "funcra f(x: integer, y: integer = x) -> integer ensures result >= x; { return y; } print(f(1));",
        )
        .unwrap();
        audit(&program).unwrap();
        // The caller cannot construct a default that reads the future result.
        let mut invalid = program.clone();
        let result_reference = invalid.functions[0].contracts[0].expression.clone();
        invalid.functions[0].defaults[1] = Some(result_reference);
        assert!(audit(&invalid)
            .unwrap_err()
            .contains("default 1: read from uninitialized"));
        // A precondition cannot observe the result before body evaluation.
        let mut invalid = program;
        invalid.functions[0].contracts[0].postcondition = false;
        assert!(audit(&invalid)
            .unwrap_err()
            .contains("contract: read from uninitialized"));
    }

    #[test]
    fn detects_a_read_of_a_deliberately_retired_live_slot() {
        let mut program = crate::compile("bind a = [1.0]; print(a);").unwrap();
        program.statements.insert(
            1,
            ResolvedOperation {
                kind: ResolvedStatement::Retire { slots: vec![0] },
                span: crate::diagnostics::Span::default(),
            },
        );
        assert!(audit(&program).unwrap_err().contains("retired slot 0"));
    }

    #[test]
    fn detects_bad_forward_transfer_and_out_of_range_retirement() {
        let mut program = crate::compile("print(1);").unwrap();
        program.statements[0].kind = ResolvedStatement::Jump { target: 99 };
        assert!(audit(&program)
            .unwrap_err()
            .contains("outside operation stream"));
        program.statements[0].kind = ResolvedStatement::Retire { slots: vec![99] };
        assert!(audit(&program).unwrap_err().contains("out-of-range"));
    }

    #[test]
    fn detects_branch_that_joins_with_a_retired_binding() {
        let mut program = crate::compile("bind a = 1; if true { print(a); } print(a);").unwrap();
        let position = program
            .statements
            .iter()
            .position(|op| matches!(op.kind, ResolvedStatement::Output { .. }))
            .unwrap();
        program.statements.insert(
            position,
            ResolvedOperation {
                kind: ResolvedStatement::Retire { slots: vec![0] },
                span: crate::diagnostics::Span::default(),
            },
        );
        assert!(audit(&program).is_err());
    }
}
