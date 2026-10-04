use anyhow::{Context, Result, ensure};
use v30::{Instruction, decode_bytes};

pub(crate) const FONT_ENTRY_ID: usize = 147;
pub(crate) const TEXT_ENTRY_ID: usize = 148;
pub(crate) const MOTION_TRACK_ENTRY_IDS: [usize; 3] = [149, 150, 151];

pub(crate) const SCREEN_COUNT: usize = 10;
pub(crate) const LINES_PER_SCREEN: usize = 3;
pub(crate) const CELLS_PER_LINE: usize = 20;
pub(crate) const FONT_GLYPH_COUNT: usize = 65;
pub(crate) const FONT_GLYPH_SIZE: usize = 96;
pub(crate) const FONT_GLYPH_WIDTH: usize = 24;
pub(crate) const FONT_GLYPH_HEIGHT: usize = 32;

pub(crate) const ROLE_LINE_SPECS: [EndingCreditRoleLineSpec; 10] = [
    EndingCreditRoleLineSpec::new(1, 2, "STAFF"),
    EndingCreditRoleLineSpec::new(2, 1, "PLANNING"),
    EndingCreditRoleLineSpec::new(3, 1, "GRAPHIC DESIGN"),
    EndingCreditRoleLineSpec::new(4, 1, "PROGRAM"),
    EndingCreditRoleLineSpec::new(5, 1, "SOUND"),
    EndingCreditRoleLineSpec::new(6, 1, "SPECIAL THANKS"),
    EndingCreditRoleLineSpec::new(7, 1, "SPECIAL THANKS"),
    EndingCreditRoleLineSpec::new(8, 1, "DIRECTOR"),
    EndingCreditRoleLineSpec::new(9, 1, "PRODUCER"),
    EndingCreditRoleLineSpec::new(10, 1, "Presented By"),
];

const DOS_EOF: u8 = 0x1a;
const TRACK_TERMINATOR: u16 = 0x0280;
const COM_ORIGIN: usize = 0x100;
const FONT_LOAD_REFERENCE_FILE_OFFSET: usize = 0x32aa;
const ENTRY_LOAD_REFERENCE_FILE_OFFSETS: [(usize, usize); 4] =
    [(149, 0x3367), (150, 0x33ae), (151, 0x33f5), (148, 0x343c)];
const SEQUENCE_CALL_SITE_FILE_OFFSET: usize = 0x34ca;
const SEQUENCE_FILE_OFFSET: usize = 0x1be4;
const SEQUENCE_BODY_LEN: usize = 0x369;
const SEQUENCE_SIGNATURE: &[u8] = &[
    0xa1, 0x0b, 0x49, 0x8e, 0xc0, 0xb8, 0x00, 0x00, 0xbf, 0x00, 0x00, 0x50, 0xbb, 0x00, 0x00, 0x2e,
    0xc7, 0x06, 0x48, 0x17, 0x00, 0x00,
];
const GLYPH_RENDERER_FILE_OFFSET: usize = 0x186d;
const GLYPH_RENDERER_BODY_LEN: usize = 0x7f;
const GLYPH_RENDERER_SIGNATURE: &[u8] = &[
    0x1e, 0x06, 0x0e, 0x1f, 0x48, 0xc1, 0xe0, 0x05, 0x8b, 0xf0, 0x03, 0xf0, 0x03, 0xf0,
];
const GLYPH_RENDERER_CALL_SITE_FILE_OFFSET: usize = 0x1c20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EndingCreditRoleLineSpec {
    pub screen_number: usize,
    pub line_number: usize,
    pub source_text: &'static str,
}

impl EndingCreditRoleLineSpec {
    const fn new(screen_number: usize, line_number: usize, source_text: &'static str) -> Self {
        Self {
            screen_number,
            line_number,
            source_text,
        }
    }

    pub(crate) fn record_id(self) -> String {
        format!(
            "screen-{:02}-line-{:02}",
            self.screen_number, self.line_number
        )
    }

