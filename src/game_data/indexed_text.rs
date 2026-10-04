use anyhow::{Context, Result, ensure};
use v30::{Instruction, decode_bytes};

pub(crate) const FONT_ENTRY_ID: usize = 6;
pub(crate) const FONT_GLYPH_COUNT: usize = 256;
pub(crate) const FONT_GLYPH_SIZE: usize = 32;
pub(crate) const FONT_DECODED_SIZE: usize = FONT_GLYPH_COUNT * FONT_GLYPH_SIZE;

const COM_ORIGIN: usize = 0x100;
const SELECT_INTERPRETER_FILE_OFFSET: usize = 0x2082;
const SELECT_INTERPRETER_RUNTIME_ADDRESS: usize = SELECT_INTERPRETER_FILE_OFFSET + COM_ORIGIN;
const SELECT_INTERPRETER_BODY_LEN: usize = 0x231;
const SELECT_POINTER_TABLE_FILE_OFFSET: usize = 0x42a8;
const SELECT_POINTER_TABLE_RUNTIME_ADDRESS: usize = SELECT_POINTER_TABLE_FILE_OFFSET + COM_ORIGIN;
const SELECT_STAGE_COUNT: usize = 9;
const SELECT_SCRIPTS_PER_STAGE: usize = 6;
const SELECT_SCRIPT_COUNT: usize = SELECT_STAGE_COUNT * SELECT_SCRIPTS_PER_STAGE;
const SELECT_CALL_SITE_FILE_OFFSETS: [usize; 4] = [0x3ff9, 0x403f, 0x405b, 0x406a];
const SELECT_INTERPRETER_SIGNATURE: &[u8] = &[
    0x55, 0x2e, 0x8b, 0x1e, 0xa6, 0x43, 0x2e, 0x8a, 0x07, 0xbb, 0x44, 0x02, 0xfe, 0xc8, 0x74, 0x03,
    0xbb, 0xfe, 0x01,
];
const MAD_RENDERER_FILE_OFFSET: usize = 0x4db4;
const MAD_RENDERER_RUNTIME_ADDRESS: usize = MAD_RENDERER_FILE_OFFSET + COM_ORIGIN;
const MAD_RENDERER_BODY_LEN: usize = 0x57;
const MAD_RENDERER_SIGNATURE: &[u8] = &[
    0x2e, 0x8e, 0x1e, 0x24, 0xc0, 0x2e, 0x89, 0x3e, 0x07, 0xc1, 0x33, 0xc0, 0x2e, 0x8a, 0x04, 0x2e,
    0x89, 0x36, 0x09, 0xc1, 0x3c, 0xff,
];
const ENDING_INTERLUDE_CONSUMER_FILE_OFFSET: usize = 0x2610;
const ENDING_INTERLUDE_CONSUMER_RUNTIME_ADDRESS: usize =
    ENDING_INTERLUDE_CONSUMER_FILE_OFFSET + COM_ORIGIN;
const ENDING_INTERLUDE_CONSUMER_BODY_LEN: usize = 0x2a;
const ENDING_INTERLUDE_CONSUMER_SIGNATURE: &[u8] = &[
    0xbb, 0x4d, 0x4a, 0xb9, 0x20, 0x00, 0xba, 0x05, 0x00, 0x2e, 0x8e, 0x1e, 0x8d, 0x3d,
];
const ENDING_INTERLUDE_FONT_LOAD_FILE_OFFSET: usize = 0x24bc;
const ENDING_INTERLUDE_BACKGROUND_LOAD_FILE_OFFSET: usize = 0x259a;
const ENDING_INTERLUDE_BACKGROUND_ENTRY_ID: usize = 154;
const ENDING_INTERLUDE_GLYPH_CALL_FILE_OFFSET: usize = 0x262e;
const ENDING_INTERLUDE_TEXT_FILE_OFFSET: usize = 0x494d;
const ENDING_INTERLUDE_TEXT_RUNTIME_ADDRESS: usize = ENDING_INTERLUDE_TEXT_FILE_OFFSET + COM_ORIGIN;
const ENDING_PASSWORD_CONSUMER_FILE_OFFSET: usize = 0x3b5a;
const ENDING_PASSWORD_CONSUMER_RUNTIME_ADDRESS: usize =
    ENDING_PASSWORD_CONSUMER_FILE_OFFSET + COM_ORIGIN;
