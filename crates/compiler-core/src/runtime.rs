use crate::{
    ast::{BinaryOp, Literal, Program, UnaryOp},
    diagnostic::Diagnostic,
    semantic::{SemanticResult, Type},
    source::Span,
};
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use std::sync::{Arc, atomic::AtomicBool};
const MAX_CALL_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeLimits {
    pub instructions: usize,
    pub call_depth: usize,
    pub heap_objects: usize,
    pub output_bytes: usize,
    pub wall_time_ms: u64,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            instructions: 1_000_000,
            call_depth: MAX_CALL_DEPTH,
            heap_objects: 10_000,
            output_bytes: 1_048_576,
            wall_time_ms: 2_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Decimal(Decimal),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(String),
    Char(char),
    Object(usize),
    Class(String),
    Function(usize),
    Native(String),
    Constructor(usize),
    BoundMethod(usize, usize),
    Void,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeResult {
    pub output: String,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn execute(program: &Program, semantic: &SemanticResult) -> RuntimeResult {
    match crate::bytecode::compile(program, semantic) {
        Ok(chunk) => execute_chunk(&chunk, semantic, RuntimeLimits::default(), None),
        Err(diagnostic) => RuntimeResult {
            output: String::new(),
            diagnostics: vec![diagnostic],
        },
    }
}
pub fn execute_chunk(
    chunk: &crate::bytecode::Chunk,
    _semantic: &SemanticResult,
    limits: RuntimeLimits,
    cancelled: Option<Arc<AtomicBool>>,
) -> RuntimeResult {
    crate::vm::execute(chunk, limits, cancelled)
}
#[cfg(feature = "reference-interpreter")]
pub use crate::reference::{execute_reference, execute_reference_with_options};
pub(crate) fn literal_value(literal: &Literal, ty: &Type, span: Span) -> Result<Value, Diagnostic> {
    match literal {
        Literal::Bool(value) => Ok(Value::Bool(*value)),
        Literal::String(value) => Ok(Value::Str(unquote(value))),
        Literal::Char(value) => Ok(Value::Char(unquote(value).chars().next().unwrap_or('\0'))),
        Literal::Number(value) => match ty {
            Type::Decimal => Decimal::from_str_exact(value.trim_end_matches("dec"))
                .map(Value::Decimal)
                .map_err(|_| error("E4001", "decimal fuera del rango representable", span)),
            Type::Float => value
                .trim_end_matches("f32")
                .parse()
                .map(Value::Float)
                .map_err(|_| error("E4001", "float inválido", span)),
            Type::Double => value
                .trim_end_matches("f64")
                .parse()
                .map(Value::Double)
                .map_err(|_| error("E4001", "double inválido", span)),
            ty => {
                let value = value
                    .trim_end_matches("i8")
                    .trim_end_matches("i16")
                    .trim_end_matches("i32")
                    .trim_end_matches("i64")
                    .parse()
                    .map(Value::Int)
                    .map_err(|_| error("E4001", "entero fuera de rango", span))?;
                cast(value, ty, span)
            }
        },
    }
}

pub(crate) fn builtin(
    name: &str,
    values: &[Value],
    output: &mut String,
    output_bytes: usize,
    span: Span,
) -> Result<Value, Diagnostic> {
    match (name, values) {
        ("print", [Value::Str(value)]) => {
            if output.len().saturating_add(value.len() + 1) > output_bytes {
                return Err(error("E5006", "límite de salida excedido", span));
            }
            output.push_str(value);
            output.push('\n');
            Ok(Value::Void)
        }
        ("round", [Value::Decimal(value), Value::Int(scale)]) => {
            let scale = u32::try_from(*scale)
                .ok()
                .filter(|scale| *scale <= Decimal::MAX_SCALE)
                .ok_or_else(|| error("E4005", "escala decimal inválida", span))?;
            Ok(Value::Decimal(value.round_dp_with_strategy(
                scale,
                RoundingStrategy::MidpointNearestEven,
            )))
        }
        ("scale", [Value::Decimal(value)]) => Ok(Value::Int(i64::from(value.scale()))),
        ("to_string", [Value::Decimal(value)]) => Ok(Value::Str(value.to_string())),
        ("to_string", [Value::Int(value)]) => Ok(Value::Str(value.to_string())),
        _ => Err(error("E4000", "llamada nativa inválida", span)),
    }
}

pub(crate) fn unary(op: UnaryOp, value: Value, span: Span) -> Result<Value, Diagnostic> {
    match (op, value) {
        (UnaryOp::Not, Value::Bool(value)) => Ok(Value::Bool(!value)),
        (
            UnaryOp::Plus,
            value @ (Value::Int(_) | Value::Decimal(_) | Value::Float(_) | Value::Double(_)),
        ) => Ok(value),
        (UnaryOp::Minus, Value::Int(value)) => value
            .checked_neg()
            .map(Value::Int)
            .ok_or_else(|| overflow(span)),
        (UnaryOp::Minus, Value::Decimal(value)) => Decimal::ZERO
            .checked_sub(value)
            .map(Value::Decimal)
            .ok_or_else(|| overflow(span)),
        (UnaryOp::Minus, Value::Float(value)) => Ok(Value::Float(-value)),
        (UnaryOp::Minus, Value::Double(value)) => Ok(Value::Double(-value)),
        _ => Err(error("E4000", "operador unario inválido", span)),
    }
}

pub(crate) fn binary(
    left: Value,
    op: BinaryOp,
    right: Value,
    span: Span,
) -> Result<Value, Diagnostic> {
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => integer(left, op, right, span),
        (Value::Decimal(left), Value::Decimal(right)) => decimal(left, op, right, span),
        (Value::Float(left), Value::Float(right)) if comparison(op) => {
            compare(left, op, right, span)
        }
        (Value::Float(_), Value::Float(0.0))
            if matches!(op, BinaryOp::Divide | BinaryOp::Remainder) =>
        {
            Err(error("E4003", "división por cero", span))
        }
        (Value::Float(left), Value::Float(right)) if op == BinaryOp::Power => {
            let result = left.powf(right);
            result
                .is_finite()
                .then_some(Value::Float(result))
                .ok_or_else(|| overflow(span))
        }
        (Value::Float(left), Value::Float(right)) => {
            let result = float(left, op, right, span)?;
            result
                .is_finite()
                .then_some(Value::Float(result))
                .ok_or_else(|| overflow(span))
        }
        (Value::Double(left), Value::Double(right)) if comparison(op) => {
            compare(left, op, right, span)
        }
        (Value::Double(_), Value::Double(0.0))
            if matches!(op, BinaryOp::Divide | BinaryOp::Remainder) =>
        {
            Err(error("E4003", "división por cero", span))
        }
        (Value::Double(left), Value::Double(right)) if op == BinaryOp::Power => {
            let result = left.powf(right);
            result
                .is_finite()
                .then_some(Value::Double(result))
                .ok_or_else(|| overflow(span))
        }
        (Value::Double(left), Value::Double(right)) => {
            let result = float(left, op, right, span)?;
            result
                .is_finite()
                .then_some(Value::Double(result))
                .ok_or_else(|| overflow(span))
        }
        (Value::Bool(left), Value::Bool(right)) => compare(left, op, right, span),
        (Value::Str(left), Value::Str(right)) if op == BinaryOp::Add => {
            Ok(Value::Str(left + &right))
        }
        (Value::Str(left), Value::Str(right)) => compare(left, op, right, span),
        (Value::Char(left), Value::Char(right)) => compare(left, op, right, span),
        (Value::Object(left), Value::Object(right))
            if matches!(op, BinaryOp::Equal | BinaryOp::NotEqual) =>
        {
            compare(left, op, right, span)
        }
        _ => Err(error("E4000", "operandos incompatibles", span)),
    }
}
fn integer(left: i64, op: BinaryOp, right: i64, span: Span) -> Result<Value, Diagnostic> {
    let value = match op {
        BinaryOp::Add => left.checked_add(right),
        BinaryOp::Subtract => left.checked_sub(right),
        BinaryOp::Multiply => left.checked_mul(right),
        BinaryOp::Divide => left.checked_div(right),
        BinaryOp::Remainder => left.checked_rem(right),
        BinaryOp::Power => u32::try_from(right)
            .ok()
            .and_then(|right| left.checked_pow(right)),
        _ => return compare(left, op, right, span),
    };
    value
        .map(Value::Int)
        .ok_or_else(|| arithmetic_error(right, span))
}

fn decimal(left: Decimal, op: BinaryOp, right: Decimal, span: Span) -> Result<Value, Diagnostic> {
    let value = match op {
        BinaryOp::Add => left.checked_add(right),
        BinaryOp::Subtract => left.checked_sub(right),
        BinaryOp::Multiply => left.checked_mul(right),
        BinaryOp::Divide => left.checked_div(right),
        BinaryOp::Remainder => left.checked_rem(right),
        BinaryOp::Power => return decimal_power(left, right, span).map(Value::Decimal),
        _ => return compare(left, op, right, span),
    };
    value.map(Value::Decimal).ok_or_else(|| {
        if right.is_zero() && matches!(op, BinaryOp::Divide | BinaryOp::Remainder) {
            error("E4003", "división por cero", span)
        } else {
            overflow(span)
        }
    })
}

fn decimal_power(base: Decimal, exponent: Decimal, span: Span) -> Result<Decimal, Diagnostic> {
    let exponent = exponent
        .to_i32()
        .filter(|value| Decimal::from(*value) == exponent)
        .ok_or_else(|| error("E4000", "el exponente decimal debe ser entero", span))?;
    let mut power = exponent.unsigned_abs();
    let mut factor = base;
    let mut result = Decimal::ONE;
    while power > 0 {
        if power % 2 == 1 {
            result = result.checked_mul(factor).ok_or_else(|| overflow(span))?;
        }
        power /= 2;
        if power > 0 {
            factor = factor.checked_mul(factor).ok_or_else(|| overflow(span))?;
        }
    }
    if exponent < 0 {
        Decimal::ONE
            .checked_div(result)
            .ok_or_else(|| arithmetic_error_decimal(result, span))
    } else {
        Ok(result)
    }
}

fn float<T>(left: T, op: BinaryOp, right: T, span: Span) -> Result<T, Diagnostic>
where
    T: Copy
        + PartialEq
        + PartialOrd
        + std::ops::Add<Output = T>
        + std::ops::Sub<Output = T>
        + std::ops::Mul<Output = T>
        + std::ops::Div<Output = T>
        + std::ops::Rem<Output = T>,
{
    match op {
        BinaryOp::Add => Ok(left + right),
        BinaryOp::Subtract => Ok(left - right),
        BinaryOp::Multiply => Ok(left * right),
        BinaryOp::Divide => Ok(left / right),
        BinaryOp::Remainder => Ok(left % right),
        _ => Err(error("E4000", "operación float inválida", span)),
    }
}

fn compare<T: PartialEq + PartialOrd>(
    left: T,
    op: BinaryOp,
    right: T,
    span: Span,
) -> Result<Value, Diagnostic> {
    Ok(Value::Bool(match op {
        BinaryOp::Equal => left == right,
        BinaryOp::NotEqual => left != right,
        BinaryOp::Less => left < right,
        BinaryOp::LessEqual => left <= right,
        BinaryOp::Greater => left > right,
        BinaryOp::GreaterEqual => left >= right,
        BinaryOp::And | BinaryOp::Or => {
            return Err(error("E4000", "operación lógica inválida", span));
        }
        _ => return Err(error("E4000", "comparación inválida", span)),
    }))
}

pub(crate) fn cast(value: Value, target: &Type, span: Span) -> Result<Value, Diagnostic> {
    match (value, target) {
        (value, Type::Error) => Ok(value),
        (Value::Int(value), Type::Byte) => i8::try_from(value)
            .map(|value| Value::Int(i64::from(value)))
            .map_err(|_| precision(span)),
        (Value::Int(value), Type::Short) => i16::try_from(value)
            .map(|value| Value::Int(i64::from(value)))
            .map_err(|_| precision(span)),
        (Value::Int(value), Type::Int) => i32::try_from(value)
            .map(|value| Value::Int(i64::from(value)))
            .map_err(|_| precision(span)),
        (value @ Value::Int(_), Type::Long) => Ok(value),
        (Value::Int(value), Type::Decimal) => Ok(Value::Decimal(Decimal::from(value))),
        (Value::Int(value), Type::Float) if value.unsigned_abs() <= (1_u64 << 24) => {
            Ok(Value::Float(value as f32))
        }
        (Value::Int(value), Type::Double) if value.unsigned_abs() <= (1_u64 << 53) => {
            Ok(Value::Double(value as f64))
        }
        (Value::Decimal(value), Type::Byte | Type::Short | Type::Int | Type::Long) => value
            .to_i64()
            .filter(|integer| Decimal::from(*integer) == value)
            .map(Value::Int)
            .and_then(|value| cast(value, target, span).ok())
            .ok_or_else(|| precision(span)),
        (Value::Decimal(value), Type::Float) => {
            let converted = value
                .to_string()
                .parse::<f32>()
                .map_err(|_| precision(span))?;
            (Decimal::from_f32_retain(converted) == Some(value))
                .then_some(Value::Float(converted))
                .ok_or_else(|| precision(span))
        }
        (Value::Decimal(value), Type::Double) => {
            let converted = value
                .to_string()
                .parse::<f64>()
                .map_err(|_| precision(span))?;
            (Decimal::from_f64_retain(converted) == Some(value))
                .then_some(Value::Double(converted))
                .ok_or_else(|| precision(span))
        }
        (Value::Float(value), Type::Decimal) => Decimal::from_f32_retain(value)
            .map(Value::Decimal)
            .ok_or_else(|| precision(span)),
        (Value::Double(value), Type::Decimal) => Decimal::from_f64_retain(value)
            .map(Value::Decimal)
            .ok_or_else(|| precision(span)),
        (Value::Float(value), Type::Double) => Ok(Value::Double(f64::from(value))),
        (Value::Double(value), Type::Float) if f64::from(value as f32) == value => {
            Ok(Value::Float(value as f32))
        }
        (Value::Float(value), Type::Byte | Type::Short | Type::Int | Type::Long)
            if value.is_finite() && value.fract() == 0.0 =>
        {
            let integer = value as i64;
            (integer as f32 == value)
                .then(|| cast(Value::Int(integer), target, span))
                .transpose()?
                .ok_or_else(|| precision(span))
        }
        (Value::Double(value), Type::Byte | Type::Short | Type::Int | Type::Long)
            if value.is_finite() && value.fract() == 0.0 =>
        {
            let integer = value as i64;
            (integer as f64 == value)
                .then(|| cast(Value::Int(integer), target, span))
                .transpose()?
                .ok_or_else(|| precision(span))
        }
        (value, ty) if value_type(&value) == *ty => Ok(value),
        _ => Err(error("E4005", "cast fuera de rango o con pérdida", span)),
    }
}

fn value_type(value: &Value) -> Type {
    match value {
        Value::Int(_) => Type::Long,
        Value::Decimal(_) => Type::Decimal,
        Value::Float(_) => Type::Float,
        Value::Double(_) => Type::Double,
        Value::Bool(_) => Type::Bool,
        Value::Str(_) => Type::Str,
        Value::Char(_) => Type::Char,
        Value::Object(_)
        | Value::Class(_)
        | Value::Function(_)
        | Value::Native(_)
        | Value::Constructor(_)
        | Value::BoundMethod(_, _) => Type::Error,
        Value::Void => Type::Void,
    }
}

fn comparison(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
    )
}

fn unquote(value: &str) -> String {
    value[1..value.len().saturating_sub(1)]
        .replace("\\n", "\n")
        .replace("\\r", "\r")
        .replace("\\t", "\t")
        .replace("\\0", "\0")
        .replace("\\\"", "\"")
        .replace("\\'", "'")
        .replace("\\\\", "\\")
}

fn arithmetic_error(divisor: i64, span: Span) -> Diagnostic {
    if divisor == 0 {
        error("E4003", "división por cero", span)
    } else {
        overflow(span)
    }
}

fn arithmetic_error_decimal(divisor: Decimal, span: Span) -> Diagnostic {
    if divisor.is_zero() {
        error("E4003", "división por cero", span)
    } else {
        overflow(span)
    }
}

fn overflow(span: Span) -> Diagnostic {
    error("E4002", "resultado numérico fuera de rango", span)
}

fn precision(span: Span) -> Diagnostic {
    error("E4005", "conversión con pérdida de precisión", span)
}

pub(crate) fn error(code: &str, message: &str, span: Span) -> Diagnostic {
    Diagnostic::error(code, message, span)
}
