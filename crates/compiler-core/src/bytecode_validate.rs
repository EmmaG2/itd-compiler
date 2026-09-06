use crate::{
    bytecode::{Chunk, Instruction},
    diagnostic::Diagnostic,
    runtime::{Value, error},
    source::{SourceId, Span},
};
use std::collections::VecDeque;

pub fn validate(chunk: &Chunk) -> Result<(), Diagnostic> {
    let fallback = Span::new(SourceId(0), 0, 0);
    let invalid = |span| error("E5002", "bytecode inválido", span);
    if chunk.functions.is_empty() {
        return Err(invalid(fallback));
    }
    let instruction_count = chunk.functions.iter().try_fold(0usize, |count, f| count.checked_add(f.code.len())).ok_or_else(|| invalid(fallback))?;
    if chunk.globals > instruction_count { return Err(invalid(fallback)); }
    for constant in &chunk.constants {
        match constant {
            Value::Function(id) if *id < chunk.functions.len() => {}
            Value::Constructor(id) if *id < chunk.classes.len() => {}
            Value::Native(name)
                if ["print", "round", "scale", "to_string"].contains(&name.as_str()) => {}
            Value::Int(_)
            | Value::Decimal(_)
            | Value::Bool(_)
            | Value::Str(_)
            | Value::Char(_)
            | Value::Void => {}
            Value::Float(v) if v.is_finite() => {}
            Value::Double(v) if v.is_finite() => {}
            _ => return Err(invalid(fallback)),
        }
    }
    for (id, class) in chunk.classes.iter().enumerate() {
        if class.fields > instruction_count || class.statics > instruction_count { return Err(invalid(fallback)); }
        if !chunk
            .functions
            .get(class.constructor)
            .is_some_and(|f| f.constructor == Some(id))
        {
            return Err(invalid(fallback));
        }
        for (method, function) in &class.methods {
            if *method >= chunk.method_names.len()
                || !chunk
                    .functions
                    .get(*function)
                    .is_some_and(|f| f.receiver && f.constructor.is_none())
            {
                return Err(invalid(fallback));
            }
        }
    }
    for (id, function) in chunk.functions.iter().enumerate() {
        let invalid = || invalid(function.span);
        if function.code.is_empty()
            || function.locals > 65_536
            || function.code.len() != function.spans.len()
            || function
                .parameters
                .checked_add(usize::from(function.receiver))
                .is_none_or(|n| n > function.locals)
            || function
                .constructor
                .is_some_and(|c| c >= chunk.classes.len() || !function.receiver)
            || id == 0
                && (function.parameters != 0 || function.receiver || function.constructor.is_some())
        {
            return Err(invalid());
        }
        for instruction in &function.code {
            let valid = match instruction {
                Instruction::Constant(n) => *n < chunk.constants.len(),
                Instruction::LoadLocal(n) | Instruction::StoreLocal(n) => *n < function.locals,
                Instruction::LoadGlobal(n) | Instruction::StoreGlobal(n) => *n < chunk.globals,
                Instruction::Jump(n) | Instruction::JumpIfFalse(n) | Instruction::JumpIfTrue(n) => {
                    *n < function.code.len()
                }
                Instruction::BindMethod(n) => *n < chunk.method_names.len(),
                Instruction::GetStatic(c, n) | Instruction::SetStatic(c, n) => {
                    chunk.classes.get(*c).is_some_and(|c| *n < c.statics)
                }
                Instruction::GetField(n) | Instruction::SetField(n) => {
                    chunk.classes.iter().any(|c| *n < c.fields)
                }
                Instruction::Halt => id == 0,
                Instruction::Return => id != 0,
                _ => true,
            };
            if !valid {
                return Err(invalid());
            }
        }
        let mut heights = vec![None; function.code.len()];
        let mut pending = VecDeque::from([(0, 0usize)]);
        while let Some((offset, height)) = pending.pop_front() {
            if offset >= function.code.len() {
                return Err(invalid());
            }
            if let Some(previous) = heights[offset] {
                if previous != height {
                    return Err(invalid());
                }
                continue;
            }
            heights[offset] = Some(height);
            let instruction = &function.code[offset];
            let (pops, pushes) = match instruction {
                Instruction::Constant(_)
                | Instruction::LoadLocal(_)
                | Instruction::LoadGlobal(_)
                | Instruction::GetStatic(_, _) => (0, 1),
                Instruction::StoreLocal(_)
                | Instruction::StoreGlobal(_)
                | Instruction::Pop
                | Instruction::JumpIfFalse(_)
                | Instruction::JumpIfTrue(_) => (1, 0),
                Instruction::Dup => (1, 2),
                Instruction::Binary(_) | Instruction::SetField(_) => (2, 1),
                Instruction::Unary(_)
                | Instruction::Cast(_)
                | Instruction::BindMethod(_)
                | Instruction::GetField(_)
                | Instruction::SetStatic(_, _) => (1, 1),
                Instruction::Call(n) => (n.checked_add(1).ok_or_else(invalid)?, 1),
                Instruction::Trap(_) => continue,
                Instruction::Return => {
                    if height != 1 {
                        return Err(invalid());
                    }
                    continue;
                }
                Instruction::Halt => {
                    if height != 0 {
                        return Err(invalid());
                    }
                    continue;
                }
                Instruction::Jump(_) => (0, 0),
            };
            let next_height = height
                .checked_sub(pops)
                .and_then(|n| n.checked_add(pushes))
                .ok_or_else(invalid)?;
            match instruction {
                Instruction::Jump(target) => pending.push_back((*target, next_height)),
                Instruction::JumpIfFalse(target) | Instruction::JumpIfTrue(target) => {
                    pending.push_back((*target, next_height));
                    pending.push_back((offset + 1, next_height));
                }
                _ => pending.push_back((offset + 1, next_height)),
            }
        }
    }
    Ok(())
}
