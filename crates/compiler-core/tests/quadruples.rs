use compiler_core::{
    quadruple::{
        CallSiteId, CallTarget, Class, ClassId, Function, FunctionId, LabelId, LocalSlot, Operand,
        Operation, Parameter, Quadruple, QuadrupleProgram, RuntimeFunction, RuntimeFunctionId,
        Signature, Temporary, ValueType, validate,
    },
    source::{SourceId, Span},
};

fn span() -> Span {
    Span::new(SourceId(0), 0, 1)
}

fn quad(operation: Operation) -> Quadruple {
    Quadruple {
        operation,
        span: span(),
    }
}

fn signature(parameters: Vec<ValueType>, return_type: ValueType) -> Signature {
    Signature {
        parameters: parameters
            .into_iter()
            .enumerate()
            .map(|(index, ty)| Parameter {
                index,
                ty,
                span: span(),
            })
            .collect(),
        return_type,
        receiver: None,
    }
}

fn function(code: Vec<Quadruple>) -> Function {
    Function {
        name: "main".into(),
        signature: signature(vec![], ValueType::Void),
        locals: Vec::new(),
        code,
        span: span(),
    }
}

fn program(main: Function, functions: Vec<Function>) -> QuadrupleProgram {
    QuadrupleProgram {
        main: FunctionId(0),
        functions: std::iter::once(main).chain(functions).collect(),
        runtime_functions: Vec::new(),
        strings: Vec::new(),
        classes: Vec::new(),
        fields: Vec::new(),
        statics: Vec::new(),
    }
}

#[test]
fn accepts_typed_i32_and_f64_operations() {
    let integer = Temporary {
        index: 0,
        ty: ValueType::I32,
    };
    let decimal = Temporary {
        index: 1,
        ty: ValueType::F64,
    };
    let sum = Temporary {
        index: 2,
        ty: ValueType::F64,
    };
    let main = function(vec![
        quad(Operation::I32ToF64 {
            source: Operand::I32(7),
            result: decimal,
        }),
        quad(Operation::Copy {
            source: Operand::I32(3),
            result: integer,
        }),
        quad(Operation::Add {
            left: Operand::Temporary(decimal),
            right: Operand::F64(0.5),
            result: sum,
        }),
        quad(Operation::Halt),
    ]);
    assert!(validate(&program(main, vec![])).is_ok());
}

#[test]
fn rejects_i32_to_f64_with_a_non_i32_source() {
    let result = Temporary {
        index: 0,
        ty: ValueType::F64,
    };
    let main = function(vec![
        quad(Operation::I32ToF64 {
            source: Operand::F64(1.0),
            result,
        }),
        quad(Operation::Halt),
    ]);
    assert_eq!(validate(&program(main, vec![])).unwrap_err().code, "E7001");
}

#[test]
fn rejects_unknown_labels() {
    let main = function(vec![
        quad(Operation::Jump(compiler_core::quadruple::LabelId(99))),
        quad(Operation::Halt),
    ]);
    assert_eq!(validate(&program(main, vec![])).unwrap_err().code, "E7001");
}

#[test]
fn rejects_invalid_call_parameter_count_and_type() {
    let callee = Function {
        name: "sum".into(),
        signature: signature(vec![ValueType::I32], ValueType::I32),
        locals: Vec::new(),
        code: vec![quad(Operation::Return(Some(Operand::Parameter {
            index: 0,
            ty: ValueType::I32,
        })))],
        span: span(),
    };
    let result = Temporary {
        index: 0,
        ty: ValueType::I32,
    };
    let main = function(vec![
        quad(Operation::Param {
            call: CallSiteId(0),
            index: 0,
            value: Operand::F64(1.0),
        }),
        quad(Operation::Call {
            call: CallSiteId(0),
            target: CallTarget::Function(FunctionId(1)),
            result: Some(result),
        }),
        quad(Operation::Halt),
    ]);
    assert_eq!(
        validate(&program(main, vec![callee])).unwrap_err().code,
        "E7001"
    );
}

