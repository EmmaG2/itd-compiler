use super::{LocationDto, location};
use compiler_core::{
    bytecode::{Chunk, Instruction},
    source::SourceMap,
};
use serde::Serialize;
const MAX_INSTRUCTIONS: usize = 20_000;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BytecodeDto {
    pub rows: Vec<InstructionDto>,
    pub truncated: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstructionDto {
    pub function: String,
    pub offset: usize,
    pub instruction: String,
    pub location: LocationDto,
}
pub(crate) fn bytecode_dto(
    chunk: Option<&Chunk>,
    sources: &SourceMap,
    fallback: &str,
) -> BytecodeDto {
    let Some(chunk) = chunk else {
        return BytecodeDto::default();
    };
    let mut rows = Vec::new();
    for function in &chunk.functions {
        for (offset, (instruction, span)) in function.code.iter().zip(&function.spans).enumerate() {
            if rows.len() == MAX_INSTRUCTIONS {
                return BytecodeDto {
                    rows,
                    truncated: true,
                };
            }
            let text = match instruction {
                Instruction::Constant(index) => {
                    format!("Constant {index} {:?}", chunk.constants.get(*index))
                }
                instruction => format!("{instruction:?}"),
            };
            rows.push(InstructionDto {
                function: function.name.chars().take(256).collect(),
                offset,
                instruction: text.chars().take(256).collect(),
                location: location(sources, *span, fallback),
            });
        }
    }
    BytecodeDto {
        rows,
        truncated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exposes_instructions_with_original_locations_and_a_bound() {
        let result = compiler_core::compile_source("let x := 0.1dec + 0.2dec * 0.3dec;");
        let dto = bytecode_dto(result.chunk.as_ref(), &result.sources, "main.itd");
        let multiply = dto
            .rows
            .iter()
            .position(|r| r.instruction.contains("Multiply"))
            .unwrap();
        let add = dto
            .rows
            .iter()
            .position(|r| r.instruction.contains("Add"))
            .unwrap();
        assert!(multiply < add);
        assert_eq!(dto.rows[add].location.file, "main.itd");
        let mut chunk = result.chunk.unwrap();
        chunk.functions[0].code = vec![Instruction::Halt; MAX_INSTRUCTIONS + 1];
        chunk.functions[0].spans = vec![chunk.functions[0].span; MAX_INSTRUCTIONS + 1];
        let dto = bytecode_dto(Some(&chunk), &result.sources, "main.itd");
        assert_eq!(dto.rows.len(), MAX_INSTRUCTIONS);
        assert!(dto.truncated);
        assert!(
            bytecode_dto(None, &result.sources, "main.itd")
                .rows
                .is_empty()
        );
    }
}