const ENDING_PASSWORD_CONSUMER_BODY_LEN: usize = 0x131;
const ENDING_PASSWORD_CONSUMER_SIGNATURE: &[u8] = &[
    0xb0, 0x08, 0xe6, 0xa8, 0xb0, 0x0f, 0xe6, 0xaa, 0xb0, 0x0f, 0xe6, 0xac, 0xb0, 0x0f, 0xe6, 0xae,
];
const ENDING_PASSWORD_CALL_SITE_FILE_OFFSET: usize = 0x3632;
const ENDING_PASSWORD_FONT_LOAD_FILE_OFFSET: usize = 0x3be1;
const ENDING_PASSWORD_GLYPH_CALL_FILE_OFFSET: usize = 0x3c7e;
const ENDING_PASSWORD_POINTER_TABLE_FILE_OFFSET: usize = 0x4916;
const ENDING_PASSWORD_POINTER_TABLE_RUNTIME_ADDRESS: usize =
    ENDING_PASSWORD_POINTER_TABLE_FILE_OFFSET + COM_ORIGIN;
const ENDING_PASSWORD_SELECTOR_FILE_OFFSET: usize = 0x4a1c;
const ENDING_PASSWORD_SELECTOR_RUNTIME_ADDRESS: usize =
    ENDING_PASSWORD_SELECTOR_FILE_OFFSET + COM_ORIGIN;
