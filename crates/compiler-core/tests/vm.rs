use compiler_core::{
    analyze,
    bytecode::{Instruction, compile, validate},
    run, run_with_options,
    runtime::RuntimeLimits,
};
use std::sync::{Arc, atomic::AtomicBool};

fn check(source: &str, output: &str) {
    let result = run(source);
    assert!(
        result.diagnostics.is_empty(),
        "{source}\n{:?}",
        result.diagnostics
    );
    assert_eq!(result.output, output);
    #[cfg(feature = "reference-interpreter")]
    {
        let analysis = analyze(source);
        let reference = compiler_core::runtime::execute_reference(
            &analysis.program,
            analysis.semantic.as_ref().unwrap(),
        );
        assert_eq!(
            result,
            compiler_core::RunResult {
                output: reference.output,
                diagnostics: reference.diagnostics
            }
        );
    }
}

#[test]
fn converts_integers_to_string_without_decimal_casts() {
    check(
        r#"
        print(to_string(0));
        print(to_string(-127i8 - 1i8));
        print(to_string(32767i16));
        print(to_string(-2147483647 - 1));
        print(to_string(9223372036854775807i64));
        print(to_string(-9223372036854775807i64 - 1i64));
        fn answer() -> int { return 42; }
        let value := answer();
        let text: str := to_string(value);
        print(text);
        print(to_string(7));
        print(to_string(1.00dec));
        "#,
        "0\n-128\n32767\n-2147483648\n9223372036854775807\n-9223372036854775808\n42\n7\n1.00\n",
    );
    let alias = run("let convert := to_string; print(convert(7)); print(convert(1.00dec));");
    assert!(alias.diagnostics.is_empty(), "{:?}", alias.diagnostics);
    assert_eq!(alias.output, "7\n1.00\n");
    for source in [
        "to_string(true);",
        "to_string(1f32);",
        "to_string(1f64);",
        "to_string();",
        "to_string(1, 2);",
        "fn decimal_only(n: decimal) -> str { return to_string(n); } decimal_only(1);",
    ] {
        let result = run(source);
        assert!(!result.diagnostics.is_empty(), "{source}");
        assert!(result.output.is_empty(), "{source}");
    }
}

#[test]
fn arithmetic_precedence_associativity_and_short_circuit() {
    check(
        "let a := 0.1dec; let b := 0.2dec; let c := 0.3dec; print(to_string(a+b+c)); print(to_string(a+b*c)); print(to_string((a+b)*c)); print(to_string((20-5-3) as decimal)); print(to_string((2**3**2) as decimal));",
        "0.6\n0.16\n0.09\n12\n512\n",
    );
    check(
        r#"
        let mut calls := 0;
        fn yes() -> bool { calls = calls + 1; return true; }
        fn no() -> bool { calls = calls + 1; return false; }
        if yes() and yes() { print("and"); }
        if no() or yes() { print("or"); }
        false and yes(); true or no();
        print(to_string(calls as decimal));
    "#,
        "and\nor\n4\n",
    );
}

#[test]
fn scopes_calls_methods_and_composition() {
    check(
        r#"
        fn fib(n: int) -> int { if n < 2 { return n; } return fib(n-1) + fib(n-2); }
        let n := 9; { let n := 5; print(to_string(fib(n) as decimal)); }
        print(to_string(n as decimal));
        class Item { pub let mut value: int := 2; pub fn this() {} }
        class Box {
            let item: Item := Item();
            pub fn this() {}
            pub fn add(n: int) -> void { this.item.value = this.item.value + n; }
            pub fn value() -> int { return this.item.value; }
        }
        let a := Box(); let b := Box(); a.add(3);
        print(to_string(a.value() as decimal)); print(to_string(b.value() as decimal));
    "#,
        "5\n9\n5\n2\n",
    );
}

#[test]
fn rejects_unimplemented_indexing_before_execution() {
    let result = run("let text := \"abc\"; text[0];");
    assert_eq!(result.diagnostics[0].code, "E2015");
    assert!(result.output.is_empty());
}

