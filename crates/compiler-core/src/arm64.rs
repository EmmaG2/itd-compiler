//! Darwin ARM64 emission from validated quadruples.  This module deliberately has
//! no AST, bytecode, VM, or runtime::Value dependency.
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use crate::{
    diagnostic::Diagnostic,
    quadruple::{
        self, CallTarget, Function, FunctionId, Operand, Operation, QuadrupleProgram, Temporary,
        ValueType,
    },
    source::{SourceId, Span},
};

const ERROR: &str = "E7201";

/// Emit standalone Apple Silicon Mach-O assembly from already validated typed IR.
pub fn emit(program: &QuadrupleProgram) -> Result<String, Diagnostic> {
    quadruple::validate(program)?;
    let optimized = crate::quadruple_optimize::optimize(program)?;
    let program = &optimized;
    for string in &program.strings {
        if string.as_bytes().contains(&0) {
            return Err(error(
                "las cadenas con NUL no son compatibles con el emisor nativo",
                fallback(program),
            ));
        }
    }
    let reachability = reachability(program);
    preflight(program, &reachability.functions)?;
    let mut emitter = Emitter {
        program,
        reachability,
        output: String::new(),
    };
    emitter.preamble();
    for (id, function) in program.functions.iter().enumerate() {
        if emitter.reachability.functions.contains(&id) {
            emitter.function(FunctionId(id), function)?;
        }
    }
    emitter.runtime()?;
    Ok(emitter.output)
}

struct Reachability {
    functions: BTreeSet<usize>,
    runtimes: BTreeSet<usize>,
    strings: BTreeSet<usize>,
    statics: BTreeSet<usize>,
    traps: bool,
}

fn reachability(program: &QuadrupleProgram) -> Reachability {
    let mut functions = BTreeSet::from([program.main.0]);
    let mut runtimes = BTreeSet::new();
    let mut strings = BTreeSet::new();
    let mut statics = BTreeSet::new();
    let mut traps = false;
    let mut pending = vec![program.main.0];
    while let Some(id) = pending.pop() {
        for quad in &program.functions[id].code {
            match quad.operation {
                Operation::Call {
                    target: CallTarget::Function(function),
                    ..
                } => {
                    if functions.insert(function.0) {
                        pending.push(function.0);
                    }
                }
                Operation::Call {
                    target: CallTarget::Constructor(class),
                    ..
                } => {
                    traps = true; // allocation failure is a native trap.
                    if let Some(function) = program.classes[class.0].constructor
                        && functions.insert(function.0)
                    {
                        pending.push(function.0);
                    }
                }
                Operation::Call {
                    target: CallTarget::Runtime(runtime),
                    ..
                } => {
                    runtimes.insert(runtime.0);
                }
                Operation::BindMethod { method, .. } if functions.insert(method.0) => {
                    pending.push(method.0);
                }
                Operation::GetStatic { static_, .. } | Operation::SetStatic { static_, .. } => {
                    statics.insert(static_.0);
                }
                Operation::Add { .. }
                | Operation::Sub { .. }
                | Operation::Mul { .. }
                | Operation::Div { .. }
                | Operation::Mod { .. }
                | Operation::Pow { .. }
                | Operation::Neg { .. }
                | Operation::Trap(_) => traps = true,
                _ => {}
            }
            for operand in operation_operands(&quad.operation) {
                if let Operand::String(string) = operand {
                    strings.insert(string.0);
                }
            }
        }
    }
    Reachability {
        functions,
        runtimes,
        strings,
        statics,
        traps,
    }
}

fn operation_operands(operation: &Operation) -> Vec<Operand> {
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
        | Operation::Param { value: operand, .. }
        | Operation::Return(Some(operand))
        | Operation::GetField { owner: operand, .. }
        | Operation::SetStatic { value: operand, .. }
        | Operation::BindMethod {
            receiver: operand, ..
        } => vec![*operand],
        Operation::SetField { owner, value, .. } => vec![*owner, *value],
        Operation::Call {
            target: CallTarget::BoundMethod(temporary),
            ..
        } => vec![Operand::Temporary(*temporary)],
        _ => Vec::new(),
    }
}

