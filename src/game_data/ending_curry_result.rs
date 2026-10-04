use anyhow::{Context, Result, ensure};
use v30::{Instruction, decode_bytes};

use super::indexed_text::{IndexedGlyph, indexed_glyph_text};

const COM_ORIGIN: usize = 0x100;
const CONSUMER_FILE_OFFSET: usize = 0x28b7;
const CONSUMER_RUNTIME_ADDRESS: usize = CONSUMER_FILE_OFFSET + COM_ORIGIN;
const CONSUMER_BODY_LEN: usize = 0x116;
const CONSUMER_SIGNATURE: &[u8] = &[
    0x2e, 0xa0, 0x5f, 0x4a, 0x3c, 0x00, 0x75, 0x05, 0xbb, 0xdb, 0x3d, 0xeb, 0x1e,
];
const SELECTOR_RUNTIME_ADDRESS: usize = 0x4a5f;
const MESSAGE_POINTER_FILE_OFFSETS: [usize; 4] = [0x28bf, 0x28c8, 0x28d1, 0x28da];
pub(crate) const MESSAGE_FILE_OFFSETS: [usize; 4] = [0x3cdb, 0x3c9c, 0x3d14, 0x3d49];
pub(crate) const MESSAGE_RUNTIME_ADDRESSES: [usize; 4] = [0x3ddb, 0x3d9c, 0x3e14, 0x3e49];
pub(crate) const MESSAGE_POOL_FILE_OFFSET: usize = 0x3c9c;
pub(crate) const MESSAGE_POOL_END_FILE_OFFSET: usize = 0x3d8e;
pub(crate) const DECIMAL_GLYPH_INDICES: [u8; 10] =
    [0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xab, 0xac, 0xad];
pub(crate) const MONTH_TEMPLATE_FILE_OFFSET: usize = 0x44b7;
pub(crate) const DAY_TEMPLATE_FILE_OFFSET: usize = 0x44bc;
pub(crate) const DATE_TEMPLATE_BYTE_SIZE: usize = 5;
pub(crate) const DATE_DIGIT_CELL_COUNT: usize = 2;
const MONTH_TEMPLATE_REFERENCE_FILE_OFFSET: usize = 0x28ec;
const DAY_TEMPLATE_REFERENCE_FILE_OFFSET: usize = 0x2936;
const GLYPH_RENDERER_CALL_FILE_OFFSETS: [usize; 3] = [0x2916, 0x2964, 0x29b3];
const GLYPH_RENDERER_RUNTIME_ADDRESS: usize = 0x2fda;
const LINE_BREAK: u8 = 0xfe;
const TERMINATOR: u8 = 0xff;
const DIGIT_ZERO_GLYPH_INDEX: u8 = 0xa4;
const DIGIT_ONE_GLYPH_INDEX: u8 = 0xa5;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCurryResultCatalog<'a> {
    pub consumer: EndingCurryResultConsumerEvidence,
    pub month: EndingDynamicDateTemplate<'a>,
    pub day: EndingDynamicDateTemplate<'a>,
    pub messages: Vec<EndingCurryResultMessage<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCurryResultConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub selector_runtime_address: usize,
    pub message_runtime_addresses: [usize; 4],
    pub month_template_reference_file_offset: usize,
    pub day_template_reference_file_offset: usize,
    pub glyph_renderer_call_file_offsets: [usize; 3],
    pub glyph_renderer_runtime_address: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EndingDynamicDateUnit {
    Month,
    Day,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingDynamicDateTemplate<'a> {
    pub unit: EndingDynamicDateUnit,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub bytes: &'a [u8],
    pub initial_digit_indices: [u8; DATE_DIGIT_CELL_COUNT],
    pub suffix_glyphs: Vec<IndexedGlyph>,
    pub suffix_text: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCurryResultMessage<'a> {
    pub result_class: usize,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub bytes: &'a [u8],
    pub lines: Vec<EndingCurryResultLine>,
    pub line_break_counts: Vec<usize>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCurryResultLine {
    pub glyphs: Vec<IndexedGlyph>,
    pub text: String,
}

pub(crate) fn parse_ending_curry_result(bytes: &[u8]) -> Result<EndingCurryResultCatalog<'_>> {
    let consumer = verify_consumer(bytes)?;
    let month = parse_date_template(
        bytes,
        EndingDynamicDateUnit::Month,
        MONTH_TEMPLATE_FILE_OFFSET,
        [DIGIT_ONE_GLYPH_INDEX, DIGIT_ZERO_GLYPH_INDEX],
    )?;
    let day = parse_date_template(
        bytes,
        EndingDynamicDateUnit::Day,
        DAY_TEMPLATE_FILE_OFFSET,
        [DIGIT_ZERO_GLYPH_INDEX, DIGIT_ZERO_GLYPH_INDEX],
    )?;
    let messages = MESSAGE_FILE_OFFSETS
        .into_iter()
        .zip(MESSAGE_RUNTIME_ADDRESSES)
        .enumerate()
        .map(|(result_class, (file_offset, runtime_address))| {
            parse_message(bytes, result_class, file_offset, runtime_address)
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        messages
            .iter()
            .flat_map(|message| &message.lines)
            .all(|line| !line.text.is_empty()),
        "ENDING curry-result catalog contains a blank translatable line"
    );

    Ok(EndingCurryResultCatalog {
        consumer,
        month,
        day,
        messages,
    })
}

fn parse_date_template<'a>(
    bytes: &'a [u8],
    unit: EndingDynamicDateUnit,
    file_offset: usize,
    expected_digits: [u8; DATE_DIGIT_CELL_COUNT],
) -> Result<EndingDynamicDateTemplate<'a>> {
    let template = bytes
        .get(file_offset..file_offset + DATE_TEMPLATE_BYTE_SIZE)
        .with_context(|| format!("ENDING {unit:?} template is truncated"))?;
    ensure!(
        template[..DATE_DIGIT_CELL_COUNT] == expected_digits,
        "ENDING {unit:?} template digit cells changed"
    );
    ensure!(
        template[DATE_TEMPLATE_BYTE_SIZE - 1] == TERMINATOR,
        "ENDING {unit:?} template has no fixed terminator"
    );
    let suffix_glyphs = template[DATE_DIGIT_CELL_COUNT..DATE_TEMPLATE_BYTE_SIZE - 1]
        .iter()
        .copied()
        .map(|index| IndexedGlyph {
            index,
            text: indexed_glyph_text(index),
        })
        .collect::<Vec<_>>();
    ensure!(
        suffix_glyphs.iter().all(|glyph| glyph.text.is_some()),
        "ENDING {unit:?} template contains an unresolved suffix glyph"
    );
    let suffix_text = suffix_glyphs
        .iter()
        .map(|glyph| {
            glyph
                .text
                .expect("date suffix glyph resolution was checked")
        })
        .collect();
    Ok(EndingDynamicDateTemplate {
        unit,
        file_offset,
        runtime_address: file_offset + COM_ORIGIN,
        bytes: template,
        initial_digit_indices: expected_digits,
        suffix_glyphs,
        suffix_text,
    })
}

