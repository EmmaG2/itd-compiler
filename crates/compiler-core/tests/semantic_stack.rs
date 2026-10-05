use compiler_core::{analyze, run, semantic::Type};

#[test]
fn nested_reductions_preserve_operand_and_argument_order() {
    let source = "fn subtract(a: int, b: int) -> int { return a - b; } print(to_string(subtract(20, 2 + 3 * 4))); print(to_string(24 / (2 * 3)));";
    let result = run(source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.output, "6\n4\n");
}

#[test]
fn errors_do_not_contaminate_following_expressions() {
    let result = analyze("missing + true; 1[false]; let answer: long := 2i8 + 3i16; !false;");
    let semantic = result.semantic.unwrap();
    assert!(semantic.diagnostics.iter().any(|d| d.code == "E2002"));
    assert!(semantic.diagnostics.iter().any(|d| d.code == "E2015"));
    assert!(!semantic.diagnostics.iter().any(|d| d.code == "E2099"));
    assert!(
        semantic
            .symbols
            .iter()
            .any(|s| s.name == "answer" && s.ty == Type::Long)
    );
    assert_eq!(semantic.coercions.len(), 2);
}

#[test]
fn assignment_keeps_rhs_first_without_reading_its_destination() {
    let result = analyze("let mut x := (x = missing); let y := 3 as long;");
    let codes: Vec<_> = result.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert_eq!(codes, ["E2002"]);
}

#[test]
fn qualified_imports_do_not_read_module_names_as_values() {
    use compiler_core::semantic::{ImportedSymbol, analyze_with_imports};
    let parsed = analyze("domain.value + 2;");
    let result = analyze_with_imports(
        &parsed.program,
        &[ImportedSymbol {
            name: "domain.value".into(),
            ty: Type::Int,
            declaration_span: compiler_core::source::Span::new(
                compiler_core::source::SourceId(0),
                0,
                0,
            ),
            class: None,
            trait_info: None,
        }],
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.resolutions.len(), 1);
}