struct Emitter<'a> {
    program: &'a QuadrupleProgram,
    reachability: Reachability,
    output: String,
}
impl Emitter<'_> {
    fn preamble(&mut self) {
        self.output.push_str(
            ".section __TEXT,__text,regular,pure_instructions\n.p2align 2\n.globl _main\n",
        );
    }
    fn function(&mut self, id: FunctionId, function: &Function) -> Result<(), Diagnostic> {
        let layout = Layout::new(function)?;
        if id == self.program.main {
            self.output.push_str("_main:\n");
        }
        writeln!(self.output, "_itd_f{}:", id.0).unwrap();
        if layout.needs_frame_record {
            self.output
                .push_str("\tstp x29, x30, [sp, #-16]!\n\tmov x29, sp\n");
        }
        self.adjust_sp(layout.frame, false);
        // String storage owns one dynamic reference when its pointer is tagged.
        // Clearing every home first makes cleanup and replacement safe even across
        // colored temporary slots.
        for offset in &layout.string_slots {
            self.store_x(*offset, "xzr");
        }
        // Materialize arguments before any nested call.  Integer and floating
        // register sequences are independent under AAPCS64.
        let mut integer = usize::from(function.signature.receiver.is_some());
        let mut float = 0;
        if function.signature.receiver.is_some() {
            self.store_x(layout.receiver(), "x0");
        }
        for parameter in &function.signature.parameters {
            match parameter.ty {
                ValueType::F64 => {
                    if float >= 8 {
                        return Err(error(
                            "más de ocho argumentos f64 nativos no están soportados",
                            parameter.span,
                        ));
                    }
                    self.store_d(layout.param(parameter.index), &format!("d{float}"));
                    if layout.locals.contains_key(&parameter.index) {
                        self.store_d(layout.local(parameter.index), &format!("d{float}"));
                    }
                    float += 1;
                }
                _ => {
                    if integer >= 8 {
                        return Err(error(
                            "más de ocho argumentos enteros/puntero nativos no están soportados",
                            parameter.span,
                        ));
                    }
                    if parameter.ty == ValueType::I32 || parameter.ty == ValueType::Bool {
                        self.store_w(layout.param(parameter.index), &format!("w{integer}"));
                    } else if parameter.ty == ValueType::Str {
                        self.own_string_register(
                            &format!("x{integer}"),
                            layout.param(parameter.index),
                        );
                    } else {
                        self.store_x(layout.param(parameter.index), &format!("x{integer}"));
                    }
                    if layout.locals.contains_key(&parameter.index) {
                        if parameter.ty == ValueType::I32 || parameter.ty == ValueType::Bool {
                            self.store_w(layout.local(parameter.index), &format!("w{integer}"));
                        } else if parameter.ty == ValueType::Str {
                            self.own_string_register(
                                &format!("x{integer}"),
                                layout.local(parameter.index),
                            );
                        } else {
                            self.store_x(layout.local(parameter.index), &format!("x{integer}"));
                        }
                    }
                    integer += 1;
                }
            }
        }
        for (offset, quad) in function.code.iter().enumerate() {
            let falls_into_epilogue = function.code[offset + 1..]
                .iter()
                .all(|next| matches!(next.operation, Operation::Label(_)));
            self.operation(
                id,
                function,
                &layout,
                offset,
                &quad.operation,
                falls_into_epilogue,
            )?;
        }
        writeln!(self.output, ".L{}_epilogue:", id.0).unwrap();
        // Cleanup may reach libc through _free. Preserve f64 returns in a
        // dedicated frame slot rather than relying on caller-saved d registers.
        if let Some(offset) = layout.return_spill {
            self.store_d(offset, "d0");
        }
        // The release helper preserves x10 across free, so scalar and string
        // return values survive cleanup of the function's owned homes.
        if id != self.program.main {
            self.output.push_str("\tmov x10, x0\n");
        }
        for offset in &layout.string_slots {
            self.release_string_at(*offset);
        }
        if id == self.program.main {
            self.release_string_statics();
            self.output.push_str("\tmov w0, #0\n");
        } else {
            self.output.push_str("\tmov x0, x10\n");
        }
        if let Some(offset) = layout.return_spill {
            self.load_d(offset, "d0");
        }
        self.adjust_sp(layout.frame, true);
        if layout.needs_frame_record {
            self.output.push_str("\tldp x29, x30, [sp], #16\n");
        }
        self.output.push_str("\tret\n");
        Ok(())
    }
    fn operation(
        &mut self,
        id: FunctionId,
        function: &Function,
        layout: &Layout,
        offset: usize,
        op: &Operation,
        falls_into_epilogue: bool,
    ) -> Result<(), Diagnostic> {
        let span = function.code[offset].span;
        use Operation::*;
        match op {
            Add {
                left,
                right,
                result,
            }
            | Sub {
                left,
                right,
                result,
            }
            | Mul {
                left,
                right,
                result,
            } if result.ty == ValueType::I32 => {
                self.i32_binary_operands(function, offset, left, right, layout);
                let ins = if matches!(op, Add { .. }) {
                    "adds"
                } else if matches!(op, Sub { .. }) {
                    "subs"
                } else {
                    "smull"
                };
                if ins == "smull" {
                    writeln!(self.output, "\tsmull x11, w9, w10\n\tcmp x11, w11, sxtw\n\tb.ne _itd_trap\n\tmov w9, w11").unwrap();
                } else {
                    writeln!(self.output, "\t{ins} w9, w9, w10\n\tb.vs _itd_trap").unwrap();
                }
                if !keep_scalar_result(function, offset, *result) {
                    self.store_w(layout.temp(*result), "w9");
                }
            }
            Add {
                left,
                right,
                result,
            }
            | Sub {
                left,
                right,
                result,
            }
            | Mul {
                left,
                right,
                result,
            }
            | Div {
                left,
                right,
                result,
            } if result.ty == ValueType::F64 => {
                self.f64_binary_operands(function, offset, left, right, layout);
                let ins = match op {
                    Add { .. } => "fadd",
                    Sub { .. } => "fsub",
                    Mul { .. } => "fmul",
                    _ => "fdiv",
                };
                writeln!(self.output, "\t{ins} d16, d16, d17").unwrap();
                if !keep_scalar_result(function, offset, *result) {
                    self.store_d(layout.temp(*result), "d16");
                }
            }
            Div {
                left,
                right,
                result,
            }
            | Mod {
                left,
                right,
                result,
            } if result.ty == ValueType::I32 => {
                self.i32_binary_operands(function, offset, left, right, layout);
                writeln!(self.output, "\tcbz w10, _itd_trap\n\tmov w13, #0x80000000\n\tcmp w9, w13\n\tb.ne 1f\n\tcmn w10, #1\n\tb.eq _itd_trap\n1:").unwrap();
                writeln!(self.output, "\tsdiv w11, w9, w10").unwrap();
                if matches!(op, Mod { .. }) {
                    writeln!(self.output, "\tmsub w9, w11, w10, w9").unwrap();
                } else {
                    self.output.push_str("\tmov w9, w11\n");
                }
                if !keep_scalar_result(function, offset, *result) {
                    self.store_w(layout.temp(*result), "w9");
                }
            }
            Pow {
                left,
                right,
                result,
            } if result.ty == ValueType::I32 => {
                self.w(right, layout);
                self.output.push_str("\tmov w10, w9\n");
                self.w(left, layout);
                self.output.push_str("\ttbnz w10, #31, _itd_trap\n\tmov w11, #1\n2:\n\tcbz w10, 3f\n\tmul w12, w11, w9\n\tsmull x13, w11, w9\n\tcmp x13, w12, sxtw\n\tb.ne _itd_trap\n\tmov w11, w12\n\tsub w10, w10, #1\n\tb 2b\n3:\n");
                if !keep_scalar_result(function, offset, *result) {
                    self.store_w(layout.temp(*result), "w11");
                }
            }
            Pow { .. } | Mod { .. } => {
                return Err(error(
                    "POW y MOD decimal no son compatibles con el emisor nativo",
                    span,
                ));
            }
            Neg { operand, result } if result.ty == ValueType::I32 => {
                if !resident_scalar_operand(function, offset, operand) {
                    self.w(operand, layout);
                }
                self.output.push_str("\tnegs w9, w9\n\tb.vs _itd_trap\n");
                if !keep_scalar_result(function, offset, *result) {
                    self.store_w(layout.temp(*result), "w9");
                }
            }
            Neg { operand, result } => {
                if !resident_scalar_operand(function, offset, operand) {
                    self.d(operand, layout);
                }
                self.output.push_str("\tfneg d16, d16\n");
                if !keep_scalar_result(function, offset, *result) {
                    self.store_d(layout.temp(*result), "d16");
                }
            }
            Not { operand, result } => {
                if !resident_scalar_operand(function, offset, operand) {
                    self.w(operand, layout);
                }
                self.output.push_str("\teor w9, w9, #1\n");
                if !keep_scalar_result(function, offset, *result) {
                    self.store_w(layout.temp(*result), "w9");
                }
            }
            Eq {
                left,
                right,
                result,
            }
            | Ne {
                left,
                right,
                result,
            }
            | Lt {
                left,
                right,
                result,
            }
            | Le {
                left,
                right,
                result,
            }
            | Gt {
                left,
                right,
                result,
            } => {
                self.compare(function, offset, op, left, right, layout);
                if !keep_scalar_result(function, offset, *result) {
                    self.store_w(layout.temp(*result), "w9");
                }
            }
            Copy { source, result } => {
                if keep_scalar_result(function, offset, *result) {
                    // A scalar copy with one immediate use is only a rename:
                    // leave its value in the consumer's operand register.
                    if !resident_scalar_operand(function, offset, source) {
                        if source.ty() == ValueType::F64 {
                            self.d(source, layout);
                        } else {
                            self.w(source, layout);
                        }
                    }
                } else {
                    self.copy(source, layout.temp(*result), layout);
                }
            }
            LoadLocal { local, result } => {
                if keep_scalar_result(function, offset, *result) {
                    match local.ty {
                        ValueType::F64 => self.load_d(layout.local(local.index), "d16"),
                        ValueType::I32 | ValueType::Bool => {
                            self.load_w(layout.local(local.index), "w9")
                        }
                        _ => unreachable!("only scalar results may be resident"),
                    }
                } else {
                    self.copy_slot(layout.local(local.index), layout.temp(*result), local.ty);
                }
            }
            StoreLocal { local, value } => self.copy(value, layout.local(local.index), layout),
            I32ToF64 { source, result } => {
                if !resident_scalar_operand(function, offset, source) {
                    self.w(source, layout);
                }
                self.output.push_str("\tscvtf d16, w9\n");
                if !keep_scalar_result(function, offset, *result) {
                    self.store_d(layout.temp(*result), "d16");
                }
            }
            Label(label) => writeln!(self.output, ".L{}_{}:", id.0, label.0).unwrap(),
            Jump(label) => writeln!(self.output, "\tb .L{}_{}", id.0, label.0).unwrap(),
            JumpIfFalse { condition, target } => {
                self.w(condition, layout);
                writeln!(self.output, "\tcbz w9, .L{}_{}", id.0, target.0).unwrap();
            }
            Param { .. } => {}
            Call {
                call,
                target,
                result,
            } => self.call(function, layout, *call, *target, *result, span)?,
            Return(value) => {
                if value.is_none() && function.signature.receiver.is_some() {
                    self.load_x_at(layout.receiver(), 0, "x0");
                }
                if let Some(value) = value {
                    match value.ty() {
                        ValueType::Str => {
                            self.x(value, layout);
                            self.return_string();
                        }
                        ValueType::F64 => {
                            self.d(value, layout);
                            self.output.push_str("\tfmov d0, d16\n");
                        }
                        _ => {
                            self.x(value, layout);
                            self.output.push_str("\tmov x0, x9\n");
                        }
                    }
                }
                if !falls_into_epilogue {
                    writeln!(self.output, "\tb .L{}_epilogue", id.0).unwrap();
                }
            }
            GetField {
                owner,
                field,
                result,
            } => {
                self.x(owner, layout);
                self.load_x("x9", field.0 * 8, "x9");
                if matches!(result.ty, ValueType::I32 | ValueType::Bool) {
                    self.store_w(layout.temp(*result), "w9");
                } else if result.ty == ValueType::Str {
                    self.copy_string_register_to_slot("x9", layout.temp(*result));
                } else {
                    self.store_x(layout.temp(*result), "x9");
                }
            }
            SetField {
                owner,
                field,
                value,
            } => {
                if value.ty() == ValueType::F64 {
                    self.d(value, layout);
                    self.output.push_str("\tfmov x10, d16\n");
                } else {
                    self.x(value, layout);
                    self.output.push_str("\tmov x10, x9\n");
                }
                self.x(owner, layout);
                self.store_at("x9", field.0 * 8, "x10");
            }
            GetStatic { static_, result } => {
                writeln!(
                    self.output,
                    "\tadrp x9, _itd_static{}@PAGE\n\tldr x9, [x9, _itd_static{}@PAGEOFF]",
                    static_.0, static_.0
                )
                .unwrap();
                if matches!(result.ty, ValueType::I32 | ValueType::Bool) {
                    self.store_w(layout.temp(*result), "w9");
                } else if result.ty == ValueType::Str {
                    self.copy_string_register_to_slot("x9", layout.temp(*result));
                } else {
                    self.store_x(layout.temp(*result), "x9");
                }
            }
            SetStatic { static_, value } => {
                if value.ty() == ValueType::Str {
                    self.x(value, layout);
                    self.replace_string_static(static_.0);
                } else if value.ty() == ValueType::F64 {
                    self.d(value, layout);
                    self.output.push_str("\tfmov x9, d16\n");
                } else {
                    self.x(value, layout);
                }
                if value.ty() != ValueType::Str {
                    writeln!(
                        self.output,
                        "\tadrp x10, _itd_static{}@PAGE\n\tstr x9, [x10, _itd_static{}@PAGEOFF]",
                        static_.0, static_.0
                    )
                    .unwrap();
                }
            }
            BindMethod {
                receiver,
                method,
                result,
            } => {
                self.x(receiver, layout);
                writeln!(
                    self.output,
                    "\tadrp x10, _itd_f{}@PAGE\n\tadd x10, x10, _itd_f{}@PAGEOFF",
                    method.0, method.0
                )
                .unwrap();
                self.store_x(layout.temp(*result), "x10");
                self.store_x_at(layout.temp(*result), 8, "x9");
            }
            Halt => {
                self.output.push_str("\tmov w0, #0\n");
                if !falls_into_epilogue {
                    writeln!(self.output, "\tb .L{}_epilogue", id.0).unwrap();
                }
            }
            Trap(_) => self.output.push_str("\tb _itd_trap\n"),
            _ => {
                return Err(error(
                    "operación de cuádruplo no compatible con el emisor nativo",
                    span,
                ));
            }
        };
        Ok(())
    }
    fn call(
        &mut self,
        function: &Function,
        layout: &Layout,
        call: crate::quadruple::CallSiteId,
        target: CallTarget,
        result: Option<Temporary>,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let position = function
            .code
            .iter()
            .position(|q| matches!(q.operation, Operation::Call { call: c, .. } if c == call))
            .unwrap_or(0);
        let params: Vec<_> = function.code[..position]
            .iter()
            .rev()
            .take_while(|q| matches!(q.operation, Operation::Param { call: c, .. } if c == call))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let mut ints = 0;
        let mut floats = 0;
        let constructor = match target {
            CallTarget::Constructor(class) => Some(class),
            _ => None,
        };
        // Constructor allocation happens before argument register lowering.
        // Its `_malloc` call clobbers caller-saved result/residency registers,
        // so constructor arguments must come from their already materialized
        // homes rather than the forwarding fast paths.
        let constructor_arguments_must_materialize = constructor.is_some();
        if let Some(class) = constructor {
            let bytes = self.program.classes[class.0].fields.len().max(1) * 8;
            self.mov_usize("x0", bytes);
            self.output.push_str("\tbl _malloc\n\tcbz x0, _itd_trap\n");
            self.store_x(layout.call_scratch(), "x0");
        }
        // Direct instance calls already carry their receiver as PARAM 0.  Only
        // constructors and bound calls inject an additional receiver register.
        if constructor.is_some() || matches!(target, CallTarget::BoundMethod(_)) {
            ints = 1;
        }
        // Materialize forwarding operands before normal argument lowering can
        // reuse d16/w9.  These are the only registers intentionally resident;
        // all other live scalar values already have colored homes, so every
        // ordinary call is a full caller-clobber barrier.
        let mut preload_ints = ints;
        let mut preload_floats = floats;
        for q in &params {
            let Operation::Param { value, .. } = &q.operation else {
                unreachable!()
            };
            if value.ty() == ValueType::F64 {
                if !constructor_arguments_must_materialize
                    && resident_call_argument(function, position, value)
                {
                    writeln!(self.output, "\tfmov d{preload_floats}, d16").unwrap();
                } else if !constructor_arguments_must_materialize
                    && forwarded_call_result(function, position, value)
                    && preload_floats != 0
                {
                    writeln!(self.output, "\tfmov d{preload_floats}, d0").unwrap();
                }
                preload_floats += 1;
            } else {
                if !constructor_arguments_must_materialize
                    && resident_call_argument(function, position, value)
                {
                    let register = if matches!(value.ty(), ValueType::I32 | ValueType::Bool) {
                        format!("w{preload_ints}")
                    } else {
                        format!("x{preload_ints}")
                    };
                    writeln!(self.output, "\tmov {register}, w9").unwrap();
                } else if direct_string_print_argument(
                    self.program,
                    function,
                    position,
                    target,
                    value,
                ) {
                    // `consume_string_return` restored x0 after storing the owned
                    // reference. This immediate one-argument print is the only
                    // string forwarding edge: PARAM is a no-op and no call can
                    // clobber x0 between the producer and this consumer.
                } else if !constructor_arguments_must_materialize
                    && forwarded_call_result(function, position, value)
                    && preload_ints != 0
                {
                    let register = if matches!(value.ty(), ValueType::I32 | ValueType::Bool) {
                        format!("w{preload_ints}")
                    } else {
                        format!("x{preload_ints}")
                    };
                    let source = if matches!(value.ty(), ValueType::I32 | ValueType::Bool) {
                        "w0"
                    } else {
                        "x0"
                    };
                    writeln!(self.output, "\tmov {register}, {source}").unwrap();
                }
                preload_ints += 1;
            }
        }
        for q in &params {
            let Operation::Param { value, .. } = &q.operation else {
                unreachable!()
            };
            if value.ty() == ValueType::F64 {
                if floats >= 8 {
                    return Err(error(
                        "más de ocho argumentos f64 nativos no están soportados",
                        span,
                    ));
                }
                if !constructor_arguments_must_materialize
                    && resident_call_argument(function, position, value)
                {
                    // Preloaded from d16 before scratch-register lowering.
                } else if !constructor_arguments_must_materialize
                    && forwarded_call_result(function, position, value)
                {
                    if floats != 0 {
                        writeln!(self.output, "\tfmov d{floats}, d0").unwrap();
                    }
                } else {
                    match value {
                        Operand::Temporary(temporary) => {
                            self.load_d(layout.temp(*temporary), &format!("d{floats}"));
                        }
                        Operand::Parameter { index, .. } => {
                            self.load_d(layout.param(*index), &format!("d{floats}"));
                        }
                        _ => {
                            self.d(value, layout);
                            writeln!(self.output, "\tfmov d{floats}, d16").unwrap();
                        }
                    }
                }
                floats += 1;
            } else {
                if ints >= 8 {
                    return Err(error(
                        "más de ocho argumentos enteros/puntero nativos no están soportados",
                        span,
                    ));
                }
                let register = if matches!(value.ty(), ValueType::I32 | ValueType::Bool) {
                    format!("w{ints}")
                } else {
                    format!("x{ints}")
                };
                if !constructor_arguments_must_materialize
                    && resident_call_argument(function, position, value)
                {
                    // Preloaded from w9 before scratch-register lowering.
                } else if direct_string_print_argument(
                    self.program,
                    function,
                    position,
                    target,
                    value,
                ) {
                    // x0 still holds the owned result; do not reload its home.
                } else if !constructor_arguments_must_materialize
                    && forwarded_call_result(function, position, value)
                {
                    if ints != 0 {
                        let source = if matches!(value.ty(), ValueType::I32 | ValueType::Bool) {
                            "w0"
                        } else {
                            "x0"
                        };
                        writeln!(self.output, "\tmov {register}, {source}").unwrap();
                    }
                } else {
                    match value {
                        Operand::Temporary(temporary)
                            if matches!(value.ty(), ValueType::I32 | ValueType::Bool) =>
                        {
                            self.load_w(layout.temp(*temporary), &register);
                        }
                        Operand::Parameter { index, .. }
                            if matches!(value.ty(), ValueType::I32 | ValueType::Bool) =>
                        {
                            self.load_w(layout.param(*index), &register);
                        }
                        Operand::Temporary(temporary) => {
                            self.load_x_at(layout.temp(*temporary), 0, &register);
                        }
                        Operand::Parameter { index, .. } => {
                            self.load_x_at(layout.param(*index), 0, &register);
                        }
                        _ => {
                            self.x(value, layout);
                            let source = if matches!(value.ty(), ValueType::I32 | ValueType::Bool) {
                                "w9"
                            } else {
                                "x9"
                            };
                            writeln!(self.output, "\tmov {register}, {source}").unwrap();
                        }
                    }
                }
                ints += 1;
            }
        }
        match target {
            CallTarget::Function(target) => {
                writeln!(self.output, "\tbl _itd_f{}", target.0).unwrap()
            }
            CallTarget::Constructor(class) => {
                self.load_x_at(layout.call_scratch(), 0, "x0");
                if let Some(constructor) = self.program.classes[class.0].constructor {
                    self.output.push_str("\tbl _itd_f");
                    writeln!(self.output, "{}", constructor.0).unwrap();
                }
            }
            CallTarget::Runtime(runtime) => {
                let name = &self.program.runtime_functions[runtime.0].name;
                let ty = self.program.runtime_functions[runtime.0]
                    .signature
                    .parameters[0]
                    .ty;
                let suffix = match ty {
                    ValueType::I32 => "i32",
                    ValueType::F64 => "f64",
                    ValueType::Bool => "bool",
                    ValueType::Str => "str",
                    _ => return Err(error("runtime nativo no compatible", span)),
                };
                writeln!(self.output, "\tbl _itd_{}_{}", name, suffix).unwrap();
            }
            CallTarget::BoundMethod(bound) => {
                self.load_x_at(layout.temp(bound), 0, "x16");
                self.load_x_at(layout.temp(bound), 8, "x0");
                self.output.push_str("\tblr x16\n");
            }
        }
        if let Some(result) = result.filter(|result| !forwarded_result(function, position, *result))
        {
            if result.ty == ValueType::F64 {
                self.store_d(layout.temp(result), "d0");
            } else if matches!(result.ty, ValueType::I32 | ValueType::Bool) {
                self.store_w(layout.temp(result), "w0");
            } else if result.ty == ValueType::Str {
                self.consume_string_return(layout.temp(result));
            } else {
                self.store_x(layout.temp(result), "x0");
            }
        }
        Ok(())
    }
    fn compare(
        &mut self,
        function: &Function,
        offset: usize,
        op: &Operation,
        left: &Operand,
        right: &Operand,
        layout: &Layout,
    ) {
        let condition = match op {
            Operation::Eq { .. } => "eq",
            Operation::Ne { .. } => "ne",
            Operation::Lt { .. } => "lt",
            Operation::Le { .. } => "le",
            _ => "gt",
        };
        if left.ty() == ValueType::F64 {
            self.f64_binary_operands(function, offset, left, right, layout);
            self.output.push_str("\tfcmp d16, d17\n");
        } else if matches!(left.ty(), ValueType::I32 | ValueType::Bool) {
            self.i32_binary_operands(function, offset, left, right, layout);
            self.output.push_str("\tcmp w9, w10\n");
        } else {
            self.x_into(right, layout, "x10");
            self.x_into(left, layout, "x9");
            self.output.push_str("\tcmp x9, x10\n");
        }
        writeln!(self.output, "\tcset w9, {condition}").unwrap();
        if left.ty() == ValueType::F64 && matches!(op, Operation::Lt { .. } | Operation::Le { .. })
        {
            // ARM's signed LT/LE accept unordered FCMP flags; IEEE requires false.
            self.output.push_str("\tcset w10, vc\n\tand w9, w9, w10\n");
        }
    }
    fn x(&mut self, operand: &Operand, layout: &Layout) {
        self.x_into(operand, layout, "x9");
    }
    fn x_into(&mut self, operand: &Operand, layout: &Layout, out: &str) {
        let word = out.strip_prefix('x').expect("integer register");
        match operand {
            Operand::I32(v) => writeln!(self.output, "\tmov w{word}, #{v}").unwrap(),
            Operand::Bool(v) => writeln!(self.output, "\tmov w{word}, #{}", u8::from(*v)).unwrap(),
            Operand::String(id) => writeln!(
                self.output,
                "\tadrp {out}, _itd_str{}@PAGE\n\tadd {out}, {out}, _itd_str{}@PAGEOFF",
                id.0, id.0
            )
            .unwrap(),
            Operand::Receiver { .. } => self.load_x_at(layout.receiver(), 0, out),
            Operand::Parameter {
                index,
                ty: ValueType::I32 | ValueType::Bool,
            } => self.load_w(layout.param(*index), &format!("w{word}")),
            Operand::Parameter { index, .. } => self.load_x_at(layout.param(*index), 0, out),
            Operand::Temporary(t) if matches!(t.ty, ValueType::I32 | ValueType::Bool) => {
                self.load_w(layout.temp(*t), &format!("w{word}"))
            }
            Operand::Temporary(t) => self.load_x_at(layout.temp(*t), 0, out),
            Operand::F64(_) => {}
        }
    }
    fn w(&mut self, operand: &Operand, layout: &Layout) {
        self.w_into(operand, layout, "w9");
    }
    fn w_into(&mut self, operand: &Operand, layout: &Layout, out: &str) {
        match operand {
            Operand::I32(value) => self.mov_i32(out, *value),
            Operand::Bool(value) => {
                writeln!(self.output, "\tmov {out}, #{}", u8::from(*value)).unwrap()
            }
            Operand::Temporary(temporary) => self.load_w(layout.temp(*temporary), out),
            Operand::Parameter { index, .. } => self.load_w(layout.param(*index), out),
            _ => self.x_into(operand, layout, &format!("x{}", &out[1..])),
        }
    }
    fn d(&mut self, operand: &Operand, layout: &Layout) {
        self.d_into(operand, layout, "d16");
    }
    fn d_into(&mut self, operand: &Operand, layout: &Layout, out: &str) {
        match operand {
            Operand::F64(value) => {
                let bits = value.to_bits();
                writeln!(self.output, "\tmovz x9, #{}", bits & 0xffff).unwrap();
                writeln!(
                    self.output,
                    "\tmovk x9, #{}, lsl #16",
                    (bits >> 16) & 0xffff
                )
                .unwrap();
                writeln!(
                    self.output,
                    "\tmovk x9, #{}, lsl #32",
                    (bits >> 32) & 0xffff
                )
                .unwrap();
                writeln!(self.output, "\tmovk x9, #{}, lsl #48", bits >> 48).unwrap();
                writeln!(self.output, "\tfmov {out}, x9").unwrap();
            }
            Operand::Temporary(t) => self.load_d(layout.temp(*t), out),
            Operand::Parameter { index, .. } => self.load_d(layout.param(*index), out),
            _ => self.x_into(operand, layout, "x9"),
        }
    }
    fn i32_binary_operands(
        &mut self,
        function: &Function,
        offset: usize,
        left: &Operand,
        right: &Operand,
        layout: &Layout,
    ) {
        if resident_scalar_operand(function, offset, left) {
            self.w_into(right, layout, "w10");
        } else if resident_scalar_operand(function, offset, right) {
            self.output.push_str("\tmov w10, w9\n");
            self.w_into(left, layout, "w9");
        } else {
            self.w_into(right, layout, "w10");
            self.w_into(left, layout, "w9");
        }
    }
    fn f64_binary_operands(
        &mut self,
        function: &Function,
        offset: usize,
        left: &Operand,
        right: &Operand,
        layout: &Layout,
    ) {
        if resident_scalar_operand(function, offset, left) {
            // The producer left its single-use result in d16; loading the
            // right operand directly into d17 preserves operand ordering.
            self.d_into(right, layout, "d17");
        } else if resident_scalar_operand(function, offset, right) {
            // Preserve a previous d16 result before loading the left operand.
            self.output.push_str("\tfmov d17, d16\n");
            self.d_into(left, layout, "d16");
        } else {
            self.d_into(left, layout, "d16");
            self.d_into(right, layout, "d17");
        }
    }
    fn copy(&mut self, source: &Operand, destination: usize, layout: &Layout) {
        if source.ty() == ValueType::F64 {
            self.d(source, layout);
            self.store_d(destination, "d16");
        } else if matches!(source.ty(), ValueType::I32 | ValueType::Bool) {
            self.w(source, layout);
            self.store_w(destination, "w9");
        } else {
            self.x(source, layout);
            if source.ty() == ValueType::Str {
                self.copy_string_register_to_slot("x9", destination);
            } else {
                self.store_x(destination, "x9");
            }
            if matches!(source.ty(), ValueType::BoundMethod(_)) {
                self.bound_receiver(source, layout);
                self.store_x_at(destination, 8, "x9");
            }
        }
    }
    fn copy_slot(&mut self, source: usize, destination: usize, ty: ValueType) {
        match ty {
            ValueType::I32 | ValueType::Bool => {
                self.load_w(source, "w9");
                self.store_w(destination, "w9");
            }
            ValueType::F64 => {
                self.load_d(source, "d16");
                self.store_d(destination, "d16");
            }
            ValueType::Str => {
                self.load_x_at(source, 0, "x9");
                self.copy_string_register_to_slot("x9", destination);
            }
            _ => {
                self.load_x_at(source, 0, "x9");
                self.store_x(destination, "x9");
                if matches!(ty, ValueType::BoundMethod(_)) {
                    self.load_x_at(source, 8, "x9");
                    self.store_x_at(destination, 8, "x9");
                }
            }
        }
    }
    fn bound_receiver(&mut self, operand: &Operand, layout: &Layout) {
        match operand {
            Operand::Temporary(temporary) => self.load_x_at(layout.temp(*temporary), 8, "x9"),
            Operand::Parameter { index, .. } => self.load_x_at(layout.param(*index), 8, "x9"),
            _ => unreachable!("validated bound methods have storage"),
        }
    }
    fn own_string_register(&mut self, source: &str, destination: usize) {
        writeln!(
            self.output,
            "\tmov x10, {source}\n\tmov x0, x10\n\tbl _itd_str_retain"
        )
        .unwrap();
        self.store_x(destination, "x10");
    }
    fn copy_string_register_to_slot(&mut self, source: &str, destination: usize) {
        writeln!(
            self.output,
            "\tmov x10, {source}\n\tmov x0, x10\n\tbl _itd_str_retain"
        )
        .unwrap();
        self.release_string_at(destination);
        self.store_x(destination, "x10");
    }
    fn consume_string_return(&mut self, destination: usize) {
        // A returned dynamic string transfers one owned reference. Keep it in
        // its home before any later call, then restore x0 so an immediately
        // following proven `print(str)` can consume it without reloading.
        // `_itd_str_release` may call `_free`, clobbering x0/x9, but preserves x10.
        self.output.push_str("\tmov x10, x0\n");
        self.release_string_at(destination);
        self.store_x(destination, "x10");
        self.output.push_str("\tmov x0, x10\n");
    }
    fn release_string_at(&mut self, offset: usize) {
        self.load_x_at(offset, 0, "x0");
        self.output.push_str("\tbl _itd_str_release\n");
    }
    fn return_string(&mut self) {
        self.output
            .push_str("\tmov x10, x9\n\tmov x0, x10\n\tbl _itd_str_retain\n\tmov x0, x10\n");
    }
    fn replace_string_static(&mut self, static_: usize) {
        self.output
            .push_str("\tmov x10, x9\n\tmov x0, x10\n\tbl _itd_str_retain\n");
        // `_itd_str_release` may free and uses caller-saved x9, so recover
        // the static address before publishing the retained replacement.
        writeln!(self.output, "\tadrp x9, _itd_static{static_}@PAGE\n\tldr x0, [x9, _itd_static{static_}@PAGEOFF]\n\tbl _itd_str_release\n\tadrp x9, _itd_static{static_}@PAGE\n\tstr x10, [x9, _itd_static{static_}@PAGEOFF]").unwrap();
    }
    fn release_string_statics(&mut self) {
        for static_ in &self.reachability.statics {
            if self.program.statics[*static_].ty == ValueType::Str {
                writeln!(self.output, "\tadrp x9, _itd_static{static_}@PAGE\n\tldr x0, [x9, _itd_static{static_}@PAGEOFF]\n\tbl _itd_str_release").unwrap();
            }
        }
    }
    fn load_x(&mut self, base: &str, offset: usize, out: &str) {
        self.memory("ldr", out, base, offset, 8);
    }
    fn load_x_at(&mut self, offset: usize, at: usize, out: &str) {
        self.memory("ldr", out, "sp", offset + at, 8);
    }
    fn load_w(&mut self, offset: usize, out: &str) {
        self.memory("ldr", out, "sp", offset, 4);
    }
    fn mov_i32(&mut self, register: &str, value: i32) {
        let bits = value as u32;
        writeln!(self.output, "\tmovz {register}, #{}", bits & 0xffff).unwrap();
        if bits >> 16 != 0 {
            writeln!(self.output, "\tmovk {register}, #{}, lsl #16", bits >> 16).unwrap();
        }
    }
    fn load_d(&mut self, offset: usize, out: &str) {
        self.memory("ldr", out, "sp", offset, 8);
    }
    fn store_x(&mut self, offset: usize, value: &str) {
        self.memory("str", value, "sp", offset, 8);
    }
    fn store_w(&mut self, offset: usize, value: &str) {
        self.memory("str", value, "sp", offset, 4);
    }
    fn store_d(&mut self, offset: usize, value: &str) {
        self.memory("str", value, "sp", offset, 8);
    }
    fn store_at(&mut self, base: &str, offset: usize, value: &str) {
        self.memory("str", value, base, offset, 8);
    }
    fn store_x_at(&mut self, offset: usize, at: usize, value: &str) {
        self.memory("str", value, "sp", offset + at, 8);
    }
    fn memory(&mut self, instruction: &str, value: &str, base: &str, offset: usize, scale: usize) {
        if offset <= 4095 * scale {
            writeln!(self.output, "\t{instruction} {value}, [{base}, #{offset}]").unwrap();
        } else {
            self.mov_usize("x15", offset);
            writeln!(
                self.output,
                "\tadd x15, {base}, x15\n\t{instruction} {value}, [x15]"
            )
            .unwrap();
        }
    }
    fn mov_usize(&mut self, register: &str, value: usize) {
        let value = value as u64;
        writeln!(self.output, "\tmovz {register}, #{}", value & 0xffff).unwrap();
        for shift in [16, 32, 48] {
            let part = (value >> shift) & 0xffff;
            if part != 0 {
                writeln!(self.output, "\tmovk {register}, #{part}, lsl #{shift}").unwrap();
            }
        }
    }
    fn adjust_sp(&mut self, amount: usize, add: bool) {
        if amount == 0 {
            return;
        }
        let instruction = if add { "add" } else { "sub" };
        if amount <= 4095 {
            writeln!(self.output, "\t{instruction} sp, sp, #{amount}").unwrap();
        } else if amount.is_multiple_of(4096) && amount / 4096 <= 4095 {
            writeln!(
                self.output,
                "\t{instruction} sp, sp, #{}, lsl #12",
                amount / 4096
            )
            .unwrap();
        } else {
            // x15 is caller-saved and no value is live at either frame boundary.
            self.mov_usize("x15", amount);
            writeln!(self.output, "\t{instruction} sp, sp, x15").unwrap();
        }
    }
    fn runtime(&mut self) -> Result<(), Diagnostic> {
        let mut needs = RuntimeNeeds {
            trap: self.reachability.traps,
            string_release: needs_string_release(self.program, &self.reachability.functions),
            string_retain: needs_string_retain(self.program, &self.reachability.functions),
            ..RuntimeNeeds::default()
        };
        for (id, runtime) in self.program.runtime_functions.iter().enumerate() {
            if !self.reachability.runtimes.contains(&id) {
                continue;
            }
            let ty = runtime.signature.parameters[0].ty;
            match (runtime.name.as_str(), ty) {
                ("print", ValueType::Str) => needs.format_str = true,
                ("print", ValueType::I32) => needs.format_i32 = true,
                ("print", ValueType::F64) => needs.format_f64 = true,
                ("print", ValueType::Bool) => needs.format_i32 = true,
                ("to_string", ValueType::Bool) => needs.bool_strings = true,
                ("to_string", ValueType::I32) => {
                    needs.format_i32 = true;
                    needs.trap = true;
                }
                ("to_string", ValueType::F64) => {
                    needs.format_f64 = true;
                    needs.trap = true;
                }
                ("to_string", ValueType::Str) => {}
                _ => return Err(error("runtime nativo no compatible", runtime.span)),
            }
        }
        if needs.string_retain {
            self.output.push_str(".section __TEXT,__text,regular,pure_instructions\n.p2align 2\n_itd_str_retain:\n\ttbz x0, #1, 1f\n\tmov x10, x0\n\tand x9, x0, #-3\n\tldr x0, [x9, #-16]\n\tadd x0, x0, #1\n\tstr x0, [x9, #-16]\n\tmov x0, x10\n1:\n\tret\n");
        }
        if needs.string_release {
            self.output.push_str(".section __TEXT,__text,regular,pure_instructions\n.p2align 2\n_itd_str_release:\n\ttbz x0, #1, 1f\n\tand x9, x0, #-3\n\tldr x0, [x9, #-16]\n\tsubs x0, x0, #1\n\tstr x0, [x9, #-16]\n\tb.ne 1f\n\tsub x0, x9, #16\n\tstp x10, x30, [sp, #-16]!\n\tbl _free\n\tldp x10, x30, [sp], #16\n\tret\n1:\n\tret\n");
        }
        if needs.trap {
            self.output.push_str(".section __TEXT,__text,regular,pure_instructions\n.p2align 2\n_itd_trap:\n\tmov w0, #2\n\tadrp x1, _itd_trap_message@PAGE\n\tadd x1, x1, _itd_trap_message@PAGEOFF\n\tmov x2, #20\n\tbl _write\n\tmov w0, #1\n\tb _exit\n");
        }
        for (i, s) in self.program.strings.iter().enumerate() {
            if self.reachability.strings.contains(&i) {
                writeln!(
                    self.output,
                    ".section __TEXT,__cstring,cstring_literals\n.p2align 2\n_itd_str{i}:\n\t.asciz \"{}\"",
                    escape(s)
                )
                .unwrap();
            }
        }
        for i in &self.reachability.statics {
            writeln!(
                self.output,
                ".section __DATA,__data\n.p2align 3\n_itd_static{i}:\n\t.quad 0"
            )
            .unwrap();
        }
        for (id, runtime) in self.program.runtime_functions.iter().enumerate() {
            if !self.reachability.runtimes.contains(&id) {
                continue;
            }
            let ty = runtime.signature.parameters[0].ty;
            let suffix = match ty {
                ValueType::I32 => "i32",
                ValueType::F64 => "f64",
                ValueType::Bool => "bool",
                ValueType::Str => "str",
                _ => return Err(error("runtime nativo no compatible", runtime.span)),
            };
            let label = format!("_itd_{}_{}", runtime.name, suffix);
            self.output
                .push_str(".section __TEXT,__text,regular,pure_instructions\n.p2align 2\n");
            match (runtime.name.as_str(), ty) {
                ("print", ValueType::Str | ValueType::I32 | ValueType::Bool | ValueType::F64) => {
                    let setup = match ty {
                        ValueType::Str => "tbnz x0, #1, 1f\n\tb 2f\n1:\n\tand x0, x0, #-3\n2:\n\tmov x1, x0\n\tadrp x0, _itd_fmt_s@PAGE\n\tadd x0, x0, _itd_fmt_s@PAGEOFF\n\tstr x1, [sp]",
                        ValueType::I32 | ValueType::Bool => "mov x1, x0\n\tadrp x0, _itd_fmt_d@PAGE\n\tadd x0, x0, _itd_fmt_d@PAGEOFF\n\tstr x1, [sp]",
                        _ => "adrp x0, _itd_fmt_f@PAGE\n\tadd x0, x0, _itd_fmt_f@PAGEOFF\n\tstr d0, [sp]",
                    };
                    writeln!(self.output, "{label}:\n\tstp x29, x30, [sp, #-16]!\n\tmov x29, sp\n\tsub sp, sp, #16\n\t{setup}\n\tbl _printf\n\tadd sp, sp, #16\n\tldp x29, x30, [sp], #16\n\tret").unwrap();
                }
                ("to_string", ValueType::Str) => writeln!(self.output, "{label}:\n\tb _itd_str_retain").unwrap(),
                ("to_string", ValueType::Bool) => writeln!(self.output, "{label}:\n\tcmp w0, #0\n\tadrp x0, _itd_bool_false@PAGE\n\tadd x0, x0, _itd_bool_false@PAGEOFF\n\tb.eq 1f\n\tadrp x0, _itd_bool_true@PAGE\n\tadd x0, x0, _itd_bool_true@PAGEOFF\n1:\n\tret").unwrap(),
                ("to_string", ValueType::I32 | ValueType::F64) => {
                    let format = if ty == ValueType::I32 { "_itd_fmt_d" } else { "_itd_fmt_f" };
                    let save = if ty == ValueType::I32 { "str w0, [sp, #8]" } else { "str d0, [sp, #8]" };
                    writeln!(self.output, "{label}:\n\tstp x29, x30, [sp, #-16]!\n\tmov x29, sp\n\tsub sp, sp, #32\n\t{save}\n\tmov x0, #80\n\tbl _malloc\n\tcbz x0, _itd_trap\n\tstr x0, [sp, #16]\n\tmov x9, #1\n\tstr x9, [x0]\n\tadd x0, x0, #16\n\tstr x0, [sp, #24]\n\tmov x1, #64\n\tadrp x2, {format}@PAGE\n\tadd x2, x2, {format}@PAGEOFF\n\tldr x3, [sp, #8]\n\tstr x3, [sp]\n\tbl _snprintf\n\ttbnz w0, #31, 1f\n\tcmp w0, #64\n\tb.ge 1f\n\tldr x0, [sp, #24]\n\torr x0, x0, #2\n\tadd sp, sp, #32\n\tldp x29, x30, [sp], #16\n\tret\n1:\n\tldr x0, [sp, #16]\n\tbl _free\n\tb _itd_trap").unwrap();
                }
                _ => return Err(error("runtime nativo no compatible", runtime.span)),
            }
        }
        self.output
            .push_str(".section __TEXT,__cstring,cstring_literals\n");
        if needs.format_str {
            self.output.push_str("_itd_fmt_s: .asciz \"%s\"\n");
        }
        if needs.format_i32 {
            self.output.push_str("_itd_fmt_d: .asciz \"%d\"\n");
        }
        if needs.format_f64 {
            self.output.push_str("_itd_fmt_f: .asciz \"%g\"\n");
        }
        if needs.bool_strings {
            self.output
                .push_str(".p2align 2\n_itd_bool_true: .asciz \"true\"\n.p2align 2\n_itd_bool_false: .asciz \"false\"\n");
        }
        if needs.trap {
            self.output
                .push_str("_itd_trap_message: .asciz \"native runtime trap\\n\"\n");
        }
        Ok(())
    }
}