fn parse_message(
    bytes: &[u8],
    result_class: usize,
    file_offset: usize,
    runtime_address: usize,
) -> Result<EndingCurryResultMessage<'_>> {
    let mut cursor = file_offset;
    let mut line_glyphs = Vec::new();
    let mut lines = Vec::new();
    let mut line_break_counts = Vec::new();
    loop {
        let byte = *bytes.get(cursor).with_context(|| {
            format!("ENDING curry-result class {result_class} has no terminator")
        })?;
        cursor += 1;
        match byte {
            TERMINATOR => {
                push_line(result_class, &mut lines, &mut line_glyphs)?;
                break;
            }
            LINE_BREAK if line_glyphs.is_empty() => {
                let count = line_break_counts.last_mut().with_context(|| {
                    format!("ENDING curry-result class {result_class} starts with a line break")
                })?;
                *count += 1;
            }
            LINE_BREAK => {
                push_line(result_class, &mut lines, &mut line_glyphs)?;
                line_break_counts.push(1);
            }
            index => {
                let text = indexed_glyph_text(index).with_context(|| {
                    format!(
                        "ENDING curry-result class {result_class} uses unresolved glyph {index:02X}"
                    )
                })?;
                line_glyphs.push(IndexedGlyph {
                    index,
                    text: Some(text),
                });
            }
        }
    }
    ensure!(
        line_break_counts.len() + 1 == lines.len(),
        "ENDING curry-result class {result_class} line-break structure is inconsistent"
    );
    let message_bytes = &bytes[file_offset..cursor];
    Ok(EndingCurryResultMessage {
        result_class,
        file_offset,
        runtime_address,
        bytes: message_bytes,
        lines,
        line_break_counts,
    })
}

fn push_line(
    result_class: usize,
    lines: &mut Vec<EndingCurryResultLine>,
    glyphs: &mut Vec<IndexedGlyph>,
) -> Result<()> {
    ensure!(
        !glyphs.is_empty(),
        "ENDING curry-result class {result_class} ends with a blank line"
    );
    let glyphs = std::mem::take(glyphs);
    let text = glyphs
        .iter()
        .map(|glyph| glyph.text.expect("message glyph resolution was checked"))
        .collect();
    lines.push(EndingCurryResultLine { glyphs, text });
    Ok(())
}