    pub(crate) fn target_id(self) -> String {
        format!("ending-credit:{}", self.record_id())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCreditGrid {
    pub screens: Vec<EndingCreditScreen>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCreditScreen {
    pub lines: Vec<EndingCreditLine>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCreditLine {
    pub glyph_indices: [u8; CELLS_PER_LINE],
    pub text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EndingCreditPoint {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCreditMotionTrack {
    pub points: Vec<EndingCreditPoint>,
    pub terminator: u16,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingCreditConsumerEvidence {
    pub font_load_reference_file_offset: usize,
    pub entry_load_reference_file_offsets: Vec<(usize, usize)>,
    pub sequence_call_site_file_offset: usize,
    pub sequence_file_offset: usize,
    pub sequence_runtime_address: usize,
    pub sequence_decoded_instruction_count: usize,
    pub glyph_renderer_call_site_file_offset: usize,
    pub glyph_renderer_file_offset: usize,
    pub glyph_renderer_runtime_address: usize,
    pub glyph_renderer_decoded_instruction_count: usize,
}

pub(crate) fn parse_ending_credit_grid(bytes: &[u8]) -> Result<EndingCreditGrid> {
    let grid_size = SCREEN_COUNT * LINES_PER_SCREEN * CELLS_PER_LINE;
    ensure!(
        bytes.len() == grid_size + 1 && bytes[grid_size] == DOS_EOF,
        "ending credit grid must contain {grid_size} cells followed by DOS EOF"
    );

    let mut screens = Vec::with_capacity(SCREEN_COUNT);
    for screen_index in 0..SCREEN_COUNT {
        let mut lines = Vec::with_capacity(LINES_PER_SCREEN);
        for line_index in 0..LINES_PER_SCREEN {
            let start = (screen_index * LINES_PER_SCREEN + line_index) * CELLS_PER_LINE;
            let glyph_indices: [u8; CELLS_PER_LINE] = bytes[start..start + CELLS_PER_LINE]
                .try_into()
                .expect("a twenty-byte range converts to a twenty-byte array");
            let mut text = String::new();
            for index in glyph_indices {
                text.push_str(credit_glyph_text(index).with_context(|| {
                    format!(
                        "ending credit screen {}, line {} uses unresolved glyph {index:02X}",
                        screen_index + 1,
                        line_index + 1
                    )
                })?);
            }
            lines.push(EndingCreditLine {
                glyph_indices,
                text: text.trim_end().to_owned(),
            });
        }
        screens.push(EndingCreditScreen { lines });
    }

    let grid = EndingCreditGrid { screens };
    ensure!(
        reconstruct_ending_credit_grid(&grid)? == bytes,
        "ending credit grid does not reconstruct exactly"
    );
    Ok(grid)
}

pub(crate) fn reconstruct_ending_credit_grid(grid: &EndingCreditGrid) -> Result<Vec<u8>> {
    ensure!(
        grid.screens.len() == SCREEN_COUNT,
        "ending credit grid must contain {SCREEN_COUNT} screens"
    );
    let mut bytes = Vec::with_capacity(SCREEN_COUNT * LINES_PER_SCREEN * CELLS_PER_LINE + 1);
    for screen in &grid.screens {
        ensure!(
            screen.lines.len() == LINES_PER_SCREEN,
            "ending credit screen must contain {LINES_PER_SCREEN} lines"
        );
        for line in &screen.lines {
            for index in line.glyph_indices {
                ensure!(
                    credit_glyph_text(index).is_some(),
                    "ending credit grid uses unresolved glyph {index:02X}"
                );
                bytes.push(index);
            }
        }
    }
    bytes.push(DOS_EOF);
    Ok(bytes)
}

pub(crate) fn parse_ending_credit_motion_track(bytes: &[u8]) -> Result<EndingCreditMotionTrack> {
    let mut cursor = 0;
    let mut points = Vec::new();
    let terminator = loop {
        let x = read_u16(bytes, &mut cursor, "motion x coordinate or terminator")?;
        if x >= TRACK_TERMINATOR {
            ensure!(
                x == TRACK_TERMINATOR,
                "ending credit motion track has unexpected terminator {x:#06x}"
            );
            break x;
        }
        let y = read_u16(bytes, &mut cursor, "motion y coordinate")?;
        points.push(EndingCreditPoint { x, y });
    };
    ensure!(
        cursor == bytes.len(),
        "ending credit motion track has bytes after its terminator"
    );
    ensure!(
        points.len() == 190,
        "ending credit motion track has {} points instead of 190",
        points.len()
    );
    let track = EndingCreditMotionTrack { points, terminator };
    ensure!(
        reconstruct_ending_credit_motion_track(&track)? == bytes,
        "ending credit motion track does not reconstruct exactly"
    );
    Ok(track)
}

pub(crate) fn reconstruct_ending_credit_motion_track(
    track: &EndingCreditMotionTrack,
) -> Result<Vec<u8>> {
    ensure!(
        track.points.len() == 190,
        "ending credit motion track must contain 190 points"
    );
    ensure!(
        track.terminator == TRACK_TERMINATOR,
        "ending credit motion track must end with {TRACK_TERMINATOR:#06x}"
    );
    let mut bytes = Vec::with_capacity(track.points.len() * 4 + 2);
    for point in &track.points {
        ensure!(
            point.x < TRACK_TERMINATOR,
            "ending credit x coordinate collides with the terminator"
        );
        bytes.extend_from_slice(&point.x.to_le_bytes());
        bytes.extend_from_slice(&point.y.to_le_bytes());
    }
    bytes.extend_from_slice(&track.terminator.to_le_bytes());
    Ok(bytes)
}

pub(crate) fn verify_ending_credit_font(bytes: &[u8]) -> Result<()> {
    ensure!(
        FONT_GLYPH_WIDTH.is_multiple_of(8)
            && FONT_GLYPH_SIZE == FONT_GLYPH_WIDTH / 8 * FONT_GLYPH_HEIGHT
            && bytes.len() == FONT_GLYPH_COUNT * FONT_GLYPH_SIZE,
        "ending credit font must contain {FONT_GLYPH_COUNT} glyphs of {FONT_GLYPH_SIZE} bytes"
    );
    Ok(())
}

pub(crate) fn ending_credit_line_offset(screen_number: usize, line_number: usize) -> Result<usize> {
    ensure!(
        (1..=SCREEN_COUNT).contains(&screen_number),
        "ending-credit screen {screen_number} is outside 1..={SCREEN_COUNT}"
    );
    ensure!(
        (1..=LINES_PER_SCREEN).contains(&line_number),
        "ending-credit line {line_number} is outside 1..={LINES_PER_SCREEN}"
    );
    Ok(((screen_number - 1) * LINES_PER_SCREEN + (line_number - 1)) * CELLS_PER_LINE)
}

pub(crate) fn ending_credit_glyph_offset(index: u8) -> Result<usize> {
    ensure!(
        (1..=FONT_GLYPH_COUNT).contains(&usize::from(index)),
        "ending-credit glyph index {index:#04x} is outside 1..={FONT_GLYPH_COUNT}"
    );
    Ok((usize::from(index) - 1) * FONT_GLYPH_SIZE)
}

pub(crate) fn role_line_spec(
    screen_number: usize,
    line_number: usize,
) -> Option<EndingCreditRoleLineSpec> {
    ROLE_LINE_SPECS
        .iter()
        .copied()
        .find(|spec| spec.screen_number == screen_number && spec.line_number == line_number)
}

pub(crate) fn verify_ending_credit_consumer(bytes: &[u8]) -> Result<EndingCreditConsumerEvidence> {
    verify_entry_load(bytes, FONT_ENTRY_ID, FONT_LOAD_REFERENCE_FILE_OFFSET)?;
    for (entry_id, file_offset) in ENTRY_LOAD_REFERENCE_FILE_OFFSETS {
        verify_entry_load(bytes, entry_id, file_offset)?;
    }

    ensure!(
        bytes.get(SEQUENCE_FILE_OFFSET..SEQUENCE_FILE_OFFSET + SEQUENCE_SIGNATURE.len())
            == Some(SEQUENCE_SIGNATURE),
        "ENDING credit-sequence consumer signature does not match"
    );
    let sequence = bytes
        .get(SEQUENCE_FILE_OFFSET..SEQUENCE_FILE_OFFSET + SEQUENCE_BODY_LEN)
        .context("ENDING credit-sequence consumer is truncated")?;
    let sequence_decoded_instruction_count = decode_complete_block(sequence, "credit sequence")?;
    verify_typed_call(
        bytes,
        SEQUENCE_CALL_SITE_FILE_OFFSET,
        "credit-sequence consumer",
    )?;

    ensure!(
        bytes.get(
            GLYPH_RENDERER_FILE_OFFSET..GLYPH_RENDERER_FILE_OFFSET + GLYPH_RENDERER_SIGNATURE.len()
        ) == Some(GLYPH_RENDERER_SIGNATURE),
        "ENDING credit glyph-renderer signature does not match"
    );
    let renderer = bytes
        .get(GLYPH_RENDERER_FILE_OFFSET..GLYPH_RENDERER_FILE_OFFSET + GLYPH_RENDERER_BODY_LEN)
        .context("ENDING credit glyph renderer is truncated")?;
    let glyph_renderer_decoded_instruction_count =
        decode_complete_block(renderer, "credit glyph renderer")?;
    verify_typed_call(
        bytes,
        GLYPH_RENDERER_CALL_SITE_FILE_OFFSET,
        "credit glyph renderer",
    )?;

    Ok(EndingCreditConsumerEvidence {
        font_load_reference_file_offset: FONT_LOAD_REFERENCE_FILE_OFFSET,
        entry_load_reference_file_offsets: ENTRY_LOAD_REFERENCE_FILE_OFFSETS.to_vec(),
        sequence_call_site_file_offset: SEQUENCE_CALL_SITE_FILE_OFFSET,
        sequence_file_offset: SEQUENCE_FILE_OFFSET,
        sequence_runtime_address: SEQUENCE_FILE_OFFSET + COM_ORIGIN,
        sequence_decoded_instruction_count,
        glyph_renderer_call_site_file_offset: GLYPH_RENDERER_CALL_SITE_FILE_OFFSET,
        glyph_renderer_file_offset: GLYPH_RENDERER_FILE_OFFSET,
        glyph_renderer_runtime_address: GLYPH_RENDERER_FILE_OFFSET + COM_ORIGIN,
        glyph_renderer_decoded_instruction_count,
    })
}

fn credit_glyph_text(index: u8) -> Option<&'static str> {
    Some(match index {
        0x00 => " ",
        0x01 => "A",
        0x02 => "B",
        0x03 => "C",
        0x04 => "D",
        0x05 => "E",
        0x06 => "F",
        0x07 => "G",
        0x08 => "H",
        0x09 => "I",
        0x0a => "J",
        0x0b => "K",
        0x0c => "L",
        0x0d => "M",
        0x0e => "N",
        0x0f => "O",
        0x10 => "P",
        0x11 => "Q",
        0x12 => "R",
        0x13 => "S",
        0x14 => "T",
        0x15 => "U",
        0x16 => "V",
        0x17 => "W",
        0x18 => "X",
        0x19 => "Y",
        0x1a => "Z",
        0x1b => "a",
        0x1c => "b",
        0x1d => "c",
        0x1e => "d",
        0x1f => "e",
        0x20 => "f",
        0x21 => "g",
        0x22 => "h",
        0x23 => "i",
        0x24 => "j",
        0x25 => "k",
        0x26 => "l",
        0x27 => "m",
        0x28 => "n",
        0x29 => "o",
        0x2a => "p",
        0x2b => "q",
        0x2c => "r",
        0x2d => "s",
        0x2e => "t",
        0x2f => "u",
        0x30 => "v",
        0x31 => "w",
        0x32 => "x",
        0x33 => "y",
        0x34 => "z",
        0x41 => "[PUYO]",
        _ => return None,
    })
}

fn verify_entry_load(bytes: &[u8], entry_id: usize, file_offset: usize) -> Result<()> {
    let immediate = u16::try_from(entry_id)
        .context("ending credit MADDAT entry ID does not fit in a V30 immediate")?
        .to_le_bytes();
    ensure!(
        bytes.get(file_offset..file_offset + 3) == Some(&[0xb8, immediate[0], immediate[1]]),
        "ENDING MADDAT entry {entry_id} load reference does not match at {file_offset:#x}"
    );
    Ok(())
}

fn verify_typed_call(bytes: &[u8], file_offset: usize, label: &str) -> Result<()> {
    let call = decode_bytes(
        bytes
            .get(file_offset..file_offset + 3)
            .with_context(|| format!("ENDING {label} call is truncated"))?,
    )
    .with_context(|| format!("decode ENDING {label} call"))?;
    ensure!(
        call.byte_len == 3 && matches!(call.instruction, Instruction::Call { .. }),
        "ENDING {label} is not a typed V30 near CALL"
    );
    Ok(())
}

fn decode_complete_block(bytes: &[u8], label: &str) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    let mut last_instruction = None;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..]).with_context(|| {
            format!("decode V30 instruction at ENDING {label} byte {cursor:#x}")
        })?;
        cursor += decoded.byte_len;
        instruction_count += 1;
        last_instruction = Some(decoded.instruction);
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed ENDING {label} boundary"
    );
    ensure!(
        matches!(last_instruction, Some(Instruction::Ret { pop: 0 })),
        "ENDING {label} does not end in a typed V30 RET"
    );
    Ok(instruction_count)
}

fn read_u16(bytes: &[u8], cursor: &mut usize, label: &str) -> Result<u16> {
    let raw: [u8; 2] = bytes
        .get(*cursor..*cursor + 2)
        .with_context(|| format!("ending credit track has truncated {label}"))?
        .try_into()
        .expect("a two-byte range converts to a two-byte array");
    *cursor += 2;
    Ok(u16::from_le_bytes(raw))
}

#[cfg(test)]
#[path = "ending_credits_tests.rs"]
mod ending_credits_tests;
