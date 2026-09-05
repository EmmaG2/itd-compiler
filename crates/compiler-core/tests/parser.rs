use compiler_core::{
    ast::{BinaryOp, Decl, ExprKind, Item, Stmt},
    lexer::lex,
    parser::{ParseResult, parse},
    source::SourceId,
};

fn parse_source(source: &str) -> ParseResult {
    parse(&lex(source, SourceId(0)).tokens)
}

fn first_initializer(source: &str) -> compiler_core::ast::Expr {
    let result = parse_source(source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let Item::Decl(Decl::Variable(declaration)) = &result.program.items[0] else {
        panic!("expected variable")
    };
    declaration.initializer.clone()
}

#[test]
fn preserves_precedence_associativity_and_unary_grouping() {
    let expression = first_initializer("let value := 1 + 2 * 3;");
    let ExprKind::Binary {
        op: BinaryOp::Add,
        right,
        ..
    } = expression.kind
    else {
        panic!("expected addition")
    };
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            op: BinaryOp::Multiply,
            ..
        }
    ));

    let result = parse_source("let mut a := 0; let mut b := 0; a = b = -(1 + 2);");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let Item::Stmt(Stmt::Expression { expression, .. }) = &result.program.items[2] else {
        panic!("expected expression")
    };
    let ExprKind::Assign { value, .. } = &expression.kind else {
        panic!("expected assignment")
    };
    assert!(matches!(value.kind, ExprKind::Assign { .. }));
}

#[test]
fn parses_postfix_chains_and_recovers_at_next_declaration() {
    let result = parse_source("let result := service.make(1, 2).value[0];");
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(matches!(
        first_initializer("let result := service.make(1, 2).value[0];").kind,
        ExprKind::Index { .. }
    ));

    let recovered = parse_source("let broken := 1 let valid := 2;");
    assert!(
        recovered
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E1001")
    );
    assert!(
        recovered
            .program
            .items
            .iter()
            .any(|item| matches!(item, Item::Decl(Decl::Variable(decl)) if decl.name == "valid"))
    );

    let delimiter = parse_source("let broken := (1 + 2;");
    let diagnostic = delimiter
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "E1001")
        .expect("syntax diagnostic");
    assert_eq!(diagnostic.labels[0].span.start, 20);
    assert!(diagnostic.message.contains("`)`"));
}