/// A call result may remain in x0/w0/d0 only when its sole use is a parameter
/// in the immediately following call sequence.  This is a single-basic-block
/// value: any branch, intervening instruction, second use, or pointer result
/// falls back to its stack home before another call can clobber it.
/// Forward a returned string only to the immediate single-argument native
/// `print(str)` call. The result was already stored in its owned home; this
/// predicate merely proves x0 has not been clobbered since then.
fn direct_string_print_argument(
    program: &QuadrupleProgram,
    function: &Function,
    call_offset: usize,
    target: CallTarget,
    value: &Operand,
) -> bool {
    let Operand::Temporary(temporary) = value else {
        return false;
    };
    if temporary.ty != ValueType::Str || !matches!(target, CallTarget::Runtime(_)) {
        return false;
    }
    let Some(first_param) = (0..call_offset)
        .rev()
        .take_while(|offset| matches!(function.code[*offset].operation, Operation::Param { .. }))
        .last()
    else {
        return false;
    };
    if first_param != call_offset - 1 {
        return false;
    }
    let Some(producer) = first_param.checked_sub(1) else {
        return false;
    };
    let Operation::Call {
        target: CallTarget::Runtime(runtime),
        result: Some(result),
        ..
    } = function.code[producer].operation
    else {
        return false;
    };
    result == *temporary
        && program.runtime_functions[runtime.0].name == "to_string"
        && program.runtime_functions[runtime.0].signature.return_type == ValueType::Str
        && matches!(
            target,
            CallTarget::Runtime(print)
                if program.runtime_functions[print.0].name == "print"
                    && program.runtime_functions[print.0].signature.parameters.len() == 1
                    && program.runtime_functions[print.0].signature.parameters[0].ty == ValueType::Str
        )
}

