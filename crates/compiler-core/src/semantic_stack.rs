use crate::{ast::ExprId, semantic::Type, source::Span};

pub const MAX_TRACE_EVENTS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceEventKind {
    Push,
    Reduce,
    Consume,
    Conversion,
    Recover,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceAttribute {
    pub expression: ExprId,
    pub type_label: String,
    pub span: Span,
}

/// A delta over the actual stack. Conversion events do not mutate it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTraceEvent {
    pub kind: TraceEventKind,
    pub expression: ExprId,
    pub span: Span,
    pub rule: &'static str,
    pub depth_before: usize,
    pub depth_after: usize,
    pub pop_count: usize,
    pub pushed: Option<TraceAttribute>,
    pub conversion_type: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticTrace {
    pub events: Vec<SemanticTraceEvent>,
    pub truncated: bool,
}

struct TraceCapture {
    trace: SemanticTrace,
    limit: usize,
}

impl TraceCapture {
    fn record(&mut self, event: impl FnOnce() -> SemanticTraceEvent) {
        if self.trace.events.len() == self.limit {
            self.trace.truncated = true;
            return;
        }
        self.trace.events.push(event());
    }
}

/// Never format recursive function signatures or unbounded names for inspection.
fn type_label(ty: &Type) -> String {
    let name = match ty {
        Type::Byte => "byte",
        Type::Short => "short",
        Type::Int => "int",
        Type::Long => "long",
        Type::Decimal => "decimal",
        Type::IntegerOrDecimal => "integer or decimal",
        Type::Float => "float",
        Type::Double => "double",
        Type::Bool => "bool",
        Type::Str => "str",
        Type::Char => "char",
        Type::Void => "void",
        Type::Function(_, _) => "function",
        Type::Class(name) | Type::Trait(name) | Type::ClassObject(name) => name,
        Type::Error => "error",
    };
    name.chars().take(80).collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticAttribute {
    pub expression: ExprId,
    pub ty: Type,
    pub span: Span,
}

/// The lowest height an expression is allowed to consume or recover.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Checkpoint(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StackError {
    InvalidCheckpoint,
    Underflow,
    UnexpectedExpression,
    Unbalanced,
}

#[derive(Default)]
pub(crate) struct SemanticStack {
    attributes: Vec<SemanticAttribute>,
    capture: Option<TraceCapture>,
}

impl SemanticStack {
    pub fn enable_trace(&mut self, limit: usize) {
        self.capture = Some(TraceCapture {
            trace: SemanticTrace::default(),
            limit: limit.min(MAX_TRACE_EVENTS),
        });
    }

    pub fn take_trace(&mut self) -> Option<SemanticTrace> {
        self.capture.take().map(|capture| capture.trace)
    }

    pub fn conversion(&mut self, expression: ExprId, ty: &Type) {
        let Some(capture) = &mut self.capture else {
            return;
        };
        if capture.trace.events.len() == capture.limit {
            capture.trace.truncated = true;
            return;
        }
        let Some(span) = capture.trace.events.iter().rev().find_map(|event| {
            event
                .pushed
                .as_ref()
                .filter(|value| value.expression == expression)
                .map(|value| value.span)
        }) else {
            return;
        };
        let depth = self.attributes.len();
        capture.record(|| SemanticTraceEvent {
            kind: TraceEventKind::Conversion,
            expression,
            span,
            rule: "implicit conversion",
            depth_before: depth,
            depth_after: depth,
            pop_count: 0,
            pushed: None,
            conversion_type: Some(type_label(ty)),
        });
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint(self.attributes.len())
    }

    #[cfg(test)]
    pub fn push(&mut self, attribute: SemanticAttribute) {
        self.push_result(attribute, "literal", false);
    }

    pub fn push_result(&mut self, attribute: SemanticAttribute, rule: &'static str, reduced: bool) {
        let depth = self.attributes.len();
        if let Some(capture) = &mut self.capture {
            capture.record(|| SemanticTraceEvent {
                kind: if reduced {
                    TraceEventKind::Reduce
                } else {
                    TraceEventKind::Push
                },
                expression: attribute.expression,
                span: attribute.span,
                rule,
                depth_before: depth,
                depth_after: depth + 1,
                pop_count: 0,
                pushed: Some(TraceAttribute {
                    expression: attribute.expression,
                    type_label: type_label(&attribute.ty),
                    span: attribute.span,
                }),
                conversion_type: None,
            });
        }
        self.attributes.push(attribute);
    }

    pub fn pop(
        &mut self,
        checkpoint: Checkpoint,
        expected: ExprId,
    ) -> Result<SemanticAttribute, StackError> {
        self.validate(checkpoint)?;
        if self.attributes.len() == checkpoint.0 {
            return Err(StackError::Underflow);
        }
        if self.attributes.last().map(|value| value.expression) != Some(expected) {
            return Err(StackError::UnexpectedExpression);
        }
        let depth = self.attributes.len();
        let attribute = self.attributes.pop().ok_or(StackError::Underflow)?;
        if let Some(capture) = &mut self.capture {
            capture.record(|| SemanticTraceEvent {
                kind: TraceEventKind::Consume,
                expression: attribute.expression,
                span: attribute.span,
                rule: "consume attribute",
                depth_before: depth,
                depth_after: depth - 1,
                pop_count: 1,
                pushed: None,
                conversion_type: None,
            });
        }
        Ok(attribute)
    }

    pub fn finish(
        &mut self,
        checkpoint: Checkpoint,
        expected: ExprId,
    ) -> Result<SemanticAttribute, StackError> {
        self.validate(checkpoint)?;
        if self.attributes.len() - checkpoint.0 != 1 {
            return Err(StackError::Unbalanced);
        }
        self.pop(checkpoint, expected)
    }

    pub fn recover(&mut self, checkpoint: Checkpoint) -> Result<(), StackError> {
        self.validate(checkpoint)?;
        if let Some(attribute) = self.attributes.last()
            && let Some(capture) = &mut self.capture
        {
            let depth = self.attributes.len();
            capture.record(|| SemanticTraceEvent {
                kind: TraceEventKind::Recover,
                expression: attribute.expression,
                span: attribute.span,
                rule: "recover frame",
                depth_before: depth,
                depth_after: checkpoint.0,
                pop_count: depth - checkpoint.0,
                pushed: None,
                conversion_type: None,
            });
        }
        self.attributes.truncate(checkpoint.0);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.attributes.is_empty()
    }

    fn validate(&self, checkpoint: Checkpoint) -> Result<(), StackError> {
        if checkpoint.0 > self.attributes.len() {
            return Err(StackError::InvalidCheckpoint);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    fn attribute(id: usize) -> SemanticAttribute {
        SemanticAttribute {
            expression: ExprId(id),
            ty: Type::Int,
            span: Span::new(SourceId(0), id, id + 1),
        }
    }

    #[test]
    fn recovery_trace_discards_only_current_frame() {
        let mut stack = SemanticStack::default();
        stack.enable_trace(20);
        let root = stack.checkpoint();
        stack.push(attribute(0));
        let child = stack.checkpoint();
        stack.push(attribute(1));
        assert_eq!(
            stack.pop(child, ExprId(9)),
            Err(StackError::UnexpectedExpression)
        );
        stack.recover(child).unwrap();
        stack.finish(root, ExprId(0)).unwrap();
        let trace = stack.take_trace().unwrap();
        let recovery = &trace.events[2];
        assert_eq!(recovery.kind, TraceEventKind::Recover);
        assert_eq!(
            (
                recovery.depth_before,
                recovery.depth_after,
                recovery.pop_count
            ),
            (2, 1, 1)
        );
        assert_eq!(trace.events.last().unwrap().depth_after, 0);
    }

    #[test]
    fn pops_children_in_lifo_order_and_preserves_parent() {
        let mut stack = SemanticStack::default();
        let root = stack.checkpoint();
        stack.push(attribute(0));
        let child = stack.checkpoint();
        stack.push(attribute(1));
        stack.push(attribute(2));
        assert_eq!(stack.pop(child, ExprId(2)), Ok(attribute(2)));
        assert_eq!(stack.pop(child, ExprId(1)), Ok(attribute(1)));
        assert_eq!(stack.pop(child, ExprId(0)), Err(StackError::Underflow));
        assert_eq!(stack.pop(root, ExprId(0)), Ok(attribute(0)));
        assert!(stack.is_empty());
    }

    #[test]
    fn wrong_id_does_not_consume_an_attribute() {
        let mut stack = SemanticStack::default();
        let frame = stack.checkpoint();
        stack.push(attribute(1));
        assert_eq!(
            stack.pop(frame, ExprId(2)),
            Err(StackError::UnexpectedExpression)
        );
        assert_eq!(stack.pop(frame, ExprId(1)), Ok(attribute(1)));
    }

    #[test]
    fn finish_requires_exactly_one_matching_result() {
        let mut stack = SemanticStack::default();
        let frame = stack.checkpoint();
        assert_eq!(stack.finish(frame, ExprId(1)), Err(StackError::Unbalanced));
        stack.push(attribute(1));
        stack.push(attribute(2));
        assert_eq!(stack.finish(frame, ExprId(2)), Err(StackError::Unbalanced));
        stack.recover(frame).unwrap();
        stack.push(attribute(3));
        assert_eq!(stack.finish(frame, ExprId(3)), Ok(attribute(3)));
        assert!(stack.is_empty());
    }

    #[test]
    fn recovery_only_discards_the_current_segment() {
        let mut stack = SemanticStack::default();
        let root = stack.checkpoint();
        stack.push(attribute(0));
        let child = stack.checkpoint();
        stack.push(attribute(1));
        stack.push(attribute(2));
        stack.recover(child).unwrap();
        assert_eq!(stack.finish(root, ExprId(0)), Ok(attribute(0)));
        assert_eq!(stack.recover(child), Err(StackError::InvalidCheckpoint));
    }
}
