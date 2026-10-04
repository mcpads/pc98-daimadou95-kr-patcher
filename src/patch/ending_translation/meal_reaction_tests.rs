use super::*;
use crate::game_data::ending_meal_reaction::{
    CONSUMER_BODY_LEN, CONSUMER_FILE_OFFSET, CONSUMER_SIGNATURE, GLYPH_RENDERER_CALL_FILE_OFFSETS,
    GLYPH_RENDERER_RUNTIME_ADDRESS, SOURCE_GLYPH_INDICES,
};

#[test]
fn rewrites_all_fixed_glyph_sources_and_blanks_unused_slots() {
    let source = synthetic_ending_com();
    let codebook = BTreeMap::from([('냠', 0x10), ('!', 0x11)]);

    let patched = replace_ending_meal_reaction(&source, "냠!", &codebook).unwrap();

    assert_eq!(patched.report.glyph_slot_count, 3);
    assert_eq!(patched.report.translated_glyph_count, 2);
    assert_eq!(
        patched
            .report
            .glyph_load_file_offsets
            .map(|offset| patched.bytes[offset + 1]),
        [0x10, 0x11, BLANK_GLYPH_INDEX]
    );
}

#[test]
fn rejects_overflow_and_source_revision_drift() {
    let source = synthetic_ending_com();
    let codebook = BTreeMap::from([('가', 0x10), ('나', 0x11), ('다', 0x12), ('라', 0x13)]);
    assert!(
        replace_ending_meal_reaction(&source, "가나다라", &codebook)
            .unwrap_err()
            .to_string()
            .contains("requires 1..=3")
    );

    let mut drifted = source;
    drifted[GLYPH_LOAD_FILE_OFFSETS[0] + 1] ^= 1;
    assert!(replace_ending_meal_reaction(&drifted, "가", &codebook).is_err());
}

fn synthetic_ending_com() -> Vec<u8> {
    let mut bytes = vec![0x90; CONSUMER_FILE_OFFSET + CONSUMER_BODY_LEN];
    bytes[CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_SIGNATURE.len()]
        .copy_from_slice(CONSUMER_SIGNATURE);
    for (file_offset, glyph_index) in GLYPH_LOAD_FILE_OFFSETS
        .into_iter()
        .zip(SOURCE_GLYPH_INDICES)
    {
        bytes[file_offset..file_offset + 3].copy_from_slice(&[0xbe, glyph_index, 0x00]);
    }
    for file_offset in GLYPH_RENDERER_CALL_FILE_OFFSETS {
        let displacement = i16::try_from(
            GLYPH_RENDERER_RUNTIME_ADDRESS as isize - (file_offset + COM_ORIGIN + 3) as isize,
        )
        .unwrap()
        .to_le_bytes();
        bytes[file_offset..file_offset + 3].copy_from_slice(&[
            0xe8,
            displacement[0],
            displacement[1],
        ]);
    }
    bytes
}