#[test]
fn rejects_a_temporary_not_defined_on_every_branch() {
    let condition = Temporary {
        index: 0,
        ty: ValueType::Bool,
    };
    let value = Temporary {
        index: 1,
        ty: ValueType::I32,
    };
    let copied = Temporary {
        index: 2,
        ty: ValueType::I32,
    };
    let main = function(vec![
        quad(Operation::Eq {
            left: Operand::I32(1),
            right: Operand::I32(1),
            result: condition,
        }),
        quad(Operation::JumpIfFalse {
            condition: Operand::Temporary(condition),
            target: compiler_core::quadruple::LabelId(0),
        }),
        quad(Operation::Copy {
            source: Operand::I32(1),
            result: value,
        }),
        quad(Operation::Label(compiler_core::quadruple::LabelId(0))),
        quad(Operation::Copy {
            source: Operand::Temporary(value),
            result: copied,
        }),
        quad(Operation::Halt),
    ]);
    assert_eq!(validate(&program(main, vec![])).unwrap_err().code, "E7001");
}

fn method(class: ClassId) -> Function {
    let receiver = Temporary {
        index: 0,
        ty: ValueType::Object(class),
    };
    Function {
        name: "Counter.read".into(),
        signature: Signature {
            parameters: vec![Parameter {
                index: 0,
                ty: ValueType::I32,
                span: span(),
            }],
            return_type: ValueType::I32,
            receiver: Some(class),
        },
        locals: Vec::new(),
        code: vec![
            quad(Operation::Copy {
                source: Operand::Receiver { class },
                result: receiver,
            }),
            quad(Operation::Return(Some(Operand::Parameter {
                index: 0,
                ty: ValueType::I32,
            }))),
        ],
        span: span(),
    }
}

fn with_counter(main: Function, functions: Vec<Function>) -> QuadrupleProgram {
    let mut ir = program(main, functions);
    ir.classes.push(Class {
        fields: Vec::new(),
        statics: Vec::new(),
        constructor: None,
        span: span(),
    });
    ir
}

#[test]
fn accepts_direct_static_dispatch_with_explicit_receiver() {
    let result = Temporary {
        index: 0,
        ty: ValueType::I32,
    };
    let caller = Function {
        name: "caller".into(),
        signature: signature(vec![ValueType::Object(ClassId(0))], ValueType::I32),
        locals: Vec::new(),
        code: vec![
            quad(Operation::Param {
                call: CallSiteId(1),
                index: 0,
                value: Operand::Parameter {
                    index: 0,
                    ty: ValueType::Object(ClassId(0)),
                },
            }),
            quad(Operation::Param {
                call: CallSiteId(1),
                index: 1,
                value: Operand::I32(7),
            }),
            quad(Operation::Call {
                call: CallSiteId(1),
                target: CallTarget::Function(FunctionId(1)),
                result: Some(result),
            }),
            quad(Operation::Return(Some(Operand::Temporary(result)))),
        ],
        span: span(),
    };
    assert!(
        validate(&with_counter(
            function(vec![quad(Operation::Halt)]),
            vec![method(ClassId(0)), caller]
        ))
        .is_ok()
    );
}

#[test]
fn accepts_bound_method_call_without_a_repeated_receiver() {
    let bound = Temporary {
        index: 0,
        ty: ValueType::BoundMethod(FunctionId(1)),
    };
    let result = Temporary {
        index: 1,
        ty: ValueType::I32,
    };
    let caller = Function {
        name: "caller".into(),
        signature: signature(vec![ValueType::Object(ClassId(0))], ValueType::I32),
        locals: Vec::new(),
        code: vec![
            quad(Operation::BindMethod {
                receiver: Operand::Parameter {
                    index: 0,
                    ty: ValueType::Object(ClassId(0)),
                },
                method: FunctionId(1),
                result: bound,
            }),
            quad(Operation::Param {
                call: CallSiteId(2),
                index: 0,
                value: Operand::I32(9),
            }),
            quad(Operation::Call {
                call: CallSiteId(2),
                target: CallTarget::BoundMethod(bound),
                result: Some(result),
            }),
            quad(Operation::Return(Some(Operand::Temporary(result)))),
        ],
        span: span(),
    };
    assert!(
        validate(&with_counter(
            function(vec![quad(Operation::Halt)]),
            vec![method(ClassId(0)), caller]
        ))
        .is_ok()
    );
}

