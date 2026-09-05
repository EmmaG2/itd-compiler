use crate::{
    ast::{Item, Program},
    diagnostic::Diagnostic,
    source::{SourceId, Span},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Instruction {
    Execute(u32),
    Halt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chunk {
    pub code: Vec<Instruction>,
    pub constants: Vec<Item>,
    pub spans: Vec<Span>,
}

pub fn compile(program: &Program) -> Result<Chunk, Diagnostic> {
    let mut code = Vec::with_capacity(program.items.len() + 1);
    let mut constants = Vec::with_capacity(program.items.len());
    let mut spans = Vec::with_capacity(program.items.len() + 1);
    for item in &program.items {
        let index = u32::try_from(constants.len()).map_err(|_| {
            Diagnostic::error(
                "E5001",
                "demasiadas constantes de bytecode",
                item_span(item),
            )
        })?;
        code.push(Instruction::Execute(index));
        spans.push(item_span(item));
        constants.push(item.clone());
    }
    code.push(Instruction::Halt);
    spans.push(
        spans
            .last()
            .copied()
            .unwrap_or(Span::new(SourceId(0), 0, 0)),
    );
    Ok(Chunk {
        code,
        constants,
        spans,
    })
}

pub fn validate(chunk: &Chunk) -> Result<(), Diagnostic> {
    let fallback = Span::new(SourceId(0), 0, 0);
    if chunk.code.len() != chunk.spans.len() {
        return Err(Diagnostic::error(
            "E5002",
            "mapa de spans de bytecode inválido",
            fallback,
        ));
    }
    if !matches!(chunk.code.last(), Some(Instruction::Halt)) {
        return Err(Diagnostic::error(
            "E5002",
            "bytecode sin instrucción Halt final",
            chunk.spans.last().copied().unwrap_or(fallback),
        ));
    }
    if chunk.code[..chunk.code.len() - 1]
        .iter()
        .any(|value| matches!(value, Instruction::Halt))
    {
        return Err(Diagnostic::error(
            "E5002",
            "instrucción después de Halt",
            fallback,
        ));
    }
    for (offset, instruction) in chunk.code.iter().enumerate() {
        if let Instruction::Execute(index) = instruction
            && usize::try_from(*index)
                .ok()
                .and_then(|value| chunk.constants.get(value))
                .is_none()
        {
            return Err(Diagnostic::error(
                "E5002",
                format!("operando inválido en bytecode {offset}"),
                chunk.spans[offset],
            ));
        }
    }
    Ok(())
}

fn item_span(item: &Item) -> Span {
    match item {
        Item::Decl(value) | Item::Export(value) => match value {
            crate::ast::Decl::Variable(value) => value.span,
            crate::ast::Decl::Function(value) => value.span,
        },
        Item::Class(value) | Item::ExportClass(value) => value.span,
        Item::Trait(value) | Item::ExportTrait(value) => value.span,
        Item::Module(value) => value.span,
        Item::Use(value) => value.span,
        Item::Stmt(value) => value.span(),
    }
}