fn forwarded_call_result(function: &Function, call_offset: usize, value: &Operand) -> bool {
    let Operand::Temporary(temporary) = value else {
        return false;
    };
    let Some(first_param) = (0..call_offset)
        .rev()
        .take_while(|offset| matches!(function.code[*offset].operation, Operation::Param { .. }))
        .last()
    else {
        return false;
    };
    let Some(previous) = first_param.checked_sub(1) else {
        return false;
    };
    matches!(
        function.code[previous].operation,
        Operation::Call {
            result: Some(result),
            ..
        } if result == *temporary && forwarded_result(function, previous, result)
    )
}

/// Keep a scalar temporary in its producer register only on a proven
/// single-successor, single-use edge.  The validated IR gives each temporary
/// one definition, while mutable `LocalSlot`s deliberately stay in memory.
/// This makes labels, joins, loops, repeated uses, and all call-clobber edges
/// fall back to the colored stack home.
fn keep_scalar_result(function: &Function, offset: usize, result: Temporary) -> bool {
    if !matches!(result.ty, ValueType::I32 | ValueType::Bool | ValueType::F64)
        || offset + 1 >= function.code.len()
        || !matches!(
            function.code[offset].operation,
            Operation::Add { .. }
                | Operation::Sub { .. }
                | Operation::Mul { .. }
                | Operation::Copy { .. }
                | Operation::LoadLocal { .. }
                | Operation::Div { .. }
                | Operation::Mod { .. }
                | Operation::Neg { .. }
                | Operation::Not { .. }
                | Operation::Eq { .. }
                | Operation::Ne { .. }
                | Operation::Lt { .. }
                | Operation::Le { .. }
                | Operation::Gt { .. }
                | Operation::I32ToF64 { .. }
        )
        || !temporary_defs(&function.code[offset].operation).contains(&result)
        || temporary_successors(function, offset, &label_offsets(function)) != vec![offset + 1]
    {
        return false;
    }
    // Count operand occurrences, not just instructions: `t * t` consumes
    // the same SSA value twice and therefore needs its stack home.
    let uses = function
        .code
        .iter()
        .enumerate()
        .flat_map(|(index, quad)| {
            temporary_uses(&quad.operation)
                .into_iter()
                .filter(move |temporary| *temporary == result)
                .map(move |_| index)
        })
        .collect::<Vec<_>>();
    uses.len() == 1 && uses[0] == offset + 1 && scalar_residency_consumer(function, offset + 1)
}

