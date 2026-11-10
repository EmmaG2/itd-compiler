//! Small, proof-oriented simplifications for validated quadruples.
//!
//! This deliberately stays out of SSA territory. Propagation is confined to a
//! basic block, integer arithmetic is folded only when it cannot trap, and
//! dead-code removal retains every operation with an observable effect or trap.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    diagnostic::Diagnostic,
    quadruple::{self, LabelId, Operand, Operation, Quadruple, QuadrupleProgram, Temporary},
};

/// Optimize a validated program without removing effects or possible traps.
pub fn optimize(program: &QuadrupleProgram) -> Result<QuadrupleProgram, Diagnostic> {
    quadruple::validate(program)?;
    let mut optimized = program.clone();
    for function in &mut optimized.functions {
        let rewritten = rewrite_function(&function.code);
        let reachable = retain_reachable(rewritten)?;
        let live = remove_dead_pure_temporaries(reachable)?;
        function.code = remove_fallthrough_jumps(retain_reachable(live)?);
    }
    quadruple::validate(&optimized)?;
    Ok(optimized)
}

/// Values are only propagated within a straight-line block. A label has
/// multiple possible predecessors, so clearing here makes joins conservative
/// without having to prove an SSA-style incoming-value equivalence.
fn rewrite_function(code: &[Quadruple]) -> Vec<Quadruple> {
    let mut values = BTreeMap::new();
    // A local fact means that this block has stored this exact value into the
    // slot. It is intentionally not a global value-numbering table: calls and
    // every control-flow boundary discard it, so mutable captured locals keep
    // their current snapshot rather than an earlier store.
    let mut locals = BTreeMap::new();
    let mut rewritten = Vec::with_capacity(code.len());
    for quad in code {
        let operation = rewrite(&quad.operation, &mut values, &mut locals);
        if !matches!(
            operation,
            Operation::JumpIfFalse {
                condition: Operand::Bool(true),
                ..
            }
        ) {
            rewritten.push(Quadruple {
                operation,
                span: quad.span,
            });
        }
    }
    rewritten
}

fn rewrite(
    operation: &Operation,
    values: &mut BTreeMap<usize, Operand>,
    locals: &mut BTreeMap<usize, Operand>,
) -> Operation {
    let operand = |value: Operand, values: &BTreeMap<usize, Operand>| {
        let mut value = value;
        while let Operand::Temporary(temporary) = value {
            let Some(next) = values.get(&temporary.index).copied() else {
                break;
            };
            value = next;
        }
        value
    };
    let record_copy = |values: &mut BTreeMap<usize, Operand>, result: Temporary, source| {
        if matches!(
            source,
            Operand::Temporary(_)
                | Operand::I32(_)
                | Operand::F64(_)
                | Operand::Bool(_)
                | Operand::String(_)
                | Operand::Parameter { .. }
                | Operand::Receiver { .. }
        ) {
            values.insert(result.index, source);
        } else {
            values.remove(&result.index);
        }
    };
    match operation {
        Operation::Copy { source, result } => {
            let source = operand(*source, values);
            record_copy(values, *result, source);
            Operation::Copy {
                source,
                result: *result,
            }
        }
        Operation::Add {
            left,
            right,
            result,
        }
        | Operation::Sub {
            left,
            right,
            result,
        }
        | Operation::Mul {
            left,
            right,
            result,
        } => {
            let left = operand(*left, values);
            let right = operand(*right, values);
            let folded = match (operation, left, right) {
                (Operation::Add { .. }, Operand::I32(a), Operand::I32(b)) => a.checked_add(b),
                (Operation::Sub { .. }, Operand::I32(a), Operand::I32(b)) => a.checked_sub(b),
                (Operation::Mul { .. }, Operand::I32(a), Operand::I32(b)) => a.checked_mul(b),
                _ => None,
            };
            if let Some(value) = folded {
                let source = Operand::I32(value);
                record_copy(values, *result, source);
                Operation::Copy {
                    source,
                    result: *result,
                }
            } else {
                values.remove(&result.index);
                match operation {
                    Operation::Add { .. } => Operation::Add {
                        left,
                        right,
                        result: *result,
                    },
                    Operation::Sub { .. } => Operation::Sub {
                        left,
                        right,
                        result: *result,
                    },
                    _ => Operation::Mul {
                        left,
                        right,
                        result: *result,
                    },
                }
            }
        }
        Operation::JumpIfFalse { condition, target } => {
            let condition = operand(*condition, values);
            // Both successors start distinct basic blocks. Even the immediate
            // fallthrough must not inherit a fact past this terminator.
            values.clear();
            locals.clear();
            match condition {
                Operand::Bool(false) => Operation::Jump(*target),
                Operand::Bool(true) => Operation::JumpIfFalse {
                    condition: Operand::Bool(true),
                    target: *target,
                },
                condition => Operation::JumpIfFalse {
                    condition,
                    target: *target,
                },
            }
        }
        Operation::Param { call, index, value } => Operation::Param {
            call: *call,
            index: *index,
            value: operand(*value, values),
        },
        Operation::LoadLocal { local, result } => {
            if let Some(source) = locals.get(&local.index).copied() {
                let source = operand(source, values);
                // Materializing an f64 literal requires four movk fragments;
                // the slot load is cheaper in the ARM64 fixture and preserves
                // its existing constant-pool-like access cost.
                if matches!(source, Operand::F64(_)) {
                    values.remove(&result.index);
                    return operation.clone();
                }
                record_copy(values, *result, source);
                Operation::Copy {
                    source,
                    result: *result,
                }
            } else {
                values.remove(&result.index);
                operation.clone()
            }
        }
        Operation::StoreLocal { local, value } => {
            let value = operand(*value, values);
            locals.insert(local.index, value);
            Operation::StoreLocal {
                local: *local,
                value,
            }
        }
        Operation::Call { .. } | Operation::SetField { .. } | Operation::SetStatic { .. } => {
            // Calls can mutate captured locals. Field/static writes are effect
            // barriers too: keeping local snapshots scoped to effect-free code
            // avoids relying on alias or ownership details outside this pass.
            locals.clear();
            for result in results(operation) {
                values.remove(&result.index);
            }
            operation.clone()
        }
        Operation::Label(_) | Operation::Jump(_) => {
            values.clear();
            locals.clear();
            operation.clone()
        }
        _ => {
            for result in results(operation) {
                values.remove(&result.index);
            }
            operation.clone()
        }
    }
}

