use super::*;

fn source_consumer() -> Vec<u8> {
    let destination = assemble_packed_destination(OUTPUT_SEGMENT_WORD_ADDRESS).unwrap();
    let unpack = assemble_source_unpack_block().unwrap();
    let mut bytes = vec![0xa5; UNPACK_BLOCK_FILE_OFFSET + unpack.bytes().len() + 32];
    bytes[PACKED_DESTINATION_FILE_OFFSET
        ..PACKED_DESTINATION_FILE_OFFSET + destination.bytes().len()]
        .copy_from_slice(destination.bytes());
    bytes[UNPACK_BLOCK_FILE_OFFSET..UNPACK_BLOCK_FILE_OFFSET + unpack.bytes().len()]
        .copy_from_slice(unpack.bytes());
    bytes
}

#[test]
fn loader_keeps_packed_and_decoded_banks_in_separate_existing_segments() {
    let source = source_consumer();

    let patched = install_ending_meal_bank_loader(&source, PACKED_STAGING_CAPACITY).unwrap();

    assert_eq!(patched.bytes.len(), source.len());
    assert_eq!(patched.packed_staging_capacity, PACKED_STAGING_CAPACITY);
    assert_eq!(
        &patched.bytes[PACKED_DESTINATION_FILE_OFFSET
            ..PACKED_DESTINATION_FILE_OFFSET
                + assemble_packed_destination(STAGING_SEGMENT_WORD_ADDRESS)
                    .unwrap()
                    .bytes()
                    .len()],
        assemble_packed_destination(STAGING_SEGMENT_WORD_ADDRESS)
            .unwrap()
            .bytes()
    );
    let first = v30::decode_bytes(&patched.bytes[UNPACK_BLOCK_FILE_OFFSET..]).unwrap();
    assert_eq!(
        first.instruction,
        Instruction::Push {
            src: Operand::Sreg(SegmentRegister::DS)
        }
    );
}

#[test]
fn loader_rejects_a_packed_entry_that_cannot_fit_the_reused_staging_segment() {
    let error = install_ending_meal_bank_loader(&source_consumer(), PACKED_STAGING_CAPACITY + 1)
        .unwrap_err();

    assert!(error.to_string().contains("reusable staging segment"));
}

#[test]
fn loader_rejects_source_code_drift_before_writing() {
    let mut source = source_consumer();
    source[UNPACK_BLOCK_FILE_OFFSET] ^= 0xff;

    let error = install_ending_meal_bank_loader(&source, 1).unwrap_err();

    assert!(error.to_string().contains("verified typed V30 source"));
}
