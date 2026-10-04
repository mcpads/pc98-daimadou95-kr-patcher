use super::*;

#[test]
fn allocation_expansion_keeps_the_opponent_bank_outside_arles_32_kib() {
    let original = assemble_bank_allocation(SOURCE_BANK_PARAGRAPHS).unwrap();
    let replacement = assemble_bank_allocation(EXPANDED_BANK_PARAGRAPHS).unwrap();
    let tile_id_address = assemble_tile_id_address().unwrap();
    let mut source = vec![0; MAD_TILE_ID_ADDRESS_FILE_OFFSET + tile_id_address.bytes().len()];
    source
        [MAD_BANK_ALLOCATION_FILE_OFFSET..MAD_BANK_ALLOCATION_FILE_OFFSET + original.bytes().len()]
        .copy_from_slice(original.bytes());
    source[MAD_TILE_ID_ADDRESS_FILE_OFFSET..].copy_from_slice(tile_id_address.bytes());

    let patched = expand_portrait_bank_allocation(&source).unwrap();

    assert_eq!(
        &patched[MAD_BANK_ALLOCATION_FILE_OFFSET
            ..MAD_BANK_ALLOCATION_FILE_OFFSET + replacement.bytes().len()],
        replacement.bytes()
    );
}

#[test]
fn allocation_expansion_rejects_an_unknown_mad_consumer() {
    let original = assemble_bank_allocation(SOURCE_BANK_PARAGRAPHS).unwrap();
    let tile_id_address = assemble_tile_id_address().unwrap();
    let mut source = vec![0; MAD_TILE_ID_ADDRESS_FILE_OFFSET + tile_id_address.bytes().len()];
    source
        [MAD_BANK_ALLOCATION_FILE_OFFSET..MAD_BANK_ALLOCATION_FILE_OFFSET + original.bytes().len()]
        .copy_from_slice(original.bytes());
    source[MAD_TILE_ID_ADDRESS_FILE_OFFSET..].copy_from_slice(tile_id_address.bytes());
    source[MAD_TILE_ID_ADDRESS_FILE_OFFSET] ^= 0xff;

    let error = expand_portrait_bank_allocation(&source).unwrap_err();

    assert!(error.to_string().contains("tile-ID address calculation"));
}
