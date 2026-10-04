use super::*;

#[test]
fn loader_replaces_verified_sites_and_appends_a_typed_body() {
    let mut source = vec![0_u8; 0xf9a2];
    let initializer = assemble_font_initializer(SOURCE_FONT_ENTRY_ID).unwrap();
    source[FONT_INITIALIZER_FILE_OFFSET..FONT_INITIALIZER_FILE_OFFSET + initializer.bytes().len()]
        .copy_from_slice(initializer.bytes());
    let renderer = assemble_original_hook_site().unwrap();
    source[RENDERER_HOOK_SITE_FILE_OFFSET..RENDERER_HOOK_SITE_FILE_OFFSET + renderer.bytes().len()]
        .copy_from_slice(renderer.bytes());
    let ranges = BTreeMap::from([(2, 0x7b00..0x7c00), (3, 0x7c00..0x7d00)]);
    let entries = BTreeMap::from([(2, 167), (3, 168)]);

    let patched = install_mad_font_bank_loader(&source, &ranges, &entries).unwrap();

    assert_eq!(patched.hook_file_offset, source.len());
    assert!(patched.hook_byte_size > 0);
    assert!(patched.bytes.len() > source.len());
    assert_eq!(
        &patched.bytes[FONT_INITIALIZER_FILE_OFFSET..FONT_INITIALIZER_FILE_OFFSET + 3],
        assemble_font_initializer(MAD_FONT_FIRST_ENTRY_ID as u16)
            .unwrap()
            .bytes()
    );
    assert_eq!(
        &patched.bytes[patched.bytes.len() - 2..],
        &(MAD_FONT_FIRST_ENTRY_ID as u16).to_le_bytes()
    );
}
