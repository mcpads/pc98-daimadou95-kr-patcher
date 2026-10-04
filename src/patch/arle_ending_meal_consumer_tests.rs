use super::*;

#[test]
fn consumer_patch_replaces_maps_and_delegates_the_verified_address_calculation() {
    let mut baseline = vec![0xa5; MAP_FILE_OFFSET + MAP_BYTE_SIZE + 32];
    baseline[TILE_ADDRESS_FILE_OFFSET..TILE_ADDRESS_FILE_OFFSET + SOURCE_TILE_ADDRESS_BYTES.len()]
        .copy_from_slice(&SOURCE_TILE_ADDRESS_BYTES);
    let replacement_maps = std::array::from_fn(|frame_index| {
        vec![u8::try_from(frame_index).unwrap(); MAP_RECORD_SIZE]
    });

    let patched =
        replace_ending_meal_maps_and_install_bank_selector(&baseline, &replacement_maps).unwrap();

    assert_eq!(
        &patched.bytes[MAP_FILE_OFFSET..MAP_FILE_OFFSET + MAP_BYTE_SIZE],
        replacement_maps.concat()
    );
    let hook_call = v30::decode_bytes(&patched.bytes[TILE_ADDRESS_FILE_OFFSET..]).unwrap();
    assert!(matches!(hook_call.instruction, Instruction::Call { .. }));
    assert_eq!(patched.bytes.len(), baseline.len() + patched.hook_byte_size);
    assert_eq!(
        &patched.bytes[patched.hook_file_offset..],
        &patched.bytes[baseline.len()..]
    );
}

#[test]
fn original_tile_address_source_is_the_exact_id_times_132_sequence() {
    let mut baseline = vec![0; TILE_ADDRESS_FILE_OFFSET + SOURCE_TILE_ADDRESS_BYTES.len()];
    baseline[TILE_ADDRESS_FILE_OFFSET..].copy_from_slice(&SOURCE_TILE_ADDRESS_BYTES);

    verify_source_tile_address(&baseline).unwrap();
}
