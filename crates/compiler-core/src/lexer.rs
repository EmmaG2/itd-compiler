use crate::{
    diagnostic::Diagnostic,
    source::{SourceId, Span},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier,
    Number,
    String,
    Char,
    Pub,
    Priv,
    Protected,
    Int,
    Long,
    Str,
    Decimal,
    Double,
    Float,
    Short,
    Byte,
    Bool,
    Void,
    True,
    False,
    If,
    Else,
    Repeat,
    While,
    Each,
    In,
    Enum,
    Struct,
    Trait,
    Class,
    Let,
    Const,
    Mut,
    Static,
    Fn,
    Lambda,
    Async,
    Await,
    Return,
    Break,
    Continue,
    And,
    Or,
    Not,
    Use,
    From,
    Export,
    Module,
    Try,
    Catch,
    Throw,
    Finally,
    Raise,
    This,
    As,
    Plus,
    Minus,
    Star,
    Slash,
    Power,
    Percent,
    EqualEqual,
    BangEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Equal,
    Define,
    Arrow,
    Colon,
    Semicolon,
    Comma,
    Dot,
    Bang,
    LeftBrace,
    RightBrace,
    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    Error,
    Eof,
}

impl TokenKind {
    fn keyword(text: &str) -> Option<Self> {
        Some(match text {
            "pub" => Self::Pub,
            "priv" => Self::Priv,
            "protected" => Self::Protected,
            "int" => Self::Int,
            "long" => Self::Long,
            "str" => Self::Str,
            "decimal" => Self::Decimal,
            "double" => Self::Double,
            "float" => Self::Float,
            "short" => Self::Short,
            "byte" => Self::Byte,
            "bool" => Self::Bool,
            "void" => Self::Void,
            "true" => Self::True,
            "false" => Self::False,
            "if" => Self::If,
            "else" => Self::Else,
            "repeat" => Self::Repeat,
            "while" => Self::While,
            "each" => Self::Each,
            "in" => Self::In,
            "enum" => Self::Enum,
            "struct" => Self::Struct,
            "trait" => Self::Trait,
            "class" => Self::Class,
            "let" => Self::Let,
            "const" => Self::Const,
            "mut" => Self::Mut,
            "static" => Self::Static,
            "fn" => Self::Fn,
            "lambda" => Self::Lambda,
            "async" => Self::Async,
            "await" => Self::Await,
            "return" => Self::Return,
            "break" => Self::Break,
            "continue" => Self::Continue,
            "and" => Self::And,
            "or" => Self::Or,
            "not" => Self::Not,
            "use" => Self::Use,
            "from" => Self::From,
            "export" => Self::Export,
            "module" => Self::Module,
            "try" => Self::Try,
            "catch" => Self::Catch,
            "throw" => Self::Throw,
            "finally" => Self::Finally,
            "raise" => Self::Raise,
            "this" => Self::This,
            "as" => Self::As,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    pub lexeme: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexResult {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn lex(source: &str, source_id: SourceId) -> LexResult {
    Lexer::new(source, source_id).scan()
}

struct Lexer<'a> {
    source: &'a str,
    source_id: SourceId,
    position: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str, source_id: SourceId) -> Self {
        Self {
            source,
            source_id,
            position: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn scan(mut self) -> LexResult {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.bump();
                continue;
            }

            let start = self.position;
            if is_identifier_start(ch) {
                self.identifier(start);
            } else if ch.is_ascii_digit() {
                self.number(start);
            } else {
                self.symbol_or_literal(start, ch);
            }
        }

        self.push(TokenKind::Eof, self.position, self.position);
        LexResult {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn identifier(&mut self, start: usize) {
        self.bump();
        while self.peek().is_some_and(is_identifier_continue) {
            self.bump();
        }
        let text = &self.source[start..self.position];
        self.push(
            TokenKind::keyword(text).unwrap_or(TokenKind::Identifier),
            start,
            self.position,
        );
    }

    fn number(&mut self, start: usize) {
        self.consume_ascii_digits();

        if self.peek() == Some('.') {
            self.bump();
            if !self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.error(
                    "E0003",
                    "se esperaba un dígito después del punto decimal",
                    start,
                );
            }
            self.consume_ascii_digits();
        }

        if self.peek().is_some_and(|ch| matches!(ch, 'e' | 'E')) {
            self.bump();
            if self.peek().is_some_and(|ch| matches!(ch, '+' | '-')) {
                self.bump();
            }
            if !self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.error("E0003", "el exponente debe contener dígitos", start);
            }
            self.consume_ascii_digits();
        }

        let suffix_start = self.position;
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            self.bump();
        }
        let suffix = &self.source[suffix_start..self.position];
        if !suffix.is_empty()
            && !matches!(suffix, "i8" | "i16" | "i32" | "i64" | "dec" | "f32" | "f64")
        {
            self.error(
                "E0003",
                format!("sufijo numérico inválido: {suffix}"),
                start,
            );
        }
        self.push(TokenKind::Number, start, self.position);
    }

    fn symbol_or_literal(&mut self, start: usize, ch: char) {
        match ch {
            '"' => self.quoted(start, '"', TokenKind::String),
            '\'' => self.quoted(start, '\'', TokenKind::Char),
            '/' if self.peek_next() == Some('/') => self.line_comment(),
            '/' if self.peek_next() == Some('*') => self.block_comment(start),
            '+' => self.single(TokenKind::Plus, start),
            '-' if self.peek_next() == Some('>') => self.double(TokenKind::Arrow, start),
            '-' => self.single(TokenKind::Minus, start),
            '*' if self.peek_next() == Some('*') => self.double(TokenKind::Power, start),
            '*' => self.single(TokenKind::Star, start),
            '/' => self.single(TokenKind::Slash, start),
            '%' => self.single(TokenKind::Percent, start),
            '=' if self.peek_next() == Some('=') => self.double(TokenKind::EqualEqual, start),
            '=' => self.single(TokenKind::Equal, start),
            '!' if self.peek_next() == Some('=') => self.double(TokenKind::BangEqual, start),
            '!' => self.single(TokenKind::Bang, start),
            '<' if self.peek_next() == Some('=') => self.double(TokenKind::LessEqual, start),
            '<' => self.single(TokenKind::Less, start),
            '>' if self.peek_next() == Some('=') => self.double(TokenKind::GreaterEqual, start),
            '>' => self.single(TokenKind::Greater, start),
            ':' if self.peek_next() == Some('=') => self.double(TokenKind::Define, start),
            ':' => self.single(TokenKind::Colon, start),
            ';' => self.single(TokenKind::Semicolon, start),
            ',' => self.single(TokenKind::Comma, start),
            '.' => self.single(TokenKind::Dot, start),
            '{' => self.single(TokenKind::LeftBrace, start),
            '}' => self.single(TokenKind::RightBrace, start),
            '(' => self.single(TokenKind::LeftParen, start),
            ')' => self.single(TokenKind::RightParen, start),
            '[' => self.single(TokenKind::LeftBracket, start),
            ']' => self.single(TokenKind::RightBracket, start),
            _ => {
                self.bump();
                self.error("E0001", format!("carácter inválido: {ch}"), start);
                self.push(TokenKind::Error, start, self.position);
            }
        }
    }

    fn quoted(&mut self, start: usize, quote: char, kind: TokenKind) {
        self.bump();
        let mut decoded_chars = 0;
        let mut closed = false;

        while let Some(ch) = self.peek() {
            if ch == quote {
                self.bump();
                closed = true;
                break;
            }
            if matches!(ch, '\n' | '\r') {
                break;
            }
            if ch == '\\' {
                self.bump();
                match self.peek() {
                    Some('n' | 'r' | 't' | '0' | '\\' | '"' | '\'') => {
                        self.bump();
                        decoded_chars += 1;
                    }
                    Some(invalid) => {
                        self.bump();
                        self.error(
                            "E0004",
                            format!("escape inválido: \\{invalid}"),
                            self.position - invalid.len_utf8() - 1,
                        );
                        decoded_chars += 1;
                    }
                    None => break,
                }
            } else {
                self.bump();
                decoded_chars += 1;
            }
        }

        if !closed {
            self.error("E0002", "literal sin cerrar", start);
            self.push(TokenKind::Error, start, self.position);
            return;
        }
        if kind == TokenKind::Char && decoded_chars != 1 {
            self.error(
                "E0005",
                "un char debe contener exactamente un carácter",
                start,
            );
        }
        self.push(kind, start, self.position);
    }

    fn line_comment(&mut self) {
        self.bump();
        self.bump();
        while self.peek().is_some_and(|ch| !matches!(ch, '\n' | '\r')) {
            self.bump();
        }
    }

    fn block_comment(&mut self, start: usize) {
        self.bump();
        self.bump();
        while let Some(ch) = self.peek() {
            if ch == '*' && self.peek_next() == Some('/') {
                self.bump();
                self.bump();
                return;
            }
            self.bump();
        }
        self.error("E0002", "comentario de bloque sin cerrar", start);
    }

    fn consume_ascii_digits(&mut self) {
        while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
            self.bump();
        }
    }

    fn single(&mut self, kind: TokenKind, start: usize) {
        self.bump();
        self.push(kind, start, self.position);
    }

    fn double(&mut self, kind: TokenKind, start: usize) {
        self.bump();
        self.bump();
        self.push(kind, start, self.position);
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(self.source_id, start, end),
            lexeme: self.source[start..end].to_owned(),
        });
    }

    fn error(&mut self, code: &str, message: impl Into<String>, start: usize) {
        self.diagnostics.push(Diagnostic::error(
            code,
            message,
            Span::new(self.source_id, start, self.position),
        ));
    }

    fn peek(&self) -> Option<char> {
        self.source[self.position..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        self.source[self.position..].chars().nth(1)
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.position += ch.len_utf8();
        Some(ch)
    }
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}