fn retain_reachable(code: Vec<Quadruple>) -> Result<Vec<Quadruple>, Diagnostic> {
    let labels = label_offsets(&code);
    let mut reachable = vec![false; code.len()];
    let mut pending = VecDeque::from([0usize]);
    while let Some(offset) = pending.pop_front() {
        if offset >= code.len() || std::mem::replace(&mut reachable[offset], true) {
            continue;
        }
        let next = offset + 1;
        let mut push = |candidate| pending.push_back(candidate);
        match code[offset].operation {
            Operation::Jump(label) => push(labels[&label]),
            Operation::JumpIfFalse { target, .. } => {
                push(labels[&target]);
                push(next);
            }
            Operation::Return(_) | Operation::Halt | Operation::Trap(_) => {}
            _ => push(next),
        }
    }
    Ok(code
        .into_iter()
        .enumerate()
        .filter_map(|(offset, quad)| reachable[offset].then_some(quad))
        .collect())
}

fn remove_dead_pure_temporaries(code: Vec<Quadruple>) -> Result<Vec<Quadruple>, Diagnostic> {
    let labels = label_offsets(&code);
    let successors = (0..code.len())
        .map(|offset| successors(&code, offset, &labels))
        .collect::<Result<Vec<_>, _>>()?;
    let mut live_in = vec![BTreeSet::new(); code.len()];
    let mut changed = true;
    while changed {
        changed = false;
        for offset in (0..code.len()).rev() {
            let mut live = successors[offset]
                .iter()
                .flat_map(|successor| live_in[*successor].iter().copied())
                .collect::<BTreeSet<_>>();
            let result = results(&code[offset].operation);
            let removable = result.len() == 1
                && !live.contains(&result[0].index)
                && is_removable_pure(&code[offset].operation);
            if !removable {
                for result in result {
                    live.remove(&result.index);
                }
                live.extend(temporary_operands(&code[offset].operation));
            }
            if live != live_in[offset] {
                live_in[offset] = live;
                changed = true;
            }
        }
    }
    Ok(code
        .into_iter()
        .enumerate()
        .filter_map(|(offset, quad)| {
            let live_out = successors[offset]
                .iter()
                .flat_map(|successor| live_in[*successor].iter().copied())
                .collect::<BTreeSet<_>>();
            let result = results(&quad.operation);
            let dead = result.len() == 1
                && !live_out.contains(&result[0].index)
                && is_removable_pure(&quad.operation);
            (!dead).then_some(quad)
        })
        .collect())
}

fn is_removable_pure(operation: &Operation) -> bool {
    match operation {
        Operation::Copy { .. }
        | Operation::LoadLocal { .. }
        | Operation::I32ToF64 { .. }
        | Operation::Not { .. } => true,
        // Keep floating-point comparisons: their status flags can be observed
        // by the target environment, and this optimizer deliberately makes no
        // assumptions about host or target floating-point state.
        Operation::Eq { left, .. }
        | Operation::Ne { left, .. }
        | Operation::Lt { left, .. }
        | Operation::Le { left, .. }
        | Operation::Gt { left, .. } => left.ty() != crate::quadruple::ValueType::F64,
        // f64 operations have no language-visible traps. They are removed only
        // when dead; they are never folded, reassociated, or fused here.
        Operation::Add { result, .. }
        | Operation::Sub { result, .. }
        | Operation::Mul { result, .. }
        | Operation::Div { result, .. }
        | Operation::Mod { result, .. }
        | Operation::Neg { result, .. } => result.ty == crate::quadruple::ValueType::F64,
        _ => false,
    }
}

