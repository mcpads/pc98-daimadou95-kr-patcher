use super::*;

const TEST_PACKED_TILE_BANK_SIZE: usize = 14_582;
const TEST_PACKED_TILE_MAP_SIZE: usize = 259;

fn supported_consumer_fixture() -> Vec<u8> {
    let mut source = vec![0; 0x4f00];
    for (offset, program) in [
        (
            ALLOCATION_CALL_FILE_OFFSET,
            assemble_call_site(
                ALLOCATION_CALL_RUNTIME_ADDRESS,
                ORIGINAL_INITIALIZER_RUNTIME_ADDRESS,
            )
            .unwrap(),
        ),
        (
            LOAD_HOOK_SITE_FILE_OFFSET,
            assemble_original_load_site().unwrap(),
        ),
        (
            CLEANUP_HOOK_SITE_FILE_OFFSET,
            assemble_original_cleanup_site().unwrap(),
        ),
        (
            INITIAL_BANK_SEGMENT_LOAD_FILE_OFFSET,
            assemble_bank_segment_load(
                INITIAL_BANK_SEGMENT_LOAD_RUNTIME_ADDRESS,
                ORIGINAL_RESOURCE_SEGMENT_ADDRESS,
            )
            .unwrap(),
        ),
        (
            INITIAL_MAP_POINTER_FILE_OFFSET,
            assemble_map_pointer(INITIAL_MAP_POINTER_RUNTIME_ADDRESS, 0x4a38).unwrap(),
        ),
        (
            STATE_BANK_SEGMENT_LOAD_FILE_OFFSET,
            assemble_bank_segment_load(
                STATE_BANK_SEGMENT_LOAD_RUNTIME_ADDRESS,
                ORIGINAL_RESOURCE_SEGMENT_ADDRESS,
            )
            .unwrap(),
        ),
        (
            STATE_MAP_POINTER_FILE_OFFSET,
            assemble_map_pointer(STATE_MAP_POINTER_RUNTIME_ADDRESS, 0x4a38).unwrap(),
        ),
        (
            TILE_RENDERER_FILE_OFFSET,
            assemble_tile_renderer_entry().unwrap(),
        ),
        (
            INITIAL_TILE_ID_LOAD_FILE_OFFSET,
            assemble_tile_id_load(INITIAL_TILE_ID_LOAD_RUNTIME_ADDRESS).unwrap(),
        ),
        (
            STATE_TILE_ID_LOAD_FILE_OFFSET,
            assemble_tile_id_load(STATE_TILE_ID_LOAD_RUNTIME_ADDRESS).unwrap(),
        ),
    ] {
        source[offset..offset + program.bytes().len()].copy_from_slice(program.bytes());
    }
    source
}

#[test]
fn expanded_bank_uses_one_segment_for_decoded_resources_and_packed_staging() {
    let source = supported_consumer_fixture();

    let patched = install_expanded_portrait_bank(
        &source,
        TEST_PACKED_TILE_BANK_SIZE,
        TEST_PACKED_TILE_MAP_SIZE,
    )
    .unwrap();

    assert!(patched.bytes.len() > source.len());
    assert_eq!(patched.hook_file_offset, source.len());
    assert_eq!(patched.hook_runtime_address, source.len() + COM_ORIGIN);
    assert!(patched.hook_byte_size > 0);
    assert_eq!(&patched.bytes[patched.bytes.len() - 2..], &[0, 0]);

    let hook_origin = u16::try_from(patched.hook_runtime_address).unwrap();
    let paragraphs = portrait_segment_paragraphs(TEST_PACKED_TILE_BANK_SIZE).unwrap();
    let provisional = assemble_hook_body(hook_origin, 0, paragraphs).unwrap();
    let segment_word = hook_origin + u16::try_from(provisional.bytes().len()).unwrap();
    let expected_hook = assemble_hook_body(hook_origin, segment_word, paragraphs).unwrap();
    assert_eq!(
        &patched.bytes[patched.hook_file_offset..patched.hook_file_offset + patched.hook_byte_size],
        expected_hook.bytes()
    );
    assert_eq!(
        &patched.bytes[INITIAL_MAP_POINTER_FILE_OFFSET..INITIAL_MAP_POINTER_FILE_OFFSET + 3],
        assemble_map_pointer(INITIAL_MAP_POINTER_RUNTIME_ADDRESS, PORTRAIT_MAP_OFFSET)
            .unwrap()
            .bytes()
    );
    assert_eq!(
        &patched.bytes[STATE_MAP_POINTER_FILE_OFFSET..STATE_MAP_POINTER_FILE_OFFSET + 3],
        assemble_map_pointer(STATE_MAP_POINTER_RUNTIME_ADDRESS, PORTRAIT_MAP_OFFSET)
            .unwrap()
            .bytes()
    );
    assert_eq!(PORTRAIT_MAP_OFFSET, 0x8000);
    assert_eq!(DECODED_TILE_MAP_SIZE, 264);
    assert_eq!(PACKED_ENTRY_OFFSET, 0x8110);
    let required_byte_size = usize::from(PACKED_ENTRY_OFFSET) + TEST_PACKED_TILE_BANK_SIZE;
    assert_eq!(patched.portrait_segment_byte_size % 16, 0);
    assert!(patched.portrait_segment_byte_size >= required_byte_size);
    assert!(patched.portrait_segment_byte_size < required_byte_size + 16);
    assert!(patched.portrait_segment_byte_size <= 0x10000);
}

#[test]
fn expanded_bank_rejects_an_unknown_select_consumer() {
    let mut source = supported_consumer_fixture();
    source[LOAD_HOOK_SITE_FILE_OFFSET] ^= 0xff;

    let error = install_expanded_portrait_bank(
        &source,
        TEST_PACKED_TILE_BANK_SIZE,
        TEST_PACKED_TILE_MAP_SIZE,
    )
    .unwrap_err();

    assert!(error.to_string().contains("verified typed V30 source"));
}

#[test]
fn expanded_bank_rejects_a_consumer_that_truncates_high_tile_ids() {
    let mut source = supported_consumer_fixture();
    source[STATE_TILE_ID_LOAD_FILE_OFFSET + 1] ^= 0xff;

    let error = install_expanded_portrait_bank(
        &source,
        TEST_PACKED_TILE_BANK_SIZE,
        TEST_PACKED_TILE_MAP_SIZE,
    )
    .unwrap_err();

    assert!(error.to_string().contains("tile-ID load"));
}

#[test]
fn expanded_bank_rejects_packed_staging_that_cannot_fit_one_segment() {
    let source = supported_consumer_fixture();

    let error =
        install_expanded_portrait_bank(&source, 0x8000, TEST_PACKED_TILE_MAP_SIZE).unwrap_err();

    assert!(error.to_string().contains("exceed one segment"));
}
