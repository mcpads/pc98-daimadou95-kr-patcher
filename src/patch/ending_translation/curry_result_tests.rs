use std::collections::BTreeMap;

use super::*;
use crate::game_data::ending_curry_result::{
    DAY_TEMPLATE_FILE_OFFSET, MESSAGE_FILE_OFFSETS, MESSAGE_POOL_END_FILE_OFFSET,
    MESSAGE_POOL_FILE_OFFSET, MESSAGE_RUNTIME_ADDRESSES, MONTH_TEMPLATE_FILE_OFFSET,
};

const CONSUMER_FILE_OFFSET: usize = 0x28b7;
const CONSUMER_SIGNATURE: &[u8] = &[
    0x2e, 0xa0, 0x5f, 0x4a, 0x3c, 0x00, 0x75, 0x05, 0xbb, 0xdb, 0x3d, 0xeb, 0x1e,
];
const MESSAGE_POINTER_FILE_OFFSETS: [usize; 4] = [0x28bf, 0x28c8, 0x28d1, 0x28da];
const GLYPH_RENDERER_RUNTIME_ADDRESS: usize = 0x2fda;

#[test]
fn replaces_dynamic_suffixes_and_every_fixed_evaluation_record() {
    let source = synthetic_ending_com();
    let translations = translations();
    let codebook = codebook();

    let patched = replace_ending_curry_result(&source, &translations, &codebook).unwrap();

    assert_eq!(
        &patched.bytes[MONTH_TEMPLATE_FILE_OFFSET..MONTH_TEMPLATE_FILE_OFFSET + 5],
        &[0xa5, 0xa4, 0x10, 0xff, 0xff]
    );
    assert_eq!(
        &patched.bytes[DAY_TEMPLATE_FILE_OFFSET..DAY_TEMPLATE_FILE_OFFSET + 5],
        &[0xa4, 0xa4, 0x11, 0xff, 0xff]
    );
    assert_eq!(
        &patched.bytes[MESSAGE_FILE_OFFSETS[0]..MESSAGE_FILE_OFFSETS[0] + 5],
        &[0x12, 0xfe, 0xfe, 0x13, 0xff]
    );
    assert!(
        patched.bytes[MESSAGE_FILE_OFFSETS[0] + 5..MESSAGE_FILE_OFFSETS[2]]
            .iter()
            .all(|byte| *byte == 0xff)
    );
    assert_eq!(patched.report.date_templates_byte_size, 10);
    assert_eq!(
        patched.report.message_pool_capacity,
        MESSAGE_POOL_END_FILE_OFFSET - MESSAGE_POOL_FILE_OFFSET
    );
    assert_eq!(patched.report.message_pool_used, 11);
    assert_eq!(patched.report.message_count, 4);
    assert_eq!(
        &patched.bytes[CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_SIGNATURE.len()],
        CONSUMER_SIGNATURE
    );
}

#[test]
fn rejects_a_changed_date_or_line_shape() {
    let source = synthetic_ending_com();
    let codebook = codebook();

    let mut missing_date = translations();
    missing_date.remove("day-suffix");
    assert!(
        replace_ending_curry_result(&source, &missing_date, &codebook)
            .unwrap_err()
            .to_string()
            .contains("supplies 5 records")
    );

    let mut changed_lines = translations();
    changed_lines.insert("result-class-01".into(), vec!["가".into()]);
    assert!(
        replace_ending_curry_result(&source, &changed_lines, &codebook)
            .unwrap_err()
            .to_string()
            .contains("supplies 1 lines for 2 protected lines")
    );
}

#[test]
fn rejects_output_that_exceeds_its_fixed_record() {
    let source = synthetic_ending_com();
    let codebook = codebook();
    let mut translations = translations();
    translations.insert(
        "result-class-01".into(),
        vec!["가".repeat(30), "나".repeat(30)],
    );

    assert!(
        replace_ending_curry_result(&source, &translations, &codebook)
            .unwrap_err()
            .to_string()
            .contains("fixed record has 57")
    );
}

