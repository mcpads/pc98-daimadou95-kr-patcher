use anyhow::{Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::font::rasterize_hangul_syllable;
use crate::game_data::gaiji::parse_gaiji_program;

const GLYPH_RECORD_FILE_OFFSET: usize = 0x01d5;
pub(super) const BITMAP_FILE_OFFSET: usize = GLYPH_RECORD_FILE_OFFSET + 2;
pub(super) const PROBE_CHARACTER_CODE: u16 = 0x7622;
pub(super) const PROBE_SHIFT_JIS_CODE: u16 = 0xeba0;
const EXPECTED_BLANK_BITMAP: [u8; 32] = [0; 32];

pub(super) fn replace_probe_glyph(bytes: &[u8]) -> Result<Vec<u8>> {
    let program = parse_gaiji_program(bytes)?;
    let glyph = program
        .glyphs
        .first()
        .expect("GAIJI parser admits a non-empty glyph table");
    ensure!(
        glyph.offset == GLYPH_RECORD_FILE_OFFSET
            && glyph.bytes.len() == 34
            && glyph.character_code == PROBE_CHARACTER_CODE
            && glyph.shift_jis_code == PROBE_SHIFT_JIS_CODE,
        "GAIJI probe slot differs from the verified first glyph record"
    );

    write_plan()?.apply(bytes, None).map_err(Into::into)
}

pub(super) fn write_plan() -> Result<WritePlan> {
    let hangul_ga_bitmap = rasterize_hangul_syllable('가')?;
    Ok(WritePlan::new()
        .region(ImageRegion {
            id: "gaiji-probe-bitmap".into(),
            range: BITMAP_FILE_OFFSET..BITMAP_FILE_OFFSET + hangul_ga_bitmap.len(),
            kind: RegionKind::Data,
            reason: "bitmap payload of the verified first PC-98 external-character record".into(),
        })
        .write(ExpectedWrite {
            id: "write-hangul-ga-bitmap".into(),
            owner: "hangul-probe-glyph-producer".into(),
            purpose: "replace one blank external-character bitmap with Hangul ga".into(),
            offset: BITMAP_FILE_OFFSET,
            expected_original: EXPECTED_BLANK_BITMAP.to_vec(),
            replacement: hangul_ga_bitmap.to_vec(),
            intent: WriteIntent::Data,
        }))
}
