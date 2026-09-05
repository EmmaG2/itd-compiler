use crate::source::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: String,
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: &str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code: code.to_owned(),
            severity: Severity::Error,
            message: message.into(),
            labels: vec![Label {
                span,
                message: String::new(),
            }],
        }
    }

    #[must_use]
    pub fn warning(code: &str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code: code.to_owned(),
            severity: Severity::Warning,
            message: message.into(),
            labels: vec![Label {
                span,
                message: String::new(),
            }],
        }
    }
}