fn scalar_residency_consumer(function: &Function, offset: usize) -> bool {
    match function.code[offset].operation {
        Operation::Param { .. } => {
            let call = function.code[offset..]
                .iter()
                .find_map(|quad| match quad.operation {
                    Operation::Call { target, .. } => Some(target),
                    Operation::Param { .. } => None,
                    _ => Some(CallTarget::Constructor(crate::quadruple::ClassId(
                        usize::MAX,
                    ))),
                });
            !matches!(call, Some(CallTarget::Constructor(_)))
        }
        Operation::Add { .. }
        | Operation::Sub { .. }
        | Operation::Mul { .. }
        | Operation::Copy { .. }
        | Operation::Div { .. }
        | Operation::Mod { .. }
        | Operation::Neg { .. }
        | Operation::Not { .. }
        | Operation::Eq { .. }
        | Operation::Ne { .. }
        | Operation::Lt { .. }
        | Operation::Le { .. }
        | Operation::Gt { .. }
        | Operation::I32ToF64 { .. } => true,
        _ => false,
    }
}

fn constructor_argument_consumer(function: &Function, first_param: usize) -> bool {
    for quad in &function.code[first_param..] {
        match quad.operation {
            Operation::Param { .. } => {}
            Operation::Call { target, .. } => return matches!(target, CallTarget::Constructor(_)),
            _ => return false,
        }
    }
    false
}

