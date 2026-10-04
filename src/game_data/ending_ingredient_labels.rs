use anyhow::{Context, Result, ensure};
use v30::{Instruction, decode_bytes};

use super::indexed_text::{IndexedGlyph, indexed_glyph_text};

pub(crate) const GROUP_COUNT: usize = 9;
pub(crate) const VARIANT_COUNT: usize = 3;
pub(crate) const RECORD_CELL_COUNT: usize = 16;
pub(crate) const TABLE_FILE_OFFSET: usize = 0x45e1;
pub(crate) const TABLE_RUNTIME_ADDRESS: usize = TABLE_FILE_OFFSET + COM_ORIGIN;
pub(crate) const TABLE_BYTE_SIZE: usize = GROUP_COUNT * VARIANT_COUNT * RECORD_CELL_COUNT;
pub(crate) const GROUP_STRIDE: usize = VARIANT_COUNT * RECORD_CELL_COUNT;
pub(crate) const BLANK_GLYPH_INDEX: u8 = 0x4d;

const COM_ORIGIN: usize = 0x100;
const CONSUMER_FILE_OFFSET: usize = 0x3a28;
const CONSUMER_RUNTIME_ADDRESS: usize = CONSUMER_FILE_OFFSET + COM_ORIGIN;
const CONSUMER_BODY_LEN: usize = 0x131;
const CONSUMER_SIGNATURE: &[u8] = &[0xbf, 0x39, 0x4a, 0xcd, 0x7d];
const POINTER_LIST_RUNTIME_ADDRESS: usize = 0x4a39;
const FIXED_CELL_LOOP_FILE_OFFSET: usize = 0x3b1a;
const GLYPH_RENDERER_CALL_FILE_OFFSET: usize = 0x3b29;
const GLYPH_RENDERER_RUNTIME_ADDRESS: usize = 0x3024;
const SELECTOR_LOAD_FILE_OFFSETS: [usize; GROUP_COUNT] = [
    0x3a2d, 0x3a44, 0x3a5b, 0x3a72, 0x3a89, 0x3aa0, 0x3ab7, 0x3ace, 0x3ae5,
];
pub(crate) const INPUT_STATE_OFFSETS: [u8; GROUP_COUNT] =
    [0x20, 0x21, 0x24, 0x23, 0x22, 0x25, 0x26, 0x27, 0x28];
pub(crate) const GROUP_RUNTIME_ADDRESSES: [usize; GROUP_COUNT] = [
    0x46e1, 0x4711, 0x4741, 0x4771, 0x47a1, 0x47d1, 0x4801, 0x4831, 0x4861,
];
const GROUP_BASE_ADD_FILE_OFFSETS: [usize; GROUP_COUNT] = [
    0x3a3b, 0x3a52, 0x3a69, 0x3a80, 0x3a97, 0x3aae, 0x3ac5, 0x3adc, 0x3af3,
];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingIngredientLabelCatalog<'a> {
    pub consumer: EndingIngredientLabelConsumerEvidence,
    pub table_file_offset: usize,
    pub table_runtime_address: usize,
    pub table_byte_size: usize,
    pub records: Vec<EndingIngredientLabelRecord<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingIngredientLabelConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub pointer_list_runtime_address: usize,
    pub input_state_offsets: [u8; GROUP_COUNT],
    pub group_runtime_addresses: [usize; GROUP_COUNT],
    pub fixed_cell_count: usize,
    pub fixed_cell_loop_file_offset: usize,
    pub glyph_renderer_call_file_offset: usize,
    pub glyph_renderer_runtime_address: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingIngredientLabelRecord<'a> {
    pub group_index: usize,
    pub variant_index: usize,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub bytes: &'a [u8],
    pub glyphs: Vec<IndexedGlyph>,
    pub text: String,
}

pub(crate) fn parse_ending_ingredient_labels(
    bytes: &[u8],
) -> Result<EndingIngredientLabelCatalog<'_>> {
    let consumer = verify_consumer(bytes)?;
    let table = bytes
        .get(TABLE_FILE_OFFSET..TABLE_FILE_OFFSET + TABLE_BYTE_SIZE)
        .context("ENDING ingredient label table is truncated")?;
    let mut records = Vec::with_capacity(GROUP_COUNT * VARIANT_COUNT);
    for group_index in 0..GROUP_COUNT {
        for variant_index in 0..VARIANT_COUNT {
            let table_offset = group_index * GROUP_STRIDE + variant_index * RECORD_CELL_COUNT;
            let record_bytes = &table[table_offset..table_offset + RECORD_CELL_COUNT];
            let glyphs = record_bytes
                .iter()
                .copied()
                .map(|index| IndexedGlyph {
                    index,
                    text: indexed_glyph_text(index),
                })
                .collect::<Vec<_>>();
            ensure!(
                glyphs.iter().all(|glyph| glyph.text.is_some()),
                "ENDING ingredient group {}, variant {} contains an unresolved glyph",
                group_index + 1,
                variant_index + 1
            );
            let text = glyphs
                .iter()
                .map(|glyph| glyph.text.expect("glyph resolution was checked"))
                .collect::<String>()
                .trim_matches(' ')
                .to_owned();
            ensure!(
                !text.is_empty(),
                "ENDING ingredient group {}, variant {} is blank",
                group_index + 1,
                variant_index + 1
            );
            let file_offset = TABLE_FILE_OFFSET + table_offset;
            records.push(EndingIngredientLabelRecord {
                group_index,
                variant_index,
                file_offset,
                runtime_address: file_offset + COM_ORIGIN,
                bytes: record_bytes,
                glyphs,
                text,
            });
        }
    }

    Ok(EndingIngredientLabelCatalog {
        consumer,
        table_file_offset: TABLE_FILE_OFFSET,
        table_runtime_address: TABLE_RUNTIME_ADDRESS,
        table_byte_size: TABLE_BYTE_SIZE,
        records,
    })
}

