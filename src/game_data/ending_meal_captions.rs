use anyhow::{Context, Result, bail, ensure};
use v30::{Instruction, decode_bytes};

use super::ending_ingredient_labels::BLANK_GLYPH_INDEX;
use super::indexed_text::{IndexedGlyph, indexed_glyph_text};

pub(crate) const COM_ORIGIN: usize = 0x100;
pub(crate) const INTERPRETER_FILE_OFFSET: usize = 0x2e5c;
const INTERPRETER_RUNTIME_ADDRESS: usize = INTERPRETER_FILE_OFFSET + COM_ORIGIN;
pub(crate) const INTERPRETER_BODY_LEN: usize = 0x7e;
pub(crate) const INTERPRETER_SIGNATURE: &[u8] = &[
    0x1e, 0x0e, 0x1f, 0x2e, 0x8b, 0x36, 0x8b, 0x3d, 0xac, 0x3c, 0x05,
];
pub(crate) const GLYPH_RENDERER_CALL_FILE_OFFSET: usize = 0x2e89;
pub(crate) const GLYPH_RENDERER_RUNTIME_ADDRESS: usize = 0x3076;

const GLYPH_COMMAND: u8 = 0x0a;
const CLEAR_COMMAND: u8 = 0x0b;
const YIELD_COMMAND: u8 = 0x1e;
const END_COMMAND: u8 = 0x28;
const TRANSITION_COMMAND: u8 = 0x46;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingMealCaptionCatalog<'a> {
    pub consumer: EndingMealCaptionConsumerEvidence,
    pub records: Vec<EndingMealCaptionRecord<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingMealCaptionConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub glyph_renderer_call_file_offset: usize,
    pub glyph_renderer_runtime_address: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingMealCaptionRecord<'a> {
    pub id: &'static str,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub bytes: &'a [u8],
    pub visual_state_indices: Vec<u8>,
    pub spans: Vec<EndingMealCaptionSpan>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingMealCaptionSpan {
    pub slot_file_offsets: Vec<usize>,
    pub glyphs: Vec<IndexedGlyph>,
    pub text: String,
}

struct CaptionRecordSpec {
    id: &'static str,
    file_offset: usize,
    end_file_offset: usize,
    visual_state_indices: &'static [u8],
    source_spans: &'static [&'static str],
}

const RECORD_SPECS: [CaptionRecordSpec; 5] = [
    CaptionRecordSpec {
        id: "before-eating",
        file_offset: 0x404e,
        end_file_offset: 0x4074,
        visual_state_indices: &[1],
        source_spans: &["いただきま～す"],
    },
    CaptionRecordSpec {
        id: "ordinary-reaction",
        file_offset: 0x4074,
        end_file_offset: 0x40b2,
        visual_state_indices: &[2],
        source_spans: &["おいしい！！"],
    },
    CaptionRecordSpec {
        id: "excellent-reaction",
        file_offset: 0x40b2,
        end_file_offset: 0x40f4,
        visual_state_indices: &[4],
        source_spans: &["ちょ～おいしい！！！"],
    },
    CaptionRecordSpec {
        id: "bad-reaction",
        file_offset: 0x40f4,
        end_file_offset: 0x412d,
        visual_state_indices: &[3],
        source_spans: &["・・・・・・・・・・"],
    },
    CaptionRecordSpec {
        id: "legendary-reaction",
        file_offset: 0x412d,
        end_file_offset: 0x41a7,
        visual_state_indices: &[2, 4],
        source_spans: &["・・・これが・・・", "でんせつのカレ～！！！！"],
    },
];

pub(crate) fn parse_ending_meal_captions(bytes: &[u8]) -> Result<EndingMealCaptionCatalog<'_>> {
    let consumer = verify_consumer(bytes)?;
    let records = RECORD_SPECS
        .iter()
        .map(|spec| parse_record(bytes, spec))
        .collect::<Result<Vec<_>>>()?;
    Ok(EndingMealCaptionCatalog { consumer, records })
}

fn parse_record<'a>(
    bytes: &'a [u8],
    spec: &CaptionRecordSpec,
) -> Result<EndingMealCaptionRecord<'a>> {
    let record_bytes = bytes
        .get(spec.file_offset..spec.end_file_offset)
        .with_context(|| format!("ENDING meal caption {} is truncated", spec.id))?;
    ensure!(
        record_bytes.last() == Some(&END_COMMAND),
        "ENDING meal caption {} has no fixed end command",
        spec.id
    );

    let mut cursor = 0;
    let mut pending_glyphs = Vec::new();
    let mut visual_state_indices = Vec::new();
    let mut spans = Vec::new();
    while cursor < record_bytes.len() {
        let command_offset = cursor;
        let command = record_bytes[cursor];
        cursor += 1;
        match command {
            GLYPH_COMMAND => {
                let index = *record_bytes.get(cursor).with_context(|| {
                    format!(
                        "ENDING meal caption {} ends inside a glyph command",
                        spec.id
                    )
                })?;
                pending_glyphs.push((spec.file_offset + cursor, index));
                cursor += 1;
            }
            CLEAR_COMMAND | TRANSITION_COMMAND => {
                push_visible_span(spec.id, &mut spans, &mut pending_glyphs)?;
            }
            END_COMMAND => {
                push_visible_span(spec.id, &mut spans, &mut pending_glyphs)?;
                ensure!(
                    cursor == record_bytes.len(),
                    "ENDING meal caption {} has bytes after its end command",
                    spec.id
                );
            }
            0x00..=0x05 => visual_state_indices.push(command),
            YIELD_COMMAND => {}
            other => bail!(
                "ENDING meal caption {} uses unknown command {other:02X} at file offset {:#x}",
                spec.id,
                spec.file_offset + command_offset
            ),
        }
    }

    let actual_text = spans
        .iter()
        .map(|span| span.text.as_str())
        .collect::<Vec<_>>();
    ensure!(
        actual_text == spec.source_spans,
        "ENDING meal caption {} source text changed: expected {:?}, got {:?}",
        spec.id,
        spec.source_spans,
        actual_text
    );
    ensure!(
        visual_state_indices == spec.visual_state_indices,
        "ENDING meal caption {} visual-state sequence changed: expected {:?}, got {:?}",
        spec.id,
        spec.visual_state_indices,
        visual_state_indices
    );
    Ok(EndingMealCaptionRecord {
        id: spec.id,
        file_offset: spec.file_offset,
        runtime_address: spec.file_offset + COM_ORIGIN,
        bytes: record_bytes,
        visual_state_indices,
        spans,
    })
}