#[test]
fn rejects_wrong_or_missing_direct_method_receiver() {
    let wrong_receiver = Function {
        name: "wrong_receiver".into(),
        signature: signature(vec![ValueType::I32], ValueType::I32),
        locals: Vec::new(),
        code: vec![
            quad(Operation::Param {
                call: CallSiteId(3),
                index: 0,
                value: Operand::Parameter {
                    index: 0,
                    ty: ValueType::I32,
                },
            }),
            quad(Operation::Param {
                call: CallSiteId(3),
                index: 1,
                value: Operand::I32(1),
            }),
            quad(Operation::Call {
                call: CallSiteId(3),
                target: CallTarget::Function(FunctionId(1)),
                result: Some(Temporary {
                    index: 0,
                    ty: ValueType::I32,
                }),
            }),
            quad(Operation::Return(Some(Operand::Temporary(Temporary {
                index: 0,
                ty: ValueType::I32,
            })))),
        ],
        span: span(),
    };
    let missing_receiver = Function {
        name: "missing_receiver".into(),
        signature: signature(vec![], ValueType::I32),
        locals: Vec::new(),
        code: vec![
            quad(Operation::Call {
                call: CallSiteId(4),
                target: CallTarget::Function(FunctionId(1)),
                result: Some(Temporary {
                    index: 0,
                    ty: ValueType::I32,
                }),
            }),
            quad(Operation::Return(Some(Operand::Temporary(Temporary {
                index: 0,
                ty: ValueType::I32,
            })))),
        ],
        span: span(),
    };
    assert!(
        validate(&with_counter(
            function(vec![quad(Operation::Halt)]),
            vec![method(ClassId(0)), wrong_receiver]
        ))
        .is_err()
    );
    assert!(
        validate(&with_counter(
            function(vec![quad(Operation::Halt)]),
            vec![method(ClassId(0)), missing_receiver]
        ))
        .is_err()
    );
}

