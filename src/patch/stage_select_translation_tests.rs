use std::collections::BTreeMap;
use std::{env, fs};

use super::*;

#[test]
#[ignore = "requires STAGE_SELECT_MADDAT pointing to a user-owned extracted MADDAT"]
fn source_stage_select_pixels_survive_gcs_and_flink_round_trips() {
    let path = env::var_os("STAGE_SELECT_MADDAT").expect("set STAGE_SELECT_MADDAT");
    let maddat = fs::read(path).unwrap();
    let archive = parse_flink_archive(&maddat).unwrap();
    let entry = &archive.entries[MADDAT_ENTRY_ID - 1];
    let source = decode_stage_select_graphic(entry.bytes).unwrap();
    let composition = compose_stage_select_graphic(&source.pixels, "스테이지 선택").unwrap();
    assert_eq!(
        composition.erased_source_label_pixel_count,
        EXPECTED_SOURCE_LABEL_PIXEL_COUNT
    );

    let packed = encode_stage_select_graphic(&source, &composition.pixels).unwrap();
    let repacked =
        repack_contiguous_flink_archive(&maddat, &BTreeMap::from([(MADDAT_ENTRY_ID, packed)]), &[])
            .unwrap();
    let candidate = parse_flink_archive(&repacked.bytes).unwrap();
    let decoded =
        decode_stage_select_graphic(candidate.entries[MADDAT_ENTRY_ID - 1].bytes).unwrap();

    assert_eq!(decoded.pixels, composition.pixels);
    for (source_entry, candidate_entry) in archive.entries.iter().zip(&candidate.entries) {
        if source_entry.id != MADDAT_ENTRY_ID {
            assert_eq!(candidate_entry.bytes, source_entry.bytes);
        }
    }
}