fn verify_consumer(bytes: &[u8]) -> Result<EndingIngredientLabelConsumerEvidence> {
    ensure!(
        bytes.get(CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_SIGNATURE.len())
            == Some(CONSUMER_SIGNATURE),
        "ENDING ingredient-label consumer signature does not match"
    );
    for ((load_offset, state_offset), (add_offset, runtime_address)) in SELECTOR_LOAD_FILE_OFFSETS
        .into_iter()
        .zip(INPUT_STATE_OFFSETS)
        .zip(
            GROUP_BASE_ADD_FILE_OFFSETS
                .into_iter()
                .zip(GROUP_RUNTIME_ADDRESSES),
        )
    {
        ensure!(
            bytes.get(load_offset..load_offset + 3) == Some(&[0x8a, 0x44, state_offset]),
            "ENDING ingredient state selector does not match at {load_offset:#x}"
        );
        let immediate = u16::try_from(runtime_address)
            .context("ENDING ingredient table runtime address exceeds 16 bits")?
            .to_le_bytes();
        ensure!(
            bytes.get(add_offset..add_offset + 3) == Some(&[0x05, immediate[0], immediate[1]]),
            "ENDING ingredient table base does not match at {add_offset:#x}"
        );
    }
    ensure!(
        bytes.get(FIXED_CELL_LOOP_FILE_OFFSET..FIXED_CELL_LOOP_FILE_OFFSET + 3)
            == Some(&[0xbf, RECORD_CELL_COUNT as u8, 0x00]),
        "ENDING ingredient renderer no longer reads exactly {RECORD_CELL_COUNT} cells"
    );
    let call_bytes = bytes
        .get(GLYPH_RENDERER_CALL_FILE_OFFSET..GLYPH_RENDERER_CALL_FILE_OFFSET + 3)
        .context("ENDING ingredient glyph renderer call is truncated")?;
    let call = decode_bytes(call_bytes).context("decode ENDING ingredient glyph renderer call")?;
    ensure!(
        call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
        "ENDING ingredient glyph renderer is not a typed V30 near CALL"
    );
    let displacement = i16::from_le_bytes([call_bytes[1], call_bytes[2]]);
    let call_runtime_address = GLYPH_RENDERER_CALL_FILE_OFFSET + COM_ORIGIN;
    let target = (call_runtime_address + 3).wrapping_add_signed(isize::from(displacement));
    ensure!(
        target == GLYPH_RENDERER_RUNTIME_ADDRESS,
        "ENDING ingredient glyph renderer CALL targets {target:#x} instead of {GLYPH_RENDERER_RUNTIME_ADDRESS:#x}"
    );

    let body = bytes
        .get(CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_BODY_LEN)
        .context("ENDING ingredient-label consumer body is truncated")?;
    let decoded_instruction_count = decode_exact_block(body)?;
    Ok(EndingIngredientLabelConsumerEvidence {
        file_offset: CONSUMER_FILE_OFFSET,
        runtime_address: CONSUMER_RUNTIME_ADDRESS,
        decoded_instruction_count,
        pointer_list_runtime_address: POINTER_LIST_RUNTIME_ADDRESS,
        input_state_offsets: INPUT_STATE_OFFSETS,
        group_runtime_addresses: GROUP_RUNTIME_ADDRESSES,
        fixed_cell_count: RECORD_CELL_COUNT,
        fixed_cell_loop_file_offset: FIXED_CELL_LOOP_FILE_OFFSET,
        glyph_renderer_call_file_offset: GLYPH_RENDERER_CALL_FILE_OFFSET,
        glyph_renderer_runtime_address: GLYPH_RENDERER_RUNTIME_ADDRESS,
    })
}

fn decode_exact_block(bytes: &[u8]) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..]).with_context(|| {
            format!("decode V30 instruction at ENDING ingredient consumer byte {cursor:#x}")
        })?;
        cursor += decoded.byte_len;
        instruction_count += 1;
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed ENDING ingredient consumer boundary"
    );
    Ok(instruction_count)
}

#[cfg(test)]
#[path = "ending_ingredient_labels_tests.rs"]
mod ending_ingredient_labels_tests;