fn translations() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([
        ("month-suffix".into(), vec!["월".into()]),
        ("day-suffix".into(), vec!["일".into()]),
        ("result-class-01".into(), vec!["가".into(), "나".into()]),
        ("result-class-02".into(), vec!["다".into()]),
        ("result-class-03".into(), vec!["라".into()]),
        ("result-class-04".into(), vec!["가".into()]),
    ])
}

fn codebook() -> BTreeMap<char, u8> {
    BTreeMap::from([
        ('월', 0x10),
        ('일', 0x11),
        ('가', 0x12),
        ('나', 0x13),
        ('다', 0x14),
        ('라', 0x15),
    ])
}

fn synthetic_ending_com() -> Vec<u8> {
    let mut bytes = vec![0x90; DAY_TEMPLATE_FILE_OFFSET + DATE_TEMPLATE_BYTE_SIZE];
    bytes[CONSUMER_FILE_OFFSET..CONSUMER_FILE_OFFSET + CONSUMER_SIGNATURE.len()]
        .copy_from_slice(CONSUMER_SIGNATURE);
    for ((reference_offset, runtime_address), file_offset) in MESSAGE_POINTER_FILE_OFFSETS
        .into_iter()
        .zip(MESSAGE_RUNTIME_ADDRESSES)
        .zip(MESSAGE_FILE_OFFSETS)
    {
        assert_eq!(runtime_address, file_offset + 0x100);
        let immediate = u16::try_from(runtime_address).unwrap().to_le_bytes();
        bytes[reference_offset..reference_offset + 3].copy_from_slice(&[
            0xbb,
            immediate[0],
            immediate[1],
        ]);
    }
    for (reference_offset, template_offset) in [
        (0x28ec, MONTH_TEMPLATE_FILE_OFFSET),
        (0x2936, DAY_TEMPLATE_FILE_OFFSET),
    ] {
        let immediate = u16::try_from(template_offset + 0x100)
            .unwrap()
            .to_le_bytes();
        bytes[reference_offset..reference_offset + 3].copy_from_slice(&[
            0xbb,
            immediate[0],
            immediate[1],
        ]);
    }
    for call_offset in [0x2916, 0x2964, 0x29b3] {
        let displacement = i16::try_from(
            GLYPH_RENDERER_RUNTIME_ADDRESS as isize - (call_offset + 0x100 + 3) as isize,
        )
        .unwrap()
        .to_le_bytes();
        bytes[call_offset..call_offset + 3].copy_from_slice(&[
            0xe8,
            displacement[0],
            displacement[1],
        ]);
    }
    bytes[MONTH_TEMPLATE_FILE_OFFSET..MONTH_TEMPLATE_FILE_OFFSET + DATE_TEMPLATE_BYTE_SIZE]
        .copy_from_slice(&[0xa5, 0xa4, 0x03, 0x26, 0xff]);
    bytes[DAY_TEMPLATE_FILE_OFFSET..DAY_TEMPLATE_FILE_OFFSET + DATE_TEMPLATE_BYTE_SIZE]
        .copy_from_slice(&[0xa4, 0xa4, 0x18, 0x16, 0xff]);

    let mut record_starts = MESSAGE_FILE_OFFSETS.to_vec();
    record_starts.sort_unstable();
    record_starts.push(MESSAGE_POOL_END_FILE_OFFSET);
    for bounds in record_starts.windows(2) {
        bytes[bounds[0]..bounds[1]].fill(0x00);
        bytes[bounds[1] - 1] = 0xff;
    }
    bytes[MESSAGE_FILE_OFFSETS[0] + 1] = 0xfe;
    bytes[MESSAGE_FILE_OFFSETS[0] + 2] = 0xfe;
    bytes
}