const ENDING_PASSWORD_COUNT: usize = 11;
const ENDING_PASSWORD_POOL_FILE_START: usize = 0x4961;
const ENDING_PASSWORD_POOL_FILE_END: usize = ENDING_PASSWORD_SELECTOR_FILE_OFFSET;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct IndexedFontEvidence {
    pub entry_id: usize,
    pub decoded_size: usize,
    pub glyph_count: usize,
    pub glyph_size: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MadIndexedRendererEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct IndexedLineStream<'a> {
    pub byte_len: usize,
    pub bytes: &'a [u8],
    pub lines: Vec<IndexedLine>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct IndexedLine {
    pub byte_offset: usize,
    pub glyphs: Vec<IndexedGlyph>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IndexedGlyph {
    pub index: u8,
    pub text: Option<&'static str>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SelectDialogueCatalog<'a> {
    pub interpreter: SelectInterpreterEvidence,
    pub pointer_table_file_offset: usize,
    pub pointer_table_runtime_address: usize,
    pub stages: Vec<SelectDialogueStage<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SelectInterpreterEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub call_site_file_offsets: [usize; SELECT_CALL_SITE_FILE_OFFSETS.len()],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingPasswordCatalog<'a> {
    pub consumer: EndingPasswordConsumerEvidence,
    pub pointer_table_file_offset: usize,
    pub pointer_table_runtime_address: usize,
    pub selector_file_offset: usize,
    pub selector_runtime_address: usize,
    pub records: Vec<EndingPasswordRecord<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingInterludeCaption<'a> {
    pub consumer: EndingInterludeConsumerEvidence,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub bytes: &'a [u8],
    pub glyphs: Vec<IndexedGlyph>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingInterludeConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub font_load_file_offset: usize,
    pub background_entry_id: usize,
    pub background_load_file_offset: usize,
    pub glyph_renderer_call_file_offset: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingPasswordConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub call_site_file_offset: usize,
    pub font_load_file_offset: usize,
    pub glyph_renderer_call_file_offset: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingPasswordRecord<'a> {
    pub selection_index: usize,
    pub pointer_entry_file_offset: usize,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub bytes: &'a [u8],
    pub glyphs: Vec<IndexedGlyph>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SelectDialogueStage<'a> {
    pub stage_index: usize,
    pub scripts: Vec<SelectDialogueScript<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SelectDialogueScript<'a> {
    pub phase_index: usize,
    pub pointer_entry_file_offset: usize,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub bytes: &'a [u8],
    pub initial_portrait: [u8; 2],
    pub tokens: Vec<IndexedTextToken>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IndexedTextToken {
    Glyph { index: u8, text: &'static str },
    Voice(u8),
    Portrait([u8; 2]),
    LineBreak([u8; 2]),
    Terminator,
}

pub(crate) fn verify_indexed_font(bytes: &[u8]) -> Result<IndexedFontEvidence> {
    ensure!(
        bytes.len() == FONT_DECODED_SIZE,
        "MADDAT entry {FONT_ENTRY_ID} decoded to {} bytes instead of the verified {FONT_DECODED_SIZE}",
        bytes.len()
    );
    Ok(IndexedFontEvidence {
        entry_id: FONT_ENTRY_ID,
        decoded_size: bytes.len(),
        glyph_count: FONT_GLYPH_COUNT,
        glyph_size: FONT_GLYPH_SIZE,
    })
}

pub(crate) fn verify_mad_indexed_renderer(bytes: &[u8]) -> Result<MadIndexedRendererEvidence> {
    ensure!(
        bytes
            .get(MAD_RENDERER_FILE_OFFSET..MAD_RENDERER_FILE_OFFSET + MAD_RENDERER_SIGNATURE.len())
            == Some(MAD_RENDERER_SIGNATURE),
        "MAD indexed-text renderer signature does not match"
    );
    let body = bytes
        .get(MAD_RENDERER_FILE_OFFSET..MAD_RENDERER_FILE_OFFSET + MAD_RENDERER_BODY_LEN)
        .context("MAD indexed-text renderer body is truncated")?;
    let decoded_instruction_count = decode_complete_block(body, "MAD indexed-text renderer")?;
    Ok(MadIndexedRendererEvidence {
        file_offset: MAD_RENDERER_FILE_OFFSET,
        runtime_address: MAD_RENDERER_RUNTIME_ADDRESS,
        decoded_instruction_count,
    })
}

pub(crate) fn parse_indexed_lines(
    bytes: &[u8],
    line_count: usize,
) -> Result<IndexedLineStream<'_>> {
    ensure!(
        line_count > 0,
        "indexed text must contain at least one line"
    );
    let mut cursor = 0;
    let mut lines = Vec::with_capacity(line_count);
    for line_index in 0..line_count {
        let byte_offset = cursor;
        let mut glyphs = Vec::new();
        loop {
            let index = *bytes.get(cursor).with_context(|| {
                format!("indexed text line {} has no FF terminator", line_index + 1)
            })?;
            cursor += 1;
            if index == 0xff {
                break;
            }
            glyphs.push(IndexedGlyph {
                index,
                text: indexed_glyph_text(index),
            });
        }
        lines.push(IndexedLine {
            byte_offset,
            glyphs,
        });
    }
    Ok(IndexedLineStream {
        byte_len: cursor,
        bytes: &bytes[..cursor],
        lines,
    })
}

pub(crate) fn reconstruct_indexed_lines(lines: &[IndexedLine]) -> Result<Vec<u8>> {
    ensure!(
        !lines.is_empty(),
        "indexed text must contain at least one line"
    );
    let mut bytes = Vec::new();
    let mut expected_offset = 0;
    for line in lines {
        ensure!(
            line.byte_offset == expected_offset,
            "indexed text line offsets are not contiguous"
        );
        for glyph in &line.glyphs {
            ensure!(
                indexed_glyph_text(glyph.index) == glyph.text,
                "indexed glyph {:02X} does not match the font codebook",
                glyph.index
            );
            bytes.push(glyph.index);
        }
        bytes.push(0xff);
        expected_offset = bytes.len();
    }
    Ok(bytes)
}

pub(crate) fn parse_select_dialogue(bytes: &[u8]) -> Result<SelectDialogueCatalog<'_>> {
    let interpreter = verify_select_interpreter(bytes)?;
    let table_end = SELECT_POINTER_TABLE_FILE_OFFSET + SELECT_SCRIPT_COUNT * 2;
    ensure!(
        table_end <= bytes.len(),
        "SELECT dialogue pointer table is truncated"
    );

    let mut stages = Vec::with_capacity(SELECT_STAGE_COUNT);
    let mut previous_script_end = None;
    for stage_index in 0..SELECT_STAGE_COUNT {
        let mut scripts = Vec::with_capacity(SELECT_SCRIPTS_PER_STAGE);
        for phase_index in 0..SELECT_SCRIPTS_PER_STAGE {
            let pointer_index = stage_index * SELECT_SCRIPTS_PER_STAGE + phase_index;
            let pointer_entry_file_offset = SELECT_POINTER_TABLE_FILE_OFFSET + pointer_index * 2;
            let runtime_address = read_u16(bytes, pointer_entry_file_offset)? as usize;
            let file_offset = runtime_address
                .checked_sub(COM_ORIGIN)
                .context("SELECT dialogue pointer lies below the COM origin")?;
            if pointer_index == 0 {
                ensure!(
                    file_offset == table_end,
                    "SELECT dialogue does not begin immediately after its pointer table"
                );
            }
            if let Some(previous_end) = previous_script_end {
                ensure!(
                    file_offset == previous_end,
                    "SELECT dialogue scripts are not contiguous before stage {}, phase {}",
                    stage_index + 1,
                    phase_index + 1
                );
            }

            let script = parse_select_script(
                bytes,
                stage_index,
                phase_index,
                pointer_entry_file_offset,
                runtime_address,
                file_offset,
            )?;
            previous_script_end = Some(file_offset + script.bytes.len());
            scripts.push(script);
        }
        stages.push(SelectDialogueStage {
            stage_index,
            scripts,
        });
    }

    Ok(SelectDialogueCatalog {
        interpreter,
        pointer_table_file_offset: SELECT_POINTER_TABLE_FILE_OFFSET,
        pointer_table_runtime_address: SELECT_POINTER_TABLE_RUNTIME_ADDRESS,
        stages,
    })
}

pub(crate) fn reconstruct_select_script(script: &SelectDialogueScript<'_>) -> Result<Vec<u8>> {
    ensure!(
        matches!(script.tokens.last(), Some(IndexedTextToken::Terminator)),
        "SELECT dialogue token stream must end with a terminator"
    );
    ensure!(
        !script.tokens[..script.tokens.len() - 1]
            .iter()
            .any(|token| matches!(token, IndexedTextToken::Terminator)),
        "SELECT dialogue token stream contains an early terminator"
    );

    let mut bytes = Vec::with_capacity(script.bytes.len());
    bytes.extend_from_slice(&script.initial_portrait);
    for token in &script.tokens {
        match token {
            IndexedTextToken::Glyph { index, text } => {
                ensure!(
                    indexed_glyph_text(*index) == Some(*text),
                    "SELECT dialogue glyph {index:02X} does not match the indexed font codebook"
                );
                bytes.push(*index);
            }
            IndexedTextToken::Voice(parameter) => bytes.extend_from_slice(&[0xfa, *parameter]),
            IndexedTextToken::Portrait(parameters) => {
                bytes.push(0xfd);
                bytes.extend_from_slice(parameters);
            }
            IndexedTextToken::LineBreak(parameters) => {
                bytes.push(0xfe);
                bytes.extend_from_slice(parameters);
            }
            IndexedTextToken::Terminator => bytes.push(0xff),
        }
    }
    Ok(bytes)
}

pub(crate) fn parse_ending_passwords(bytes: &[u8]) -> Result<EndingPasswordCatalog<'_>> {
    ensure!(
        bytes.get(ENDING_PASSWORD_FONT_LOAD_FILE_OFFSET..ENDING_PASSWORD_FONT_LOAD_FILE_OFFSET + 3)
            == Some(&[0xb8, FONT_ENTRY_ID as u8, 0x00]),
        "ENDING password consumer does not load the shared indexed font"
    );
    ensure!(
        bytes.get(
            ENDING_PASSWORD_CONSUMER_FILE_OFFSET
                ..ENDING_PASSWORD_CONSUMER_FILE_OFFSET + ENDING_PASSWORD_CONSUMER_SIGNATURE.len()
        ) == Some(ENDING_PASSWORD_CONSUMER_SIGNATURE),
        "ENDING password consumer signature does not match"
    );
    let body = bytes
        .get(
            ENDING_PASSWORD_CONSUMER_FILE_OFFSET
                ..ENDING_PASSWORD_CONSUMER_FILE_OFFSET + ENDING_PASSWORD_CONSUMER_BODY_LEN,
        )
        .context("ENDING password consumer body is truncated")?;
    let decoded_instruction_count = decode_complete_block(body, "ENDING password consumer")?;
    verify_typed_call(
        bytes,
        ENDING_PASSWORD_CALL_SITE_FILE_OFFSET,
        "ENDING password consumer",
    )?;
    verify_typed_call(
        bytes,
        ENDING_PASSWORD_GLYPH_CALL_FILE_OFFSET,
        "ENDING password glyph renderer",
    )?;

    let table_end = ENDING_PASSWORD_POINTER_TABLE_FILE_OFFSET + ENDING_PASSWORD_COUNT * 2;
    ensure!(
        table_end <= bytes.len(),
        "ENDING password pointer table is truncated"
    );
    let mut records = Vec::with_capacity(ENDING_PASSWORD_COUNT);
    let mut selected_addresses = Vec::with_capacity(ENDING_PASSWORD_COUNT);
    for selection_index in 0..ENDING_PASSWORD_COUNT {
        let pointer_entry_file_offset =
            ENDING_PASSWORD_POINTER_TABLE_FILE_OFFSET + selection_index * 2;
        let runtime_address = read_u16(bytes, pointer_entry_file_offset)? as usize;
        let file_offset = runtime_address
            .checked_sub(COM_ORIGIN)
            .context("ENDING password pointer lies below the COM origin")?;
        let line = parse_indexed_lines(
            bytes.get(file_offset..).with_context(|| {
                format!("ENDING password {selection_index} lies outside the file")
            })?,
            1,
        )?;
        ensure!(
            reconstruct_indexed_lines(&line.lines)? == line.bytes,
            "ENDING password {selection_index} does not reconstruct exactly"
        );
        ensure!(
            line.lines[0]
                .glyphs
                .iter()
                .all(|glyph| glyph.text.is_some()),
            "ENDING password {selection_index} contains an unresolved glyph"
        );
        selected_addresses.push(runtime_address);
        records.push(EndingPasswordRecord {
            selection_index,
            pointer_entry_file_offset,
            runtime_address,
            file_offset,
            bytes: line.bytes,
            glyphs: line
                .lines
                .into_iter()
                .next()
                .expect("one line exists")
                .glyphs,
        });
    }

    selected_addresses.sort_unstable();
    selected_addresses.dedup();
    ensure!(
        selected_addresses.len() == ENDING_PASSWORD_COUNT,
        "ENDING password table contains duplicate pointers"
    );
    let mut expected_file_offset = ENDING_PASSWORD_POOL_FILE_START;
    for runtime_address in selected_addresses {
        let record = records
            .iter()
            .find(|record| record.runtime_address == runtime_address)
            .expect("selected address belongs to one record");
        ensure!(
            record.file_offset == expected_file_offset,
            "ENDING password pool is not contiguous at {expected_file_offset:#x}"
        );
        expected_file_offset += record.bytes.len();
    }
    ensure!(
        expected_file_offset == ENDING_PASSWORD_POOL_FILE_END,
        "ENDING password pool does not end at its selector byte"
    );

    Ok(EndingPasswordCatalog {
        consumer: EndingPasswordConsumerEvidence {
            file_offset: ENDING_PASSWORD_CONSUMER_FILE_OFFSET,
            runtime_address: ENDING_PASSWORD_CONSUMER_RUNTIME_ADDRESS,
            decoded_instruction_count,
            call_site_file_offset: ENDING_PASSWORD_CALL_SITE_FILE_OFFSET,
            font_load_file_offset: ENDING_PASSWORD_FONT_LOAD_FILE_OFFSET,
            glyph_renderer_call_file_offset: ENDING_PASSWORD_GLYPH_CALL_FILE_OFFSET,
        },
        pointer_table_file_offset: ENDING_PASSWORD_POINTER_TABLE_FILE_OFFSET,
        pointer_table_runtime_address: ENDING_PASSWORD_POINTER_TABLE_RUNTIME_ADDRESS,
        selector_file_offset: ENDING_PASSWORD_SELECTOR_FILE_OFFSET,
        selector_runtime_address: ENDING_PASSWORD_SELECTOR_RUNTIME_ADDRESS,
        records,
    })
}

pub(crate) fn parse_ending_interlude_caption(bytes: &[u8]) -> Result<EndingInterludeCaption<'_>> {
    ensure!(
        bytes.get(
            ENDING_INTERLUDE_CONSUMER_FILE_OFFSET
                ..ENDING_INTERLUDE_CONSUMER_FILE_OFFSET + ENDING_INTERLUDE_CONSUMER_SIGNATURE.len()
        ) == Some(ENDING_INTERLUDE_CONSUMER_SIGNATURE),
        "ENDING interlude-caption consumer signature does not match"
    );
    ensure!(
        bytes.get(
            ENDING_INTERLUDE_FONT_LOAD_FILE_OFFSET..ENDING_INTERLUDE_FONT_LOAD_FILE_OFFSET + 3
        ) == Some(&[0xb8, FONT_ENTRY_ID as u8, 0x00]),
        "ENDING interlude caption does not load the shared indexed font"
    );
    ensure!(
        bytes.get(
            ENDING_INTERLUDE_BACKGROUND_LOAD_FILE_OFFSET
                ..ENDING_INTERLUDE_BACKGROUND_LOAD_FILE_OFFSET + 3
        ) == Some(&[0xb8, ENDING_INTERLUDE_BACKGROUND_ENTRY_ID as u8, 0x00,]),
        "ENDING interlude caption does not load its verified background entry"
    );
    let body = bytes
        .get(
            ENDING_INTERLUDE_CONSUMER_FILE_OFFSET
                ..ENDING_INTERLUDE_CONSUMER_FILE_OFFSET + ENDING_INTERLUDE_CONSUMER_BODY_LEN,
        )
        .context("ENDING interlude-caption consumer body is truncated")?;
    let decoded_instruction_count = decode_exact_block(body, "ENDING interlude-caption consumer")?;
    verify_typed_call(
        bytes,
        ENDING_INTERLUDE_GLYPH_CALL_FILE_OFFSET,
        "ENDING interlude-caption glyph renderer",
    )?;

    let line = parse_indexed_lines(
        bytes
            .get(ENDING_INTERLUDE_TEXT_FILE_OFFSET..)
            .context("ENDING interlude caption lies outside the file")?,
        1,
    )?;
    ensure!(
        line.lines[0]
            .glyphs
            .iter()
            .all(|glyph| glyph.text.is_some()),
        "ENDING interlude caption contains an unresolved glyph"
    );
    ensure!(
        reconstruct_indexed_lines(&line.lines)? == line.bytes,
        "ENDING interlude caption does not reconstruct exactly"
    );

    Ok(EndingInterludeCaption {
        consumer: EndingInterludeConsumerEvidence {
            file_offset: ENDING_INTERLUDE_CONSUMER_FILE_OFFSET,
            runtime_address: ENDING_INTERLUDE_CONSUMER_RUNTIME_ADDRESS,
            decoded_instruction_count,
            font_load_file_offset: ENDING_INTERLUDE_FONT_LOAD_FILE_OFFSET,
            background_entry_id: ENDING_INTERLUDE_BACKGROUND_ENTRY_ID,
            background_load_file_offset: ENDING_INTERLUDE_BACKGROUND_LOAD_FILE_OFFSET,
            glyph_renderer_call_file_offset: ENDING_INTERLUDE_GLYPH_CALL_FILE_OFFSET,
        },
        file_offset: ENDING_INTERLUDE_TEXT_FILE_OFFSET,
        runtime_address: ENDING_INTERLUDE_TEXT_RUNTIME_ADDRESS,
        bytes: line.bytes,
        glyphs: line
            .lines
            .into_iter()
            .next()
            .expect("one line exists")
            .glyphs,
    })
}

pub(crate) fn indexed_glyph_text(index: u8) -> Option<&'static str> {
    Some(match index {
        0x00 => "あ",
        0x01 => "ぁ",
        0x02 => "か",
        0x03 => "が",
        0x04 => "さ",
        0x05 => "ざ",
        0x06 => "た",
        0x07 => "だ",
        0x08 => "な",
        0x09 => "は",
        0x0a => "ば",
        0x0b => "ぱ",
        0x0c => "ま",
        0x0d => "や",
        0x0e => "ら",
        0x0f => "わ",
        0x10 => "い",
        0x11 => "ぃ",
        0x12 => "き",
        0x13 => "ぎ",
        0x14 => "し",
        0x15 => "じ",
        0x16 => "ち",
        0x17 => "ぢ",
        0x18 => "に",
        0x19 => "ひ",
        0x1a => "び",
        0x1b => "ぴ",
        0x1c => "み",
        0x1d => "ゆ",
        0x1e => "り",
        0x1f => "を",
        0x20 => "う",
        0x21 => "ぅ",
        0x22 => "く",
        0x23 => "ぐ",
        0x24 => "す",
        0x25 => "ず",
        0x26 => "つ",
        0x27 => "づ",
        0x28 => "ぬ",
        0x29 => "ふ",
        0x2a => "ぶ",
        0x2b => "ぷ",
        0x2c => "む",
        0x2d => "よ",
        0x2e => "る",
        0x2f => "ん",
        0x30 => "え",
        0x31 => "ぇ",
        0x32 => "け",
        0x33 => "げ",
        0x34 => "せ",
        0x35 => "ぜ",
        0x36 => "て",
        0x37 => "で",
        0x38 => "ね",
        0x39 => "へ",
        0x3a => "べ",
        0x3b => "ぺ",
        0x3c => "め",
        0x3d | 0x3f => " ",
        0x3e => "れ",
        0x40 => "お",
        0x41 => "ぉ",
        0x42 => "こ",
        0x43 => "ご",
        0x44 => "そ",
        0x45 => "ぞ",
        0x46 => "と",
        0x47 => "ど",
        0x48 => "の",
        0x49 => "も",
        0x4a => "ほ",
        0x4b => "ぼ",
        0x4c => "ぽ",
        0x4d | 0x4f => " ",
        0x4e => "ろ",
        0x50 => "ア",
        0x51 => "ァ",
        0x52 => "カ",
        0x53 => "ガ",
        0x54 => "サ",
        0x55 => "ザ",
        0x56 => "タ",
        0x57 => "ダ",
        0x58 => "ナ",
        0x59 => "ハ",
        0x5a => "バ",
        0x5b => "パ",
        0x5c => "マ",
        0x5d => "ヤ",
        0x5e => "ラ",
        0x5f => "ワ",
        0x60 => "イ",
        0x61 => "ィ",
        0x62 => "キ",
        0x63 => "ギ",
        0x64 => "シ",
        0x65 => "ジ",
        0x66 => "チ",
        0x67 => "ヂ",
        0x68 => "ニ",
        0x69 => "ヒ",
        0x6a => "ビ",
        0x6b => "ピ",
        0x6c => "ミ",
        0x6d => "ユ",
        0x6e => "リ",
        0x6f => "ヲ",
        0x70 => "ウ",
        0x71 => "ゥ",
        0x72 => "ク",
        0x73 => "グ",
        0x74 => "ス",
        0x75 => "ズ",
        0x76 => "ツ",
        0x77 => "ヅ",
        0x78 => "ヌ",
        0x79 => "フ",
        0x7a => "ブ",
        0x7b => "プ",
        0x7c => "ム",
        0x7d => "ヨ",
        0x7e => "ル",
        0x7f => "ン",
        0x80 => "エ",
        0x81 => "ェ",
        0x82 => "ケ",
        0x83 => "ゲ",
        0x84 => "セ",
        0x85 => "ゼ",
        0x86 => "テ",
        0x87 => "デ",
        0x88 => "ネ",
        0x89 => "ヘ",
        0x8a => "ベ",
        0x8b => "ペ",
        0x8c => "メ",
        0x8e => "レ",
        0x90 => "オ",
        0x91 => "ォ",
        0x92 => "コ",
        0x93 => "ゴ",
        0x94 => "ソ",
        0x95 => "ゾ",
        0x96 => "ト",
        0x97 => "ド",
        0x98 => "ノ",
        0x99 => "ホ",
        0x9a => "ボ",
        0x9b => "ポ",
        0x9c => "モ",
        0x9e => "ロ",
        0xa0 => "ゃ",
        0xa1 => "！",
        0xa2 => "」",
        0xa3 => "ャ",
        0xa4 => "0",
        0xa5 => "1",
        0xa6 => "2",
        0xa7 => "3",
        0xa8 => "4",
        0xa9 => "5",
        0xaa => "6",
        0xab => "7",
        0xac => "8",
        0xad => "9",
        0xae => "T",
        0xaf => "☆",
        0xb0 => "ゅ",
        0xb1 => "？",
        0xb2 => "ー",
        0xb3 => "ュ",
        0xbb => "゛",
        0xbd => "u",
        0xbe => "P",
        0xbf => "E",
        0xc0 => "ょ",
        0xc1 => "、",
        0xc2 => "～",
        0xc3 => "ョ",
        0xd0 => "っ",
        0xd1 => "。",
        0xd2 => "…",
        0xd3 => "ッ",
        0xe0 => "・",
        0xe1 => "「",
        _ => return None,
    })
}

