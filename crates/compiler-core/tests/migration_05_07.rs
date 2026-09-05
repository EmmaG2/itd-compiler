use std::{fs, time::SystemTime};

use compiler_core::{
    analyze,
    project::{Project, analyze_project},
    run,
};

#[test]
fn executes_exact_decimals_and_bankers_rounding() {
    let result = run(r#"
        print(to_string(0.1dec + 0.2dec));
        print(to_string(1 + 2dec));
        print(to_string(1.00dec));
        print(to_string(round(2.345dec, 2)));
        "#);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.output, "0.3\n3\n1.00\n2.34\n");

    let division = run("let invalid := 1dec / 0dec;");
    assert!(
        division
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E4003")
    );
    let overflow = run("let invalid := 79228162514264337593543950335dec + 1dec;");
    assert!(
        overflow
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E4002")
    );
    let lossy_cast = run("let invalid := 0.1dec as double;");
    assert!(
        lossy_cast
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E4005")
    );
}

#[test]
fn executes_functions_recursion_and_loop_control() {
    let result = run(r#"
        fn factorial(value: int) -> int {
            if value == 0 { return 1; }
            return value * factorial(value - 1);
        }
        let mut index := 0;
        while index < 5 {
            index = index + 1;
            if index == 2 { continue; }
            if index == 4 { break; }
        }
        print(to_string(factorial(index) as decimal));
        "#);
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.starts_with('E')),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(result.output, "24\n");

    let invalid = analyze(
        "fn missing(value: bool) -> int { if value { return 1; } } break; continue; return;",
    );
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E2009")
    );
    assert_eq!(
        invalid
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "E2008")
            .count(),
        3
    );
}

#[test]
fn loads_exports_once_and_rejects_cycles_and_missing_modules() {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("compiler-core-{unique}"));
    fs::create_dir_all(&root).expect("create project");
    fs::write(
        root.join("math.itd"),
        "module math; export fn twice(value: int) -> int { return value * 2; } let hidden := 1;",
    )
    .expect("write dependency");
    fs::write(
        root.join("util.itd"),
        "module util; use math; export fn four(value: int) -> int { return twice(twice(value)); }",
    )
    .expect("write transitive dependency");
    fs::write(
        root.join("main.itd"),
        "module main; use math as numbers; use util; let answer := numbers.twice(four(5));",
    )
    .expect("write entry");

    let valid = Project::open(&root).expect("open project").analyze("main");
    assert!(valid.diagnostics.is_empty(), "{:?}", valid.diagnostics);
    assert_eq!(valid.modules.len(), 3);

    fs::write(
        root.join("private.itd"),
        "module private; use math as numbers; let invalid := numbers.hidden;",
    )
    .expect("write private import");
    let private = analyze_project(&root, "private");
    assert!(
        private
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E2002")
    );

    fs::write(
        root.join("duplicate.itd"),
        "module duplicate; use math; use math; let answer := twice(2);",
    )
    .expect("write duplicate import");
    let duplicate = analyze_project(&root, "duplicate");
    assert_eq!(
        duplicate
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "E3002")
            .count(),
        1
    );
    assert_eq!(duplicate.modules.len(), 2);

    fs::write(root.join("one.itd"), "module one; use two;").expect("write first cycle");
    fs::write(root.join("two.itd"), "module two; use one;").expect("write second cycle");
    let cycle = analyze_project(&root, "one");
    assert!(
        cycle
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3003")
    );

    fs::write(root.join("three.itd"), "module three; use four;").expect("write indirect cycle 1");
    fs::write(root.join("four.itd"), "module four; use five;").expect("write indirect cycle 2");
    fs::write(root.join("five.itd"), "module five; use three;").expect("write indirect cycle 3");
    let indirect = analyze_project(&root, "three");
    assert!(
        indirect
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3003")
    );

    fs::write(root.join("alpha.itd"), "module alpha; let hidden := 1;").expect("write alpha");
    fs::write(root.join("beta.itd"), "module beta; let hidden := 2;").expect("write beta");
    fs::write(
        root.join("private_names.itd"),
        "module private_names; use alpha; use beta;",
    )
    .expect("write private names entry");
    assert!(
        analyze_project(&root, "private_names")
            .diagnostics
            .is_empty()
    );

    fs::write(root.join("missing.itd"), "module missing; use absent;")
        .expect("write missing import");
    let missing = analyze_project(&root, "missing");
    assert!(
        missing
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E3001")
    );

    #[cfg(unix)]
    {
        let outside = std::env::temp_dir().join(format!("compiler-outside-{unique}.itd"));
        fs::write(&outside, "module escape;").expect("write outside module");
        std::os::unix::fs::symlink(&outside, root.join("escape.itd"))
            .expect("create escaping symlink");
        let escape = analyze_project(&root, "escape");
        assert!(
            escape
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E3005")
        );
        fs::remove_file(outside).expect("remove outside module");

        fs::create_dir(root.join("alias")).expect("create alias directory");
        std::os::unix::fs::symlink(root.join("math.itd"), root.join("alias/math.itd"))
            .expect("create module alias");
        fs::write(
            root.join("logical.itd"),
            "module logical; use math; use alias.math;",
        )
        .expect("write logical aliases");
        let logical = analyze_project(&root, "logical");
        assert!(
            logical
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "E3004")
        );
        assert_eq!(logical.modules.len(), 2);
    }
    fs::remove_dir_all(root).expect("remove project");
}