fn label_offsets(function: &Function) -> BTreeMap<quadruple::LabelId, usize> {
    function
        .code
        .iter()
        .enumerate()
        .filter_map(|(offset, quad)| match quad.operation {
            Operation::Label(label) => Some((label, offset)),
            _ => None,
        })
        .collect()
}

fn resident_scalar_operand(function: &Function, offset: usize, operand: &Operand) -> bool {
    matches!(operand, Operand::Temporary(temporary) if offset > 0 && keep_scalar_result(function, offset - 1, *temporary))
}

fn resident_call_argument(function: &Function, call_offset: usize, operand: &Operand) -> bool {
    let Operand::Temporary(temporary) = operand else {
        return false;
    };
    let Some(first_param) = (0..call_offset)
        .rev()
        .take_while(|offset| matches!(function.code[*offset].operation, Operation::Param { .. }))
        .last()
    else {
        return false;
    };
    first_param
        .checked_sub(1)
        .is_some_and(|producer| keep_scalar_result(function, producer, *temporary))
}

fn forwarded_result(function: &Function, call_offset: usize, result: Temporary) -> bool {
    if !matches!(result.ty, ValueType::I32 | ValueType::Bool | ValueType::F64) {
        return false;
    }
    let uses: Vec<_> = function
        .code
        .iter()
        .enumerate()
        .filter_map(|(offset, quad)| {
            temporary_uses(&quad.operation)
                .into_iter()
                .any(|temporary| temporary.index == result.index)
                .then_some(offset)
        })
        .collect();
    uses.len() == 1
        && uses[0] > call_offset
        && !constructor_argument_consumer(function, uses[0])
        && matches!(
            function.code[uses[0]].operation,
            Operation::Param {
                value: Operand::Temporary(temporary),
                ..
            } if temporary == result
        )
        && function.code[call_offset + 1..uses[0]]
            .iter()
            .all(|quad| matches!(quad.operation, Operation::Param { .. }))
}

