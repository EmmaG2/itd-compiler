use compiler_core::{
    analyze, compile_quadruples, compile_source,
    quadruple::{Operand, Operation, ValueType, validate},
};

#[test]
fn lowers_precedence_conversions_and_i32_boundaries() {
    let result = compile_quadruples(
        "let value: decimal := (2 + 3 * 4) as decimal; let min: int := -2147483648;",
    );
    let program = result.program.expect("valid quadruples");
    assert!(result.diagnostics.is_empty());
    assert!(validate(&program).is_ok());
    assert!(
        program.functions[0]
            .code
            .iter()
            .any(|quad| matches!(quad.operation, Operation::I32ToF64 { .. }))
    );
    assert!(program.functions[0].code.iter().any(|quad| matches!(
        quad.operation,
        Operation::StoreLocal {
            value: Operand::I32(-2147483648),
            ..
        }
    )));
}

#[test]
fn lowers_control_short_circuit_and_direct_calls() {
    let result = compile_quadruples(
        "fn twice(x: int) -> int { return x + x; } let mut n: int := 0; while n < 2 { if n == 1 or false { break; } n = twice(n + 1); }",
    );
    let program = result.program.expect("valid quadruples");
    assert!(result.diagnostics.is_empty());
    assert!(validate(&program).is_ok());
    assert!(
        program.functions[0]
            .code
            .iter()
            .any(|quad| matches!(quad.operation, Operation::Call { .. }))
    );
    assert!(
        program.functions[0]
            .code
            .iter()
            .any(|quad| matches!(quad.operation, Operation::JumpIfFalse { .. }))
    );
}

#[test]
fn rejects_unsupported_numeric_forms_and_semantic_errors() {
    for source in [
        "let value: double := 1.0;",
        "let value := 2147483648;",
        "missing;",
        "let value := 1; value = 2;",
        "fn f(x: int) {} f();",
    ] {
        let result = compile_quadruples(source);
        assert!(result.program.is_none(), "{source}");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.severity == compiler_core::diagnostic::Severity::Error)
        );
    }
}

#[test]
fn lowers_runtime_overloads_and_decodes_strings_once() {
    let result = compile_quadruples(
        "let mut balance: decimal := 0dec; while balance < 2dec { balance = balance + 1; } print(to_string(balance)); print(\"\\\\n\\n\"); print(true); print(1);",
    );
    let program = result.program.expect("valid quadruples");
    assert!(result.diagnostics.is_empty());
    assert_eq!(program.strings, ["\\n\n"]);
    assert!(program.runtime_functions.iter().any(|runtime| {
        runtime.name == "to_string"
            && runtime.signature.parameters[0].ty == ValueType::F64
            && runtime.signature.return_type == ValueType::Str
    }));
    for ty in [ValueType::Str, ValueType::Bool, ValueType::I32] {
        assert!(program.runtime_functions.iter().any(|runtime| {
            runtime.name == "print"
                && runtime.signature.parameters[0].ty == ty
                && runtime.signature.return_type == ValueType::Void
        }));
    }
    assert!(validate(&program).is_ok());
}

