use crate::{ast::ExprId, semantic::Type, source::Span};

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
}

impl SemanticStack {
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint(self.attributes.len())
    }

    pub fn push(&mut self, attribute: SemanticAttribute) {
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
        self.attributes.pop().ok_or(StackError::Underflow)
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
