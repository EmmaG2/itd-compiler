use super::{DiagnosticDto, LocationDto, diagnostics_dto, location};
use compiler_core::{
    quadruple::{CallTarget, Operand, Operation, QuadrupleProgram, Temporary, ValueType},
    source::SourceMap,
};
use serde::Serialize;

const MAX_QUADRUPLES: usize = 20_000;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeInspectionDto {
    pub quadruples: Vec<QuadrupleDto>,
    pub truncated: bool,
    pub assembly: Option<String>,
    pub diagnostics: Vec<DiagnosticDto>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuadrupleDto {
    pub function: String,
    pub operation: String,
    pub arg1: String,
    pub arg2: String,
    pub result: String,
    pub value_type: Option<String>,
    pub location: LocationDto,
}

pub(crate) fn native_inspection_dto(source: &str, fallback: &str) -> NativeInspectionDto {
    let compiled = compiler_core::compile_quadruples(source);
    let mut diagnostics = compiled.diagnostics;
    let Some(program) = compiled.program else {
        return NativeInspectionDto {
            diagnostics: diagnostics_dto(&diagnostics, &compiled.sources, fallback),
            ..NativeInspectionDto::default()
        };
    };
    let (quadruples, truncated) = quadruples_dto(&program, &compiled.sources, fallback);
    let assembly = match compiler_core::emit_arm64(&program) {
        Ok(assembly) => Some(assembly),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            None
        }
    };
    NativeInspectionDto {
        quadruples,
        truncated,
        assembly,
        diagnostics: diagnostics_dto(&diagnostics, &compiled.sources, fallback),
    }
}

fn quadruples_dto(
    program: &QuadrupleProgram,
    sources: &SourceMap,
    fallback: &str,
) -> (Vec<QuadrupleDto>, bool) {
    let mut rows = Vec::new();
    for function in &program.functions {
        for quadruple in &function.code {
            if rows.len() == MAX_QUADRUPLES {
                return (rows, true);
            }
            let (operation, arg1, arg2, result, value_type) =
                format_operation(&quadruple.operation);
            rows.push(QuadrupleDto {
                function: function.name.clone(),
                operation,
                arg1,
                arg2,
                result,
                value_type,
                location: location(sources, quadruple.span, fallback),
            });
        }
    }
    (rows, false)
}

fn format_operation(operation: &Operation) -> (String, String, String, String, Option<String>) {
    let binary = |name: &str, left: Operand, right: Operand, result: Temporary| {
        (
            name.into(),
            operand(left),
            operand(right),
            temporary(result),
            Some(value_type(result.ty)),
        )
    };
    match operation {
        Operation::Add {
            left,
            right,
            result,
        } => binary("ADD", *left, *right, *result),
        Operation::Sub {
            left,
            right,
            result,
        } => binary("SUB", *left, *right, *result),
        Operation::Mul {
            left,
            right,
            result,
        } => binary("MUL", *left, *right, *result),
        Operation::Div {
            left,
            right,
            result,
        } => binary("DIV", *left, *right, *result),
        Operation::Mod {
            left,
            right,
            result,
        } => binary("MOD", *left, *right, *result),
        Operation::Pow {
            left,
            right,
            result,
        } => binary("POW", *left, *right, *result),
        Operation::Eq {
            left,
            right,
            result,
        } => binary("EQ", *left, *right, *result),
        Operation::Ne {
            left,
            right,
            result,
        } => binary("NE", *left, *right, *result),
        Operation::Lt {
            left,
            right,
            result,
        } => binary("LT", *left, *right, *result),
        Operation::Le {
            left,
            right,
            result,
        } => binary("LE", *left, *right, *result),
        Operation::Gt {
            left,
            right,
            result,
        } => binary("GT", *left, *right, *result),
        Operation::Neg {
            operand: value,
            result,
        } => unary("NEG", *value, *result),
        Operation::Not {
            operand: value,
            result,
        } => unary("NOT", *value, *result),
        Operation::Copy { source, result } => unary("COPY", *source, *result),
        Operation::I32ToF64 { source, result } => unary("I32_TO_F64", *source, *result),
        Operation::LoadLocal { local, result } => (
            "LOAD_LOCAL".into(),
            format!("local#{}", local.index),
            String::new(),
            temporary(*result),
            Some(value_type(result.ty)),
        ),
        Operation::StoreLocal { local, value } => (
            "STORE_LOCAL".into(),
            operand(*value),
            String::new(),
            format!("local#{}", local.index),
            Some(value_type(local.ty)),
        ),
        Operation::Label(label) => (
            "LABEL".into(),
            String::new(),
            String::new(),
            label_name(label.0),
            None,
        ),
        Operation::Jump(label) => (
            "JUMP".into(),
            String::new(),
            String::new(),
            label_name(label.0),
            None,
        ),
        Operation::JumpIfFalse { condition, target } => (
            "JUMP_IF_FALSE".into(),
            operand(*condition),
            String::new(),
            label_name(target.0),
            None,
        ),
        Operation::Param { call, index, value } => (
            "PARAM".into(),
            operand(*value),
            format!("call#{}", call.0),
            format!("arg#{index}"),
            Some(value_type(value.ty())),
        ),
        Operation::Call {
            call,
            target,
            result,
        } => (
            "CALL".into(),
            call_target(*target),
            format!("call#{}", call.0),
            result.map_or_else(String::new, temporary),
            result.map(|value| value_type(value.ty)),
        ),
        Operation::Return(value) => (
            "RETURN".into(),
            value.map_or_else(String::new, operand),
            String::new(),
            String::new(),
            value.map(|value| value_type(value.ty())),
        ),
        Operation::GetField {
            owner,
            field,
            result,
        } => (
            "GET_FIELD".into(),
            operand(*owner),
            format!("field#{}", field.0),
            temporary(*result),
            Some(value_type(result.ty)),
        ),
        Operation::SetField {
            owner,
            field,
            value,
        } => (
            "SET_FIELD".into(),
            operand(*owner),
            operand(*value),
            format!("field#{}", field.0),
            Some(value_type(value.ty())),
        ),
        Operation::GetStatic { static_, result } => (
            "GET_STATIC".into(),
            format!("static#{}", static_.0),
            String::new(),
            temporary(*result),
            Some(value_type(result.ty)),
        ),
        Operation::SetStatic { static_, value } => (
            "SET_STATIC".into(),
            operand(*value),
            String::new(),
            format!("static#{}", static_.0),
            Some(value_type(value.ty())),
        ),
        Operation::BindMethod {
            receiver,
            method,
            result,
        } => (
            "BIND_METHOD".into(),
            operand(*receiver),
            format!("function#{}", method.0),
            temporary(*result),
            Some(value_type(result.ty)),
        ),
        Operation::Halt => (
            "HALT".into(),
            String::new(),
            String::new(),
            String::new(),
            None,
        ),
        Operation::Trap(diagnostic) => (
            "TRAP".into(),
            diagnostic.code.clone(),
            diagnostic.message.clone(),
            String::new(),
            None,
        ),
    }
}

