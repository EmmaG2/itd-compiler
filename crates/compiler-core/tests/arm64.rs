use std::{
    fs,
    io::Read,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant, SystemTime},
};

use compiler_core::{
    compile_arm64, compile_quadruples, emit_arm64,
    quadruple::{
        Function, FunctionId, LocalSlot, Operation, Quadruple, QuadrupleProgram, Signature,
        Temporary, ValueType,
    },
    source::{SourceId, Span},
};

static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

const DECIMAL_FORMAT_PROGRAM: &str =
    include_str!("../../../docs/language/examples/arm64-optimization.itd");
const CONSTRUCTOR_FORWARDING_PROGRAM: &str = "
    fn seed_i() -> int { return 4; }
    fn seed_f() -> decimal { return 1.5dec; }
    class Box {
        pub let mut value: int := 0;
        pub fn this(initial: int) { this.value = initial; }
        pub fn read() -> int { return this.value; }
    }
    class DecimalBox {
        pub let mut value: decimal := 0dec;
        pub fn this(initial: decimal) { this.value = initial; }
        pub fn read() -> decimal { return this.value; }
    }
    let box := Box(seed_i());
    let decimal_box := DecimalBox(seed_f());
    print(to_string(box.read()));
    print(to_string(decimal_box.read()));
";

fn native_available() -> bool {
    cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")
}

