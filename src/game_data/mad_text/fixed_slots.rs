use anyhow::{Context, Result, ensure};

use super::stream::{
    ParsedTextStream, RendererTextToken, parse_renderer_text_stream,
    reconstruct_renderer_text_stream,
};

const COM_ORIGIN: usize = 0x100;
const POINTER_TABLE_FILE_OFFSET: usize = 0x0e9f;
const POINTER_COUNT: usize = 10;
const SLOT_COUNT: usize = 5;
pub(crate) const SLOT_SIZE: usize = 0x60;
const ROW_COUNT: usize = 6;
const CELLS_PER_ROW: usize = 6;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FixedTextSlot<'a> {
    pub index: usize,
    pub pointer_entry_offsets: [usize; 2],
    pub file_offset: usize,
    pub bytes: &'a [u8],
    pub stream: ParsedTextStream,
}

pub(crate) fn parse_fixed_text_slots(bytes: &[u8]) -> Result<Vec<FixedTextSlot<'_>>> {
    let table_end = POINTER_TABLE_FILE_OFFSET + POINTER_COUNT * 2;
    ensure!(
        table_end <= bytes.len(),
        "MAD fixed-text pointer table is truncated"
    );

    let mut slots: Vec<FixedTextSlot<'_>> = Vec::with_capacity(SLOT_COUNT);
    for index in 0..SLOT_COUNT {
        let first_pointer_offset = POINTER_TABLE_FILE_OFFSET + index * 4;
        let second_pointer_offset = first_pointer_offset + 2;
        let first_address = read_u16(bytes, first_pointer_offset)? as usize;
        let second_address = read_u16(bytes, second_pointer_offset)? as usize;
        ensure!(
            first_address == second_address,
            "MAD fixed-text pointer pair {} does not select the same slot",
            index + 1
        );
        let file_offset = first_address
            .checked_sub(COM_ORIGIN)
            .context("MAD fixed-text pointer lies below the COM origin")?;
        if let Some(previous) = slots.last() {
            ensure!(
                file_offset == previous.file_offset + SLOT_SIZE,
                "MAD fixed-text slots do not use the verified {SLOT_SIZE:#x}-byte stride"
            );
        }
        let end = file_offset
            .checked_add(SLOT_SIZE)
            .context("MAD fixed-text slot boundary overflow")?;
        let slot_bytes = bytes
            .get(file_offset..end)
            .with_context(|| format!("MAD fixed-text slot {} is truncated", index + 1))?;
        let stream = parse_renderer_text_stream(slot_bytes)
            .with_context(|| format!("parse MAD fixed-text slot {}", index + 1))?;
        ensure!(
            stream.byte_len == SLOT_SIZE,
            "MAD fixed-text slot {} terminates before its fixed boundary",
            index + 1
        );
        verify_fixed_layout(&stream.tokens)
            .with_context(|| format!("verify MAD fixed-text slot {} layout", index + 1))?;
        ensure!(
            reconstruct_renderer_text_stream(&stream.tokens)? == slot_bytes,
            "MAD fixed-text slot {} does not reconstruct exactly",
            index + 1
        );

        slots.push(FixedTextSlot {
            index,
            pointer_entry_offsets: [first_pointer_offset, second_pointer_offset],
            file_offset,
            bytes: slot_bytes,
            stream,
        });
    }
    Ok(slots)
}

fn verify_fixed_layout(tokens: &[RendererTextToken]) -> Result<()> {
    let mut row = 0;
    let mut cells = 0;
    let mut expects_control = true;

    for token in tokens {
        match token {
            RendererTextToken::DisplayControl(_) if expects_control => {
                expects_control = false;
            }
            RendererTextToken::SingleByte(_) | RendererTextToken::DoubleByte(_)
                if !expects_control =>
            {
                cells += 1;
                ensure!(
                    cells <= CELLS_PER_ROW,
                    "fixed-text row has more than {CELLS_PER_ROW} cells"
                );
            }
            RendererTextToken::LineBreak if !expects_control => {
                ensure!(
                    cells == CELLS_PER_ROW,
                    "fixed-text row has {cells} cells instead of {CELLS_PER_ROW}"
                );
                row += 1;
                ensure!(row < ROW_COUNT, "fixed-text stream has too many rows");
                cells = 0;
                expects_control = true;
            }
            RendererTextToken::Terminator if !expects_control => {
                ensure!(
                    row + 1 == ROW_COUNT && cells == CELLS_PER_ROW,
                    "fixed-text final row does not complete the {ROW_COUNT} by {CELLS_PER_ROW} layout"
                );
            }
            _ => ensure!(
                false,
                "fixed-text token appears outside the verified layout"
            ),
        }
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let raw: [u8; 2] = bytes
        .get(offset..offset + 2)
        .with_context(|| format!("MAD field is truncated at file offset {offset:#x}"))?
        .try_into()
        .expect("a two-byte range converts to a two-byte array");
    Ok(u16::from_le_bytes(raw))
}

#[cfg(test)]
#[path = "fixed_slots_tests.rs"]
mod fixed_slots_tests;
