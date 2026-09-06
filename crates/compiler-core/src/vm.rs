use crate::{
    bytecode::{Chunk, Instruction, validate},
    diagnostic::{Diagnostic, Label},
    runtime::{RuntimeLimits, RuntimeResult, Value, binary, builtin, cast, error, unary},
    source::Span,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

struct Frame {
    function: usize,
    ip: usize,
    base: usize,
    locals: Vec<Option<Value>>,
}
struct Object {
    class: usize,
    fields: Vec<Option<Value>>,
}
struct Vm<'a> {
    chunk: &'a Chunk,
    stack: Vec<Value>,
    frames: Vec<Frame>,
    globals: Vec<Option<Value>>,
    heap: Vec<Object>,
    statics: Vec<Vec<Option<Value>>>,
    output: String,
    limits: RuntimeLimits,
    remaining: usize,
    cancelled: Option<Arc<AtomicBool>>,
    started: Instant,
}

pub fn execute(
    chunk: &Chunk,
    limits: RuntimeLimits,
    cancelled: Option<Arc<AtomicBool>>,
) -> RuntimeResult {
    if let Err(diagnostic) = validate(chunk) {
        return RuntimeResult {
            output: String::new(),
            diagnostics: vec![diagnostic],
        };
    }
    let mut vm = Vm {
        chunk,
        stack: Vec::new(),
        frames: vec![Frame {
            function: 0,
            ip: 0,
            base: 0,
            locals: vec![None; chunk.functions[0].locals],
        }],
        globals: vec![None; chunk.globals],
        heap: Vec::new(),
        statics: chunk
            .classes
            .iter()
            .map(|c| vec![None; c.statics])
            .collect(),
        output: String::new(),
        limits,
        remaining: limits.instructions,
        cancelled,
        started: Instant::now(),
    };
    let diagnostics = vm
        .run()
        .err()
        .map(|mut d| {
            for frame in vm.frames.iter().skip(1).rev() {
                let f = &chunk.functions[frame.function];
                d.labels.push(Label {
                    span: f.span,
                    message: format!("en llamada a {}", f.name),
                });
            }
            d
        })
        .into_iter()
        .collect();
    RuntimeResult {
        output: vm.output,
        diagnostics,
    }
}
impl Vm<'_> {
    fn pop(&mut self, span: Span) -> Result<Value, Diagnostic> {
        self.stack
            .pop()
            .ok_or_else(|| error("E5002", "pila vacía", span))
    }
    fn tick(&mut self, span: Span) -> Result<(), Diagnostic> {
        if self
            .cancelled
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
        {
            return Err(error("E5004", "ejecución cancelada", span));
        }
        if self.started.elapsed().as_millis() > u128::from(self.limits.wall_time_ms) {
            return Err(error("E5007", "límite de tiempo excedido", span));
        }
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| error("E5003", "límite de instrucciones excedido", span))?;
        Ok(())
    }
    fn call(&mut self, count: usize, span: Span) -> Result<(), Diagnostic> {
        let start = self
            .stack
            .len()
            .checked_sub(count)
            .ok_or_else(|| error("E5002", "argumentos inválidos", span))?;
        let arguments = self.stack.split_off(start);
        let callee = self.pop(span)?;
        let (function, receiver) = match callee {
            Value::Native(name) => {
                let value = builtin(
                    &name,
                    &arguments,
                    &mut self.output,
                    self.limits.output_bytes,
                    span,
                )?;
                self.stack.push(value);
                return Ok(());
            }
            Value::Function(function) => (function, None),
            Value::BoundMethod(function, object) => (function, Some(object)),
            Value::Constructor(class) => {
                let class = self
                    .chunk
                    .classes
                    .get(class)
                    .ok_or_else(|| error("E5002", "clase inválida", span))?;
                (class.constructor, None)
            }
            _ => return Err(error("E4000", "el valor no es invocable", span)),
        };
        let f = self
            .chunk
            .functions
            .get(function)
            .ok_or_else(|| error("E5002", "función inválida", span))?;
        if function == 0 || count != f.parameters {
            return Err(error("E5002", "aridad inválida", span));
        }
        if self.frames.len() > self.limits.call_depth {
            return Err(error("E4004", "límite de recursión excedido", span));
        }
        let receiver = if let Some(class) = f.constructor {
            if self.heap.len() >= self.limits.heap_objects {
                return Err(error("E5005", "límite de objetos excedido", span));
            }
            let handle = self.heap.len();
            self.heap.push(Object {
                class,
                fields: vec![None; self.chunk.classes[class].fields],
            });
            Some(handle)
        } else {
            receiver
        };
        if f.receiver != receiver.is_some() {
            return Err(error("E5002", "receptor inválido", span));
        }
        let mut locals = vec![None; f.locals];
        if let Some(object) = receiver {
            locals[0] = Some(Value::Object(object));
        }
        for (slot, value) in locals
            .iter_mut()
            .skip(usize::from(f.receiver))
            .zip(arguments)
        {
            *slot = Some(value);
        }
        self.frames.push(Frame {
            function,
            ip: 0,
            base: self.stack.len(),
            locals,
        });
        Ok(())
    }
    fn run(&mut self) -> Result<(), Diagnostic> {
        loop {
            let frame = self.frames.last().expect("VM entry frame");
            let f = &self.chunk.functions[frame.function];
            let span = f.spans.get(frame.ip).copied().unwrap_or(f.span);
            let instruction = f
                .code
                .get(frame.ip)
                .ok_or_else(|| error("E5002", "instrucción fuera de rango", span))?;
            self.tick(span)?;
            self.frames.last_mut().expect("VM frame").ip += 1;
            match instruction {
                Instruction::Constant(id) => self.stack.push(self.chunk.constants[*id].clone()),
                Instruction::LoadLocal(id) => {
                    let value = self
                        .frames
                        .last()
                        .and_then(|f| f.locals.get(*id))
                        .and_then(Clone::clone)
                        .ok_or_else(|| error("E4000", "variable no inicializada", span))?;
                    self.stack.push(value);
                }
                Instruction::LoadGlobal(id) => {
                    let value = self
                        .globals
                        .get(*id)
                        .and_then(Clone::clone)
                        .ok_or_else(|| error("E4000", "variable no inicializada", span))?;
                    self.stack.push(value);
                }
                Instruction::StoreLocal(id) => {
                    let value = self.pop(span)?;
                    self.frames.last_mut().expect("VM frame").locals[*id] = Some(value);
                }
                Instruction::StoreGlobal(id) => {
                    let value = self.pop(span)?;
                    self.globals[*id] = Some(value);
                }
                Instruction::Pop => {
                    self.pop(span)?;
                }
                Instruction::Dup => {
                    let v = self
                        .stack
                        .last()
                        .cloned()
                        .ok_or_else(|| error("E5002", "pila vacía", span))?;
                    self.stack.push(v);
                }
                Instruction::Unary(op) => {
                    let v = self.pop(span)?;
                    self.stack.push(unary(*op, v, span)?);
                }
                Instruction::Binary(op) => {
                    let right = self.pop(span)?;
                    let left = self.pop(span)?;
                    self.stack.push(binary(left, *op, right, span)?);
                }
                Instruction::Cast(ty) => {
                    let value = self.pop(span)?;
                    self.stack.push(cast(value, ty, span)?);
                }
                Instruction::Jump(target) => self.frames.last_mut().expect("VM frame").ip = *target,
                Instruction::JumpIfFalse(target) | Instruction::JumpIfTrue(target) => {
                    let Value::Bool(value) = self.pop(span)? else {
                        return Err(error("E4000", "la condición no es bool", span));
                    };
                    if value == matches!(instruction, Instruction::JumpIfTrue(_)) {
                        self.frames.last_mut().expect("VM frame").ip = *target;
                    }
                }
                Instruction::Call(count) => self.call(*count, span)?,
                Instruction::BindMethod(method) => {
                    let Value::Object(handle) = self.pop(span)? else {
                        return Err(error("E4000", "receptor inválido", span));
                    };
                    let object = self
                        .heap
                        .get(handle)
                        .ok_or_else(|| error("E4000", "objeto inválido", span))?;
                    let function = self.chunk.classes[object.class]
                        .methods
                        .get(method)
                        .ok_or_else(|| error("E4000", "método no disponible", span))?;
                    self.stack.push(Value::BoundMethod(*function, handle));
                }
                Instruction::GetField(field) => {
                    let Value::Object(handle) = self.pop(span)? else {
                        return Err(error("E4000", "receptor inválido", span));
                    };
                    let value = self
                        .heap
                        .get(handle)
                        .and_then(|o| o.fields.get(*field))
                        .and_then(Clone::clone)
                        .ok_or_else(|| error("E4000", "campo no inicializado", span))?;
                    self.stack.push(value);
                }
                Instruction::SetField(field) => {
                    let Value::Object(handle) = self.pop(span)? else {
                        return Err(error("E4000", "receptor inválido", span));
                    };
                    let value = self.pop(span)?;
                    let slot = self
                        .heap
                        .get_mut(handle)
                        .and_then(|o| o.fields.get_mut(*field))
                        .ok_or_else(|| error("E4000", "campo inválido", span))?;
                    *slot = Some(value.clone());
                    self.stack.push(value);
                }
                Instruction::GetStatic(class, field) => {
                    let value = self.statics[*class][*field]
                        .clone()
                        .ok_or_else(|| error("E4000", "campo estático no inicializado", span))?;
                    self.stack.push(value);
                }
                Instruction::SetStatic(class, field) => {
                    let value = self.pop(span)?;
                    self.statics[*class][*field] = Some(value.clone());
                    self.stack.push(value);
                }
                Instruction::Return => {
                    let value = self.pop(span)?;
                    let frame = self.frames.pop().expect("VM frame");
                    self.stack.truncate(frame.base);
                    self.stack.push(value);
                }
                Instruction::Trap(diagnostic) => return Err(diagnostic.clone()),
                Instruction::Halt => return Ok(()),
            }
        }
    }
}
