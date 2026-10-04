use std::collections::BTreeMap;

use super::consumer::{
    INTERLUDE_FONT_LOAD_FILE_OFFSET, PASSWORD_FONT_LOAD_FILE_OFFSET, select_ending_font_bank,
};
use super::text::replace_ending_text;

const INTERLUDE_BODY_OFFSET: usize = 0x2610;
const PASSWORD_BODY_OFFSET: usize = 0x3b5a;
const POINTER_TABLE_OFFSET: usize = 0x4916;
const INTERLUDE_TEXT_OFFSET: usize = 0x494d;
const PASSWORD_POOL_OFFSET: usize = 0x4961;
const SELECTOR_OFFSET: usize = 0x4a1c;

#[test]
fn ending_font_consumers_are_typed_and_revision_guarded() {
    let mut source = vec![0_u8; PASSWORD_FONT_LOAD_FILE_OFFSET + 3];
    for offset in [
        INTERLUDE_FONT_LOAD_FILE_OFFSET,
        PASSWORD_FONT_LOAD_FILE_OFFSET,
    ] {
        source[offset..offset + 3].copy_from_slice(&[0xb8, 0x06, 0x00]);
    }

    let patched = select_ending_font_bank(&source).unwrap();

    for offset in patched.font_load_file_offsets {
        assert_eq!(&patched.bytes[offset..offset + 3], &[0xb8, 0x9c, 0x00]);
    }
    source[PASSWORD_FONT_LOAD_FILE_OFFSET + 1] = 7;
    assert!(select_ending_font_bank(&source).is_err());
}

#[test]
fn ending_text_repoints_every_password_and_neutralizes_unused_pool_bytes() {
    let source = synthetic_ending_com();
    let codebook = BTreeMap::from([('가', 0x10), ('나', 0x11)]);
    let passwords = vec!["가".to_owned(); 11];

    let patched = replace_ending_text(&source, "나", &passwords, &codebook).unwrap();

    assert_eq!(patched.password_pool_capacity, 187);
    assert_eq!(patched.password_pool_used, 22);
    assert_eq!(
        &patched.bytes[INTERLUDE_TEXT_OFFSET..INTERLUDE_TEXT_OFFSET + 3],
        &[0x11, 0xff, 0xff]
    );
    for index in 0..11 {
        let pointer_offset = POINTER_TABLE_OFFSET + index * 2;
        let pointer = u16::from_le_bytes(
            patched.bytes[pointer_offset..pointer_offset + 2]
                .try_into()
                .unwrap(),
        );
        assert_eq!(
            usize::from(pointer),
            PASSWORD_POOL_OFFSET + 0x100 + index * 2
        );
        assert_eq!(
            &patched.bytes[PASSWORD_POOL_OFFSET + index * 2..PASSWORD_POOL_OFFSET + index * 2 + 2],
            &[0x10, 0xff]
        );
    }
    assert!(
        patched.bytes[PASSWORD_POOL_OFFSET + patched.password_pool_used..SELECTOR_OFFSET]
            .iter()
            .all(|byte| *byte == 0xff)
    );
}

fn synthetic_ending_com() -> Vec<u8> {
    let mut bytes = vec![0x90; SELECTOR_OFFSET + 1];
    bytes[INTERLUDE_BODY_OFFSET..INTERLUDE_BODY_OFFSET + 14].copy_from_slice(&[
        0xbb, 0x4d, 0x4a, 0xb9, 0x20, 0x00, 0xba, 0x05, 0x00, 0x2e, 0x8e, 0x1e, 0x8d, 0x3d,
    ]);
    bytes[PASSWORD_BODY_OFFSET..PASSWORD_BODY_OFFSET + 16].copy_from_slice(&[
        0xb0, 0x08, 0xe6, 0xa8, 0xb0, 0x0f, 0xe6, 0xaa, 0xb0, 0x0f, 0xe6, 0xac, 0xb0, 0x0f, 0xe6,
        0xae,
    ]);
    bytes[INTERLUDE_FONT_LOAD_FILE_OFFSET..INTERLUDE_FONT_LOAD_FILE_OFFSET + 3]
        .copy_from_slice(&[0xb8, 0x06, 0x00]);
    bytes[0x259a..0x259d].copy_from_slice(&[0xb8, 0x9a, 0x00]);
    bytes[PASSWORD_FONT_LOAD_FILE_OFFSET..PASSWORD_FONT_LOAD_FILE_OFFSET + 3]
        .copy_from_slice(&[0xb8, 0x06, 0x00]);
    for call_offset in [0x262e, 0x3632, 0x3c7e] {
        bytes[call_offset..call_offset + 3].copy_from_slice(&[0xe8, 0x00, 0x00]);
    }
    bytes[PASSWORD_BODY_OFFSET + 0x130] = 0xc3;
    bytes[INTERLUDE_TEXT_OFFSET..INTERLUDE_TEXT_OFFSET + 9]
        .copy_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff]);
    for index in 0..11 {
        let file_offset = PASSWORD_POOL_OFFSET + index * 17;
        let runtime_address = u16::try_from(file_offset + 0x100).unwrap();
        bytes[POINTER_TABLE_OFFSET + index * 2..POINTER_TABLE_OFFSET + index * 2 + 2]
            .copy_from_slice(&runtime_address.to_le_bytes());
        bytes[file_offset..file_offset + 16].fill(0x00);
        bytes[file_offset + 16] = 0xff;
    }
    bytes
}