fn label_offsets(code: &[Quadruple]) -> BTreeMap<LabelId, usize> {
    code.iter()
        .enumerate()
        .filter_map(|(offset, quad)| match quad.operation {
            Operation::Label(label) => Some((label, offset)),
            _ => None,
        })
        .collect()
}

fn successors(
    code: &[Quadruple],
    offset: usize,
    labels: &BTreeMap<LabelId, usize>,
) -> Result<Vec<usize>, Diagnostic> {
    let next = offset + 1;
    match code[offset].operation {
        Operation::Jump(label) => Ok(vec![labels[&label]]),
        Operation::JumpIfFalse { target, .. } => Ok(vec![labels[&target], next]),
        Operation::Return(_) | Operation::Halt | Operation::Trap(_) => Ok(Vec::new()),
        _ if next < code.len() => Ok(vec![next]),
        _ => Err(Diagnostic::error(
            "E7001",
            "cuádruplos inválidos",
            code[offset].span,
        )),
    }
}

fn temporary_operands(operation: &Operation) -> BTreeSet<usize> {
    operands(operation)
        .into_iter()
        .filter_map(|operand| match operand {
            Operand::Temporary(temporary) => Some(temporary.index),
            _ => None,
        })
        .collect()
}

fn operands(operation: &Operation) -> Vec<Operand> {
    match operation {
        Operation::Add { left, right, .. }
        | Operation::Sub { left, right, .. }
        | Operation::Mul { left, right, .. }
        | Operation::Div { left, right, .. }
        | Operation::Mod { left, right, .. }
        | Operation::Pow { left, right, .. }
        | Operation::Eq { left, right, .. }
        | Operation::Ne { left, right, .. }
        | Operation::Lt { left, right, .. }
        | Operation::Le { left, right, .. }
        | Operation::Gt { left, right, .. } => vec![*left, *right],
        Operation::Neg { operand, .. }
        | Operation::Not { operand, .. }
        | Operation::Copy {
            source: operand, ..
        }
        | Operation::StoreLocal { value: operand, .. }
        | Operation::I32ToF64 {
            source: operand, ..
        }
        | Operation::JumpIfFalse {
            condition: operand, ..
        }
        | Operation::Param { value: operand, .. } => vec![*operand],
        Operation::Return(Some(operand)) => vec![*operand],
        Operation::GetField { owner, .. } => vec![*owner],
        Operation::SetField { owner, value, .. } => vec![*owner, *value],
        Operation::SetStatic { value, .. } => vec![*value],
        Operation::BindMethod { receiver, .. } => vec![*receiver],
        Operation::Call {
            target: crate::quadruple::CallTarget::BoundMethod(method),
            ..
        } => vec![Operand::Temporary(*method)],
        _ => Vec::new(),
    }
}

/// Drops only an unconditional edge whose destination is the next executable
/// instruction. Labels stay in place: they retain their CFG/debug identity and
/// may still be targeted by other edges.
fn remove_fallthrough_jumps(code: Vec<Quadruple>) -> Vec<Quadruple> {
    let labels = label_offsets(&code);
    code.iter()
        .enumerate()
        .filter_map(|(index, quad)| {
            let Operation::Jump(target) = quad.operation else {
                return Some(quad.clone());
            };
            let next = (index + 1..code.len())
                .find(|next| !matches!(code[*next].operation, Operation::Label(_)));
            let target_next = labels.get(&target).and_then(|target_index| {
                (*target_index..code.len())
                    .find(|candidate| !matches!(code[*candidate].operation, Operation::Label(_)))
            });
            (next != target_next).then(|| quad.clone())
        })
        .collect()
}

fn results(operation: &Operation) -> Vec<Temporary> {
    match operation {
        Operation::Add { result, .. }
        | Operation::Sub { result, .. }
        | Operation::Mul { result, .. }
        | Operation::Div { result, .. }
        | Operation::Mod { result, .. }
        | Operation::Pow { result, .. }
        | Operation::Neg { result, .. }
        | Operation::Not { result, .. }
        | Operation::Eq { result, .. }
        | Operation::Ne { result, .. }
        | Operation::Lt { result, .. }
        | Operation::Le { result, .. }
        | Operation::Gt { result, .. }
        | Operation::Copy { result, .. }
        | Operation::LoadLocal { result, .. }
        | Operation::I32ToF64 { result, .. }
        | Operation::GetField { result, .. }
        | Operation::GetStatic { result, .. }
        | Operation::BindMethod { result, .. } => vec![*result],
        Operation::Call {
            result: Some(result),
            ..
        } => vec![*result],
        _ => Vec::new(),
    }
}