#[test]
fn rejects_undefined_or_partial_branch_bound_method_temporaries() {
    let bound = Temporary {
        index: 0,
        ty: ValueType::BoundMethod(FunctionId(1)),
    };
    let undefined = Function {
        name: "undefined".into(),
        signature: signature(vec![], ValueType::I32),
        locals: Vec::new(),
        code: vec![
            quad(Operation::Param {
                call: CallSiteId(5),
                index: 0,
                value: Operand::I32(1),
            }),
            quad(Operation::Call {
                call: CallSiteId(5),
                target: CallTarget::BoundMethod(bound),
                result: Some(Temporary {
                    index: 1,
                    ty: ValueType::I32,
                }),
            }),
            quad(Operation::Return(Some(Operand::Temporary(Temporary {
                index: 1,
                ty: ValueType::I32,
            })))),
        ],
        span: span(),
    };
    assert!(
        validate(&with_counter(
            function(vec![quad(Operation::Halt)]),
            vec![method(ClassId(0)), undefined]
        ))
        .is_err()
    );

    let forged = Function {
        name: "forged".into(),
        signature: signature(vec![], ValueType::I32),
        locals: Vec::new(),
        code: vec![
            quad(Operation::Call {
                call: CallSiteId(7),
                target: CallTarget::BoundMethod(Temporary {
                    index: 0,
                    ty: ValueType::BoundMethod(FunctionId(0)),
                }),
                result: Some(Temporary {
                    index: 1,
                    ty: ValueType::I32,
                }),
            }),
            quad(Operation::Return(Some(Operand::Temporary(Temporary {
                index: 1,
                ty: ValueType::I32,
            })))),
        ],
        span: span(),
    };
    assert!(
        validate(&with_counter(
            function(vec![quad(Operation::Halt)]),
            vec![method(ClassId(0)), forged]
        ))
        .is_err()
    );

    let condition = Temporary {
        index: 2,
        ty: ValueType::Bool,
    };
    let partial = Function {
        name: "partial".into(),
        signature: signature(vec![ValueType::Object(ClassId(0))], ValueType::I32),
        locals: Vec::new(),
        code: vec![
            quad(Operation::Eq {
                left: Operand::I32(1),
                right: Operand::I32(1),
                result: condition,
            }),
            quad(Operation::JumpIfFalse {
                condition: Operand::Temporary(condition),
                target: LabelId(6),
            }),
            quad(Operation::BindMethod {
                receiver: Operand::Parameter {
                    index: 0,
                    ty: ValueType::Object(ClassId(0)),
                },
                method: FunctionId(1),
                result: bound,
            }),
            quad(Operation::Label(LabelId(6))),
            quad(Operation::Param {
                call: CallSiteId(6),
                index: 0,
                value: Operand::I32(1),
            }),
            quad(Operation::Call {
                call: CallSiteId(6),
                target: CallTarget::BoundMethod(bound),
                result: Some(Temporary {
                    index: 3,
                    ty: ValueType::I32,
                }),
            }),
            quad(Operation::Return(Some(Operand::Temporary(Temporary {
                index: 3,
                ty: ValueType::I32,
            })))),
        ],
        span: span(),
    };
    assert!(
        validate(&with_counter(
            function(vec![quad(Operation::Halt)]),
            vec![method(ClassId(0)), partial]
        ))
        .is_err()
    );
}

#[test]
fn accepts_cfg_backedge_and_rejects_invalid_main_or_fallthrough() {
    let value = Temporary {
        index: 0,
        ty: ValueType::I32,
    };
    let condition = Temporary {
        index: 1,
        ty: ValueType::Bool,
    };
    let loop_main = function(vec![
        quad(Operation::Copy {
            source: Operand::I32(1),
            result: value,
        }),
        quad(Operation::Label(LabelId(0))),
        quad(Operation::Eq {
            left: Operand::Temporary(value),
            right: Operand::I32(1),
            result: condition,
        }),
        quad(Operation::JumpIfFalse {
            condition: Operand::Temporary(condition),
            target: LabelId(0),
        }),
        quad(Operation::Halt),
    ]);
    assert!(validate(&program(loop_main, vec![])).is_ok());

    let mut invalid_main = function(vec![quad(Operation::Halt)]);
    invalid_main.signature = signature(vec![ValueType::I32], ValueType::Void);
    assert!(validate(&program(invalid_main, vec![])).is_err());

    let fallthrough = Function {
        name: "fallthrough".into(),
        signature: signature(vec![], ValueType::Void),
        locals: Vec::new(),
        code: vec![quad(Operation::Copy {
            source: Operand::I32(1),
            result: value,
        })],
        span: span(),
    };
    assert!(
        validate(&program(
            function(vec![quad(Operation::Halt)]),
            vec![fallthrough]
        ))
        .is_err()
    );
}

#[test]
fn rejects_undeclared_local_slot() {
    let local = LocalSlot {
        index: 0,
        ty: ValueType::I32,
    };
    let result = Temporary {
        index: 0,
        ty: ValueType::I32,
    };
    let main = function(vec![
        quad(Operation::LoadLocal { local, result }),
        quad(Operation::Halt),
    ]);
    assert_eq!(validate(&program(main, vec![])).unwrap_err().code, "E7001");
}

