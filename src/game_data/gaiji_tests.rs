use super::*;

fn gaiji_fixture() -> Vec<u8> {
    let mut bytes = vec![0_u8; 0x90];
    bytes[0x0f..0x12].copy_from_slice(&[0xb9, 0x03, 0x00]);
    bytes[0x12..0x19].copy_from_slice(&[0x2e, 0xc7, 0x06, 0x75, 0x01, 0x7d, 0x76]);
    bytes[0x19..0x1c].copy_from_slice(&[0xbb, 0x60, 0x01]);
    bytes[0x33..0x37].copy_from_slice(&[0x81, 0xfa, 0x7e, 0x76]);
    bytes[0x39..0x40].copy_from_slice(&[0x2e, 0xc7, 0x06, 0x75, 0x01, 0x20, 0x77]);
    bytes[0x60..0x66].copy_from_slice(&[0x70, 0x01, 0x78, 0x01, 0x80, 0x01]);
    bytes[0x70..0x78].copy_from_slice(b"first---");
    bytes[0x78..0x80].copy_from_slice(b"second--");
    bytes[0x80..0x90].copy_from_slice(b"third-----------");
    bytes
}

#[test]
fn pointers_and_row_transition_select_glyph_records() {
    let bytes = gaiji_fixture();

    let program = parse_gaiji_program(&bytes).unwrap();

    assert_eq!(program.pointer_table_offset, 0x60);
    assert_eq!(program.glyphs.len(), 3);
    assert_eq!(program.glyphs[0].character_code, 0x767e);
    assert_eq!(program.glyphs[1].character_code, 0x7721);
    assert_eq!(program.glyphs[2].character_code, 0x7722);
    assert_eq!(program.glyphs[0].shift_jis_code, 0xebfc);
    assert_eq!(program.glyphs[1].shift_jis_code, 0xec40);
    assert_eq!(program.glyphs[2].shift_jis_code, 0xec41);
    assert_eq!(program.glyphs[0].bytes, b"first---");
    assert_eq!(program.glyphs[1].bytes, b"second--");
    assert_eq!(program.glyphs[2].bytes, b"third-----------");
}

#[test]
fn non_increasing_glyph_pointers_are_rejected() {
    let mut bytes = gaiji_fixture();
    bytes[0x62..0x64].copy_from_slice(&0x170_u16.to_le_bytes());

    let error = parse_gaiji_program(&bytes).unwrap_err();

    assert!(error.to_string().contains("not strictly increasing"));
}