#[derive(Default)]
struct RuntimeNeeds {
    format_str: bool,
    format_i32: bool,
    format_f64: bool,
    bool_strings: bool,
    string_retain: bool,
    string_release: bool,
    trap: bool,
}

fn needs_string_release(program: &QuadrupleProgram, functions: &BTreeSet<usize>) -> bool {
    functions.iter().any(|id| {
        let function = &program.functions[*id];
        function.signature.return_type == ValueType::Str
            || function
                .signature
                .parameters
                .iter()
                .any(|parameter| parameter.ty == ValueType::Str)
            || function.locals.iter().any(|local| local.ty == ValueType::Str)
            || function.code.iter().any(|quad| {
                temporary_uses(&quad.operation)
                    .into_iter()
                    .chain(temporary_defs(&quad.operation))
                    .any(|temporary| temporary.ty == ValueType::Str)
                    || matches!(quad.operation, Operation::SetStatic { value, .. } if value.ty() == ValueType::Str)
            })
    })
}

/// Retain is only linked for emitted aliasing, parameter, return, or static
/// replacement paths. Numeric `to_string -> print` still links release because
/// its returned tagged pointer has an owned home, but it does not alias.
fn needs_string_retain(program: &QuadrupleProgram, functions: &BTreeSet<usize>) -> bool {
    functions.iter().any(|id| {
        let function = &program.functions[*id];
        function.signature.return_type == ValueType::Str
            || function
                .signature
                .parameters
                .iter()
                .any(|parameter| parameter.ty == ValueType::Str)
            || function.code.iter().any(|quad| match &quad.operation {
                Operation::Copy { source, .. } | Operation::StoreLocal { value: source, .. }
                    if source.ty() == ValueType::Str =>
                {
                    true
                }
                Operation::LoadLocal { local, .. } if local.ty == ValueType::Str => true,
                Operation::GetStatic { result, .. } if result.ty == ValueType::Str => true,
                Operation::SetStatic { value, .. } if value.ty() == ValueType::Str => true,
                Operation::Return(Some(value)) if value.ty() == ValueType::Str => true,
                Operation::Call {
                    target: CallTarget::Runtime(runtime),
                    ..
                } if program.runtime_functions[runtime.0].name == "to_string"
                    && program.runtime_functions[runtime.0].signature.parameters[0].ty
                        == ValueType::Str =>
                {
                    true
                }
                _ => false,
            })
    })
}

