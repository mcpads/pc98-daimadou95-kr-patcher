use anyhow::{Context, Result, ensure};

const COM_ORIGIN: usize = 0x100;
const GLYPH_COUNT_INSTRUCTION_OFFSET: usize = 0x0f;
const FIRST_CODE_INSTRUCTION_OFFSET: usize = 0x12;
const POINTER_TABLE_INSTRUCTION_OFFSET: usize = 0x19;
const ROW_END_INSTRUCTION_OFFSET: usize = 0x33;
const NEXT_ROW_INSTRUCTION_OFFSET: usize = 0x39;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct GaijiGlyph<'a> {
    pub index: usize,
    pub character_code: u16,
    pub shift_jis_code: u16,
    pub offset: usize,
    pub bytes: &'a [u8],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct GaijiProgram<'a> {
    pub pointer_table_offset: usize,
    pub glyphs: Vec<GaijiGlyph<'a>>,
}

pub(crate) fn parse_gaiji_program(bytes: &[u8]) -> Result<GaijiProgram<'_>> {
    ensure_instruction_prefix(bytes, GLYPH_COUNT_INSTRUCTION_OFFSET, &[0xb9])?;
    ensure_instruction_prefix(
        bytes,
        FIRST_CODE_INSTRUCTION_OFFSET,
        &[0x2e, 0xc7, 0x06, 0x75, 0x01],
    )?;
    ensure_instruction_prefix(bytes, POINTER_TABLE_INSTRUCTION_OFFSET, &[0xbb])?;
    ensure_instruction_prefix(bytes, ROW_END_INSTRUCTION_OFFSET, &[0x81, 0xfa])?;
    ensure_instruction_prefix(
        bytes,
        NEXT_ROW_INSTRUCTION_OFFSET,
        &[0x2e, 0xc7, 0x06, 0x75, 0x01],
    )?;

    let glyph_count = read_u16(bytes, GLYPH_COUNT_INSTRUCTION_OFFSET + 1)? as usize;
    let first_code_previous = read_u16(bytes, FIRST_CODE_INSTRUCTION_OFFSET + 5)?;
    let pointer_table_address = read_u16(bytes, POINTER_TABLE_INSTRUCTION_OFFSET + 1)? as usize;
    let row_end = read_u16(bytes, ROW_END_INSTRUCTION_OFFSET + 2)?;
    let next_row_previous = read_u16(bytes, NEXT_ROW_INSTRUCTION_OFFSET + 5)?;
    let pointer_table_offset = pointer_table_address
        .checked_sub(COM_ORIGIN)
        .context("GAIJI pointer table address lies below the COM origin")?;
    let pointer_table_end = pointer_table_offset
        .checked_add(
            glyph_count
                .checked_mul(2)
                .context("GAIJI pointer table size overflow")?,
        )
        .context("GAIJI pointer table boundary overflow")?;
    ensure!(
        pointer_table_end <= bytes.len(),
        "GAIJI pointer table ends outside the program"
    );

    let mut offsets = Vec::with_capacity(glyph_count);
    for index in 0..glyph_count {
        let pointer_offset = pointer_table_offset + index * 2;
        let address = read_u16(bytes, pointer_offset)? as usize;
        let offset = address
            .checked_sub(COM_ORIGIN)
            .context("GAIJI glyph address lies below the COM origin")?;
        ensure!(
            offset < bytes.len(),
            "GAIJI glyph {index} starts outside the program"
        );
        if let Some(previous) = offsets.last() {
            ensure!(
                offset > *previous,
                "GAIJI glyph pointers are not strictly increasing at index {index}"
            );
        }
        offsets.push(offset);
    }

    let mut codes = Vec::with_capacity(glyph_count);
    let mut code = first_code_previous;
    for _ in 0..glyph_count {
        if code == row_end {
            code = next_row_previous;
        }
        code = code
            .checked_add(1)
            .context("GAIJI character code overflow")?;
        codes.push(code);
    }

    let mut glyphs = Vec::with_capacity(glyph_count);
    for index in 0..glyph_count {
        let offset = offsets[index];
        let end = offsets.get(index + 1).copied().unwrap_or(bytes.len());
        glyphs.push(GaijiGlyph {
            index,
            character_code: codes[index],
            shift_jis_code: jis_row_cell_to_shift_jis(codes[index])?,
            offset,
            bytes: &bytes[offset..end],
        });
    }

    Ok(GaijiProgram {
        pointer_table_offset,
        glyphs,
    })
}

pub(crate) fn jis_row_cell_to_shift_jis(character_code: u16) -> Result<u16> {
    let row = (character_code >> 8) as u8;
    let cell = character_code as u8;
    ensure!(
        (0x21..=0x7e).contains(&row) && (0x21..=0x7e).contains(&cell),
        "GAIJI character code {character_code:04X} is outside the JIS row-cell range"
    );

    let mut lead = ((row - 0x21) >> 1) + 0x81;
    if lead > 0x9f {
        lead = lead
            .checked_add(0x40)
            .context("GAIJI Shift-JIS lead-byte overflow")?;
    }
    let trail = if row & 1 == 1 {
        let mut value = cell
            .checked_add(0x1f)
            .context("GAIJI Shift-JIS trail-byte overflow")?;
        if value >= 0x7f {
            value = value
                .checked_add(1)
                .context("GAIJI Shift-JIS trail-byte overflow")?;
        }
        value
    } else {
        cell.checked_add(0x7e)
            .context("GAIJI Shift-JIS trail-byte overflow")?
    };
    Ok(u16::from_be_bytes([lead, trail]))
}

fn ensure_instruction_prefix(bytes: &[u8], offset: usize, expected: &[u8]) -> Result<()> {
    ensure!(
        bytes.get(offset..offset + expected.len()) == Some(expected),
        "GAIJI instruction signature does not match at file offset {offset:#x}"
    );
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let raw: [u8; 2] = bytes
        .get(offset..offset + 2)
        .with_context(|| format!("GAIJI field is truncated at file offset {offset:#x}"))?
        .try_into()
        .expect("a two-byte range converts to a two-byte array");
    Ok(u16::from_le_bytes(raw))
}

#[cfg(test)]
#[path = "gaiji_tests.rs"]
mod gaiji_tests;