fn push_visible_span(
    record_id: &str,
    spans: &mut Vec<EndingMealCaptionSpan>,
    pending_glyphs: &mut Vec<(usize, u8)>,
) -> Result<()> {
    let Some(first_visible) = pending_glyphs
        .iter()
        .position(|(_, index)| *index != BLANK_GLYPH_INDEX)
    else {
        pending_glyphs.clear();
        return Ok(());
    };
    let last_visible = pending_glyphs
        .iter()
        .rposition(|(_, index)| *index != BLANK_GLYPH_INDEX)
        .expect("a first visible glyph guarantees a last visible glyph");
    let visible = &pending_glyphs[first_visible..=last_visible];
    let glyphs = visible
        .iter()
        .map(|(_, index)| {
            let text = indexed_glyph_text(*index).with_context(|| {
                format!("ENDING meal caption {record_id} uses unresolved glyph {index:02X}")
            })?;
            Ok(IndexedGlyph {
                index: *index,
                text: Some(text),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let text = glyphs
        .iter()
        .map(|glyph| glyph.text.expect("caption glyph resolution was checked"))
        .collect();
    spans.push(EndingMealCaptionSpan {
        slot_file_offsets: visible.iter().map(|(offset, _)| *offset).collect(),
        glyphs,
        text,
    });
    pending_glyphs.clear();
    Ok(())
}

fn verify_consumer(bytes: &[u8]) -> Result<EndingMealCaptionConsumerEvidence> {
    ensure!(
        bytes.get(INTERPRETER_FILE_OFFSET..INTERPRETER_FILE_OFFSET + INTERPRETER_SIGNATURE.len())
            == Some(INTERPRETER_SIGNATURE),
        "ENDING meal-caption interpreter signature does not match"
    );
    let body = bytes
        .get(INTERPRETER_FILE_OFFSET..INTERPRETER_FILE_OFFSET + INTERPRETER_BODY_LEN)
        .context("ENDING meal-caption interpreter body is truncated")?;
    let decoded_instruction_count = decode_exact_block(body)?;
    verify_renderer_call(bytes)?;
    Ok(EndingMealCaptionConsumerEvidence {
        file_offset: INTERPRETER_FILE_OFFSET,
        runtime_address: INTERPRETER_RUNTIME_ADDRESS,
        decoded_instruction_count,
        glyph_renderer_call_file_offset: GLYPH_RENDERER_CALL_FILE_OFFSET,
        glyph_renderer_runtime_address: GLYPH_RENDERER_RUNTIME_ADDRESS,
    })
}

fn verify_renderer_call(bytes: &[u8]) -> Result<()> {
    let call_bytes = bytes
        .get(GLYPH_RENDERER_CALL_FILE_OFFSET..GLYPH_RENDERER_CALL_FILE_OFFSET + 3)
        .context("ENDING meal-caption renderer call is truncated")?;
    let call = decode_bytes(call_bytes).context("decode ENDING meal-caption renderer call")?;
    ensure!(
        call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
        "ENDING meal-caption renderer is not a typed V30 near CALL"
    );
    let displacement = i16::from_le_bytes([call_bytes[1], call_bytes[2]]);
    let call_runtime_address = GLYPH_RENDERER_CALL_FILE_OFFSET + COM_ORIGIN;
    let target = (call_runtime_address + call.byte_len).wrapping_add_signed(displacement.into());
    ensure!(
        target == GLYPH_RENDERER_RUNTIME_ADDRESS,
        "ENDING meal-caption renderer call targets {target:#x} instead of {GLYPH_RENDERER_RUNTIME_ADDRESS:#x}"
    );
    Ok(())
}

fn decode_exact_block(bytes: &[u8]) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..]).with_context(|| {
            format!("decode V30 instruction at ENDING meal-caption byte {cursor:#x}")
        })?;
        cursor += decoded.byte_len;
        instruction_count += 1;
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed ENDING meal-caption interpreter boundary"
    );
    Ok(instruction_count)
}

#[cfg(test)]
#[path = "ending_meal_captions_tests.rs"]
mod ending_meal_captions_tests;
