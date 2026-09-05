use crate::{
    ast::{
        BinaryOp, ClassDecl, ClassMember, Decl, Expr, ExprId, ExprKind, FieldDecl, FunctionDecl,
        FunctionSignature, Item, Literal, MethodDecl, ModuleDecl, Parameter, Program, Stmt,
        TraitDecl, TypeName, UnaryOp, UseDecl, VariableDecl, Visibility,
    },
    diagnostic::Diagnostic,
    lexer::{Token, TokenKind},
    source::Span,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseResult {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn parse(tokens: &[Token]) -> ParseResult {
    Parser {
        tokens,
        current: 0,
        next_id: 0,
        diagnostics: Vec::new(),
    }
    .program()
}

struct Parser<'a> {
    tokens: &'a [Token],
    current: usize,
    next_id: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn program(mut self) -> ParseResult {
        let mut items = Vec::new();
        while !self.at(TokenKind::Eof) {
            let start = self.current;
            if let Some(item) = self.item() {
                items.push(item);
            }
            if self.current == start {
                self.advance();
            }
        }
        ParseResult {
            program: Program { items },
            diagnostics: self.diagnostics,
        }
    }

    fn item(&mut self) -> Option<Item> {
        let result = if self.matches(TokenKind::Module) {
            self.module().map(Item::Module)
        } else if self.matches(TokenKind::Use) {
            self.use_declaration().map(Item::Use)
        } else if self.matches(TokenKind::Export) {
            self.export()
        } else if self.at(TokenKind::Class) {
            self.class().map(Item::Class)
        } else if self.at(TokenKind::Trait) {
            self.trait_declaration().map(Item::Trait)
        } else if self.at(TokenKind::Fn) {
            self.function().map(|decl| Item::Decl(Decl::Function(decl)))
        } else if self.at(TokenKind::Let) || self.at(TokenKind::Const) {
            self.variable().map(|decl| Item::Decl(Decl::Variable(decl)))
        } else {
            self.statement().map(Item::Stmt)
        };
        if result.is_none() {
            self.synchronize();
        }
        result
    }

    fn module(&mut self) -> Option<ModuleDecl> {
        let start = self.previous().span;
        let path = self.path()?;
        let end = self.take(TokenKind::Semicolon, "`;`")?.span;
        Some(ModuleDecl {
            path,
            span: self.join(start, end),
        })
    }

    fn use_declaration(&mut self) -> Option<UseDecl> {
        let start = self.previous().span;
        let path = self.path()?;
        let alias = if self.matches(TokenKind::As) {
            Some(
                self.take(TokenKind::Identifier, "alias de módulo")?
                    .lexeme
                    .clone(),
            )
        } else {
            None
        };
        let end = self.take(TokenKind::Semicolon, "`;`")?.span;
        Some(UseDecl {
            path,
            alias,
            span: self.join(start, end),
        })
    }

    fn path(&mut self) -> Option<Vec<String>> {
        let mut path = vec![
            self.take(TokenKind::Identifier, "nombre de módulo")?
                .lexeme
                .clone(),
        ];
        while self.matches(TokenKind::Dot) {
            path.push(
                self.take(TokenKind::Identifier, "segmento de módulo")?
                    .lexeme
                    .clone(),
            );
        }
        Some(path)
    }

    fn export(&mut self) -> Option<Item> {
        if self.at(TokenKind::Fn) {
            return self
                .function()
                .map(|value| Item::Export(Decl::Function(value)));
        }
        if self.at(TokenKind::Let) || self.at(TokenKind::Const) {
            return self
                .variable()
                .map(|value| Item::Export(Decl::Variable(value)));
        }
        if self.at(TokenKind::Class) {
            return self.class().map(Item::ExportClass);
        }
        if self.at(TokenKind::Trait) {
            return self.trait_declaration().map(Item::ExportTrait);
        }
        let found = self.peek().clone();
        self.expected("declaración exportable", &found);
        None
    }

    fn visibility(&mut self) -> Visibility {
        if self.matches(TokenKind::Pub) {
            Visibility::Pub
        } else {
            if !self.matches(TokenKind::Priv) {
                self.matches(TokenKind::Protected);
            }
            Visibility::Priv
        }
    }

    fn class(&mut self) -> Option<ClassDecl> {
        let start = self.take(TokenKind::Class, "`class`")?.span;
        let name = self.take(TokenKind::Identifier, "nombre de clase")?.clone();
        let mut traits = Vec::new();
        if self.matches(TokenKind::Colon) {
            loop {
                traits.push(self.type_name()?);
                if !self.matches(TokenKind::Comma) {
                    break;
                }
            }
        }
        self.take(TokenKind::LeftBrace, "`{`")?;
        let mut members = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let visibility = self.visibility();
            let is_static = self.matches(TokenKind::Static);
            if self.at(TokenKind::Let) || self.at(TokenKind::Const) {
                let variable = self.variable()?;
                if variable.annotation.is_none() {
                    self.diagnostics.push(Diagnostic::error(
                        "E1001",
                        "los campos requieren tipo explícito",
                        variable.name_span,
                    ));
                }
                members.push(ClassMember::Field(FieldDecl {
                    visibility,
                    is_static,
                    variable,
                }));
            } else if self.matches(TokenKind::Fn) {
                let constructor = self.at(TokenKind::This);
                let function = self.function_after_fn(constructor)?;
                let method = MethodDecl {
                    visibility,
                    is_static,
                    function,
                };
                members.push(if constructor {
                    ClassMember::Constructor(method)
                } else {
                    ClassMember::Method(method)
                });
            } else {
                let found = self.peek().clone();
                self.expected("campo, método o constructor", &found);
                return None;
            }
        }
        let end = self.take(TokenKind::RightBrace, "`}`")?.span;
        Some(ClassDecl {
            name: name.lexeme,
            name_span: name.span,
            traits,
            members,
            span: self.join(start, end),
        })
    }

    fn trait_declaration(&mut self) -> Option<TraitDecl> {
        let start = self.take(TokenKind::Trait, "`trait`")?.span;
        let name = self.take(TokenKind::Identifier, "nombre de trait")?.clone();
        self.take(TokenKind::LeftBrace, "`{`")?;
        let mut methods = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let method_start = self.take(TokenKind::Fn, "`fn`")?.span;
            let method_name = self
                .take(TokenKind::Identifier, "nombre de método")?
                .clone();
            let (parameters, return_type) = self.function_header()?;
            let end = self.take(TokenKind::Semicolon, "`;`")?.span;
            methods.push(FunctionSignature {
                name: method_name.lexeme,
                name_span: method_name.span,
                parameters,
                return_type,
                span: self.join(method_start, end),
            });
        }
        let end = self.take(TokenKind::RightBrace, "`}`")?.span;
        Some(TraitDecl {
            name: name.lexeme,
            name_span: name.span,
            methods,
            span: self.join(start, end),
        })
    }

    fn function(&mut self) -> Option<FunctionDecl> {
        let start = self.take(TokenKind::Fn, "`fn`")?.span;
        self.function_after_fn_with_start(false, start)
    }

    fn function_after_fn(&mut self, constructor: bool) -> Option<FunctionDecl> {
        let start = self.previous().span;
        self.function_after_fn_with_start(constructor, start)
    }

    fn function_after_fn_with_start(
        &mut self,
        constructor: bool,
        start: Span,
    ) -> Option<FunctionDecl> {
        let expected = if constructor {
            TokenKind::This
        } else {
            TokenKind::Identifier
        };
        let name = self.take(expected, "nombre de función")?.clone();
        let (parameters, return_type) = self.function_header()?;
        let body = self.block()?;
        Some(FunctionDecl {
            name: name.lexeme,
            name_span: name.span,
            parameters,
            return_type,
            span: self.join(start, body.span()),
            body,
        })
    }

    fn function_header(&mut self) -> Option<(Vec<Parameter>, Option<TypeName>)> {
        self.take(TokenKind::LeftParen, "`(`")?;
        let mut parameters = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Eof) {
            let parameter = self
                .take(TokenKind::Identifier, "nombre de parámetro")?
                .clone();
            self.take(TokenKind::Colon, "`:`")?;
            parameters.push(Parameter {
                name: parameter.lexeme,
                span: parameter.span,
                type_name: self.type_name()?,
            });
            if !self.matches(TokenKind::Comma) {
                break;
            }
        }
        self.take(TokenKind::RightParen, "`)`")?;
        let return_type = if self.matches(TokenKind::Arrow) {
            Some(self.type_name()?)
        } else {
            None
        };
        Some((parameters, return_type))
    }

    fn variable(&mut self) -> Option<VariableDecl> {
        let start = self.advance().span;
        let mutable = self.previous().kind == TokenKind::Let && self.matches(TokenKind::Mut);
        let name = self
            .take(TokenKind::Identifier, "nombre de variable")?
            .clone();
        let annotation = if self.matches(TokenKind::Colon) {
            Some(self.type_name()?)
        } else {
            None
        };
        self.take(TokenKind::Define, "`:=`")?;
        let initializer = self.expression(0)?;
        let end = self.take(TokenKind::Semicolon, "`;`")?.span;
        Some(VariableDecl {
            name: name.lexeme,
            name_span: name.span,
            mutable,
            annotation,
            initializer,
            span: self.join(start, end),
        })
    }

    fn statement(&mut self) -> Option<Stmt> {
        if self.at(TokenKind::LeftBrace) {
            return self.block();
        }
        if self.matches(TokenKind::If) {
            return self.if_statement();
        }
        if self.matches(TokenKind::While) {
            return self.while_statement();
        }
        if self.matches(TokenKind::Return) {
            return self.return_statement();
        }
        if self.matches(TokenKind::Break) {
            return self.loop_control(true);
        }
        if self.matches(TokenKind::Continue) {
            return self.loop_control(false);
        }
        if self.at(TokenKind::Let) || self.at(TokenKind::Const) {
            return self.variable().map(Stmt::Variable);
        }
        let expression = self.expression(0)?;
        let end = self.take(TokenKind::Semicolon, "`;`")?.span;
        let span = self.join(expression.span, end);
        Some(Stmt::Expression { expression, span })
    }

    fn block(&mut self) -> Option<Stmt> {
        let start = self.take(TokenKind::LeftBrace, "`{`")?.span;
        let mut statements = Vec::new();
        while !self.at(TokenKind::RightBrace) && !self.at(TokenKind::Eof) {
            let before = self.current;
            if let Some(statement) = self.statement() {
                statements.push(statement);
            } else {
                self.synchronize();
            }
            if self.current == before {
                self.advance();
            }
        }
        let end = self.take(TokenKind::RightBrace, "`}`")?.span;
        Some(Stmt::Block {
            statements,
            span: self.join(start, end),
        })
    }

    fn if_statement(&mut self) -> Option<Stmt> {
        let start = self.previous().span;
        let condition = self.expression(0)?;
        let then_branch = Box::new(self.block()?);
        let else_branch = if self.matches(TokenKind::Else) {
            Some(Box::new(if self.matches(TokenKind::If) {
                self.if_statement()?
            } else {
                self.block()?
            }))
        } else {
            None
        };
        let end = else_branch
            .as_ref()
            .map_or_else(|| then_branch.span(), |branch| branch.span());
        Some(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: self.join(start, end),
        })
    }

    fn while_statement(&mut self) -> Option<Stmt> {
        let start = self.previous().span;
        let condition = self.expression(0)?;
        let body = Box::new(self.block()?);
        let span = self.join(start, body.span());
        Some(Stmt::While {
            condition,
            body,
            span,
        })
    }

    fn return_statement(&mut self) -> Option<Stmt> {
        let start = self.previous().span;
        let value = if self.at(TokenKind::Semicolon) {
            None
        } else {
            Some(self.expression(0)?)
        };
        let end = self.take(TokenKind::Semicolon, "`;`")?.span;
        Some(Stmt::Return {
            value,
            span: self.join(start, end),
        })
    }

    fn loop_control(&mut self, is_break: bool) -> Option<Stmt> {
        let start = self.previous().span;
        let end = self.take(TokenKind::Semicolon, "`;`")?.span;
        let span = self.join(start, end);
        Some(if is_break {
            Stmt::Break { span }
        } else {
            Stmt::Continue { span }
        })
    }

    fn expression(&mut self, min_bp: u8) -> Option<Expr> {
        let token = self.advance().clone();
        let mut left = match token.kind {
            TokenKind::Number => {
                self.expr(ExprKind::Literal(Literal::Number(token.lexeme)), token.span)
            }
            TokenKind::String => {
                self.expr(ExprKind::Literal(Literal::String(token.lexeme)), token.span)
            }
            TokenKind::Char => {
                self.expr(ExprKind::Literal(Literal::Char(token.lexeme)), token.span)
            }
            TokenKind::True | TokenKind::False => self.expr(
                ExprKind::Literal(Literal::Bool(token.kind == TokenKind::True)),
                token.span,
            ),
            TokenKind::Identifier | TokenKind::This => {
                self.expr(ExprKind::Name(token.lexeme), token.span)
            }
            TokenKind::LeftParen => {
                let expression = self.expression(0)?;
                let end = self.take(TokenKind::RightParen, "`)`")?.span;
                Expr {
                    span: self.join(token.span, end),
                    ..expression
                }
            }
            TokenKind::Plus | TokenKind::Minus | TokenKind::Not | TokenKind::Bang => {
                let op = match token.kind {
                    TokenKind::Plus => UnaryOp::Plus,
                    TokenKind::Minus => UnaryOp::Minus,
                    _ => UnaryOp::Not,
                };
                let operand = self.expression(9)?;
                let span = self.join(token.span, operand.span);
                self.expr(
                    ExprKind::Unary {
                        op,
                        operand: Box::new(operand),
                    },
                    span,
                )
            }
            _ => {
                self.expected("expresión", &token);
                return None;
            }
        };

        loop {
            if self.at(TokenKind::LeftParen) && 12 >= min_bp {
                left = self.call(left)?;
                continue;
            }
            if self.at(TokenKind::Dot) && 12 >= min_bp {
                left = self.member(left)?;
                continue;
            }
            if self.at(TokenKind::LeftBracket) && 12 >= min_bp {
                left = self.index(left)?;
                continue;
            }
            if self.matches_if(TokenKind::As, 11, min_bp) {
                let type_name = self.type_name()?;
                let span = self.join(left.span, type_name.span);
                left = self.expr(
                    ExprKind::Cast {
                        expression: Box::new(left),
                        type_name,
                    },
                    span,
                );
                continue;
            }
            let Some((left_bp, right_bp, op)) = infix(self.peek().kind) else {
                break;
            };
            if left_bp < min_bp {
                break;
            }
            let operator = self.advance().kind;
            let right = self.expression(right_bp)?;
            let span = self.join(left.span, right.span);
            left = if operator == TokenKind::Equal {
                self.expr(
                    ExprKind::Assign {
                        target: Box::new(left),
                        value: Box::new(right),
                    },
                    span,
                )
            } else {
                self.expr(
                    ExprKind::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                    },
                    span,
                )
            };
        }
        Some(left)
    }

    fn call(&mut self, callee: Expr) -> Option<Expr> {
        self.advance();
        let mut arguments = Vec::new();
        while !self.at(TokenKind::RightParen) && !self.at(TokenKind::Eof) {
            arguments.push(self.expression(0)?);
            if !self.matches(TokenKind::Comma) {
                break;
            }
        }
        let end = self.take(TokenKind::RightParen, "`)`")?.span;
        let span = self.join(callee.span, end);
        Some(self.expr(
            ExprKind::Call {
                callee: Box::new(callee),
                arguments,
            },
            span,
        ))
    }

    fn member(&mut self, object: Expr) -> Option<Expr> {
        self.advance();
        let name = self
            .take(TokenKind::Identifier, "nombre de miembro")?
            .clone();
        let span = self.join(object.span, name.span);
        Some(self.expr(
            ExprKind::Member {
                object: Box::new(object),
                name: name.lexeme,
            },
            span,
        ))
    }

    fn index(&mut self, object: Expr) -> Option<Expr> {
        self.advance();
        let index = self.expression(0)?;
        let end = self.take(TokenKind::RightBracket, "`]`")?.span;
        let span = self.join(object.span, end);
        Some(self.expr(
            ExprKind::Index {
                object: Box::new(object),
                index: Box::new(index),
            },
            span,
        ))
    }

    fn type_name(&mut self) -> Option<TypeName> {
        let token = self.advance().clone();
        if matches!(
            token.kind,
            TokenKind::Identifier
                | TokenKind::Byte
                | TokenKind::Short
                | TokenKind::Int
                | TokenKind::Long
                | TokenKind::Decimal
                | TokenKind::Float
                | TokenKind::Double
                | TokenKind::Bool
                | TokenKind::Str
                | TokenKind::Char
                | TokenKind::Void
        ) {
            let mut name = token.lexeme;
            let mut span = token.span;
            while self.matches(TokenKind::Dot) {
                let segment = self
                    .take(TokenKind::Identifier, "segmento de tipo")?
                    .clone();
                name.push('.');
                name.push_str(&segment.lexeme);
                span = self.join(span, segment.span);
            }
            return Some(TypeName { name, span });
        }
        self.expected("tipo", &token);
        None
    }

    fn expr(&mut self, kind: ExprKind, span: Span) -> Expr {
        let id = ExprId(self.next_id);
        self.next_id += 1;
        Expr { id, kind, span }
    }
    fn matches_if(&mut self, kind: TokenKind, bp: u8, min_bp: u8) -> bool {
        if bp < min_bp || !self.at(kind) {
            return false;
        }
        self.advance();
        true
    }
    fn matches(&mut self, kind: TokenKind) -> bool {
        if !self.at(kind) {
            return false;
        }
        self.advance();
        true
    }
    fn take(&mut self, kind: TokenKind, expected: &str) -> Option<&Token> {
        if self.at(kind) {
            return Some(self.advance());
        }
        let found = self.peek().clone();
        self.expected(expected, &found);
        None
    }
    fn expected(&mut self, expected: &str, found: &Token) {
        self.diagnostics.push(Diagnostic::error(
            "E1001",
            format!("se esperaba {expected}; se encontró `{}`", found.lexeme),
            found.span,
        ));
    }
    fn synchronize(&mut self) {
        while !self.at(TokenKind::Eof) {
            if self.previous_kind() == Some(TokenKind::Semicolon)
                || self.at(TokenKind::RightBrace)
                || matches!(
                    self.peek().kind,
                    TokenKind::Let
                        | TokenKind::Const
                        | TokenKind::Fn
                        | TokenKind::Class
                        | TokenKind::Trait
                        | TokenKind::Module
                        | TokenKind::Use
                        | TokenKind::Export
                        | TokenKind::If
                        | TokenKind::While
                        | TokenKind::Break
                        | TokenKind::Continue
                        | TokenKind::Return
                )
            {
                return;
            }
            self.advance();
        }
    }
    fn at(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.current.min(self.tokens.len() - 1)]
    }
    fn advance(&mut self) -> &Token {
        let index = self.current.min(self.tokens.len() - 1);
        if self.current + 1 < self.tokens.len() {
            self.current += 1;
        }
        &self.tokens[index]
    }
    fn previous(&self) -> &Token {
        &self.tokens[self.current.saturating_sub(1)]
    }
    fn previous_kind(&self) -> Option<TokenKind> {
        self.current
            .checked_sub(1)
            .map(|index| self.tokens[index].kind)
    }
    fn join(&self, start: Span, end: Span) -> Span {
        Span::new(start.source, start.start, end.end)
    }
}

fn infix(kind: TokenKind) -> Option<(u8, u8, BinaryOp)> {
    Some(match kind {
        TokenKind::Equal => (1, 1, BinaryOp::Equal),
        TokenKind::Or => (2, 3, BinaryOp::Or),
        TokenKind::And => (3, 4, BinaryOp::And),
        TokenKind::EqualEqual => (4, 5, BinaryOp::Equal),
        TokenKind::BangEqual => (4, 5, BinaryOp::NotEqual),
        TokenKind::Less => (5, 6, BinaryOp::Less),
        TokenKind::LessEqual => (5, 6, BinaryOp::LessEqual),
        TokenKind::Greater => (5, 6, BinaryOp::Greater),
        TokenKind::GreaterEqual => (5, 6, BinaryOp::GreaterEqual),
        TokenKind::Plus => (6, 7, BinaryOp::Add),
        TokenKind::Minus => (6, 7, BinaryOp::Subtract),
        TokenKind::Star => (7, 8, BinaryOp::Multiply),
        TokenKind::Slash => (7, 8, BinaryOp::Divide),
        TokenKind::Percent => (7, 8, BinaryOp::Remainder),
        TokenKind::Power => (8, 8, BinaryOp::Power),
        _ => return None,
    })
}
