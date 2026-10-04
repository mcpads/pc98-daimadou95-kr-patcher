use anyhow::{Context, Result, ensure};
use v30::{Instruction, Operand, Register16, decode_bytes};

use super::indexed_text::{IndexedGlyph, indexed_glyph_text};

pub(crate) const COM_ORIGIN: usize = 0x100;
pub(crate) const CONSUMER_FILE_OFFSET: usize = 0x2743;
const CONSUMER_RUNTIME_ADDRESS: usize = CONSUMER_FILE_OFFSET + COM_ORIGIN;
pub(crate) const CONSUMER_BODY_LEN: usize = 0x61;
pub(crate) const CONSUMER_SIGNATURE: &[u8] = &[0x2e, 0x8e, 0x1e, 0x8d, 0x3d];
pub(crate) const GLYPH_LOAD_FILE_OFFSETS: [usize; 3] = [0x2748, 0x2765, 0x2782];
pub(crate) const GLYPH_RENDERER_CALL_FILE_OFFSETS: [usize; 3] = [0x2751, 0x276e, 0x278b];
pub(crate) const GLYPH_RENDERER_RUNTIME_ADDRESS: usize = 0x312c;
pub(crate) const SOURCE_GLYPH_INDICES: [u8; 3] = [0x0b, 0x22, 0xd0];
pub(crate) const GLYPH_SLOT_COUNT: usize = SOURCE_GLYPH_INDICES.len();

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingMealReactionCatalog<'a> {
    pub consumer: EndingMealReactionConsumerEvidence,
    pub glyph_loads: Vec<EndingMealReactionGlyphLoad<'a>>,
    pub text: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingMealReactionConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub glyph_load_file_offsets: [usize; GLYPH_SLOT_COUNT],
    pub glyph_renderer_call_file_offsets: [usize; GLYPH_SLOT_COUNT],
    pub glyph_renderer_runtime_address: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingMealReactionGlyphLoad<'a> {
    pub slot_index: usize,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub bytes: &'a [u8],
    pub glyph: IndexedGlyph,
}

pub(crate) fn parse_ending_meal_reaction(bytes: &[u8]) -> Result<EndingMealReactionCatalog<'_>> {
    ensure!(
        bytes.get(CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_SIGNATURE.len())
            == Some(CONSUMER_SIGNATURE),
        "ENDING meal-reaction consumer signature does not match"
    );

    let glyph_loads = GLYPH_LOAD_FILE_OFFSETS
        .into_iter()
        .zip(SOURCE_GLYPH_INDICES)
        .enumerate()
        .map(|(slot_index, (file_offset, expected_index))| {
            parse_glyph_load(bytes, slot_index, file_offset, expected_index)
        })
        .collect::<Result<Vec<_>>>()?;
    for call_offset in GLYPH_RENDERER_CALL_FILE_OFFSETS {
        verify_typed_call(bytes, call_offset)?;
    }
    let body = bytes
        .get(CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_BODY_LEN)
        .context("ENDING meal-reaction consumer body is truncated")?;
    let decoded_instruction_count = decode_exact_block(body)?;
    let text = glyph_loads
        .iter()
        .map(|load| {
            load.glyph
                .text
                .expect("ENDING meal-reaction glyph resolution was checked")
        })
        .collect();

    Ok(EndingMealReactionCatalog {
        consumer: EndingMealReactionConsumerEvidence {
            file_offset: CONSUMER_FILE_OFFSET,
            runtime_address: CONSUMER_RUNTIME_ADDRESS,
            decoded_instruction_count,
            glyph_load_file_offsets: GLYPH_LOAD_FILE_OFFSETS,
            glyph_renderer_call_file_offsets: GLYPH_RENDERER_CALL_FILE_OFFSETS,
            glyph_renderer_runtime_address: GLYPH_RENDERER_RUNTIME_ADDRESS,
        },
        glyph_loads,
        text,
    })
}

fn parse_glyph_load(
    bytes: &[u8],
    slot_index: usize,
    file_offset: usize,
    expected_index: u8,
) -> Result<EndingMealReactionGlyphLoad<'_>> {
    let instruction_bytes = bytes
        .get(file_offset..file_offset + 3)
        .with_context(|| format!("ENDING meal-reaction glyph load {slot_index} is truncated"))?;
    let decoded = decode_bytes(instruction_bytes)
        .with_context(|| format!("decode ENDING meal-reaction glyph load {slot_index}"))?;
    ensure!(
        decoded.byte_len == 3
            && decoded.instruction
                == Instruction::Mov {
                    dest: Operand::Reg16(Register16::SI),
                    src: Operand::Imm16(u16::from(expected_index)),
                },
        "ENDING meal-reaction glyph load {slot_index} is not the verified typed V30 source"
    );
    let text = indexed_glyph_text(expected_index).with_context(|| {
        format!("ENDING meal-reaction glyph {expected_index:02X} is unresolved")
    })?;
    Ok(EndingMealReactionGlyphLoad {
        slot_index,
        file_offset,
        runtime_address: file_offset + COM_ORIGIN,
        bytes: instruction_bytes,
        glyph: IndexedGlyph {
            index: expected_index,
            text: Some(text),
        },
    })
}

fn verify_typed_call(bytes: &[u8], file_offset: usize) -> Result<()> {
    let call_bytes = bytes
        .get(file_offset..file_offset + 3)
        .context("ENDING meal-reaction glyph renderer call is truncated")?;
    let call = decode_bytes(call_bytes).context("decode ENDING meal-reaction glyph call")?;
    ensure!(
        call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
        "ENDING meal-reaction glyph renderer is not a typed V30 near CALL"
    );
    let displacement = i16::from_le_bytes([call_bytes[1], call_bytes[2]]);
    let call_runtime_address = file_offset + COM_ORIGIN;
    let target = (call_runtime_address + call.byte_len).wrapping_add_signed(displacement.into());
    ensure!(
        target == GLYPH_RENDERER_RUNTIME_ADDRESS,
        "ENDING meal-reaction glyph call targets {target:#x} instead of {GLYPH_RENDERER_RUNTIME_ADDRESS:#x}"
    );
    Ok(())
}

fn decode_exact_block(bytes: &[u8]) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..]).with_context(|| {
            format!("decode V30 instruction at ENDING meal-reaction byte {cursor:#x}")
        })?;
        cursor += decoded.byte_len;
        instruction_count += 1;
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed ENDING meal-reaction consumer boundary"
    );
    Ok(instruction_count)
}

#[cfg(test)]
#[path = "ending_meal_reaction_tests.rs"]
mod ending_meal_reaction_tests;
