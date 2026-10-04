use super::*;

#[test]
fn parses_dynamic_date_suffixes_and_score_selected_messages() {
    let source = synthetic_ending_com();

    let catalog = parse_ending_curry_result(&source).unwrap();

    assert_eq!(catalog.month.suffix_text, "がつ");
    assert_eq!(catalog.month.initial_digit_indices, [0xa5, 0xa4]);
    assert_eq!(catalog.day.suffix_text, "にち");
    assert_eq!(catalog.day.initial_digit_indices, [0xa4, 0xa4]);
    assert_eq!(catalog.messages.len(), 4);
    assert_eq!(
        catalog.messages[0]
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>(),
        ["いつ", "も"]
    );
    assert_eq!(catalog.messages[0].line_break_counts, [2]);
    assert_eq!(catalog.messages[0].runtime_address, 0x3ddb);
    assert_eq!(catalog.consumer.selector_runtime_address, 0x4a5f);
    assert_eq!(catalog.consumer.glyph_renderer_runtime_address, 0x2fda);
}

#[test]
fn rejects_changed_date_mutation_or_message_pointer_contracts() {
    let mut changed_template = synthetic_ending_com();
    changed_template[MONTH_TEMPLATE_FILE_OFFSET] = 0xa4;
    assert!(
        parse_ending_curry_result(&changed_template)
            .unwrap_err()
            .to_string()
            .contains("digit cells")
    );

    let mut changed_pointer = synthetic_ending_com();
    changed_pointer[MESSAGE_POINTER_FILE_OFFSETS[2] + 1] ^= 1;
    assert!(
        parse_ending_curry_result(&changed_pointer)
            .unwrap_err()
            .to_string()
            .contains("message pointer")
    );
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
        assert_eq!(runtime_address, file_offset + COM_ORIGIN);
        let immediate = u16::try_from(runtime_address).unwrap().to_le_bytes();
        bytes[reference_offset..reference_offset + 3].copy_from_slice(&[
            0xbb,
            immediate[0],
            immediate[1],
        ]);
    }
    for (reference_offset, template_offset) in [
        (
            MONTH_TEMPLATE_REFERENCE_FILE_OFFSET,
            MONTH_TEMPLATE_FILE_OFFSET,
        ),
        (DAY_TEMPLATE_REFERENCE_FILE_OFFSET, DAY_TEMPLATE_FILE_OFFSET),
    ] {
        let immediate = u16::try_from(template_offset + COM_ORIGIN)
            .unwrap()
            .to_le_bytes();
        bytes[reference_offset..reference_offset + 3].copy_from_slice(&[
            0xbb,
            immediate[0],
            immediate[1],
        ]);
    }
    for call_offset in GLYPH_RENDERER_CALL_FILE_OFFSETS {
        let displacement = i16::try_from(
            GLYPH_RENDERER_RUNTIME_ADDRESS as isize - (call_offset + COM_ORIGIN + 3) as isize,
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
    let records: [&[u8]; 4] = [
        &[0x10, 0x26, 0xfe, 0xfe, 0x49, 0xff],
        &[0x2f, 0xff],
        &[0x4b, 0xff],
        &[0x00, 0xff],
    ];
    for (offset, record) in MESSAGE_FILE_OFFSETS.into_iter().zip(records) {
        bytes[offset..offset + record.len()].copy_from_slice(record);
    }
    bytes
}
