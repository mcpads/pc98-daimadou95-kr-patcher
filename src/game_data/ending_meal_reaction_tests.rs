use super::*;

#[test]
fn parses_all_three_progressive_glyph_sources() {
    let source = synthetic_ending_com();

    let catalog = parse_ending_meal_reaction(&source).unwrap();

    assert_eq!(catalog.text, "ぱくっ");
    assert_eq!(catalog.glyph_loads.len(), GLYPH_SLOT_COUNT);
    assert_eq!(catalog.consumer.runtime_address, 0x2843);
    assert_eq!(catalog.consumer.glyph_renderer_runtime_address, 0x312c);
    assert_eq!(
        catalog
            .glyph_loads
            .iter()
            .map(|load| load.glyph.index)
            .collect::<Vec<_>>(),
        SOURCE_GLYPH_INDICES
    );
}

#[test]
fn rejects_a_changed_glyph_source_or_renderer_target() {
    let mut changed_glyph = synthetic_ending_com();
    changed_glyph[GLYPH_LOAD_FILE_OFFSETS[1] + 1] ^= 1;
    assert!(
        parse_ending_meal_reaction(&changed_glyph)
            .unwrap_err()
            .to_string()
            .contains("verified typed V30 source")
    );

    let mut changed_call = synthetic_ending_com();
    changed_call[GLYPH_RENDERER_CALL_FILE_OFFSETS[2] + 1] ^= 1;
    assert!(
        parse_ending_meal_reaction(&changed_call)
            .unwrap_err()
            .to_string()
            .contains("targets")
    );
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