fn preflight(program: &QuadrupleProgram, functions: &BTreeSet<usize>) -> Result<(), Diagnostic> {
    for id in functions {
        let function = &program.functions[*id];
        supported_signature(&function.signature, function.span)?;
        Layout::new(function)?;
        for quadruple in &function.code {
            match quadruple.operation {
                Operation::GetField { field, .. } | Operation::SetField { field, .. } => {
                    if program.fields[field.0].ty == ValueType::Str {
                        return Err(error(
                            "los campos string no tienen liberación de objetos nativa",
                            quadruple.span,
                        ));
                    }
                    field.0.checked_mul(8).ok_or_else(|| {
                        error(
                            "el desplazamiento de campo nativo excede el tamaño direccionable",
                            quadruple.span,
                        )
                    })?;
                }
                Operation::Call {
                    target: CallTarget::Constructor(class),
                    ..
                } => {
                    program.classes[class.0]
                        .fields
                        .len()
                        .max(1)
                        .checked_mul(8)
                        .ok_or_else(|| {
                            error(
                                "el tamaño de objeto nativo excede el tamaño direccionable",
                                quadruple.span,
                            )
                        })?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn supported_signature(
    signature: &crate::quadruple::Signature,
    span: Span,
) -> Result<(), Diagnostic> {
    if matches!(signature.return_type, ValueType::BoundMethod(_))
        || signature
            .parameters
            .iter()
            .any(|parameter| matches!(parameter.ty, ValueType::BoundMethod(_)))
    {
        return Err(error(
            "los parámetros y retornos BoundMethod no son compatibles con el ABI nativo",
            span,
        ));
    }
    let integer = signature
        .parameters
        .iter()
        .filter(|parameter| parameter.ty != ValueType::F64)
        .count()
        + usize::from(signature.receiver.is_some());
    let float = signature
        .parameters
        .iter()
        .filter(|parameter| parameter.ty == ValueType::F64)
        .count();
    if integer > 8 {
        return Err(error(
            "más de ocho argumentos enteros/puntero nativos no están soportados",
            span,
        ));
    }
    if float > 8 {
        return Err(error(
            "más de ocho argumentos f64 nativos no están soportados",
            span,
        ));
    }
    Ok(())
}

struct Layout {
    temps: BTreeMap<usize, usize>,
    locals: BTreeMap<usize, usize>,
    params: BTreeMap<usize, usize>,
    receiver: Option<usize>,
    call_scratch: Option<usize>,
    string_slots: BTreeSet<usize>,
    return_spill: Option<usize>,
    frame: usize,
    needs_frame_record: bool,
}
impl Layout {
    fn new(function: &Function) -> Result<Self, Diagnostic> {
        let mut cursor = 0usize;
        // Temporaries are SSA values.  Color their CFG liveness interference
        // graph rather than reserving one stack home per temporary.  This is
        // deliberately stack-only: caller-saved registers remain scratch space
        // and every call is therefore an explicit register-clobber barrier.
        let temporary_slots = temporary_slots(function)?;
        let mut physical_slots = Vec::with_capacity(temporary_slots.requirements.len());
        for (size, alignment) in &temporary_slots.requirements {
            physical_slots.push(allocate_slot(
                &mut cursor,
                *size,
                *alignment,
                function.span,
            )?);
        }
        let temp_slots: BTreeMap<usize, usize> = temporary_slots
            .assignments
            .into_iter()
            .map(|(index, slot)| (index, physical_slots[slot]))
            .collect();
        let mut locals = BTreeMap::new();
        for local in &function.locals {
            let (size, alignment) = slot_requirements(local.ty);
            locals.insert(
                local.index,
                allocate_slot(&mut cursor, size, alignment, function.span)?,
            );
        }
        let mut params = BTreeMap::new();
        for parameter in &function.signature.parameters {
            let (size, alignment) = slot_requirements(parameter.ty);
            params.insert(
                parameter.index,
                allocate_slot(&mut cursor, size, alignment, function.span)?,
            );
        }
        let receiver = function
            .signature
            .receiver
            .map(|class| {
                let (size, alignment) = slot_requirements(ValueType::Object(class));
                allocate_slot(&mut cursor, size, alignment, function.span)
            })
            .transpose()?;
        let has_constructor_call = function.code.iter().any(|quad| {
            matches!(
                quad.operation,
                Operation::Call {
                    target: CallTarget::Constructor(_),
                    ..
                }
            )
        });
        let call_scratch = has_constructor_call
            .then(|| {
                let (size, alignment) =
                    slot_requirements(ValueType::Object(crate::quadruple::ClassId(0)));
                allocate_slot(&mut cursor, size, alignment, function.span)
            })
            .transpose()?;
        let mut string_slots = BTreeSet::new();
        for temporary in function.code.iter().flat_map(|quad| {
            temporary_uses(&quad.operation)
                .into_iter()
                .chain(temporary_defs(&quad.operation))
        }) {
            if temporary.ty == ValueType::Str {
                string_slots.insert(temp_slots[&temporary.index]);
            }
        }
        for local in &function.locals {
            if local.ty == ValueType::Str {
                string_slots.insert(locals[&local.index]);
            }
        }
        for parameter in &function.signature.parameters {
            if parameter.ty == ValueType::Str {
                string_slots.insert(params[&parameter.index]);
            }
        }
        let return_spill = (function.signature.return_type == ValueType::F64
            && !string_slots.is_empty())
        .then(|| allocate_slot(&mut cursor, 8, 8, function.span))
        .transpose()?;
        let frame = align_up(cursor, 16, function.span)?;
        let needs_frame_record = function
            .signature
            .parameters
            .iter()
            .any(|parameter| parameter.ty == ValueType::Str)
            || function.code.iter().any(|quad| {
                matches!(quad.operation, Operation::Call { .. })
                    || operation_operands(&quad.operation)
                        .iter()
                        .any(|operand| operand.ty() == ValueType::Str)
                    || temporary_defs(&quad.operation)
                        .iter()
                        .any(|temporary| temporary.ty == ValueType::Str)
            });
        Ok(Self {
            temps: temp_slots,
            locals,
            params,
            receiver,
            call_scratch,
            string_slots,
            return_spill,
            frame,
            needs_frame_record,
        })
    }
    fn temp(&self, t: Temporary) -> usize {
        self.temps[&t.index]
    }
    fn local(&self, index: usize) -> usize {
        self.locals[&index]
    }
    fn param(&self, index: usize) -> usize {
        self.params[&index]
    }
    fn receiver(&self) -> usize {
        self.receiver
            .expect("receiver storage required by validated IR")
    }
    fn call_scratch(&self) -> usize {
        self.call_scratch
            .expect("constructor call scratch required")
    }
}

#[derive(Default)]
struct TemporarySlots {
    assignments: BTreeMap<usize, usize>,
    requirements: Vec<(usize, usize)>,
    slot_types: Vec<ValueType>,
}

fn temporary_slots(function: &Function) -> Result<TemporarySlots, Diagnostic> {
    let mut types = BTreeMap::new();
    let mut labels = BTreeMap::new();
    for (offset, quad) in function.code.iter().enumerate() {
        if let Operation::Label(label) = quad.operation {
            labels.insert(label, offset);
        }
        for temporary in temporary_uses(&quad.operation)
            .into_iter()
            .chain(temporary_defs(&quad.operation))
        {
            types.insert(temporary.index, temporary.ty);
        }
    }
    let len = function.code.len();
    let mut live_in = vec![BTreeSet::new(); len];
    let mut live_out = vec![BTreeSet::new(); len];
    loop {
        let mut changed = false;
        for offset in (0..len).rev() {
            let outgoing = temporary_successors(function, offset, &labels)
                .into_iter()
                .flat_map(|successor| live_in[successor].iter().copied())
                .collect::<BTreeSet<_>>();
            let definitions = temporary_defs(&function.code[offset].operation)
                .into_iter()
                .map(|temporary| temporary.index)
                .collect::<BTreeSet<_>>();
            let incoming = temporary_uses(&function.code[offset].operation)
                .into_iter()
                .map(|temporary| temporary.index)
                .chain(
                    outgoing
                        .iter()
                        .filter(|temporary| !definitions.contains(temporary))
                        .copied(),
                )
                .collect::<BTreeSet<_>>();
            if live_in[offset] != incoming || live_out[offset] != outgoing {
                live_in[offset] = incoming;
                live_out[offset] = outgoing;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut conflicts: BTreeMap<usize, BTreeSet<usize>> = types
        .keys()
        .map(|index| (*index, BTreeSet::new()))
        .collect();
    for (offset, quad) in function.code.iter().enumerate() {
        for defined in temporary_defs(&quad.operation) {
            for live in &live_out[offset] {
                if *live != defined.index {
                    conflicts.entry(defined.index).or_default().insert(*live);
                    conflicts.entry(*live).or_default().insert(defined.index);
                }
            }
        }
    }

    let mut slots = TemporarySlots::default();
    for (temporary, ty) in types {
        let (size, alignment) = slot_requirements(ty);
        let selected = slots
            .requirements
            .iter()
            .enumerate()
            .find_map(|(slot, requirement)| {
                ((ty == ValueType::Str) == (slots.slot_types[slot] == ValueType::Str)
                    && requirement.0 >= size
                    && requirement.1 >= alignment
                    && slots.assignments.iter().all(|(other, assigned)| {
                        *assigned != slot || !conflicts[&temporary].contains(other)
                    }))
                .then_some(slot)
            });
        let slot = selected.unwrap_or_else(|| {
            slots.requirements.push((size, alignment));
            slots.slot_types.push(ty);
            slots.requirements.len() - 1
        });
        slots.assignments.insert(temporary, slot);
    }
    Ok(slots)
}

fn temporary_successors(
    function: &Function,
    offset: usize,
    labels: &BTreeMap<quadruple::LabelId, usize>,
) -> Vec<usize> {
    let next = offset + 1;
    match function.code[offset].operation {
        Operation::Jump(label) => vec![labels[&label]],
        Operation::JumpIfFalse { target, .. } => vec![labels[&target], next],
        Operation::Return(_) | Operation::Halt | Operation::Trap(_) => Vec::new(),
        _ if next < function.code.len() => vec![next],
        _ => Vec::new(),
    }
}

fn temporary_uses(operation: &Operation) -> Vec<Temporary> {
    operation_operands(operation)
        .into_iter()
        .filter_map(|operand| match operand {
            Operand::Temporary(temporary) => Some(temporary),
            _ => None,
        })
        .collect()
}

fn temporary_defs(operation: &Operation) -> Vec<Temporary> {
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

fn allocate_slot(
    cursor: &mut usize,
    size: usize,
    alignment: usize,
    span: Span,
) -> Result<usize, Diagnostic> {
    *cursor = align_up(*cursor, alignment, span)?;
    let offset = *cursor;
    *cursor = cursor.checked_add(size).ok_or_else(|| {
        error(
            "el diseño de frame nativo excede el tamaño direccionable",
            span,
        )
    })?;
    Ok(offset)
}

fn slot_requirements(ty: ValueType) -> (usize, usize) {
    match ty {
        ValueType::I32 | ValueType::Bool => (4, 4),
        ValueType::BoundMethod(_) => (16, 8),
        ValueType::F64 | ValueType::Str | ValueType::Object(_) => (8, 8),
        ValueType::Void => (0, 1),
    }
}

fn align_up(value: usize, alignment: usize, span: Span) -> Result<usize, Diagnostic> {
    value
        .checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or_else(|| {
            error(
                "el diseño de frame nativo excede el tamaño direccionable",
                span,
            )
        })
}
fn escape(string: &str) -> String {
    string
        .bytes()
        .map(|b| match b {
            b'\\' => "\\\\".to_owned(),
            b'\"' => "\\\"".to_owned(),
            b'\n' => "\\n".to_owned(),
            b'\r' => "\\r".to_owned(),
            b'\t' => "\\t".to_owned(),
            32..=126 => (b as char).to_string(),
            _ => format!("\\{:03o}", b),
        })
        .collect()
}
fn fallback(program: &QuadrupleProgram) -> Span {
    program
        .functions
        .first()
        .map(|f| f.span)
        .unwrap_or(Span::new(SourceId(0), 0, 0))
}
fn error(message: &str, span: Span) -> Diagnostic {
    Diagnostic::error(ERROR, message, span)
}

#[cfg(test)]
mod residency_tests {
    use super::*;

    fn function(code: Vec<Operation>) -> Function {
        let span = Span::new(SourceId(0), 0, 0);
        Function {
            name: "test".into(),
            signature: crate::quadruple::Signature {
                parameters: vec![],
                return_type: ValueType::Void,
                receiver: None,
            },
            locals: vec![],
            code: code
                .into_iter()
                .map(|operation| quadruple::Quadruple { operation, span })
                .collect(),
            span,
        }
    }

    #[test]
    fn f64_residency_requires_an_arithmetic_producer_and_global_single_use() {
        let value = Temporary {
            index: 0,
            ty: ValueType::F64,
        };
        let next = Temporary {
            index: 1,
            ty: ValueType::F64,
        };
        let repeated = function(vec![
            Operation::Add {
                left: Operand::F64(1.0),
                right: Operand::F64(2.0),
                result: value,
            },
            Operation::Mul {
                left: Operand::Temporary(value),
                right: Operand::F64(3.0),
                result: next,
            },
            Operation::Add {
                left: Operand::Temporary(value),
                right: Operand::F64(4.0),
                result: Temporary {
                    index: 2,
                    ty: ValueType::F64,
                },
            },
        ]);
        assert!(!keep_scalar_result(&repeated, 0, value));

        let call = function(vec![
            Operation::Call {
                call: crate::quadruple::CallSiteId(0),
                target: CallTarget::Function(FunctionId(0)),
                result: Some(value),
            },
            Operation::Add {
                left: Operand::Temporary(value),
                right: Operand::F64(2.0),
                result: next,
            },
        ]);
        assert!(!keep_scalar_result(&call, 0, value));
    }

    #[test]
    fn scalar_loads_and_copies_are_resident_only_on_their_single_linear_use() {
        let loaded = Temporary {
            index: 0,
            ty: ValueType::I32,
        };
        let copied = Temporary {
            index: 1,
            ty: ValueType::I32,
        };
        let result = Temporary {
            index: 2,
            ty: ValueType::I32,
        };
        let linear = function(vec![
            Operation::LoadLocal {
                local: crate::quadruple::LocalSlot {
                    index: 0,
                    ty: ValueType::I32,
                },
                result: loaded,
            },
            Operation::Copy {
                source: Operand::Temporary(loaded),
                result: copied,
            },
            Operation::Add {
                left: Operand::Temporary(copied),
                right: Operand::I32(1),
                result,
            },
        ]);
        assert!(keep_scalar_result(&linear, 0, loaded));
        assert!(keep_scalar_result(&linear, 1, copied));

        let joined = function(vec![
            Operation::LoadLocal {
                local: crate::quadruple::LocalSlot {
                    index: 0,
                    ty: ValueType::I32,
                },
                result: loaded,
            },
            Operation::Label(crate::quadruple::LabelId(0)),
            Operation::Add {
                left: Operand::Temporary(loaded),
                right: Operand::I32(1),
                result,
            },
        ]);
        assert!(!keep_scalar_result(&joined, 0, loaded));
    }

    #[test]
    fn residency_counts_each_repeated_operand_occurrence() {
        for ty in [ValueType::I32, ValueType::F64] {
            let first = Temporary { index: 0, ty };
            let second = Temporary { index: 1, ty };
            let one = if ty == ValueType::I32 {
                Operand::I32(1)
            } else {
                Operand::F64(1.0)
            };
            let repeated = function(vec![
                Operation::Add {
                    left: Operand::Parameter { index: 0, ty },
                    right: one,
                    result: first,
                },
                Operation::Mul {
                    left: Operand::Temporary(first),
                    right: Operand::Temporary(first),
                    result: second,
                },
            ]);
            assert!(!keep_scalar_result(&repeated, 0, first));
        }
    }
}