fn verify_consumer(bytes: &[u8]) -> Result<EndingCurryResultConsumerEvidence> {
    ensure!(
        bytes.get(CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_SIGNATURE.len())
            == Some(CONSUMER_SIGNATURE),
        "ENDING curry-result consumer signature does not match"
    );
    for ((reference_offset, runtime_address), file_offset) in MESSAGE_POINTER_FILE_OFFSETS
        .into_iter()
        .zip(MESSAGE_RUNTIME_ADDRESSES)
        .zip(MESSAGE_FILE_OFFSETS)
    {
        ensure!(
            runtime_address == file_offset + COM_ORIGIN,
            "ENDING curry-result message address no longer matches its COM file offset"
        );
        let immediate = u16::try_from(runtime_address)
            .context("ENDING curry-result message address exceeds 16 bits")?
            .to_le_bytes();
        ensure!(
            bytes.get(reference_offset..reference_offset + 3)
                == Some(&[0xbb, immediate[0], immediate[1]]),
            "ENDING curry-result message pointer differs at {reference_offset:#x}"
        );
    }
    verify_template_reference(
        bytes,
        MONTH_TEMPLATE_REFERENCE_FILE_OFFSET,
        MONTH_TEMPLATE_FILE_OFFSET + COM_ORIGIN,
        "month",
    )?;
    verify_template_reference(
        bytes,
        DAY_TEMPLATE_REFERENCE_FILE_OFFSET,
        DAY_TEMPLATE_FILE_OFFSET + COM_ORIGIN,
        "day",
    )?;
    for call_offset in GLYPH_RENDERER_CALL_FILE_OFFSETS {
        verify_typed_call(bytes, call_offset)?;
    }
    let body = bytes
        .get(CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_BODY_LEN)
        .context("ENDING curry-result consumer body is truncated")?;
    let decoded_instruction_count = decode_exact_block(body)?;
    Ok(EndingCurryResultConsumerEvidence {
        file_offset: CONSUMER_FILE_OFFSET,
        runtime_address: CONSUMER_RUNTIME_ADDRESS,
        decoded_instruction_count,
        selector_runtime_address: SELECTOR_RUNTIME_ADDRESS,
        message_runtime_addresses: MESSAGE_RUNTIME_ADDRESSES,
        month_template_reference_file_offset: MONTH_TEMPLATE_REFERENCE_FILE_OFFSET,
        day_template_reference_file_offset: DAY_TEMPLATE_REFERENCE_FILE_OFFSET,
        glyph_renderer_call_file_offsets: GLYPH_RENDERER_CALL_FILE_OFFSETS,
        glyph_renderer_runtime_address: GLYPH_RENDERER_RUNTIME_ADDRESS,
    })
}

fn verify_template_reference(
    bytes: &[u8],
    file_offset: usize,
    runtime_address: usize,
    label: &str,
) -> Result<()> {
    let immediate = u16::try_from(runtime_address)
        .with_context(|| format!("ENDING {label} template address exceeds 16 bits"))?
        .to_le_bytes();
    ensure!(
        bytes.get(file_offset..file_offset + 3) == Some(&[0xbb, immediate[0], immediate[1]]),
        "ENDING {label} template reference differs at {file_offset:#x}"
    );
    Ok(())
}

fn verify_typed_call(bytes: &[u8], file_offset: usize) -> Result<()> {
    let call_bytes = bytes
        .get(file_offset..file_offset + 3)
        .context("ENDING curry-result glyph call is truncated")?;
    let call = decode_bytes(call_bytes).context("decode ENDING curry-result glyph call")?;
    ensure!(
        call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
        "ENDING curry-result glyph renderer is not a typed V30 near CALL"
    );
    let displacement = i16::from_le_bytes([call_bytes[1], call_bytes[2]]);
    let call_runtime_address = file_offset + COM_ORIGIN;
    let target = (call_runtime_address + call.byte_len).wrapping_add_signed(displacement.into());
    ensure!(
        target == GLYPH_RENDERER_RUNTIME_ADDRESS,
        "ENDING curry-result glyph call targets {target:#x} instead of {GLYPH_RENDERER_RUNTIME_ADDRESS:#x}"
    );
    Ok(())
}

fn decode_exact_block(bytes: &[u8]) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..]).with_context(|| {
            format!("decode V30 instruction at ENDING curry-result byte {cursor:#x}")
        })?;
        cursor += decoded.byte_len;
        instruction_count += 1;
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed ENDING curry-result consumer boundary"
    );
    Ok(instruction_count)
}

#[cfg(test)]
#[path = "ending_curry_result_tests.rs"]
mod ending_curry_result_tests;
