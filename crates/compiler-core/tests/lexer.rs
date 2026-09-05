use compiler_core::{
    lexer::{LexResult, TokenKind, lex},
    source::{SourceId, SourceMap},
};

fn lex_source(source: &str) -> LexResult {
    lex(source, SourceId(0))
}

#[test]
fn keeps_final_identifier_number_and_decimal_lexemes() {
    let result = lex_source("precio 12.30dec 12e-2dec 1f64 total");
    let kinds: Vec<_> = result.tokens.iter().map(|token| token.kind).collect();
    assert_eq!(
        kinds,
        vec![
            TokenKind::Identifier,
            TokenKind::Number,
            TokenKind::Number,
            TokenKind::Number,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert_eq!(result.tokens[1].lexeme, "12.30dec");
    assert_eq!(result.tokens[2].lexeme, "12e-2dec");
    assert_eq!(result.tokens[4].lexeme, "total");
    assert!(result.diagnostics.is_empty());
}

#[test]
fn reports_two_errors_and_recovers() {
    let result = lex_source("@ let x := 1e; 2pesos #");
    let codes: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, vec!["E0001", "E0003", "E0003", "E0001"]);
    let spans: Vec<_> = result
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic.labels[0].span;
            (span.start, span.end)
        })
        .collect();
    assert_eq!(spans, vec![(0, 1), (11, 13), (15, 21), (22, 23)]);
    assert_eq!(
        result.tokens.last().map(|token| token.kind),
        Some(TokenKind::Eof)
    );
}

#[test]
fn always_reaches_eof_for_malformed_input() {
    for source in ["", "\\", "\0", "💥", "1.", "'ab'", "\"x\nlet y := 2;"] {
        let result = lex_source(source);
        assert_eq!(
            result.tokens.last().map(|token| token.kind),
            Some(TokenKind::Eof)
        );
    }
}

#[test]
fn diagnoses_unclosed_literals_and_comments() {
    for source in ["\"texto", "/* comentario"] {
        let result = lex_source(source);
        assert_eq!(result.diagnostics[0].code, "E0002");
        assert_eq!(
            result.tokens.last().map(|token| token.kind),
            Some(TokenKind::Eof)
        );
    }
}

#[test]
fn accepts_unicode_identifiers_and_both_newline_forms() {
    let source = "let área := 1;\r\nlet año := 2;\n";
    let result = lex_source(source);
    assert!(result.diagnostics.is_empty());
    assert!(result.tokens.iter().any(|token| token.lexeme == "área"));
    assert!(result.tokens.iter().any(|token| token.lexeme == "año"));
}

#[test]
fn computes_utf8_line_and_column_from_offsets() {
    let mut sources = SourceMap::new();
    let id = sources.add("ejemplo.itd", "á\r\nbeta");
    assert_eq!(id, SourceId(0));
    assert_eq!(sources.line_column(id, 0), Some((1, 1)));
    assert_eq!(sources.line_column(id, 2), Some((1, 2)));
    assert_eq!(sources.line_column(id, 4), Some((2, 1)));
    assert_eq!(sources.line_column(id, 8), Some((2, 5)));
    assert_eq!(sources.line_column(id, 1), None);
}

#[test]
fn recognizes_escapes_operators_and_exact_spans() {
    let result = lex_source("'\\n' ** <= := ->");
    let expected = [
        (TokenKind::Char, 0, 4),
        (TokenKind::Power, 5, 7),
        (TokenKind::LessEqual, 8, 10),
        (TokenKind::Define, 11, 13),
        (TokenKind::Arrow, 14, 16),
        (TokenKind::Eof, 16, 16),
    ];
    for (token, (kind, start, end)) in result.tokens.iter().zip(expected) {
        assert_eq!(
            (token.kind, token.span.start, token.span.end),
            (kind, start, end)
        );
    }
    assert!(result.diagnostics.is_empty());
}
