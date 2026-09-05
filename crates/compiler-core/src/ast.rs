use crate::source::Span;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExprId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Decl(Decl),
    Export(Decl),
    Class(ClassDecl),
    ExportClass(ClassDecl),
    Trait(TraitDecl),
    ExportTrait(TraitDecl),
    Module(ModuleDecl),
    Use(UseDecl),
    Stmt(Stmt),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Visibility {
    Pub,
    #[default]
    Priv,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClassDecl {
    pub name: String,
    pub name_span: Span,
    pub traits: Vec<TypeName>,
    pub members: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassMember {
    Field(FieldDecl),
    Method(MethodDecl),
    Constructor(MethodDecl),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDecl {
    pub visibility: Visibility,
    pub is_static: bool,
    pub variable: VariableDecl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodDecl {
    pub visibility: Visibility,
    pub is_static: bool,
    pub function: FunctionDecl,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitDecl {
    pub name: String,
    pub name_span: Span,
    pub methods: Vec<FunctionSignature>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeName>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDecl {
    pub path: Vec<String>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseDecl {
    pub path: Vec<String>,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Decl {
    Variable(VariableDecl),
    Function(FunctionDecl),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableDecl {
    pub name: String,
    pub name_span: Span,
    pub mutable: bool,
    pub annotation: Option<TypeName>,
    pub initializer: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<Parameter>,
    pub return_type: Option<TypeName>,
    pub body: Stmt,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub span: Span,
    pub type_name: TypeName,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeName {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stmt {
    Variable(VariableDecl),
    Block {
        statements: Vec<Stmt>,
        span: Span,
    },
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    Break {
        span: Span,
    },
    Continue {
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Expression {
        expression: Expr,
        span: Span,
    },
}

impl Stmt {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Variable(decl) => decl.span,
            Self::Block { span, .. }
            | Self::If { span, .. }
            | Self::While { span, .. }
            | Self::Break { span }
            | Self::Continue { span }
            | Self::Return { span, .. }
            | Self::Expression { span, .. } => *span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Expr {
    pub id: ExprId,
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExprKind {
    Literal(Literal),
    Name(String),
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Member {
        object: Box<Expr>,
        name: String,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    Cast {
        expression: Box<Expr>,
        type_name: TypeName,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Literal {
    Number(String),
    String(String),
    Char(String),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Or,
    And,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Power,
}
