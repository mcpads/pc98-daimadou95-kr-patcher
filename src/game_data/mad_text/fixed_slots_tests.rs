use super::{POINTER_TABLE_FILE_OFFSET, SLOT_COUNT, SLOT_SIZE, parse_fixed_text_slots};

#[test]
fn paired_pointers_select_five_contiguous_reversible_slots() {
    let first_slot_offset = POINTER_TABLE_FILE_OFFSET + SLOT_COUNT * 4;
    let mut bytes = vec![0_u8; first_slot_offset + SLOT_COUNT * SLOT_SIZE];
    for index in 0..SLOT_COUNT {
        let address = u16::try_from(first_slot_offset + index * SLOT_SIZE + 0x100).unwrap();
        let pointer_offset = POINTER_TABLE_FILE_OFFSET + index * 4;
        bytes[pointer_offset..pointer_offset + 2].copy_from_slice(&address.to_le_bytes());
        bytes[pointer_offset + 2..pointer_offset + 4].copy_from_slice(&address.to_le_bytes());

        let slot_offset = first_slot_offset + index * SLOT_SIZE;
        let mut slot = Vec::new();
        for row in 0..6 {
            slot.extend_from_slice(b"$3");
            slot.extend_from_slice(&[0x81, 0x40].repeat(6));
            slot.extend_from_slice(if row == 5 { b"$$" } else { b"$0" });
        }
        bytes[slot_offset..slot_offset + SLOT_SIZE].copy_from_slice(&slot);
    }

    let slots = parse_fixed_text_slots(&bytes).unwrap();

    assert_eq!(slots.len(), SLOT_COUNT);
    assert_eq!(slots[0].file_offset, first_slot_offset);
    assert_eq!(slots[4].file_offset, first_slot_offset + 4 * SLOT_SIZE);
}

#[test]
fn mismatched_pointer_pair_is_rejected() {
    let mut bytes = vec![0_u8; POINTER_TABLE_FILE_OFFSET + SLOT_COUNT * 4];
    bytes[POINTER_TABLE_FILE_OFFSET..POINTER_TABLE_FILE_OFFSET + 2]
        .copy_from_slice(&0x100_u16.to_le_bytes());
    bytes[POINTER_TABLE_FILE_OFFSET + 2..POINTER_TABLE_FILE_OFFSET + 4]
        .copy_from_slice(&0x101_u16.to_le_bytes());

    let error = parse_fixed_text_slots(&bytes).unwrap_err();

    assert!(error.to_string().contains("does not select the same slot"));
}