fn run_native_result(source: &str) -> (std::process::ExitStatus, String) {
    assert!(
        native_available(),
        "native ARM64 tests require Apple Silicon"
    );
    let result = compile_arm64(source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "itd-arm64-{}-{nonce}-{sequence}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("temporary directory");
    let input = directory.join("program.s");
    let executable = directory.join("program");
    fs::write(&input, assembly).expect("assembly file");
    let linked = Command::new("clang")
        .arg(&input)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("Apple clang");
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let mut child = Command::new(&executable)
        .stdout(Stdio::piped())
        .spawn()
        .expect("native executable");
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait().expect("child state") {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill timed out native executable");
            child.wait().expect("reap timed out native executable");
            panic!("native executable timed out");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = Vec::new();
    child
        .stdout
        .take()
        .expect("captured stdout")
        .read_to_end(&mut stdout)
        .expect("native output");
    let _ = fs::remove_dir_all(&directory);
    (status, String::from_utf8(stdout).expect("UTF-8 output"))
}

fn run_native(source: &str) -> String {
    let (status, output) = run_native_result(source);
    assert!(status.success(), "native status: {status}");
    output
}

#[test]
fn emitter_accepts_only_validated_quadruples() {
    let mut program = compile_quadruples("print(1);").program.expect("IR");
    assert!(emit_arm64(&program).is_ok());
    program.main.0 = 99;
    assert_eq!(emit_arm64(&program).unwrap_err().code, "E7001");
}

#[test]
fn optimizer_folds_constants_and_emits_only_reachable_artifacts() {
    let constants = compile_quadruples("let value: int := 2 + 3 * 4; print(value);")
        .program
        .expect("valid IR");
    let optimized = compiler_core::quadruple_optimize::optimize(&constants).expect("optimized IR");
    assert!(compiler_core::quadruple::validate(&optimized).is_ok());
    let assembly = emit_arm64(&constants).expect("assembly");
    assert!(!assembly.contains("smull"));
    assert!(!assembly.contains("adds w9"));

    let branches = compile_quadruples(
        "fn unused() -> int { return 99; } if false { print(\"dead\"); } print(\"live\");",
    )
    .program
    .expect("valid IR");
    let assembly = emit_arm64(&branches).expect("assembly");
    assert!(assembly.contains("live"));
    assert!(!assembly.contains("dead"));
    assert!(!assembly.contains("_itd_f1:"));

    let dynamic = compile_quadruples("let mut zero: int := 0; print(1 / zero);")
        .program
        .expect("valid IR");
    let assembly = emit_arm64(&dynamic).expect("assembly");
    assert!(assembly.contains("sdiv"));
    assert!(assembly.contains("_itd_trap"));
}

#[test]
fn optimizer_keeps_cfg_joins_dynamic_branches_and_integer_traps() {
    let source = "
        fn choose(branch: bool, value: int) -> int {
            let mut result: int := value;
            if branch { result = 9; }
            return result;
        }
        let mut branch: bool := true;
        print(choose(branch, 7));
    ";
    let program = compile_quadruples(source).program.expect("valid source IR");
    let optimized = compiler_core::quadruple_optimize::optimize(&program).expect("optimized IR");
    assert!(compiler_core::quadruple::validate(&optimized).is_ok());
    assert!(
        optimized
            .functions
            .iter()
            .flat_map(|function| &function.code)
            .any(|quad| matches!(
                quad.operation,
                Operation::JumpIfFalse {
                    condition: compiler_core::quadruple::Operand::Temporary(_),
                    ..
                }
            ))
    );

    let dynamic = compile_arm64(source);
    assert!(dynamic.diagnostics.is_empty(), "{:?}", dynamic.diagnostics);
    let assembly = dynamic.assembly.expect("assembly");
    assert!(assembly.contains("cbz") || assembly.contains("b.eq"));

    for source in [
        "let mut value: int := 2147483647; let step: int := 1; value + step;",
        "let value: int := 1; let zero: int := 0; value / zero;",
    ] {
        let result = compile_arm64(source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let assembly = result.assembly.expect("assembly");
        assert!(assembly.contains("_itd_trap"), "missing trap for {source}");
    }
}

#[test]
fn optimizer_preserves_local_snapshots_across_calls_aliases_and_deferred_traps() {
    let effects = "
        class State { pub static let mut value: int := 0; }
        class Box { pub let mut value: int := 0; }
        fn change() -> int { State.value = State.value + 1; return State.value; }
        let mut x: int := 1;
        let box := Box();
        let alias := box;
        alias.value = 9;
        State.value = x + change();
        if false and (1 / 0 == 0) { print(to_string(99)); }
        print(to_string(State.value));
        print(to_string(box.value));
    ";
    let vm = compiler_core::run(effects);
    assert!(vm.diagnostics.is_empty(), "{:?}", vm.diagnostics);
    assert_eq!(vm.output, "2\n9\n");

    let arm64 = compile_arm64(effects);
    assert!(arm64.diagnostics.is_empty(), "{:?}", arm64.diagnostics);
    let assembly = arm64.assembly.expect("assembly");
    // The false short-circuit makes the division trap unreachable, while a
    // dynamic division remains a deferred trap instead of being optimized out.
    assert!(!assembly.contains("sdiv"));

    let deferred = compile_arm64("let mut zero: int := 0; if true { print(1 / zero); }");
    assert!(
        deferred.diagnostics.is_empty(),
        "{:?}",
        deferred.diagnostics
    );
    assert!(deferred.assembly.expect("assembly").contains("_itd_trap"));
}

#[test]
fn optimizer_never_folds_f64_and_removes_only_dead_pure_temporaries() {
    let decimals = compile_arm64("print(to_string(1dec + 2dec));");
    assert!(
        decimals.diagnostics.is_empty(),
        "{:?}",
        decimals.diagnostics
    );
    assert!(decimals.assembly.expect("assembly").contains("fadd"));

    let span = Span::new(SourceId(0), 0, 0);
    let dead = Temporary {
        index: 0,
        ty: ValueType::Bool,
    };
    let program = QuadrupleProgram {
        main: FunctionId(0),
        functions: vec![Function {
            name: "main".into(),
            signature: Signature {
                parameters: vec![],
                return_type: ValueType::Void,
                receiver: None,
            },
            locals: vec![],
            code: vec![
                Quadruple {
                    operation: Operation::Eq {
                        left: compiler_core::quadruple::Operand::I32(1),
                        right: compiler_core::quadruple::Operand::I32(1),
                        result: dead,
                    },
                    span,
                },
                Quadruple {
                    operation: Operation::Halt,
                    span,
                },
            ],
            span,
        }],
        runtime_functions: vec![],
        strings: vec![],
        classes: vec![],
        fields: vec![],
        statics: vec![],
    };
    let optimized = compiler_core::quadruple_optimize::optimize(&program).expect("optimized IR");
    assert!(compiler_core::quadruple::validate(&optimized).is_ok());
    assert_eq!(optimized.functions[0].code.len(), 1);
    assert!(matches!(
        optimized.functions[0].code[0].operation,
        Operation::Halt
    ));
}

#[test]
fn emitter_rejects_unsupported_abi_signatures_before_assembly() {
    for source in [
        "fn integers(a:int,b:int,c:int,d:int,e:int,f:int,g:int,h:int,i:int) -> int { return a; } integers(1,2,3,4,5,6,7,8,9);",
        "fn decimals(a:decimal,b:decimal,c:decimal,d:decimal,e:decimal,f:decimal,g:decimal,h:decimal,i:decimal) -> decimal { return a; } decimals(1dec,2dec,3dec,4dec,5dec,6dec,7dec,8dec,9dec);",
        "class C { pub fn method(a:int,b:int,c:int,d:int,e:int,f:int,g:int,h:int) -> int { return a; } } let c := C(); c.method(1,2,3,4,5,6,7,8);",
    ] {
        let result = compile_arm64(source);
        assert!(result.assembly.is_none());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E7201")
        );
    }
}

#[test]
fn emitter_uses_ieee_nan_comparison_conditions() {
    let span = Span::new(SourceId(0), 0, 0);
    let comparisons = [
        Operation::Eq {
            left: compiler_core::quadruple::Operand::F64(f64::NAN),
            right: compiler_core::quadruple::Operand::F64(1.0),
            result: Temporary {
                index: 0,
                ty: ValueType::Bool,
            },
        },
        Operation::Ne {
            left: compiler_core::quadruple::Operand::F64(f64::NAN),
            right: compiler_core::quadruple::Operand::F64(1.0),
            result: Temporary {
                index: 1,
                ty: ValueType::Bool,
            },
        },
        Operation::Lt {
            left: compiler_core::quadruple::Operand::F64(f64::NAN),
            right: compiler_core::quadruple::Operand::F64(1.0),
            result: Temporary {
                index: 2,
                ty: ValueType::Bool,
            },
        },
        Operation::Le {
            left: compiler_core::quadruple::Operand::F64(f64::NAN),
            right: compiler_core::quadruple::Operand::F64(1.0),
            result: Temporary {
                index: 3,
                ty: ValueType::Bool,
            },
        },
    ];
    let program = QuadrupleProgram {
        main: FunctionId(0),
        functions: vec![Function {
            name: "main".into(),
            signature: Signature {
                parameters: vec![],
                return_type: ValueType::Void,
                receiver: None,
            },
            locals: vec![],
            code: comparisons
                .into_iter()
                .map(|operation| Quadruple { operation, span })
                .chain(std::iter::once(Quadruple {
                    operation: Operation::Halt,
                    span,
                }))
                .collect(),
            span,
        }],
        runtime_functions: vec![],
        strings: vec![],
        classes: vec![],
        fields: vec![],
        statics: vec![],
    };
    assert!(compiler_core::quadruple::validate(&program).is_ok());
    let assembly = emit_arm64(&program).expect("assembly");
    assert!(assembly.contains("cset w9, eq"));
    assert!(assembly.contains("cset w9, ne"));
    assert!(assembly.contains("cset w9, lt"));
    assert!(assembly.contains("cset w9, le\n\tcset w10, vc\n\tand w9, w9, w10"));
}

#[test]
fn emitter_uses_fallthrough_aliases_minimal_frames_and_reachable_runtime_dependencies() {
    let hello = compile_quadruples("print(\"Hola mundo\");")
        .program
        .expect("valid IR");
    let assembly = emit_arm64(&hello).expect("assembly");
    assert!(assembly.contains("_main:\n_itd_f0:"));
    assert!(!assembly.contains("_main:\n\tb _itd_f0"));
    assert!(!assembly.contains("_itd_fmt_d"));
    assert!(!assembly.contains("_itd_fmt_f"));
    assert!(!assembly.contains("_itd_bool_true"));
    assert!(!assembly.contains("_itd_trap:"));
    assert!(!assembly.contains("_itd_to_string_"));
    assert!(!assembly.contains("_itd_str_retain:"));
    assert!(!assembly.contains("_itd_str_release:"));
    assert!(assembly.contains("_itd_print_str:"));
    assert!(assembly.contains("_itd_fmt_s:"));

    let span = Span::new(SourceId(0), 0, 0);
    let leaf = QuadrupleProgram {
        main: FunctionId(0),
        functions: vec![Function {
            name: "main".into(),
            signature: Signature {
                parameters: vec![],
                return_type: ValueType::Void,
                receiver: None,
            },
            locals: vec![],
            code: vec![Quadruple {
                operation: Operation::Halt,
                span,
            }],
            span,
        }],
        runtime_functions: vec![],
        strings: vec![],
        classes: vec![],
        fields: vec![],
        statics: vec![],
    };
    let assembly = emit_arm64(&leaf).expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");
    assert!(!main.contains("stp x29, x30"));
    assert!(!main.contains("sub sp, sp"));
    assert!(!main.contains("b .L0_epilogue"));

    let nonleaf = compile_quadruples("fn id(value: int) -> int { return value; } id(1);")
        .program
        .expect("valid IR");
    let assembly = emit_arm64(&nonleaf).expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");
    assert!(main.contains("stp x29, x30"));
    assert!(assembly.contains("ldp x29, x30, [sp], #16"));
}

#[test]
fn emitter_reuses_cfg_temp_slots_and_loads_call_arguments_directly() {
    let source = "
        fn bump(value: int) -> int { return value + 1; }
        fn choose(value: int, branch: bool) -> int {
            if branch { return bump(value); }
            return value;
        }
        let mut index: int := 0;
        let mut total: int := 0;
        while index < 4 {
            total = total + choose(index, index < 3);
            index = index + 1;
        }
        print(total);
    ";
    let result = compile_arm64(source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    assert!(assembly.contains("bl _itd_f1"));
    assert!(assembly.contains("bl _itd_f2"));
    // Temporary arguments are loaded into ABI registers from their reusable
    // slots, avoiding a scratch-register move at each dynamic call edge.
    assert!(assembly.contains("\tldr w0, [sp,"));
}

#[test]
fn emitter_keeps_single_use_decimal_arithmetic_in_d16() {
    let result = compile_arm64("print(to_string((1dec + 2dec) * 3dec));");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");
    assert!(main.contains("\tfadd d16, d16, d17"));
    assert!(main.contains("\tfmul d16, d16, d17"));
    assert!(!main.contains("\tfadd d16, d16, d17\n\tstr d16, [sp,"));
    assert_eq!(main.matches("str d16, [sp,").count(), 0);
    assert_eq!(main.matches("ldr d16, [sp,").count(), 0);
}

#[test]
fn decimal_metrics_fixture_preserves_vm_output_and_arm64_diagnostics() {
    let vm = compiler_core::run(DECIMAL_FORMAT_PROGRAM);
    assert!(vm.diagnostics.is_empty(), "{:?}", vm.diagnostics);
    assert_eq!(vm.output, "0.6\n0.16\n0.09\n");

    let arm64 = compile_arm64(DECIMAL_FORMAT_PROGRAM);
    assert!(arm64.diagnostics.is_empty(), "{:?}", arm64.diagnostics);
    assert!(arm64.assembly.is_some());
}

#[test]
fn emitter_preserves_decimal_format_program_and_uses_direct_operands() {
    let result = compile_arm64(DECIMAL_FORMAT_PROGRAM);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");

    assert!(assembly.contains("_itd_fmt_f: .asciz \"%g\""));
    // The full source exercises f64 operations without changing association;
    // the dedicated single-use regression above proves spill elision separately.
    assert!(main.contains("\tfadd d16, d16, d17"));
    assert!(main.contains("\tfmul d16, d16, d17"));
    assert!(main.contains("\tfmov d17, d16"));
}

#[test]
fn emitter_keeps_scalar_temporaries_resident_until_their_last_use() {
    let result = compile_arm64(
        "let mut base: int := 2; print(to_string((2dec + 3dec) * 4dec)); print(to_string((base + 3) * 4));",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");
    // The final scalar argument is preloaded before earlier argument lowering
    // can reuse d16/w9, and therefore never needs a temporary stack roundtrip.
    assert!(main.contains("fmov d0, d16"));
    assert!(main.contains("\tadds w9, w9, w10"));
    assert!(!main.contains("fmul d16, d16, d17\n\tstr d16, [sp,"));
    assert!(!main.contains("adds w9, w9, w10\n\tb.vs _itd_trap\n\tstr w9, [sp,"));
}

#[test]
fn emitter_coalesces_linear_scalar_local_loads_and_copy_chains() {
    let result = compile_arm64(
        "let a: decimal := 0.6dec; let b: decimal := 0.1dec; let c: decimal := 0.09dec; let x: int := 2; let y: int := 3; let z: int := 4; print(to_string(a + b + c)); print(to_string(x + y + z));",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");

    // The right local feeds d16 and is copied to d17 only because the left
    // operand is materialized afterwards; the direct local-to-register path
    // avoids a temporary-stack round trip for that operand.
    assert!(main.contains(
        "\tldr d16, [sp, #40]\n\tfmov d17, d16\n\tldr d16, [sp, #0]\n\tfadd d16, d16, d17"
    ));
    assert!(main.matches("\tadds w9, w9, w10").count() >= 2);
}

#[test]
fn emitter_spills_decimal_call_results_before_arithmetic() {
    let result = compile_arm64(
        "fn source_value() -> decimal { return 1dec; } print(to_string(source_value() + 2dec));",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");
    assert!(main.contains("bl _itd_f1\n\tstr d0, [sp,"));
    assert!(main.contains("ldr d16, [sp,"));
}

#[test]
fn emitter_forwards_chained_decimal_call_results_without_spills() {
    let result = compile_arm64(
        "fn identity(value: decimal) -> decimal { return value; } print(to_string(identity(identity(1.5dec))));",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");
    assert!(main.contains("bl _itd_f1\n\tbl _itd_f1"));
    assert!(!main.contains("str d0, [sp,"));
    assert!(!main.contains("ldr d0, [sp,"));
}

#[test]
fn optimizer_preserves_referenced_labels_and_uses_valid_stack_adjustments() {
    let span = Span::new(SourceId(0), 0, 0);
    let bool_temp = Temporary {
        index: 0,
        ty: ValueType::Bool,
    };
    let program = QuadrupleProgram {
        main: FunctionId(0),
        functions: vec![Function {
            name: "main".into(),
            signature: Signature {
                parameters: vec![],
                return_type: ValueType::Void,
                receiver: None,
            },
            locals: vec![LocalSlot {
                index: 0,
                ty: ValueType::Bool,
            }],
            code: vec![
                Quadruple {
                    operation: Operation::StoreLocal {
                        local: LocalSlot {
                            index: 0,
                            ty: ValueType::Bool,
                        },
                        value: compiler_core::quadruple::Operand::Bool(true),
                    },
                    span,
                },
                Quadruple {
                    operation: Operation::LoadLocal {
                        local: LocalSlot {
                            index: 0,
                            ty: ValueType::Bool,
                        },
                        result: bool_temp,
                    },
                    span,
                },
                Quadruple {
                    operation: Operation::JumpIfFalse {
                        condition: compiler_core::quadruple::Operand::Temporary(bool_temp),
                        target: compiler_core::quadruple::LabelId(1),
                    },
                    span,
                },
                Quadruple {
                    operation: Operation::Jump(compiler_core::quadruple::LabelId(2)),
                    span,
                },
                Quadruple {
                    operation: Operation::Label(compiler_core::quadruple::LabelId(1)),
                    span,
                },
                Quadruple {
                    operation: Operation::Jump(compiler_core::quadruple::LabelId(2)),
                    span,
                },
                Quadruple {
                    operation: Operation::Label(compiler_core::quadruple::LabelId(2)),
                    span,
                },
                Quadruple {
                    operation: Operation::Halt,
                    span,
                },
            ],
            span,
        }],
        runtime_functions: vec![],
        strings: vec![],
        classes: vec![],
        fields: vec![],
        statics: vec![],
    };
    let packed = emit_arm64(&program).expect("packed bool frame");
    // The immediate local load is forwarded, but the bool home remains a
    // 4-byte store and must never be widened by frame packing.
    assert!(packed.contains("str w9"));
    assert!(!packed.contains("ldr x9, [sp, #4]"));

    let optimized = compiler_core::quadruple_optimize::optimize(&program).expect("optimized IR");
    // The forwarded true condition makes label 1 unreachable; label 2 and
    // its still-referenced jump retain their CFG identity.
    assert!(
        optimized.functions[0]
            .code
            .iter()
            .any(|quad| matches!(quad.operation, Operation::Label(label) if label.0 == 2))
    );
    assert!(compiler_core::quadruple::validate(&optimized).is_ok());

    let large = |count| QuadrupleProgram {
        main: FunctionId(0),
        functions: vec![Function {
            name: "main".into(),
            signature: Signature {
                parameters: vec![],
                return_type: ValueType::Void,
                receiver: None,
            },
            locals: (0..count)
                .map(|index| LocalSlot {
                    index,
                    ty: ValueType::I32,
                })
                .collect(),
            code: (0..count)
                .map(|index| Quadruple {
                    operation: Operation::Copy {
                        source: compiler_core::quadruple::Operand::I32(0),
                        result: Temporary {
                            index,
                            ty: ValueType::I32,
                        },
                    },
                    span,
                })
                .chain(std::iter::once(Quadruple {
                    operation: Operation::Halt,
                    span,
                }))
                .collect(),
            span,
        }],
        runtime_functions: vec![],
        strings: vec![],
        classes: vec![],
        fields: vec![],
        statics: vec![],
    };
    assert!(
        emit_arm64(&large(1023))
            .expect("assembly")
            .contains("sub sp, sp, #1, lsl #12")
    );
    let assembly = emit_arm64(&large(1025)).expect("assembly");
    assert!(assembly.contains("movz x15"));
    assert!(assembly.contains("sub sp, sp, x15"));
}

#[test]
fn emitter_materializes_call_results_before_constructor_malloc() {
    let vm = compiler_core::run(CONSTRUCTOR_FORWARDING_PROGRAM);
    assert!(vm.diagnostics.is_empty(), "{:?}", vm.diagnostics);
    assert_eq!(vm.output, "4\n1.5\n");

    let result = compile_arm64(CONSTRUCTOR_FORWARDING_PROGRAM);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");

    // Both forwarded function results receive a home before constructor
    // allocation, then reload into x1/d0 after `_malloc` clobbers x0/w0/d0.
    let first_malloc = main.find("\tbl _malloc").expect("first malloc");
    assert!(main[..first_malloc].contains("bl _itd_f1\n\tstr w0, [sp,"));
    assert!(main[first_malloc..].contains("\tldr w1, [sp,"));
    let second_malloc = main[first_malloc + 1..]
        .find("\tbl _malloc")
        .map(|offset| first_malloc + 1 + offset)
        .expect("second malloc");
    assert!(main[first_malloc..second_malloc].contains("bl _itd_f2\n\tstr d0, [sp,"));
    assert!(main[second_malloc..].contains("\tldr d0, [sp,"));
}

#[test]
fn emitter_forwards_owned_to_string_result_to_immediate_print_without_reload() {
    let result = compile_arm64("print(to_string(42));");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    let main = assembly.split(".L0_epilogue:").next().expect("main body");

    // The returned tagged pointer is first owned by its stack home, then x0 is
    // restored for the proven PARAM -> print edge rather than reloaded.
    assert!(main.contains("bl _itd_to_string_i32\n\tmov x10, x0"));
    assert!(main.contains("\tstr x10, [sp,"));
    assert!(main.contains("\tmov x0, x10\n\tbl _itd_print_str"));
    assert!(!assembly.contains("_itd_str_retain:"));
    assert!(assembly.contains("_itd_str_release:"));
}

#[test]
fn emitter_keeps_string_tag_domains_distinct_and_retain_reachable_only_for_aliases() {
    let literals =
        compile_arm64("print(\"literal\"); print(to_string(true)); print(to_string(1));");
    assert!(
        literals.diagnostics.is_empty(),
        "{:?}",
        literals.diagnostics
    );
    let assembly = literals.assembly.expect("assembly");
    // Static literal and bool storage are aligned, so tag bit #1 stays clear;
    // numeric allocation sets that bit only on the dynamic interior pointer.
    assert!(assembly.contains(".p2align 2\n_itd_str0:"));
    assert!(assembly.contains(".p2align 2\n_itd_bool_true:"));
    assert!(assembly.contains("\torr x0, x0, #2"));
    assert!(!assembly.contains("_itd_str_retain:"));

    let aliases = compile_arm64("let first := to_string(1); let alias := first; print(alias);");
    assert!(aliases.diagnostics.is_empty(), "{:?}", aliases.diagnostics);
    assert!(
        aliases
            .assembly
            .expect("assembly")
            .contains("_itd_str_retain:")
    );
}

#[test]
fn emitter_manages_dynamic_string_ownership_and_rejects_object_string_fields() {
    let result = compile_arm64(
        "fn relay(value: str) -> str { return to_string(value); } let first := to_string(42); let alias := first; let second := relay(alias); print(to_string(second));",
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let assembly = result.assembly.expect("assembly");
    assert!(assembly.contains("_itd_str_retain:"));
    assert!(assembly.contains("_itd_str_release:"));
    assert!(assembly.contains("bl _free"));
    assert!(assembly.contains("bl _snprintf"));
    assert!(assembly.contains("cmp w0, #64"));

    let rejected = compile_arm64(
        "class Leaking { pub let mut text: str := \"static\"; } let value := Leaking(); print(value.text);",
    );
    assert!(rejected.assembly.is_none());
    assert!(
        rejected
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E7201")
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_constructor_arguments_survive_malloc_after_i32_and_f64_calls() {
    assert_eq!(run_native(CONSTRUCTOR_FORWARDING_PROGRAM), "41.5");
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_direct_numeric_to_string_print_keeps_owned_reference_across_loops() {
    assert_eq!(
        run_native(
            "fn typed(value: int) -> str { return to_string(value); } let mut index: int := 0; while index < 20 { print(to_string(index)); print(typed(index)); index = index + 1; }",
        ),
        "001122334455667788991010111112121313141415151616171718181919",
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_string_ownership_covers_aliases_returns_loops_and_print_chains() {
    assert_eq!(
        run_native(
            "fn relay(value: str) -> str { return to_string(value); } let mut index: int := 0; while index < 20 { let dynamic := to_string(index); let alias := dynamic; print(to_string(relay(alias))); index = index + 1; } print(to_string(1.25dec)); print(to_string(true)); print(to_string(\"ok\"));",
        ),
        "0123456789101112131415161718191.25trueok",
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_decimal_metrics_fixture_uses_g_without_newlines() {
    assert_eq!(run_native(DECIMAL_FORMAT_PROGRAM), "0.60.160.09");
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_scalar_residency_preserves_nested_call_arguments() {
    assert_eq!(
        run_native(
            "fn pair(left: decimal, right: decimal) -> decimal { return left + right; } fn choose(left: int, right: int) -> int { return left * 10 + right; } let mut base: int := 2; while base < 5 { print(to_string(pair(1dec, (2dec + 3dec) * 4dec))); print(to_string(choose(base, (base + 1) + (base + 2)))); base = base + 1; }",
        ),
        "212721392151",
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_repeated_scalar_operands_preserve_dynamic_values() {
    assert_eq!(
        run_native(
            "fn square_i(value: int) -> int { let next: int := value + 1; return next * next; } fn square_f(value: decimal) -> decimal { let next: decimal := value + 1dec; return next * next; } let mut input: int := 3; print(to_string(square_i(input))); print(to_string(square_f(input as decimal)));",
        ),
        "1616",
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_replacing_dynamic_static_strings_releases_previous_value() {
    assert_eq!(
        run_native(
            "class Strings { pub static let mut value: str := \"initial\"; } let mut index: int := 0; while index < 20 { Strings.value = to_string(index); print(Strings.value); index = index + 1; }",
        ),
        "012345678910111213141516171819",
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_decimal_call_then_arithmetic_preserves_output() {
    assert_eq!(
        run_native(
            "fn source_value() -> decimal { return 1dec; } print(to_string(source_value() + 2dec));",
        ),
        "3",
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_f64_return_survives_dynamic_string_cleanup() {
    assert_eq!(
        run_native(
            "fn value() -> decimal { let text := to_string(1); return 1.5dec; } let mut index: int := 0; while index < 20 { print(to_string(value())); index = index + 1; }",
        ),
        "1.51.51.51.51.51.51.51.51.51.51.51.51.51.51.51.51.51.51.51.5",
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_i32_arithmetic_and_signed_division() {
    assert_eq!(
        run_native(
            "print(2 + 3 * 4); print(-7 / 3); print(-7 % 3); print(-2147483648 < 2147483647); print(2147483647 > -2147483648);"
        ),
        "14-2-111"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_calls_and_short_circuit() {
    assert_eq!(
        run_native(
            "fn twice(value: int) -> int { return value + value; } print(twice(twice(3))); if false and (1 / 0 == 0) { print(99); }"
        ),
        "12"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_pdf_decimal_balance_uses_to_string() {
    assert_eq!(
        run_native(
            "let mut balance: decimal := 100dec; let mut month: int := 1; while month <= 3 { balance = balance + 50dec; print(to_string(balance)); month = month + 1; }"
        ),
        "150200250"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_decimal_static_initializer_and_assignments_preserve_values() {
    assert_eq!(
        run_native(
            "class DecimalStatics { pub static let mut value: decimal := 1.25dec; } print(to_string(DecimalStatics.value)); DecimalStatics.value = 2.5dec; print(to_string(DecimalStatics.value)); let base: decimal := 3dec; DecimalStatics.value = base + 0.75dec; print(to_string(DecimalStatics.value));"
        ),
        "1.252.53.75"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_decimal_nan_comparisons_follow_ieee_rules() {
    assert_eq!(
        run_native(
            "let zero: decimal := 0dec; let nan: decimal := zero / zero; print(to_string(nan < 1dec)); print(to_string(1dec < nan)); print(to_string(nan <= 1dec)); print(to_string(1dec <= nan)); print(to_string(nan == 1dec)); print(to_string(1dec == nan)); print(to_string(nan != 1dec)); print(to_string(1dec != nan)); print(to_string(1dec < 2dec)); print(to_string(2dec <= 2dec));"
        ),
        "falsefalsefalsefalsefalsefalsetruetruetruetrue"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_mixed_f64_nested_calls_and_primitive_strings() {
    assert_eq!(
        run_native(
            "fn offset(count: int, value: decimal) -> decimal { return (count as decimal) + value; } fn nested(value: decimal) -> decimal { return offset(1, offset(-2, value)); } print(to_string(nested(0.5dec))); print(to_string(true)); print(to_string(\"ok\"));"
        ),
        "-0.5trueok"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_counter_constructor_alias_static_and_bound_method() {
    assert_eq!(
        run_native(
            "fn seed() -> int { return 1; } class Counter { pub let mut value: int := 0; pub static let mut total: int := 0; pub fn this(initial: int) { this.value = initial + seed(); } pub fn read() -> int { return this.value; } pub fn add(extra: int, ratio: decimal) -> decimal { return (this.value + extra) as decimal + ratio; } } let c := Counter(3); let alias := c; alias.value = 10; let read := c.read; Counter.total = 5; print(to_string(c.read())); print(to_string(read())); c.value = 7; print(to_string(read())); print(to_string(Counter.total)); print(to_string(c.add(2, 0.5dec))); let other := Counter(1); print(to_string(other.read())); print(to_string(c.read()));"
        ),
        "1010759.527"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_recursion_preserves_nested_frames() {
    assert_eq!(
        run_native(
            "fn sum(value: int) -> int { if value <= 0 { return 0; } return value + sum(value - 1); } print(sum(10));"
        ),
        "55"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_cfg_liveness_preserves_values_across_calls_and_branches() {
    assert_eq!(
        run_native(
            "fn bump(value: int) -> int { return value + 1; } let mut index: int := 0; let mut total: int := 10; while index < 4 { if index < 2 { total = total + bump(index); } else { total = total + bump(total); } index = index + 1; } print(total);",
        ),
        "55"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_local_coalescing_preserves_reassignments_calls_joins_and_loops() {
    assert_eq!(
        run_native(
            "fn bump(value: int) -> int { return value + 1; } let mut a: int := 1; let mut b: int := 2; let mut c: int := 3; let mut total: int := 0; let mut index: int := 0; while index < 4 { if index < 2 { total = total + (a + b + c); } else { a = bump(a); total = total + (a + b + c); } index = index + 1; } print(to_string(total));",
        ),
        "27",
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_chained_decimal_calls_and_live_values_preserve_output() {
    assert_eq!(
        run_native(
            "fn add(value: decimal) -> decimal { return value + 0.5dec; } fn keep(value: int, amount: decimal) -> decimal { return amount + (value as decimal); } let stable: int := 2; print(to_string(keep(stable, add(add(1dec)))));",
        ),
        "4"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_optimized_loop_and_call_preserve_effects() {
    assert_eq!(
        run_native(
            "fn twice(value: int) -> int { return value + value; } let mut i: int := 0; let mut total: int := 0; while i < 4 { total = total + twice(i); i = i + 1; } print(total);"
        ),
        "12"
    );
}

#[test]
#[ignore = "requires Apple Silicon and Apple clang"]
fn native_checked_integer_traps_exit_nonzero() {
    for source in [
        "let mut value: int := 2147483647; let step: int := 1; value = value + step;",
        "let mut value: int := -2147483648; let step: int := 1; value = value - step;",
        "let mut value: int := 50000; let factor: int := 50000; value = value * factor;",
        "let mut value: int := -2147483648; value = -value;",
        "let value: int := 1; let zero: int := 0; value / zero;",
        "let value: int := -2147483648; let divisor: int := -1; value / divisor;",
        "let value: int := 2; let exponent: int := -1; value ** exponent;",
        "let value: int := 2; let exponent: int := 31; value ** exponent;",
    ] {
        let (status, _) = run_native_result(source);
        assert!(!status.success(), "expected native trap: {source}");
    }
}