#[test]
fn rejects_local_slot_with_a_forged_type() {
    let declared = LocalSlot {
        index: 0,
        ty: ValueType::I32,
    };
    let forged = LocalSlot {
        index: 0,
        ty: ValueType::F64,
    };
    let mut main = function(vec![
        quad(Operation::StoreLocal {
            local: declared,
            value: Operand::I32(1),
        }),
        quad(Operation::LoadLocal {
            local: forged,
            result: Temporary {
                index: 0,
                ty: ValueType::F64,
            },
        }),
        quad(Operation::Halt),
    ]);
    main.locals.push(declared);
    assert_eq!(validate(&program(main, vec![])).unwrap_err().code, "E7001");
}

#[test]
fn rejects_straight_line_uninitialized_local_load() {
    let local = LocalSlot {
        index: 0,
        ty: ValueType::I32,
    };
    let mut main = function(vec![
        quad(Operation::LoadLocal {
            local,
            result: Temporary {
                index: 0,
                ty: ValueType::I32,
            },
        }),
        quad(Operation::Halt),
    ]);
    main.locals.push(local);
    assert_eq!(validate(&program(main, vec![])).unwrap_err().code, "E7001");
}

#[test]
fn rejects_local_initialized_on_only_one_branch() {
    let local = LocalSlot {
        index: 0,
        ty: ValueType::I32,
    };
    let mut main = function(vec![
        quad(Operation::JumpIfFalse {
            condition: Operand::Bool(true),
            target: LabelId(0),
        }),
        quad(Operation::StoreLocal {
            local,
            value: Operand::I32(1),
        }),
        quad(Operation::Label(LabelId(0))),
        quad(Operation::LoadLocal {
            local,
            result: Temporary {
                index: 0,
                ty: ValueType::I32,
            },
        }),
        quad(Operation::Halt),
    ]);
    main.locals.push(local);
    assert_eq!(validate(&program(main, vec![])).unwrap_err().code, "E7001");
}

#[test]
fn rejects_local_initialized_only_by_a_zero_iteration_loop() {
    let local = LocalSlot {
        index: 0,
        ty: ValueType::I32,
    };
    let mut main = function(vec![
        quad(Operation::Label(LabelId(0))),
        quad(Operation::JumpIfFalse {
            condition: Operand::Bool(false),
            target: LabelId(1),
        }),
        quad(Operation::StoreLocal {
            local,
            value: Operand::I32(1),
        }),
        quad(Operation::Jump(LabelId(0))),
        quad(Operation::Label(LabelId(1))),
        quad(Operation::LoadLocal {
            local,
            result: Temporary {
                index: 0,
                ty: ValueType::I32,
            },
        }),
        quad(Operation::Halt),
    ]);
    main.locals.push(local);
    assert_eq!(validate(&program(main, vec![])).unwrap_err().code, "E7001");
}

#[test]
fn accepts_initialized_local_mutated_in_a_loop() {
    let local = LocalSlot {
        index: 0,
        ty: ValueType::I32,
    };
    let value = Temporary {
        index: 0,
        ty: ValueType::I32,
    };
    let result = Temporary {
        index: 1,
        ty: ValueType::I32,
    };
    let mut main = function(vec![
        quad(Operation::StoreLocal {
            local,
            value: Operand::I32(0),
        }),
        quad(Operation::Label(LabelId(0))),
        quad(Operation::JumpIfFalse {
            condition: Operand::Bool(false),
            target: LabelId(1),
        }),
        quad(Operation::LoadLocal {
            local,
            result: value,
        }),
        quad(Operation::StoreLocal {
            local,
            value: Operand::Temporary(value),
        }),
        quad(Operation::Jump(LabelId(0))),
        quad(Operation::Label(LabelId(1))),
        quad(Operation::LoadLocal { local, result }),
        quad(Operation::Halt),
    ]);
    main.locals.push(local);
    assert!(validate(&program(main, vec![])).is_ok());
}

