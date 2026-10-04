use std::collections::BTreeMap;

use super::*;
use crate::game_data::ending_ingredient_labels::{
    GROUP_COUNT, GROUP_RUNTIME_ADDRESSES, INPUT_STATE_OFFSETS, TABLE_FILE_OFFSET, VARIANT_COUNT,
};

const CONSUMER_FILE_OFFSET: usize = 0x3a28;
const FIXED_CELL_LOOP_FILE_OFFSET: usize = 0x3b1a;
const GLYPH_RENDERER_CALL_FILE_OFFSET: usize = 0x3b29;

#[test]
fn replaces_the_complete_table_with_centered_fixed_width_labels() {
    let source = synthetic_ending_com();
    let translations = (0..GROUP_COUNT)
        .flat_map(|group| {
            (0..VARIANT_COUNT).map(move |variant| {
                (
                    format!("slot-{:02}-variant-{:02}", group + 1, variant + 1),
                    if group == 0 && variant == 0 {
                        "가나".to_owned()
                    } else {
                        "가".to_owned()
                    },
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let codebook = BTreeMap::from([('가', 0x10), ('나', 0x11)]);

    let patched = replace_ending_ingredient_labels(&source, &translations, &codebook).unwrap();

    assert_eq!(patched.table_file_offset, TABLE_FILE_OFFSET);
    assert_eq!(patched.label_count, 27);
    assert_eq!(
        &patched.bytes[TABLE_FILE_OFFSET..TABLE_FILE_OFFSET + 16],
        &[
            0x4d, 0x4d, 0x4d, 0x4d, 0x4d, 0x4d, 0x4d, 0x10, 0x11, 0x4d, 0x4d, 0x4d, 0x4d, 0x4d,
            0x4d, 0x4d,
        ]
    );
    assert_eq!(
        &patched.bytes[TABLE_FILE_OFFSET + 16..TABLE_FILE_OFFSET + 32],
        &[
            0x4d, 0x4d, 0x4d, 0x4d, 0x4d, 0x4d, 0x4d, 0x10, 0x4d, 0x4d, 0x4d, 0x4d, 0x4d, 0x4d,
            0x4d, 0x4d,
        ]
    );
}

#[test]
fn rejects_missing_or_overwide_labels() {
    let source = synthetic_ending_com();
    let mut translations = BTreeMap::new();
    translations.insert("slot-01-variant-01".to_owned(), "가".to_owned());
    let codebook = BTreeMap::from([('가', 0x10)]);
    assert!(
        replace_ending_ingredient_labels(&source, &translations, &codebook)
            .unwrap_err()
            .to_string()
            .contains("supplies 1 labels")
    );

    for group in 0..GROUP_COUNT {
        for variant in 0..VARIANT_COUNT {
            translations.insert(
                format!("slot-{:02}-variant-{:02}", group + 1, variant + 1),
                "가".repeat(17),
            );
        }
    }
    assert!(
        replace_ending_ingredient_labels(&source, &translations, &codebook)
            .unwrap_err()
            .to_string()
            .contains("at most 16")
    );
}

fn synthetic_ending_com() -> Vec<u8> {
    let mut bytes = vec![0x90; TABLE_FILE_OFFSET + TABLE_BYTE_SIZE];
    bytes[CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + 5]
        .copy_from_slice(&[0xbf, 0x39, 0x4a, 0xcd, 0x7d]);
    let load_offsets = [
        0x3a2d, 0x3a44, 0x3a5b, 0x3a72, 0x3a89, 0x3aa0, 0x3ab7, 0x3ace, 0x3ae5,
    ];
    let add_offsets = [
        0x3a3b, 0x3a52, 0x3a69, 0x3a80, 0x3a97, 0x3aae, 0x3ac5, 0x3adc, 0x3af3,
    ];
    for ((load_offset, state_offset), (add_offset, runtime_address)) in load_offsets
        .into_iter()
        .zip(INPUT_STATE_OFFSETS)
        .zip(add_offsets.into_iter().zip(GROUP_RUNTIME_ADDRESSES))
    {
        bytes[load_offset..load_offset + 3].copy_from_slice(&[0x8a, 0x44, state_offset]);
        let immediate = u16::try_from(runtime_address).unwrap().to_le_bytes();
        bytes[add_offset..add_offset + 3].copy_from_slice(&[0x05, immediate[0], immediate[1]]);
    }
    bytes[FIXED_CELL_LOOP_FILE_OFFSET..FIXED_CELL_LOOP_FILE_OFFSET + 3]
        .copy_from_slice(&[0xbf, 0x10, 0x00]);
    bytes[GLYPH_RENDERER_CALL_FILE_OFFSET..GLYPH_RENDERER_CALL_FILE_OFFSET + 3]
        .copy_from_slice(&[0xe8, 0xf8, 0xf3]);
    bytes[TABLE_FILE_OFFSET..TABLE_FILE_OFFSET + TABLE_BYTE_SIZE].fill(0x4d);
    for record in 0..GROUP_COUNT * VARIANT_COUNT {
        bytes[TABLE_FILE_OFFSET + record * RECORD_CELL_COUNT] = 0x00;
    }
    bytes
}
