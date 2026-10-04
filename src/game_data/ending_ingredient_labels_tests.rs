use super::*;

#[test]
fn parses_every_fixed_label_and_trims_only_display_padding() {
    let mut source = synthetic_ending_com();
    let first = TABLE_FILE_OFFSET;
    source[first..first + RECORD_CELL_COUNT].fill(BLANK_GLYPH_INDEX);
    source[first + 6..first + 9].copy_from_slice(&[0x43, 0x09, 0x2f]);

    let catalog = parse_ending_ingredient_labels(&source).unwrap();

    assert_eq!(catalog.records.len(), 27);
    assert_eq!(catalog.records[0].text, "ごはん");
    assert_eq!(catalog.records[0].bytes.len(), RECORD_CELL_COUNT);
    assert_eq!(catalog.records[0].file_offset, TABLE_FILE_OFFSET);
    assert_eq!(catalog.records[26].file_offset, TABLE_FILE_OFFSET + 26 * 16);
    assert_eq!(catalog.consumer.input_state_offsets, INPUT_STATE_OFFSETS);
    assert_eq!(
        catalog.consumer.group_runtime_addresses,
        GROUP_RUNTIME_ADDRESSES
    );
    assert_eq!(catalog.consumer.fixed_cell_count, 16);
    assert_eq!(catalog.consumer.glyph_renderer_runtime_address, 0x3024);
}

#[test]
fn rejects_a_consumer_that_no_longer_selects_the_verified_group_base() {
    let mut source = synthetic_ending_com();
    source[GROUP_BASE_ADD_FILE_OFFSETS[4] + 1] ^= 1;

    assert!(
        parse_ending_ingredient_labels(&source)
            .unwrap_err()
            .to_string()
            .contains("table base")
    );
}

fn synthetic_ending_com() -> Vec<u8> {
    let mut bytes = vec![0x90; TABLE_FILE_OFFSET + TABLE_BYTE_SIZE];
    bytes[CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_SIGNATURE.len()]
        .copy_from_slice(CONSUMER_SIGNATURE);
    for ((load_offset, state_offset), (add_offset, runtime_address)) in SELECTOR_LOAD_FILE_OFFSETS
        .into_iter()
        .zip(INPUT_STATE_OFFSETS)
        .zip(
            GROUP_BASE_ADD_FILE_OFFSETS
                .into_iter()
                .zip(GROUP_RUNTIME_ADDRESSES),
        )
    {
        bytes[load_offset..load_offset + 3].copy_from_slice(&[0x8a, 0x44, state_offset]);
        let immediate = u16::try_from(runtime_address).unwrap().to_le_bytes();
        bytes[add_offset..add_offset + 3].copy_from_slice(&[0x05, immediate[0], immediate[1]]);
    }
    bytes[FIXED_CELL_LOOP_FILE_OFFSET..FIXED_CELL_LOOP_FILE_OFFSET + 3].copy_from_slice(&[
        0xbf,
        RECORD_CELL_COUNT as u8,
        0x00,
    ]);
    let displacement = i16::try_from(
        GLYPH_RENDERER_RUNTIME_ADDRESS as isize
            - (GLYPH_RENDERER_CALL_FILE_OFFSET + COM_ORIGIN + 3) as isize,
    )
    .unwrap()
    .to_le_bytes();
    bytes[GLYPH_RENDERER_CALL_FILE_OFFSET..GLYPH_RENDERER_CALL_FILE_OFFSET + 3].copy_from_slice(&[
        0xe8,
        displacement[0],
        displacement[1],
    ]);
    bytes[TABLE_FILE_OFFSET..TABLE_FILE_OFFSET + TABLE_BYTE_SIZE].fill(BLANK_GLYPH_INDEX);
    for record_index in 0..GROUP_COUNT * VARIANT_COUNT {
        bytes[TABLE_FILE_OFFSET + record_index * RECORD_CELL_COUNT] = 0x00;
    }
    bytes
}
