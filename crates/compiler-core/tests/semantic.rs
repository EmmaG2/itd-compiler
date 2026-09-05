use compiler_core::{analyze, semantic::Type};

fn semantic(source: &str) -> compiler_core::semantic::SemanticResult {
    let result = analyze(source);
    assert!(
        !result.diagnostics.iter().any(
            |diagnostic| diagnostic.code.starts_with("E0") || diagnostic.code.starts_with("E1")
        ),
        "{:?}",
        result.diagnostics
    );
    result.semantic.expect("valid syntax has semantic result")
}

#[test]
fn resolves_scopes_and_reports_independent_name_errors() {
    let valid = semantic("let value := 1; { let value := true; value; } value;");
    assert!(valid.diagnostics.is_empty());
    let resolved: std::collections::HashSet<_> = valid.resolutions.values().collect();
    assert_eq!(resolved.len(), 2);

    let invalid = semantic("let item := item; let item := 2; missing; item = 3;");
    let codes: Vec<_> = invalid
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert!(codes.contains(&"E2001"));
    assert!(codes.contains(&"E2002"));
    assert!(codes.iter().filter(|code| **code == "E2003").count() >= 2);
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.labels.len() == 2)
    );
}

#[test]
fn infers_checks_and_records_numeric_coercions() {
    let valid = semantic(
        "let count := 1; let enabled := true; let total: long := count; let mixed := 1i8 + 2i16; if enabled { let inner := total; }",
    );
    assert!(valid.diagnostics.is_empty(), "{:?}", valid.diagnostics);
    assert!(
        valid
            .symbols
            .iter()
            .any(|symbol| symbol.name == "count" && symbol.ty == Type::Int)
    );
    assert!(
        valid
            .symbols
            .iter()
            .any(|symbol| symbol.name == "enabled" && symbol.ty == Type::Bool)
    );
    assert_eq!(valid.coercions.len(), 2);

    let invalid = semantic("let mixed := 1 + 2.0; if 1 { let ok := true; }");
    let codes: Vec<_> = invalid
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert!(codes.contains(&"E2004"));
    assert!(codes.contains(&"E2006"));
}

#[test]
fn creates_function_scope_and_checks_calls_and_returns() {
    let result =
        semantic("fn identity(value: int) -> int { return value; } let answer := identity(42);");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .symbols
            .iter()
            .any(|symbol| symbol.name == "value" && symbol.ty == Type::Int)
    );
    assert!(
        result
            .symbols
            .iter()
            .any(|symbol| symbol.name == "answer" && symbol.ty == Type::Int)
    );
}