#[test]
fn validates_operands_stack_and_control_flow() {
    let analysis = analyze("fn value() -> int { return 1; } value();");
    let chunk = compile(&analysis.program, analysis.semantic.as_ref().unwrap()).unwrap();
    for instruction in [
        Instruction::Pop,
        Instruction::Jump(usize::MAX),
        Instruction::LoadLocal(usize::MAX),
        Instruction::Constant(usize::MAX),
        Instruction::Return,
        Instruction::Call(usize::MAX),
    ] {
        let mut bad = chunk.clone();
        bad.functions[0].code[0] = instruction;
        assert_eq!(validate(&bad).unwrap_err().code, "E5002");
    }
    let mut bad = chunk.clone();
    bad.functions[0].spans.clear();
    assert!(validate(&bad).is_err());
    let mut bad = chunk.clone();
    bad.functions[0].code.pop();
    bad.functions[0].spans.pop();
    assert!(validate(&bad).is_err());
}

#[test]
fn enforces_limits_and_isolates_runs() {
    let limits = RuntimeLimits {
        instructions: 50,
        ..RuntimeLimits::default()
    };
    assert_eq!(
        run_with_options("while true {}", limits, None).diagnostics[0].code,
        "E5003"
    );
    assert_eq!(
        run_with_options("1;", limits, Some(Arc::new(AtomicBool::new(true)))).diagnostics[0].code,
        "E5004"
    );
    let limits = RuntimeLimits {
        heap_objects: 1,
        ..RuntimeLimits::default()
    };
    assert_eq!(
        run_with_options("class C {} C(); C();", limits, None).diagnostics[0].code,
        "E5005"
    );
    check("print(\"fresh\");", "fresh\n");
    for source in [
        "print(to_string(1dec / 0dec));",
        "print(to_string((2147483647 + 1) as decimal));",
        "0.1dec as double;",
    ] {
        let result = run(source);
        assert!(!result.diagnostics.is_empty());
        #[cfg(feature = "reference-interpreter")]
        {
            let a = analyze(source);
            let r =
                compiler_core::runtime::execute_reference(&a.program, a.semantic.as_ref().unwrap());
            assert_eq!(result.output, r.output);
            assert_eq!(result.diagnostics, r.diagnostics);
        }
    }
}

#[test]
fn links_modules_once_with_distinct_type_identities() {
    use std::{fs, time::SystemTime};
    let root = std::env::temp_dir().join(format!(
        "itd-link-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("shared.itd"),
        "module shared; print(\"initialized\"); export fn value() -> int { return 3; }",
    )
    .unwrap();
    fs::write(root.join("a.itd"), "module a; use shared; export trait Value { fn get() -> int; } export class Counter: Value { pub fn this() {} pub fn get() -> int { return value(); } }").unwrap();
    fs::write(root.join("b.itd"), "module b; use shared; export class Counter { pub fn this() {} pub fn get() -> int { return value() + 4; } }").unwrap();
    fs::write(root.join("main.itd"), "module main; use a as first; use b as second; let x: first.Value := first.Counter(); let y := second.Counter(); print(to_string(x.get() as decimal)); print(to_string(y.get() as decimal));").unwrap();
    let project = compiler_core::project::Project::open(&root).unwrap();
    let result = project.run("main", RuntimeLimits::default(), None);
    fs::remove_dir_all(root).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.output, "initialized\n3\n7\n");
}

#[test]
fn literal_errors_remain_lazy_and_preserve_prior_output() {
    check(
        "if false { 999999999999999999999999999999999999999999999dec; } print(\"ok\");",
        "ok\n",
    );
    let result = run("print(\"before\"); 99999999999999999999999999999999999999999999dec;");
    assert_eq!(result.output, "before\n");
    assert_eq!(result.diagnostics[0].code, "E4001");
}

#[test]
fn rejects_forged_allocation_sizes() {
    let a = analyze("1;");
    let chunk = compile(&a.program, a.semantic.as_ref().unwrap()).unwrap();
    let mut forged = chunk.clone();
    forged.globals = usize::MAX;
    assert_eq!(validate(&forged).unwrap_err().code, "E5002");
    let mut forged = chunk;
    forged.functions[0].locals = usize::MAX;
    assert_eq!(validate(&forged).unwrap_err().code, "E5002");
}
