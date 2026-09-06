use std::{
    fs,
    sync::{Arc, atomic::AtomicBool},
    time::SystemTime,
};

use compiler_core::{
    analyze,
    bytecode::Instruction,
    project::analyze_project,
    run, run_with_options,
    runtime::{RuntimeLimits, execute_chunk},
};

#[test]
fn executes_objects_methods_static_members_and_traits() {
    let source = r#"
        trait HasValue { fn get() -> int; }
        class Counter: HasValue {
            pub let mut value: int := 0;
            static let mut created: int := 0;
            pub fn this(start: int) { this.value = start; Counter.created = Counter.created + 1; }
            pub fn add(amount: int) -> int { this.value = this.value + amount; return this.value; }
            pub fn get() -> int { return this.value; }
            pub static fn count() -> int { return Counter.created; }
        }
        let first: HasValue := Counter(2);
        let second := Counter(10);
        if first != second { print("different"); }
        second.add(5);
        print(to_string(first.get() as decimal));
        print(to_string(second.get() as decimal));
        print(to_string(Counter.count() as decimal));
        "#;
    let result = run(source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.output, "different\n2\n15\n2\n");
    #[cfg(feature = "reference-interpreter")]
    {
        let analysis = analyze(source);
        let reference = compiler_core::runtime::execute_reference(
            &analysis.program,
            analysis.semantic.as_ref().expect("semantic result"),
        );
        assert_eq!(result.output, reference.output);
    }
}

#[test]
fn rejects_private_access_and_incomplete_trait() {
    let result = analyze(
        "trait Named { fn name() -> str; } class User: Named { let id: int := 1; pub fn this() {} } let user := User(); user.id;",
    );
    assert!(result.diagnostics.iter().any(|value| value.code == "E2011"));
    assert!(result.diagnostics.iter().any(|value| value.code == "E2013"));

    let cycle = analyze("class A { let b: B := B(); } class B { let a: A := A(); }");
    assert!(cycle.diagnostics.iter().any(|value| value.code == "E2014"));

    let wrong_constructor = analyze("class Point { pub fn this(x: int) {} } Point();");
    assert!(
        wrong_constructor
            .diagnostics
            .iter()
            .any(|value| value.code == "E2004")
    );
}

#[test]
fn resolves_exported_classes_and_traits_between_modules() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("compiler-oop-{unique}"));
    fs::create_dir_all(&root).expect("create project");
    fs::write(root.join("types.itd"), "module types; export trait Value { fn get() -> int; } export class Counter: Value { pub let value: int := 1; pub fn this() {} pub fn get() -> int { return this.value; } } export fn exact() -> decimal { return 0.1dec + 0.2dec; }").expect("write types");
    fs::write(root.join("main.itd"), "module main; use types as domain; let counter := domain.Counter(); let value := counter.get(); let total := domain.exact();").expect("write main");
    let result = analyze_project(&root, "main");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    fs::remove_dir_all(root).expect("remove project");
}

#[test]
fn rejects_bad_bytecode_and_stops_at_limits() {
    let analysis = analyze("while true {}");
    let semantic = analysis.semantic.as_ref().expect("semantic result");
    let mut malformed =
        compiler_core::bytecode::compile(&analysis.program, semantic).expect("compile");
    malformed.functions[0].code[0] = Instruction::Constant(usize::MAX);
    assert_eq!(
        execute_chunk(&malformed, semantic, RuntimeLimits::default(), None).diagnostics[0].code,
        "E5002"
    );

    let chunk =
        compiler_core::bytecode::compile(&analysis.program, semantic).expect("compile bytecode");
    let limits = RuntimeLimits {
        instructions: 20,
        ..RuntimeLimits::default()
    };
    assert_eq!(
        execute_chunk(
            &chunk,
            semantic,
            limits,
            Some(Arc::new(AtomicBool::new(false)))
        )
        .diagnostics[0]
            .code,
        "E5003"
    );

    let cancelled = Arc::new(AtomicBool::new(true));
    assert_eq!(
        execute_chunk(&chunk, semantic, RuntimeLimits::default(), Some(cancelled)).diagnostics[0]
            .code,
        "E5004"
    );

    let recursion = run("fn forever() -> int { return forever(); } forever();");
    let diagnostic = recursion
        .diagnostics
        .iter()
        .find(|value| value.code == "E4004")
        .expect("recursion diagnostic");
    assert!(
        diagnostic
            .labels
            .iter()
            .any(|label| label.message.contains("forever"))
    );

    let output_limit = RuntimeLimits {
        output_bytes: 2,
        ..RuntimeLimits::default()
    };
    assert_eq!(
        run_with_options("print(\"abc\");", output_limit, None).diagnostics[0].code,
        "E5006"
    );
}