fn unary(
    name: &str,
    value: Operand,
    result: Temporary,
) -> (String, String, String, String, Option<String>) {
    (
        name.into(),
        operand(value),
        String::new(),
        temporary(result),
        Some(value_type(result.ty)),
    )
}

fn operand(value: Operand) -> String {
    match value {
        Operand::I32(value) => value.to_string(),
        Operand::F64(value) => value.to_string(),
        Operand::Bool(value) => value.to_string(),
        Operand::String(value) => format!("string#{}", value.0),
        Operand::Receiver { class } => format!("this:class#{}", class.0),
        Operand::Parameter { index, ty } => format!("param#{index}:{}", value_type(ty)),
        Operand::Temporary(value) => temporary(value),
    }
}

fn temporary(value: Temporary) -> String {
    format!("t{}:{}", value.index, value_type(value.ty))
}

fn value_type(value: ValueType) -> String {
    match value {
        ValueType::I32 => "i32".into(),
        ValueType::F64 => "f64".into(),
        ValueType::Bool => "bool".into(),
        ValueType::Str => "str".into(),
        ValueType::Object(class) => format!("object#{}", class.0),
        ValueType::BoundMethod(function) => format!("bound-method#{}", function.0),
        ValueType::Void => "void".into(),
    }
}

fn call_target(target: CallTarget) -> String {
    match target {
        CallTarget::Function(value) => format!("function#{}", value.0),
        CallTarget::Constructor(value) => format!("constructor#{}", value.0),
        CallTarget::Runtime(value) => format!("runtime#{}", value.0),
        CallTarget::BoundMethod(value) => temporary(value),
    }
}

fn label_name(index: usize) -> String {
    format!("label#{index}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_typed_rows_with_source_spans_and_assembly() {
        let dto = native_inspection_dto("let value: int := 2 + 3;", "lesson.itd");
        let add = dto.rows().find(|row| row.operation == "ADD").unwrap();
        assert_eq!(add.arg1, "2");
        assert_eq!(add.arg2, "3");
        assert_eq!(add.value_type.as_deref(), Some("i32"));
        assert_eq!(add.location.file, "lesson.itd");
        assert!(add.location.end > add.location.start);
        assert!(
            dto.assembly
                .as_deref()
                .is_some_and(|assembly| assembly.contains("_main"))
        );
    }

    #[test]
    fn formats_constructor_direct_and_bound_calls() {
        let dto = native_inspection_dto(
            "class Counter { pub fn this() {} pub fn read() -> int { return 1; } } let counter := Counter(); counter.read(); let read := counter.read; read();",
            "main.itd",
        );
        assert!(
            dto.quadruples
                .iter()
                .any(|row| row.operation == "CALL" && row.arg1.starts_with("constructor#"))
        );
        assert!(
            dto.quadruples
                .iter()
                .any(|row| row.operation == "CALL" && row.arg1.starts_with("function#"))
        );
        assert!(
            dto.quadruples
                .iter()
                .any(|row| row.operation == "BIND_METHOD")
        );
        assert!(
            dto.quadruples
                .iter()
                .any(|row| row.operation == "CALL" && row.arg1.starts_with("t"))
        );
    }

    #[test]
    fn reports_native_errors_without_ir_or_assembly() {
        let dto = native_inspection_dto("missing;", "main.itd");
        assert!(dto.quadruples.is_empty());
        assert!(dto.assembly.is_none());
        assert!(!dto.diagnostics.is_empty());
    }

    impl NativeInspectionDto {
        fn rows(&self) -> impl Iterator<Item = &QuadrupleDto> {
            self.quadruples.iter()
        }
    }
}
