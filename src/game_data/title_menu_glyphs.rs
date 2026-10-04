use anyhow::{Context, Result, ensure};

use super::compile_lz::decode_complete_compile_lz;
#[cfg(test)]
use super::compile_lz::encode_compile_lz;

pub(crate) const MADDAT_ENTRY_ID: usize = 141;
pub(crate) const GLYPH_WIDTH: usize = 24;
pub(crate) const GLYPH_HEIGHT: usize = 32;
pub(crate) const GLYPH_COUNT: usize = 12;
pub(crate) const COLOR_PLANE_COUNT: usize = 3;
pub(crate) const MASK_PLANE_COUNT: usize = 1;
pub(crate) const RECORD_HEADER: [u8; 4] = [3, 0, 32, 0];
pub(crate) const RECORD_SIZE: usize =
    RECORD_HEADER.len() + GLYPH_WIDTH / 8 * GLYPH_HEIGHT * (COLOR_PLANE_COUNT + MASK_PLANE_COUNT);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MaskedTitleMenuGlyph {
    pub pixels: Vec<Option<u8>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TitleMenuGlyphBank {
    pub packed_size: usize,
    pub decoded: Vec<u8>,
    pub glyphs: Vec<MaskedTitleMenuGlyph>,
}

pub(crate) fn decode_title_menu_glyph_bank(input: &[u8]) -> Result<TitleMenuGlyphBank> {
    let stream = decode_complete_compile_lz(input)?;
    ensure!(
        stream.output.len() == GLYPH_COUNT * RECORD_SIZE,
        "title-menu glyph bank has {} decoded bytes instead of {}",
        stream.output.len(),
        GLYPH_COUNT * RECORD_SIZE
    );
    let glyphs = stream
        .output
        .chunks_exact(RECORD_SIZE)
        .enumerate()
        .map(|(index, record)| decode_record(record, index))
        .collect::<Result<Vec<_>>>()?;
    Ok(TitleMenuGlyphBank {
        packed_size: stream.packed_size,
        decoded: stream.output,
        glyphs,
    })
}

#[cfg(test)]
pub(crate) fn encode_title_menu_glyph_bank(glyphs: &[MaskedTitleMenuGlyph]) -> Result<Vec<u8>> {
    ensure!(
        glyphs.len() == GLYPH_COUNT,
        "title-menu glyph population has {} records instead of {GLYPH_COUNT}",
        glyphs.len()
    );
    let mut decoded = Vec::with_capacity(GLYPH_COUNT * RECORD_SIZE);
    for (index, glyph) in glyphs.iter().enumerate() {
        encode_record(&mut decoded, glyph, index)?;
    }
    let packed = encode_compile_lz(&decoded)?;
    let verified = decode_title_menu_glyph_bank(&packed)?;
    ensure!(
        verified.decoded == decoded && verified.glyphs == glyphs,
        "title-menu glyph encoder failed its masked-planar round trip"
    );
    Ok(packed)
}

fn decode_record(record: &[u8], index: usize) -> Result<MaskedTitleMenuGlyph> {
    ensure!(
        record.get(..RECORD_HEADER.len()) == Some(RECORD_HEADER.as_slice()),
        "title-menu glyph {index} has an unexpected 24x32 record header"
    );
    let body = record
        .get(RECORD_HEADER.len()..)
        .context("title-menu glyph record has no planar body")?;
    let mut pixels = vec![None; GLYPH_WIDTH * GLYPH_HEIGHT];
    for y in 0..GLYPH_HEIGHT {
        for byte_x in 0..GLYPH_WIDTH / 8 {
            let source = (y * (GLYPH_WIDTH / 8) + byte_x) * (COLOR_PLANE_COUNT + MASK_PLANE_COUNT);
            let mask = body[source + COLOR_PLANE_COUNT];
            for bit in 0..8 {
                let bit_mask = 1 << (7 - bit);
                if mask & bit_mask == 0 {
                    continue;
                }
                let mut color = 0;
                for plane in 0..COLOR_PLANE_COUNT {
                    if body[source + plane] & bit_mask != 0 {
                        color |= 1 << plane;
                    }
                }
                pixels[y * GLYPH_WIDTH + byte_x * 8 + bit] = Some(color);
            }
        }
    }
    Ok(MaskedTitleMenuGlyph { pixels })
}

#[cfg(test)]
fn encode_record(output: &mut Vec<u8>, glyph: &MaskedTitleMenuGlyph, index: usize) -> Result<()> {
    ensure!(
        glyph.pixels.len() == GLYPH_WIDTH * GLYPH_HEIGHT,
        "title-menu glyph {index} has {} pixels instead of {}",
        glyph.pixels.len(),
        GLYPH_WIDTH * GLYPH_HEIGHT
    );
    ensure!(
        glyph
            .pixels
            .iter()
            .flatten()
            .all(|color| *color < (1 << COLOR_PLANE_COUNT)),
        "title-menu glyph {index} uses a color outside its three-plane palette"
    );
    output.extend_from_slice(&RECORD_HEADER);
    for y in 0..GLYPH_HEIGHT {
        for byte_x in 0..GLYPH_WIDTH / 8 {
            let mut planes = [0_u8; COLOR_PLANE_COUNT + MASK_PLANE_COUNT];
            for bit in 0..8 {
                let Some(color) = glyph.pixels[y * GLYPH_WIDTH + byte_x * 8 + bit] else {
                    continue;
                };
                let bit_mask = 1 << (7 - bit);
                planes[COLOR_PLANE_COUNT] |= bit_mask;
                for (plane, plane_byte) in planes[..COLOR_PLANE_COUNT].iter_mut().enumerate() {
                    if color & (1 << plane) != 0 {
                        *plane_byte |= bit_mask;
                    }
                }
            }
            output.extend_from_slice(&planes);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "title_menu_glyphs_tests.rs"]
mod title_menu_glyphs_tests;