#[test]
fn coerces_call_arguments_and_keeps_nested_parameter_groups_contiguous() {
    let result = compile_quadruples(
        "fn decimal_id(value: decimal) -> decimal { return value; } fn twice(value: int) -> int { return value + value; } let answer: decimal := decimal_id(3); let nested: int := twice(twice(3));",
    );
    let program = result.program.expect("valid quadruples");
    assert!(result.diagnostics.is_empty());
    let code = &program.functions[0].code;
    let conversion = code
        .iter()
        .position(|quad| matches!(quad.operation, Operation::I32ToF64 { .. }))
        .expect("int argument is converted to decimal");
    let decimal_param = code
        .iter()
        .position(|quad| matches!(quad.operation, Operation::Param { index: 0, value: Operand::Temporary(temp), .. } if temp.ty == ValueType::F64))
        .expect("converted decimal parameter");
    assert!(conversion < decimal_param);
    let nested_calls = code
        .iter()
        .filter_map(|quad| match quad.operation {
            Operation::Call { call, .. } => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(nested_calls.len() >= 3);
    for call in nested_calls {
        let call_offset = code
            .iter()
            .position(|quad| matches!(quad.operation, Operation::Call { call: candidate, .. } if candidate == call))
            .unwrap();
        assert!(matches!(
            code[call_offset - 1].operation,
            Operation::Param {
                call: parameter_call,
                index: 0,
                ..
            } if parameter_call == call
        ));
    }
    assert!(validate(&program).is_ok());
}

#[test]
fn diagnoses_overflow_unsupported_suffixes_and_semantic_errors_in_dead_code() {
    let valid = compile_quadruples("let min: int := -2147483648; let max: int := 2147483647;");
    assert!(valid.program.is_some());
    assert!(valid.diagnostics.is_empty());

    for source in [
        "if false { let bad := 2147483648; }",
        "if false { let bad := 1i8; }",
        "if false { let bad := 1i16; }",
        "if false { let bad := 1i64; }",
        "if false { let bad := 1f32; }",
        "if false { let bad := 1f64; }",
        "if false { let bad: byte := 1; }",
        "let value := 2 ** -1;",
        "missing;",
        "let value := 1; value = 2;",
        "fn f(x: int) {} f();",
    ] {
        let result = compile_quadruples(source);
        let has_overflow = source.contains("2147483648");
        if !has_overflow || source.contains("bad") {
            assert!(result.program.is_none(), "{source}");
        }
        assert!(result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == compiler_core::diagnostic::Severity::Error), "{source}");
    }
}

#[test]
fn quadruple_semantics_are_opt_in_and_preserve_vm_rejections() {
    let source = "fn decimal_id(value: decimal) -> decimal { return value; } let answer: decimal := decimal_id(3); print(true);";
    let frontend = analyze(source);
    assert!(
        frontend
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E2005")
    );
    assert!(compile_source(source).chunk.is_none());

    let backend = compile_quadruples(source);
    assert!(backend.diagnostics.is_empty());
    assert!(backend.program.is_some());
}

#[test]
fn lowers_counter_objects_from_source_with_declared_layout_and_bound_receiver() {
    let result = compile_quadruples(
        "class Counter { pub let mut value: int := 0; pub static let mut total: int := 0; pub fn this(initial: int) { this.value = initial; } pub fn read() -> int { return this.value; } } let c := Counter(3); let copy := c; copy.value; c.value = 5; Counter.total; Counter.total = 5; c.read(); let read := c.read; read();",
    );
    let program = result.program.expect("Counter source lowers");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(program.classes.len(), 1);
    let class = &program.classes[0];
    assert_eq!(class.fields, [compiler_core::quadruple::FieldId(0)]);
    assert_eq!(class.statics, [compiler_core::quadruple::StaticId(0)]);
    assert!(class.constructor.is_some());
    let main = &program.functions[0].code;
    assert!(main.iter().any(|quad| matches!(
        quad.operation,
        Operation::Call {
            target: compiler_core::quadruple::CallTarget::Constructor(_),
            result: Some(_),
            ..
        }
    )));
    assert!(
        main.iter()
            .any(|quad| matches!(quad.operation, Operation::GetField { .. }))
    );
    assert!(
        main.iter()
            .any(|quad| matches!(quad.operation, Operation::SetField { .. }))
    );
    assert!(
        main.iter()
            .any(|quad| matches!(quad.operation, Operation::GetStatic { .. }))
    );
    assert!(
        main.iter()
            .any(|quad| matches!(quad.operation, Operation::SetStatic { .. }))
    );
    assert!(main.iter().filter(|quad| matches!(quad.operation, Operation::StoreLocal { local, .. } if matches!(local.ty, ValueType::Object(_)))).count() >= 2);
    assert!(
        main.iter()
            .any(|quad| matches!(quad.operation, Operation::BindMethod { .. }))
    );
    assert!(main.iter().any(|quad| matches!(
        quad.operation,
        Operation::Call {
            target: compiler_core::quadruple::CallTarget::BoundMethod(_),
            ..
        }
    )));
    assert!(validate(&program).is_ok());
}

#[test]
fn rejects_invalid_or_unsupported_object_source() {
    for source in [
        "class Hidden { let value: int := 0; } let hidden := Hidden(); hidden.value;",
        "class Counter { pub let mut value: int := 0; } let counter := Counter(); counter.value = true;",
        "class Utility { pub static fn read() -> int { return 1; } } Utility.read();",
    ] {
        let result = compile_quadruples(source);
        assert!(result.program.is_none(), "accepted: {source}");
        assert!(result.diagnostics.iter().any(|diagnostic| diagnostic.severity == compiler_core::diagnostic::Severity::Error));
    }
}

#[test]
fn lowers_mutation_short_circuit_loops_and_greater_equal_in_source_order() {
    let result = compile_quadruples(
        "let mut value: int := 0; while value < 3 { value = value + 1; if value >= 2 and true { print(to_string(value)); } }",
    );
    let program = result.program.expect("valid quadruples");
    assert!(result.diagnostics.is_empty());
    let code = &program.functions[0].code;
    let greater_equal = code
        .iter()
        .find_map(|quad| match quad.operation {
            Operation::Le { left, right, .. } => Some((left, right)),
            _ => None,
        })
        .expect("greater-equal is lowered through LE");
    assert!(matches!(
        greater_equal.0,
        Operand::I32(2) | Operand::Temporary(_)
    ));
    assert!(
        code.iter()
            .any(|quad| matches!(quad.operation, Operation::StoreLocal { .. }))
    );
    assert!(validate(&program).is_ok());
}
