mod gaiji;
mod mad_text;

use anyhow::Result;

use crate::font::{FontProvenance, verify_hangul_repertoire};

#[derive(Debug, Eq, PartialEq)]
pub struct HangulProbeFiles {
    pub mad_com: Vec<u8>,
    pub gaiji_com: Vec<u8>,
    pub report: HangulProbeReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct HangulProbeReport {
    pub character: char,
    pub character_code: u16,
    pub shift_jis_code: u16,
    pub gaiji_bitmap_file_offset: usize,
    pub mad_text_file_offset: usize,
    pub font: FontProvenance,
}

pub fn apply_hangul_probe(mad_com: &[u8], gaiji_com: &[u8]) -> Result<HangulProbeFiles> {
    let font = verify_hangul_repertoire(['가'])?.provenance;
    let gaiji_patch = gaiji::replace_probe_glyph(gaiji_com)?;
    let mad_text_patch = mad_text::replace_probe_cell(mad_com)?;
    Ok(HangulProbeFiles {
        mad_com: mad_text_patch,
        gaiji_com: gaiji_patch,
        report: HangulProbeReport {
            character: '가',
            character_code: gaiji::PROBE_CHARACTER_CODE,
            shift_jis_code: gaiji::PROBE_SHIFT_JIS_CODE,
            gaiji_bitmap_file_offset: gaiji::BITMAP_FILE_OFFSET,
            mad_text_file_offset: mad_text::CELL_FILE_OFFSET,
            font,
        },
    })
}

#[cfg(test)]
#[path = "hangul_probe_tests.rs"]
mod hangul_probe_tests;