fn parse_select_script<'a>(
    bytes: &'a [u8],
    stage_index: usize,
    phase_index: usize,
    pointer_entry_file_offset: usize,
    runtime_address: usize,
    file_offset: usize,
) -> Result<SelectDialogueScript<'a>> {
    let initial_portrait: [u8; 2] = bytes
        .get(file_offset..file_offset + 2)
        .with_context(|| {
            format!(
                "SELECT dialogue header is truncated for stage {}, phase {}",
                stage_index + 1,
                phase_index + 1
            )
        })?
        .try_into()
        .expect("a two-byte range converts to a two-byte array");
    let mut cursor = file_offset + 2;
    let mut tokens = Vec::new();
    loop {
        let byte = *bytes.get(cursor).with_context(|| {
            format!(
                "SELECT dialogue has no terminator for stage {}, phase {}",
                stage_index + 1,
                phase_index + 1
            )
        })?;
        cursor += 1;
        let token = match byte {
            0xfa => IndexedTextToken::Voice(read_byte(bytes, &mut cursor, "voice parameter")?),
            0xfd => IndexedTextToken::Portrait(read_pair(bytes, &mut cursor, "portrait parameters")?),
            0xfe => IndexedTextToken::LineBreak(read_pair(bytes, &mut cursor, "line-break parameters")?),
            0xff => IndexedTextToken::Terminator,
            index => IndexedTextToken::Glyph {
                index,
                text: indexed_glyph_text(index).with_context(|| {
                    format!(
                        "SELECT dialogue uses unresolved font glyph {index:02X} in stage {}, phase {}",
                        stage_index + 1,
                        phase_index + 1
                    )
                })?,
            },
        };
        let is_terminator = matches!(token, IndexedTextToken::Terminator);
        tokens.push(token);
        if is_terminator {
            break;
        }
    }
    let script_bytes = &bytes[file_offset..cursor];
    let script = SelectDialogueScript {
        phase_index,
        pointer_entry_file_offset,
        runtime_address,
        file_offset,
        bytes: script_bytes,
        initial_portrait,
        tokens,
    };
    ensure!(
        reconstruct_select_script(&script)? == script_bytes,
        "SELECT dialogue does not reconstruct exactly for stage {}, phase {}",
        stage_index + 1,
        phase_index + 1
    );
    Ok(script)
}

