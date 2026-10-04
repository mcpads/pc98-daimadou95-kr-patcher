use std::ops::Range;

use anyhow::{Context, Result, bail, ensure};
use v30::{Instruction, decode_bytes};

use super::indexed_text;

pub(crate) const MADDAT_ENTRY_ID: usize = 142;

const COM_ORIGIN: usize = 0x100;
const LOAD_REFERENCE_FILE_OFFSET: usize = 0x2890;
const CALL_SITE_FILE_OFFSET: usize = 0x2b2c;
const INTERPRETER_FILE_OFFSET: usize = 0x2d5e;
const INTERPRETER_BODY_LEN: usize = 0x102;
const INTERPRETER_SIGNATURE: &[u8] = &[
    0x1e, 0x2e, 0x8e, 0x1e, 0xa2, 0x38, 0x2e, 0x8b, 0x36, 0xa4, 0x38, 0xac, 0x3c, 0x04, 0x73, 0x15,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum OpeningScriptToken {
    Mode(u8),
    Sprite { glyph_index: u8, position: u16 },
    Glyph { index: u8, text: &'static str },
    LineBreak,
    Scroll,
    Yield,
    End,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OpeningTextScript {
    pub tokens: Vec<OpeningScriptToken>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OpeningDisplayRun {
    pub glyph_range: Range<usize>,
    pub line_break_offsets: Vec<usize>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OpeningConsumerEvidence {
    pub load_reference_file_offset: usize,
    pub call_site_file_offset: usize,
    pub interpreter_file_offset: usize,
    pub interpreter_runtime_address: usize,
    pub decoded_instruction_count: usize,
}

pub(crate) fn parse_opening_text_script(bytes: &[u8]) -> Result<OpeningTextScript> {
    let mut cursor = 0;
    let mut tokens = Vec::new();
    loop {
        let command = *bytes
            .get(cursor)
            .context("opening text script has no end command")?;
        cursor += 1;
        let token = match command {
            mode @ 0x00..=0x03 => OpeningScriptToken::Mode(mode),
            0x04 => {
                let glyph_index = read_byte(bytes, &mut cursor, "sprite glyph index")?;
                let position = read_u16(bytes, &mut cursor, "sprite position")?;
                OpeningScriptToken::Sprite {
                    glyph_index,
                    position,
                }
            }
            0x0a => {
                let index = read_byte(bytes, &mut cursor, "text glyph index")?;
                let text = indexed_text::indexed_glyph_text(index).with_context(|| {
                    format!("opening text script uses unresolved font glyph {index:02X}")
                })?;
                OpeningScriptToken::Glyph { index, text }
            }
            0x0b => OpeningScriptToken::LineBreak,
            0x14 => OpeningScriptToken::Scroll,
            0x1e => OpeningScriptToken::Yield,
            0x28 => OpeningScriptToken::End,
            _ => bail!(
                "unsupported opening text command {command:02X} at byte {:#x}",
                cursor - 1
            ),
        };
        let is_end = matches!(token, OpeningScriptToken::End);
        tokens.push(token);
        if is_end {
            ensure!(
                cursor == bytes.len(),
                "opening text script has bytes after its end command"
            );
            break;
        }
    }
    let script = OpeningTextScript { tokens };
    ensure!(
        reconstruct_opening_text_script(&script.tokens)? == bytes,
        "opening text script does not reconstruct exactly"
    );
    Ok(script)
}

pub(crate) fn reconstruct_opening_text_script(tokens: &[OpeningScriptToken]) -> Result<Vec<u8>> {
    ensure!(
        matches!(tokens.last(), Some(OpeningScriptToken::End)),
        "opening text script must end with its end command"
    );
    ensure!(
        !tokens[..tokens.len() - 1]
            .iter()
            .any(|token| matches!(token, OpeningScriptToken::End)),
        "opening text script contains an early end command"
    );

    let mut bytes = Vec::new();
    for token in tokens {
        match token {
            OpeningScriptToken::Mode(mode) => {
                ensure!(*mode <= 3, "opening text mode must be 0 through 3");
                bytes.push(*mode);
            }
            OpeningScriptToken::Sprite {
                glyph_index,
                position,
            } => {
                bytes.extend_from_slice(&[0x04, *glyph_index]);
                bytes.extend_from_slice(&position.to_le_bytes());
            }
            OpeningScriptToken::Glyph { index, text } => {
                ensure!(
                    indexed_text::indexed_glyph_text(*index) == Some(*text),
                    "opening glyph {index:02X} does not match the indexed font codebook"
                );
                bytes.extend_from_slice(&[0x0a, *index]);
            }
            OpeningScriptToken::LineBreak => bytes.push(0x0b),
            OpeningScriptToken::Scroll => bytes.push(0x14),
            OpeningScriptToken::Yield => bytes.push(0x1e),
            OpeningScriptToken::End => bytes.push(0x28),
        }
    }
    Ok(bytes)
}

pub(crate) fn replace_opening_glyph_indices(
    source_bytes: &[u8],
    replacement_indices: &[u8],
) -> Result<Vec<u8>> {
    let script = parse_opening_text_script(source_bytes)?;
    let source_glyph_count = script
        .tokens
        .iter()
        .filter(|token| matches!(token, OpeningScriptToken::Glyph { .. }))
        .count();
    ensure!(
        replacement_indices.len() == source_glyph_count,
        "opening translation supplies {} glyph indices for {source_glyph_count} source glyph commands",
        replacement_indices.len()
    );

    let mut replacements = replacement_indices.iter().copied();
    let mut output = Vec::with_capacity(source_bytes.len());
    for token in &script.tokens {
        match token {
            OpeningScriptToken::Mode(mode) => output.push(*mode),
            OpeningScriptToken::Sprite {
                glyph_index,
                position,
            } => {
                output.extend_from_slice(&[0x04, *glyph_index]);
                output.extend_from_slice(&position.to_le_bytes());
            }
            OpeningScriptToken::Glyph { .. } => output.extend_from_slice(&[
                0x0a,
                replacements
                    .next()
                    .expect("replacement count was checked before encoding"),
            ]),
            OpeningScriptToken::LineBreak => output.push(0x0b),
            OpeningScriptToken::Scroll => output.push(0x14),
            OpeningScriptToken::Yield => output.push(0x1e),
            OpeningScriptToken::End => output.push(0x28),
        }
    }
    ensure!(
        replacements.next().is_none(),
        "opening translation left an unused replacement glyph index"
    );
    ensure!(
        output.len() == source_bytes.len(),
        "opening translation changed the protected command-stream length"
    );
    Ok(output)
}

pub(crate) fn opening_display_runs(
    tokens: &[OpeningScriptToken],
) -> Result<Vec<OpeningDisplayRun>> {
    let mut runs = Vec::new();
    let mut active_mode = None;
    let mut glyph_count = 0_usize;
    let mut run_start = 0_usize;
    let mut run_line_breaks = Vec::new();
    let mut saw_glyph = false;
    let mut pending_line_break = false;
    let mut pending_scroll_count = 0_usize;
    let mut pending_run_boundary = false;

    for token in tokens {
        match token {
            OpeningScriptToken::Mode(mode) => {
                if saw_glyph && active_mode.is_some_and(|active| active != *mode) {
                    pending_run_boundary = true;
                }
                active_mode = Some(*mode);
            }
            OpeningScriptToken::Glyph { .. } => {
                if pending_run_boundary {
                    push_display_run(&mut runs, run_start, glyph_count, &mut run_line_breaks)?;
                    run_start = glyph_count;
                } else if pending_line_break {
                    run_line_breaks.push(glyph_count - run_start);
                }
                glyph_count += 1;
                saw_glyph = true;
                pending_line_break = false;
                pending_scroll_count = 0;
                pending_run_boundary = false;
            }
            OpeningScriptToken::LineBreak => {
                if saw_glyph {
                    pending_line_break = true;
                }
            }
            OpeningScriptToken::Scroll => {
                if saw_glyph {
                    pending_scroll_count += 1;
                    if pending_scroll_count >= 2 {
                        pending_run_boundary = true;
                    }
                }
            }
            OpeningScriptToken::Sprite { .. }
            | OpeningScriptToken::Yield
            | OpeningScriptToken::End => {}
        }
    }

    push_display_run(&mut runs, run_start, glyph_count, &mut run_line_breaks)?;
    ensure!(!runs.is_empty(), "opening script has no display runs");
    Ok(runs)
}

fn push_display_run(
    runs: &mut Vec<OpeningDisplayRun>,
    start: usize,
    end: usize,
    line_break_offsets: &mut Vec<usize>,
) -> Result<()> {
    ensure!(end > start, "opening script contains an empty display run");
    ensure!(
        line_break_offsets
            .iter()
            .all(|offset| *offset > 0 && *offset < end - start),
        "opening display run contains a boundary line break"
    );
    runs.push(OpeningDisplayRun {
        glyph_range: start..end,
        line_break_offsets: std::mem::take(line_break_offsets),
    });
    Ok(())
}

pub(crate) fn verify_opening_text_consumer(bytes: &[u8]) -> Result<OpeningConsumerEvidence> {
    let entry_id = u16::try_from(MADDAT_ENTRY_ID)
        .context("opening text MADDAT entry ID does not fit in a V30 immediate")?
        .to_le_bytes();
    ensure!(
        bytes.get(LOAD_REFERENCE_FILE_OFFSET..LOAD_REFERENCE_FILE_OFFSET + 3)
            == Some(&[0xb8, entry_id[0], entry_id[1]]),
        "OPENING text-script MADDAT load reference does not match"
    );
    ensure!(
        bytes.get(INTERPRETER_FILE_OFFSET..INTERPRETER_FILE_OFFSET + INTERPRETER_SIGNATURE.len())
            == Some(INTERPRETER_SIGNATURE),
        "OPENING text-script interpreter signature does not match"
    );

    let body = bytes
        .get(INTERPRETER_FILE_OFFSET..INTERPRETER_FILE_OFFSET + INTERPRETER_BODY_LEN)
        .context("OPENING text-script interpreter body is truncated")?;
    let decoded_instruction_count = decode_complete_block(body)?;

    let call = decode_bytes(
        bytes
            .get(CALL_SITE_FILE_OFFSET..CALL_SITE_FILE_OFFSET + 3)
            .context("OPENING text-script interpreter call is truncated")?,
    )
    .context("decode OPENING text-script interpreter call")?;
    ensure!(
        call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
        "OPENING text-script consumer is not a typed V30 near CALL"
    );

    Ok(OpeningConsumerEvidence {
        load_reference_file_offset: LOAD_REFERENCE_FILE_OFFSET,
        call_site_file_offset: CALL_SITE_FILE_OFFSET,
        interpreter_file_offset: INTERPRETER_FILE_OFFSET,
        interpreter_runtime_address: INTERPRETER_FILE_OFFSET + COM_ORIGIN,
        decoded_instruction_count,
    })
}

fn decode_complete_block(bytes: &[u8]) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    let mut last_instruction = None;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..]).with_context(|| {
            format!("decode V30 instruction at OPENING interpreter byte {cursor:#x}")
        })?;
        cursor += decoded.byte_len;
        instruction_count += 1;
        last_instruction = Some(decoded.instruction);
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed OPENING interpreter boundary"
    );
    ensure!(
        matches!(last_instruction, Some(Instruction::Ret { pop: 0 })),
        "OPENING text-script interpreter does not end in a typed V30 RET"
    );
    Ok(instruction_count)
}

fn read_byte(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<u8> {
    let byte = *bytes
        .get(*cursor)
        .with_context(|| format!("opening text script has truncated {label}"))?;
    *cursor += 1;
    Ok(byte)
}

fn read_u16(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<u16> {
    let low = read_byte(bytes, cursor, label)?;
    let high = read_byte(bytes, cursor, label)?;
    Ok(u16::from_le_bytes([low, high]))
}

#[cfg(test)]
#[path = "opening_script_tests.rs"]
mod opening_script_tests;
