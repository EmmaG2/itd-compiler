use compiler_core::{analyze, analyze_with_trace, compile_analysis, semantic::TraceEventKind};

fn replay(trace: &compiler_core::semantic::SemanticTrace) -> Vec<usize> {
    let mut stack = Vec::new();
    for event in &trace.events {
        assert_eq!(event.depth_before, stack.len());
        assert!(event.pop_count <= stack.len());
        stack.truncate(stack.len() - event.pop_count);
        if let Some(attribute) = &event.pushed {
            assert!(attribute.type_label.chars().count() <= 80);
            stack.push(attribute.expression.0);
        }
        assert_eq!(event.depth_after, stack.len());
    }
    stack
}

#[test]
fn tracing_preserves_analysis_and_bytecode_and_replays_to_empty() {
    for source in [
        "let answer: long := 2i8 + 3i16 * 4i16;",
        "fn f(a: int) -> int { return a - 1; } f(8 / 2);",
        "missing + true; 1[false]; let good := 3;",
    ] {
        let plain = analyze(source);
        let (traced, trace) = analyze_with_trace(source, 4096);
        assert_eq!(plain, traced);
        assert_eq!(compile_analysis(plain), compile_analysis(traced));
        let trace = trace.unwrap();
        assert!(!trace.truncated);
        assert!(replay(&trace).is_empty());
        assert!(
            trace
                .events
                .iter()
                .any(|event| event.kind == TraceEventKind::Reduce)
        );
    }
}

#[test]
fn limits_keep_a_replayable_prefix_without_affecting_analysis() {
    let source = "2 + 3 * 4;";
    let (_, complete) = analyze_with_trace(source, 4096);
    let complete = complete.unwrap();
    for limit in [0, 1, 3, complete.events.len(), complete.events.len() + 1] {
        let (analysis, trace) = analyze_with_trace(source, limit);
        let trace = trace.unwrap();
        assert_eq!(analysis, analyze(source));
        assert_eq!(
            trace.events,
            complete.events[..limit.min(complete.events.len())]
        );
        assert_eq!(trace.truncated, limit < complete.events.len());
        replay(&trace);
    }
    assert!(!analyze_with_trace("", 0).1.unwrap().truncated);
    assert!(analyze_with_trace("let ;", 10).1.is_none());
    let source = "1;".repeat(3000);
    let trace = analyze_with_trace(&source, usize::MAX).1.unwrap();
    assert_eq!(trace.events.len(), 4096);
    assert!(trace.truncated);
    replay(&trace);
}

#[test]
fn conversions_are_observations_not_fabricated_stack_movements() {
    let trace = analyze_with_trace("let x: long := 1i8 + 2i16;", 100)
        .1
        .unwrap();
    let conversions: Vec<_> = trace
        .events
        .iter()
        .filter(|e| e.kind == TraceEventKind::Conversion)
        .collect();
    assert_eq!(conversions.len(), 2);
    for event in conversions {
        assert_eq!(event.depth_before, event.depth_after);
        assert_eq!(event.pop_count, 0);
        assert!(event.pushed.is_none());
        assert!(event.conversion_type.is_some());
    }
    assert!(replay(&trace).is_empty());
}

#[test]
fn imported_types_are_bounded_and_share_the_same_stack_path() {
    use compiler_core::semantic::{
        ImportedSymbol, Type, analyze_with_imports, analyze_with_imports_and_trace,
    };
    let parsed = analyze("domain.value;");
    let imports = [ImportedSymbol {
        name: "domain.value".into(),
        ty: Type::Class("x".repeat(20_000)),
        declaration_span: compiler_core::source::Span::new(
            compiler_core::source::SourceId(0),
            0,
            0,
        ),
        class: None,
        trait_info: None,
    }];
    let (traced, trace) = analyze_with_imports_and_trace(&parsed.program, &imports, 100);
    assert_eq!(traced, analyze_with_imports(&parsed.program, &imports));
    assert!(traced.diagnostics.is_empty());
    assert!(replay(&trace).is_empty());
    assert_eq!(
        trace.events[0].pushed.as_ref().unwrap().type_label.len(),
        80
    );
}
