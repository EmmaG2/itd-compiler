use crate::{
    diagnostic::Diagnostic,
    source::{SourceId, Span},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeFunctionId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClassId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FieldId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StaticId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LabelId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallSiteId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StringId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Temporary {
    pub index: usize,
    pub ty: ValueType,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ValueType {
    I32,
    F64,
    Bool,
    Str,
    Object(ClassId),
    BoundMethod(FunctionId),
    Void,
}

impl ValueType {
    fn is_numeric(self) -> bool {
        matches!(self, Self::I32 | Self::F64)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Operand {
    I32(i32),
    F64(f64),
    Bool(bool),
    String(StringId),
    Receiver { class: ClassId },
    Parameter { index: usize, ty: ValueType },
    Temporary(Temporary),
}

impl Operand {
    #[must_use]
    pub fn ty(self) -> ValueType {
        match self {
            Self::I32(_) => ValueType::I32,
            Self::F64(_) => ValueType::F64,
            Self::Bool(_) => ValueType::Bool,
            Self::String(_) => ValueType::Str,
            Self::Receiver { class } => ValueType::Object(class),
            Self::Parameter { ty, .. } => ty,
            Self::Temporary(temporary) => temporary.ty,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub index: usize,
    pub ty: ValueType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Signature {
    pub parameters: Vec<Parameter>,
    pub return_type: ValueType,
    pub receiver: Option<ClassId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    pub owner: ClassId,
    pub ty: ValueType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Static {
    pub owner: ClassId,
    pub ty: ValueType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Class {
    pub fields: Vec<FieldId>,
    pub statics: Vec<StaticId>,
    /// The constructor owns instance initialization and is selected by class ID.
    pub constructor: Option<FunctionId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeFunction {
    pub name: String,
    pub signature: Signature,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalSlot {
    pub index: usize,
    pub ty: ValueType,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub signature: Signature,
    /// Function-scoped local slots. Parameters that are materialized as slots
    /// must also appear here and are initialized at function entry.
    pub locals: Vec<LocalSlot>,
    pub code: Vec<Quadruple>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuadrupleProgram {
    pub main: FunctionId,
    pub functions: Vec<Function>,
    pub runtime_functions: Vec<RuntimeFunction>,
    pub strings: Vec<String>,
    pub classes: Vec<Class>,
    pub fields: Vec<Field>,
    pub statics: Vec<Static>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quadruple {
    pub operation: Operation,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Add {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Sub {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Mul {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Div {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Mod {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Pow {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Neg {
        operand: Operand,
        result: Temporary,
    },
    Not {
        operand: Operand,
        result: Temporary,
    },
    Eq {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Ne {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Lt {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Le {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Gt {
        left: Operand,
        right: Operand,
        result: Temporary,
    },
    Copy {
        source: Operand,
        result: Temporary,
    },
    LoadLocal {
        local: LocalSlot,
        result: Temporary,
    },
    StoreLocal {
        local: LocalSlot,
        value: Operand,
    },
    I32ToF64 {
        source: Operand,
        result: Temporary,
    },
    Label(LabelId),
    Jump(LabelId),
    JumpIfFalse {
        condition: Operand,
        target: LabelId,
    },
    Param {
        call: CallSiteId,
        index: usize,
        value: Operand,
    },
    Call {
        call: CallSiteId,
        target: CallTarget,
        result: Option<Temporary>,
    },
    Return(Option<Operand>),
    GetField {
        owner: Operand,
        field: FieldId,
        result: Temporary,
    },
    SetField {
        owner: Operand,
        field: FieldId,
        value: Operand,
    },
    GetStatic {
        static_: StaticId,
        result: Temporary,
    },
    SetStatic {
        static_: StaticId,
        value: Operand,
    },
    BindMethod {
        receiver: Operand,
        method: FunctionId,
        result: Temporary,
    },
    Halt,
    Trap(Diagnostic),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallTarget {
    Function(FunctionId),
    /// Allocation is implicit in this typed target; the emitter must use the
    /// class declaration's layout and constructor, never a textual runtime name.
    Constructor(ClassId),
    Runtime(RuntimeFunctionId),
    BoundMethod(Temporary),
}

pub fn validate(program: &QuadrupleProgram) -> Result<(), Diagnostic> {
    let fallback = Span::new(SourceId(0), 0, 0);
    if program.main.0 >= program.functions.len() || program.functions.is_empty() {
        return Err(invalid(fallback));
    }
    validate_declarations(program, fallback)?;
    for (id, function) in program.functions.iter().enumerate() {
        validate_function(program, FunctionId(id), function)?;
    }
    Ok(())
}

fn validate_declarations(program: &QuadrupleProgram, fallback: Span) -> Result<(), Diagnostic> {
    for (id, class) in program.classes.iter().enumerate() {
        if let Some(constructor) = class.constructor {
            let Some(constructor) = program.functions.get(constructor.0) else {
                return Err(invalid(class.span));
            };
            if constructor.signature.receiver != Some(ClassId(id))
                || constructor.signature.return_type != ValueType::Void
            {
                return Err(invalid(class.span));
            }
        }
        for field in &class.fields {
            if !program
                .fields
                .get(field.0)
                .is_some_and(|entry| entry.owner == ClassId(id))
            {
                return Err(invalid(class.span));
            }
        }
        for static_ in &class.statics {
            if !program
                .statics
                .get(static_.0)
                .is_some_and(|entry| entry.owner == ClassId(id))
            {
                return Err(invalid(class.span));
            }
        }
    }
    for function in &program.functions {
        validate_signature(program, &function.signature, function.span)?;
    }
    for function in &program.runtime_functions {
        validate_signature(program, &function.signature, function.span)?;
        if function.signature.receiver.is_some() {
            return Err(invalid(function.span));
        }
    }
    let main = &program.functions[program.main.0];
    if main.signature.receiver.is_some()
        || !main.signature.parameters.is_empty()
        || main.signature.return_type != ValueType::Void
    {
        return Err(invalid(main.span));
    }
    if program.classes.len() > usize::MAX - program.fields.len() {
        return Err(invalid(fallback));
    }
    Ok(())
}

fn validate_signature(
    program: &QuadrupleProgram,
    signature: &Signature,
    span: Span,
) -> Result<(), Diagnostic> {
    if !valid_type(program, signature.return_type)
        || signature
            .receiver
            .is_some_and(|class| class.0 >= program.classes.len())
        || signature
            .parameters
            .iter()
            .enumerate()
            .any(|(index, parameter)| {
                parameter.index != index || !valid_type(program, parameter.ty)
            })
    {
        return Err(invalid(span));
    }
    Ok(())
}

fn valid_type(program: &QuadrupleProgram, ty: ValueType) -> bool {
    match ty {
        ValueType::Object(class) => class.0 < program.classes.len(),
        ValueType::BoundMethod(function) => function.0 < program.functions.len(),
        ValueType::I32 | ValueType::F64 | ValueType::Bool | ValueType::Str | ValueType::Void => {
            true
        }
    }
}

fn validate_function(
    program: &QuadrupleProgram,
    id: FunctionId,
    function: &Function,
) -> Result<(), Diagnostic> {
    if function.code.is_empty() {
        return Err(invalid(function.span));
    }
    let labels = labels(function)?;
    let local_types = local_types(program, function)?;
    let temporary_types = temporary_types(program, function)?;
    validate_operations(
        program,
        id,
        function,
        &labels,
        &local_types,
        &temporary_types,
    )?;
    validate_definite_definitions(program, function, &labels, &temporary_types, &local_types)
}

fn labels(function: &Function) -> Result<BTreeMap<LabelId, usize>, Diagnostic> {
    let mut labels = BTreeMap::new();
    for (offset, quadruple) in function.code.iter().enumerate() {
        if let Operation::Label(label) = quadruple.operation
            && labels.insert(label, offset).is_some()
        {
            return Err(invalid(quadruple.span));
        }
    }
    Ok(labels)
}

fn local_types(
    program: &QuadrupleProgram,
    function: &Function,
) -> Result<BTreeMap<usize, ValueType>, Diagnostic> {
    let mut types = BTreeMap::new();
    for local in &function.locals {
        if local.ty == ValueType::Void
            || !valid_type(program, local.ty)
            || types.insert(local.index, local.ty).is_some()
        {
            return Err(invalid(function.span));
        }
    }
    Ok(types)
}

fn temporary_types(
    program: &QuadrupleProgram,
    function: &Function,
) -> Result<BTreeMap<usize, ValueType>, Diagnostic> {
    let mut types = BTreeMap::new();
    for quadruple in &function.code {
        for temporary in operands(&quadruple.operation)
            .into_iter()
            .filter_map(|operand| match operand {
                Operand::Temporary(temporary) => Some(temporary),
                _ => None,
            })
            .chain(results(&quadruple.operation))
        {
            if !valid_type(program, temporary.ty)
                || types
                    .insert(temporary.index, temporary.ty)
                    .is_some_and(|previous| previous != temporary.ty)
            {
                return Err(invalid(quadruple.span));
            }
        }
    }
    Ok(types)
}

fn validate_operations(
    program: &QuadrupleProgram,
    id: FunctionId,
    function: &Function,
    labels: &BTreeMap<LabelId, usize>,
    local_types: &BTreeMap<usize, ValueType>,
    temporary_types: &BTreeMap<usize, ValueType>,
) -> Result<(), Diagnostic> {
    let mut definitions = BTreeSet::new();
    for (offset, quadruple) in function.code.iter().enumerate() {
        let operation = &quadruple.operation;
        for operand in operands(operation) {
            validate_operand(program, function, operand, temporary_types, quadruple.span)?;
            if matches!(operand, Operand::String(id) if id.0 >= program.strings.len()) {
                return Err(invalid(quadruple.span));
            }
        }
        for result in results(operation) {
            if !definitions.insert(result.index) {
                return Err(invalid(quadruple.span));
            }
        }
        match operation {
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
            }
            | Operation::Div {
                left,
                right,
                result,
            }
            | Operation::Mod {
                left,
                right,
                result,
            }
            | Operation::Pow {
                left,
                right,
                result,
            } if left.ty() == right.ty() && left.ty().is_numeric() && result.ty == left.ty() => {}
            Operation::Neg { operand, result }
                if operand.ty().is_numeric() && result.ty == operand.ty() => {}
            Operation::Not { operand, result }
                if operand.ty() == ValueType::Bool && result.ty == ValueType::Bool => {}
            Operation::Eq {
                left,
                right,
                result,
            }
            | Operation::Ne {
                left,
                right,
                result,
            } if left.ty() == right.ty() && result.ty == ValueType::Bool => {}
            Operation::Lt {
                left,
                right,
                result,
            }
            | Operation::Le {
                left,
                right,
                result,
            }
            | Operation::Gt {
                left,
                right,
                result,
            } if left.ty() == right.ty()
                && left.ty().is_numeric()
                && result.ty == ValueType::Bool => {}
            Operation::Copy { source, result } if source.ty() == result.ty => {}
            Operation::LoadLocal { local, result }
                if local_types.get(&local.index) == Some(&local.ty) && local.ty == result.ty => {}
            Operation::StoreLocal { local, value }
                if local_types.get(&local.index) == Some(&local.ty) && local.ty == value.ty() => {}
            Operation::I32ToF64 { source, result }
                if source.ty() == ValueType::I32 && result.ty == ValueType::F64 => {}
            Operation::Label(_) | Operation::Trap(_) => {}
            Operation::Halt if id == program.main => {}
            Operation::Jump(target) if labels.contains_key(target) => {}
            Operation::JumpIfFalse { condition, target }
                if condition.ty() == ValueType::Bool && labels.contains_key(target) => {}
            Operation::Param { .. } => {}
            Operation::Call {
                call,
                target,
                result,
            } => validate_call(program, function, offset, *call, *target, *result)?,
            Operation::Return(value) => validate_return(function, *value, id == program.main)?,
            Operation::GetField {
                owner,
                field,
                result,
            } => validate_field_get(program, *owner, *field, *result)?,
            Operation::SetField {
                owner,
                field,
                value,
            } => validate_field_set(program, *owner, *field, *value)?,
            Operation::GetStatic { static_, result } => {
                validate_static_get(program, *static_, *result)?
            }
            Operation::SetStatic { static_, value } => {
                validate_static_set(program, *static_, *value)?
            }
            Operation::BindMethod {
                receiver,
                method,
                result,
            } => validate_bind_method(program, *receiver, *method, *result)?,
            _ => return Err(invalid(quadruple.span)),
        }
    }
    // Reachable fallthrough is rejected by the CFG traversal; unreachable trailing
    // labels or statements do not invalidate a function whose paths terminate.
    Ok(())
}

fn validate_operand(
    program: &QuadrupleProgram,
    function: &Function,
    operand: Operand,
    temporary_types: &BTreeMap<usize, ValueType>,
    span: Span,
) -> Result<(), Diagnostic> {
    let valid = match operand {
        Operand::I32(_) | Operand::F64(_) | Operand::Bool(_) | Operand::String(_) => true,
        Operand::Receiver { class } => function.signature.receiver == Some(class),
        Operand::Parameter { index, ty } => function
            .signature
            .parameters
            .get(index)
            .is_some_and(|parameter| parameter.ty == ty),
        Operand::Temporary(temporary) => {
            temporary_types.get(&temporary.index) == Some(&temporary.ty)
        }
    } && valid_type(program, operand.ty());
    valid.then_some(()).ok_or_else(|| invalid(span))
}

fn validate_call(
    program: &QuadrupleProgram,
    function: &Function,
    offset: usize,
    call: CallSiteId,
    target: CallTarget,
    result: Option<Temporary>,
) -> Result<(), Diagnostic> {
    let (receiver, signature) =
        call_signature(program, target).ok_or_else(|| invalid(function.code[offset].span))?;
    let parameter_count = signature.parameters.len() + usize::from(receiver.is_some());
    let start = offset
        .checked_sub(parameter_count)
        .ok_or_else(|| invalid(function.code[offset].span))?;
    let parameters = &function.code[start..offset];
    if parameters.iter().enumerate().any(|(index, quadruple)| {
        !matches!(&quadruple.operation, Operation::Param { call: parameter_call, index: parameter_index, value }
            if *parameter_call == call
                && *parameter_index == index
                && call_parameter_type(receiver, signature, index) == Some(value.ty()))
    }) {
        return Err(invalid(function.code[offset].span));
    }
    let return_type = match target {
        CallTarget::Constructor(class) => ValueType::Object(class),
        _ => signature.return_type,
    };
    match (return_type, result) {
        (ValueType::Void, None) => Ok(()),
        (return_type, Some(result)) if return_type == result.ty => Ok(()),
        _ => Err(invalid(function.code[offset].span)),
    }
}

fn call_signature(
    program: &QuadrupleProgram,
    target: CallTarget,
) -> Option<(Option<ValueType>, &Signature)> {
    match target {
        CallTarget::Function(id) => program.functions.get(id.0).map(|function| {
            (
                function.signature.receiver.map(ValueType::Object),
                &function.signature,
            )
        }),
        CallTarget::Constructor(class) => program.classes.get(class.0).and_then(|class| {
            class
                .constructor
                .and_then(|id| program.functions.get(id.0))
                .map(|constructor| (None, &constructor.signature))
        }),
        CallTarget::Runtime(id) => program
            .runtime_functions
            .get(id.0)
            .map(|function| (None, &function.signature)),
        CallTarget::BoundMethod(temporary) => match temporary.ty {
            ValueType::BoundMethod(id) => program.functions.get(id.0).and_then(|function| {
                function
                    .signature
                    .receiver
                    .map(|_| (None, &function.signature))
            }),
            _ => None,
        },
    }
}

fn call_parameter_type(
    receiver: Option<ValueType>,
    signature: &Signature,
    index: usize,
) -> Option<ValueType> {
    receiver.filter(|_| index == 0).or_else(|| {
        signature
            .parameters
            .get(index - usize::from(receiver.is_some()))
            .map(|parameter| parameter.ty)
    })
}

fn validate_return(
    function: &Function,
    value: Option<Operand>,
    is_main: bool,
) -> Result<(), Diagnostic> {
    if is_main {
        return Err(invalid(function.span));
    }
    match (function.signature.return_type, value) {
        (ValueType::Void, None) => Ok(()),
        (return_type, Some(value)) if return_type == value.ty() => Ok(()),
        _ => Err(invalid(function.span)),
    }
}

fn validate_field_get(
    program: &QuadrupleProgram,
    owner: Operand,
    field: FieldId,
    result: Temporary,
) -> Result<(), Diagnostic> {
    let field = program
        .fields
        .get(field.0)
        .ok_or_else(|| invalid(result_span(result)))?;
    (owner.ty() == ValueType::Object(field.owner) && result.ty == field.ty)
        .then_some(())
        .ok_or_else(|| invalid(field.span))
}

fn validate_field_set(
    program: &QuadrupleProgram,
    owner: Operand,
    field: FieldId,
    value: Operand,
) -> Result<(), Diagnostic> {
    let field = program
        .fields
        .get(field.0)
        .ok_or_else(|| invalid(operand_span(owner)))?;
    (owner.ty() == ValueType::Object(field.owner) && value.ty() == field.ty)
        .then_some(())
        .ok_or_else(|| invalid(field.span))
}

fn validate_static_get(
    program: &QuadrupleProgram,
    static_: StaticId,
    result: Temporary,
) -> Result<(), Diagnostic> {
    let static_ = program
        .statics
        .get(static_.0)
        .ok_or_else(|| invalid(result_span(result)))?;
    (result.ty == static_.ty)
        .then_some(())
        .ok_or_else(|| invalid(static_.span))
}

fn validate_static_set(
    program: &QuadrupleProgram,
    static_: StaticId,
    value: Operand,
) -> Result<(), Diagnostic> {
    let static_ = program
        .statics
        .get(static_.0)
        .ok_or_else(|| invalid(operand_span(value)))?;
    (value.ty() == static_.ty)
        .then_some(())
        .ok_or_else(|| invalid(static_.span))
}

fn validate_bind_method(
    program: &QuadrupleProgram,
    receiver: Operand,
    method: FunctionId,
    result: Temporary,
) -> Result<(), Diagnostic> {
    let function = program
        .functions
        .get(method.0)
        .ok_or_else(|| invalid(result_span(result)))?;
    (function.signature.receiver
        == Some(match receiver.ty() {
            ValueType::Object(class) => class,
            _ => return Err(invalid(function.span)),
        })
        && result.ty == ValueType::BoundMethod(method))
    .then_some(())
    .ok_or_else(|| invalid(function.span))
}

fn validate_definite_definitions(
    _program: &QuadrupleProgram,
    function: &Function,
    labels: &BTreeMap<LabelId, usize>,
    _temporary_types: &BTreeMap<usize, ValueType>,
    local_types: &BTreeMap<usize, ValueType>,
) -> Result<(), Diagnostic> {
    let initialized_parameters = function
        .signature
        .parameters
        .iter()
        .filter_map(|parameter| {
            (local_types.get(&parameter.index) == Some(&parameter.ty)).then_some(parameter.index)
        })
        .collect();
    let mut incoming: Vec<Option<Definitions>> = vec![None; function.code.len()];
    let mut pending = VecDeque::from([(
        0usize,
        Definitions {
            temporaries: BTreeSet::new(),
            locals: initialized_parameters,
        },
    )]);
    while let Some((offset, defined)) = pending.pop_front() {
        let Some(quadruple) = function.code.get(offset) else {
            return Err(invalid(function.span));
        };
        let merged = match &incoming[offset] {
            Some(previous) => previous.intersection(&defined),
            None => defined,
        };
        if incoming[offset].as_ref() == Some(&merged) {
            continue;
        }
        incoming[offset] = Some(merged.clone());
        if operands(&quadruple.operation).iter().any(|operand| matches!(operand, Operand::Temporary(temporary) if !merged.temporaries.contains(&temporary.index)))
            || matches!(&quadruple.operation, Operation::LoadLocal { local, .. } if !merged.locals.contains(&local.index))
        {
            return Err(invalid(quadruple.span));
        }
        let mut outgoing = merged;
        for result in results(&quadruple.operation) {
            outgoing.temporaries.insert(result.index);
        }
        if let Operation::StoreLocal { local, .. } = &quadruple.operation {
            outgoing.locals.insert(local.index);
        }
        for successor in successors(function, offset, labels)? {
            pending.push_back((successor, outgoing.clone()));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Definitions {
    temporaries: BTreeSet<usize>,
    locals: BTreeSet<usize>,
}

impl Definitions {
    fn intersection(&self, other: &Self) -> Self {
        Self {
            temporaries: self
                .temporaries
                .intersection(&other.temporaries)
                .copied()
                .collect(),
            locals: self.locals.intersection(&other.locals).copied().collect(),
        }
    }
}

fn successors(
    function: &Function,
    offset: usize,
    labels: &BTreeMap<LabelId, usize>,
) -> Result<Vec<usize>, Diagnostic> {
    let next = offset + 1;
    match function.code[offset].operation {
        Operation::Jump(label) => Ok(vec![
            *labels
                .get(&label)
                .ok_or_else(|| invalid(function.code[offset].span))?,
        ]),
        Operation::JumpIfFalse { target, .. } => Ok(vec![
            *labels
                .get(&target)
                .ok_or_else(|| invalid(function.code[offset].span))?,
            next,
        ]),
        Operation::Return(_) | Operation::Halt | Operation::Trap(_) => Ok(Vec::new()),
        _ if next < function.code.len() => Ok(vec![next]),
        _ => Err(invalid(function.code[offset].span)),
    }
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
            target: CallTarget::BoundMethod(method),
            ..
        } => vec![Operand::Temporary(*method)],
        _ => Vec::new(),
    }
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

fn invalid(span: Span) -> Diagnostic {
    Diagnostic::error("E7001", "cuádruplos inválidos", span)
}
fn result_span(_: Temporary) -> Span {
    Span::new(SourceId(0), 0, 0)
}
fn operand_span(_: Operand) -> Span {
    Span::new(SourceId(0), 0, 0)
}

#[cfg(test)]
mod return_flow_tests {
    use super::*;

    #[test]
    fn accepts_terminating_branches_and_unreachable_tail() {
        for source in [
            "fn choose(x: bool) -> int { if x { return 1; } else { return 2; } } choose(true);",
            "fn choose(x: bool, y: bool) -> int { if x { if y { return 1; } else { return 2; } } else { return 3; } } choose(true, false);",
            "fn answer() -> int { return 42; let unused: int := 1; } answer();",
        ] {
            let result = crate::compile_quadruples(source);
            // Unreachable source may produce W2001 without invalidating the IR.
            assert!(result.program.is_some(), "{:?}", result.diagnostics);
            assert!(validate(&result.program.expect("valid return flow")).is_ok());
        }
    }

    #[test]
    fn rejects_reachable_fallthrough_even_with_unreachable_return() {
        let mut program = crate::compile_quadruples("fn answer() -> int { return 42; } answer();")
            .program
            .expect("valid fixture");
        let function = &mut program.functions[1];
        let span = function.span;
        function.code = vec![
            Quadruple {
                operation: Operation::Jump(LabelId(0)),
                span,
            },
            Quadruple {
                operation: Operation::Return(Some(Operand::I32(42))),
                span,
            },
            Quadruple {
                operation: Operation::Label(LabelId(0)),
                span,
            },
        ];
        assert!(validate(&program).is_err());
    }

    #[test]
    fn unreachable_source_still_receives_numeric_diagnostics() {
        for source in [
            "fn answer() -> int { return 42; 2147483648; } answer();",
            "fn answer() -> int { return 42; let unused: long := 1; } answer();",
            "fn choose(x: bool) -> int { if x { return 1; } } choose(false);",
        ] {
            let result = crate::compile_quadruples(source);
            assert!(
                result.program.is_none(),
                "accepted invalid source: {source}"
            );
            assert!(!result.diagnostics.is_empty());
        }
    }
}
