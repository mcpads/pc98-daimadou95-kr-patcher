use anyhow::{Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::game_data::mad_text::parse_mad_text_catalog;

const FIRST_SLOT_FILE_OFFSET: usize = 0x0eb3;
pub(super) const CELL_FILE_OFFSET: usize = 0x0edb;
const EXPECTED_GLYPH_CODE: [u8; 2] = [0xeb, 0xe1];
const PROBE_GLYPH_CODE: [u8; 2] = [0xeb, 0xa0];

pub(super) fn replace_probe_cell(bytes: &[u8]) -> Result<Vec<u8>> {
    let catalog = parse_mad_text_catalog(bytes)?;
    let first_slot = catalog
        .fixed_slots
        .first()
        .expect("MAD fixed-text parser admits five slots");
    ensure!(
        first_slot.file_offset == FIRST_SLOT_FILE_OFFSET,
        "MAD probe text slot differs from the verified first fixed slot"
    );

    write_plan().apply(bytes, None).map_err(Into::into)
}

pub(super) fn write_plan() -> WritePlan {
    WritePlan::new()
        .region(ImageRegion {
            id: "mad-fixed-text-probe-cell".into(),
            range: CELL_FILE_OFFSET..CELL_FILE_OFFSET + PROBE_GLYPH_CODE.len(),
            kind: RegionKind::Data,
            reason: "one consumer-linked two-byte cell in the first fixed text slot".into(),
        })
        .write(ExpectedWrite {
            id: "write-hangul-probe-cell".into(),
            owner: "hangul-probe-text-producer".into(),
            purpose: "select the newly supplied Hangul ga external character".into(),
            offset: CELL_FILE_OFFSET,
            expected_original: EXPECTED_GLYPH_CODE.to_vec(),
            replacement: PROBE_GLYPH_CODE.to_vec(),
            intent: WriteIntent::Data,
        })
}