fn verify_select_interpreter(bytes: &[u8]) -> Result<SelectInterpreterEvidence> {
    ensure!(
        bytes.get(
            SELECT_INTERPRETER_FILE_OFFSET
                ..SELECT_INTERPRETER_FILE_OFFSET + SELECT_INTERPRETER_SIGNATURE.len()
        ) == Some(SELECT_INTERPRETER_SIGNATURE),
        "SELECT indexed-text interpreter signature does not match"
    );
    let body = bytes
        .get(
            SELECT_INTERPRETER_FILE_OFFSET
                ..SELECT_INTERPRETER_FILE_OFFSET + SELECT_INTERPRETER_BODY_LEN,
        )
        .context("SELECT indexed-text interpreter body is truncated")?;
    let decoded_instruction_count = decode_complete_block(body, "SELECT interpreter")?;

    for file_offset in SELECT_CALL_SITE_FILE_OFFSETS {
        let call =
            decode_bytes(bytes.get(file_offset..file_offset + 3).with_context(|| {
                format!("SELECT dialogue call is truncated at {file_offset:#x}")
            })?)
            .with_context(|| format!("decode SELECT dialogue call at {file_offset:#x}"))?;
        ensure!(
            call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
            "SELECT dialogue consumer is not a typed V30 near CALL at {file_offset:#x}"
        );
    }

    Ok(SelectInterpreterEvidence {
        file_offset: SELECT_INTERPRETER_FILE_OFFSET,
        runtime_address: SELECT_INTERPRETER_RUNTIME_ADDRESS,
        decoded_instruction_count,
        call_site_file_offsets: SELECT_CALL_SITE_FILE_OFFSETS,
    })
}