#[test]
fn optimizer_forwards_only_immediate_local_snapshots_and_stops_at_effects() {
    let local = LocalSlot {
        index: 0,
        ty: ValueType::I32,
    };
    let before_call = Temporary {
        index: 0,
        ty: ValueType::I32,
    };
    let after_call = Temporary {
        index: 1,
        ty: ValueType::I32,
    };
    let after_branch = Temporary {
        index: 2,
        ty: ValueType::I32,
    };
    let mut main = function(vec![
        quad(Operation::StoreLocal {
            local,
            value: Operand::I32(1),
        }),
        quad(Operation::LoadLocal {
            local,
            result: before_call,
        }),
        quad(Operation::Param {
            call: CallSiteId(0),
            index: 0,
            value: Operand::Temporary(before_call),
        }),
        quad(Operation::Call {
            call: CallSiteId(0),
            target: CallTarget::Runtime(RuntimeFunctionId(0)),
            result: None,
        }),
        // A runtime call can mutate a captured local; this load must retain
        // its slot read rather than reuse the pre-call `1` snapshot.
        quad(Operation::LoadLocal {
            local,
            result: after_call,
        }),
        quad(Operation::Param {
            call: CallSiteId(1),
            index: 0,
            value: Operand::Temporary(after_call),
        }),
        quad(Operation::Call {
            call: CallSiteId(1),
            target: CallTarget::Runtime(RuntimeFunctionId(1)),
            result: None,
        }),
        quad(Operation::JumpIfFalse {
            condition: Operand::Bool(true),
            target: LabelId(0),
        }),
        quad(Operation::StoreLocal {
            local,
            value: Operand::I32(2),
        }),
        quad(Operation::Label(LabelId(0))),
        // Branch joins likewise have no single local snapshot to forward.
        quad(Operation::LoadLocal {
            local,
            result: after_branch,
        }),
        quad(Operation::Param {
            call: CallSiteId(2),
            index: 0,
            value: Operand::Temporary(after_branch),
        }),
        quad(Operation::Call {
            call: CallSiteId(2),
            target: CallTarget::Runtime(RuntimeFunctionId(2)),
            result: None,
        }),
        quad(Operation::Halt),
    ]);
    main.locals.push(local);
    let mut ir = program(main, vec![]);
    ir.runtime_functions = (0..3)
        .map(|index| RuntimeFunction {
            name: format!("observe_{index}"),
            signature: signature(vec![ValueType::I32], ValueType::Void),
            span: span(),
        })
        .collect();

    let optimized = compiler_core::quadruple_optimize::optimize(&ir).expect("optimized IR");
    assert!(validate(&optimized).is_ok());
    let code = &optimized.functions[0].code;
    assert!(code.iter().any(|quad| matches!(
        quad.operation,
        Operation::Param {
            call: CallSiteId(0),
            value: Operand::I32(1),
            ..
        }
    )));
    assert!(code.iter().any(|quad| matches!(
        quad.operation,
        Operation::LoadLocal { result, .. } if result == after_call
    )));
    assert!(code.iter().any(|quad| matches!(
        quad.operation,
        Operation::LoadLocal { result, .. } if result == after_branch
    )));
    assert!(code.iter().any(|quad| matches!(
        quad.operation,
        Operation::Param {
            call: CallSiteId(0),
            ..
        }
    )));
}

#[test]
fn supports_declared_runtime_call_without_the_vm() {
    let result = Temporary {
        index: 0,
        ty: ValueType::I32,
    };
    let main = function(vec![
        quad(Operation::Param {
            call: CallSiteId(4),
            index: 0,
            value: Operand::I32(2),
        }),
        quad(Operation::Call {
            call: CallSiteId(4),
            target: CallTarget::Runtime(RuntimeFunctionId(0)),
            result: Some(result),
        }),
        quad(Operation::Halt),
    ]);
    let mut ir = program(main, vec![]);
    ir.runtime_functions.push(RuntimeFunction {
        name: "native_square".into(),
        signature: signature(vec![ValueType::I32], ValueType::I32),
        span: span(),
    });
    assert!(validate(&ir).is_ok());
}