fn verify_typed_call(bytes: &[u8], file_offset: usize, label: &str) -> Result<()> {
    let call = decode_bytes(
        bytes
            .get(file_offset..file_offset + 3)
            .with_context(|| format!("{label} call is truncated at {file_offset:#x}"))?,
    )
    .with_context(|| format!("decode {label} call at {file_offset:#x}"))?;
    ensure!(
        call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
        "{label} is not a typed V30 near CALL at {file_offset:#x}"
    );
    Ok(())
}

fn decode_complete_block(bytes: &[u8], label: &str) -> Result<usize> {
    let instruction_count = decode_exact_block(bytes, label)?;
    let last = decode_last_instruction(bytes, label)?;
    ensure!(
        matches!(last, Instruction::Ret { pop: 0 }),
        "{label} does not end in a typed V30 RET"
    );
    Ok(instruction_count)
}

fn decode_exact_block(bytes: &[u8], label: &str) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..])
            .with_context(|| format!("decode V30 instruction at {label} byte {cursor:#x}"))?;
        cursor += decoded.byte_len;
        instruction_count += 1;
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed {label} boundary"
    );
    Ok(instruction_count)
}

fn decode_last_instruction(bytes: &[u8], label: &str) -> Result<Instruction> {
    let mut cursor = 0;
    let mut last_instruction = None;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..])
            .with_context(|| format!("decode V30 instruction at {label} byte {cursor:#x}"))?;
        cursor += decoded.byte_len;
        last_instruction = Some(decoded.instruction);
    }
    last_instruction.with_context(|| format!("{label} is empty"))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let raw: [u8; 2] = bytes
        .get(offset..offset + 2)
        .with_context(|| format!("SELECT field is truncated at file offset {offset:#x}"))?
        .try_into()
        .expect("a two-byte range converts to a two-byte array");
    Ok(u16::from_le_bytes(raw))
}

fn read_byte(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<u8> {
    let byte = *bytes
        .get(*cursor)
        .with_context(|| format!("SELECT dialogue has truncated {label}"))?;
    *cursor += 1;
    Ok(byte)
}

fn read_pair(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<[u8; 2]> {
    let pair = [
        read_byte(bytes, cursor, label)?,
        read_byte(bytes, cursor, label)?,
    ];
    Ok(pair)
}

#[cfg(test)]
#[path = "indexed_text_tests.rs"]
mod indexed_text_tests;
