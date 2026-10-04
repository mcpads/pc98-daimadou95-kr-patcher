use super::*;

#[test]
fn indexed_text_encoding_is_explicit_and_terminated() {
    let codebook = BTreeMap::from([('가', 3), (' ', 8), ('나', 11)]);

    let encoded = encode_text("sample", "가 나", &codebook).unwrap();

    assert_eq!(encoded, [3, 8, 11, 0xff]);
}

#[test]
fn indexed_text_encoding_rejects_an_unmapped_character() {
    let error = encode_text("sample", "가나", &BTreeMap::from([('가', 3)])).unwrap_err();

    assert!(error.to_string().contains("missing '나'"));
}

#[test]
fn pointer_kind_is_not_inferred_from_a_metadata_low_byte_equal_to_mov_si() {
    let bytes = [0xbe, 0x7e, 0x01, 0x00];
    let relocated = BTreeMap::from([(0x7ebe, 0x7000)]);
    let source = MadIndexedTextCatalog {
        mad_com_sha256: String::new(),
        file_size: bytes.len(),
        renderer: crate::source::MadIndexedRendererEvidence {
            file_offset: 0,
            runtime_address: 0,
            decoded_instruction_count: 0,
            isa_profile_id: String::new(),
        },
        pools: Vec::new(),
        strings: Vec::new(),
        bindings: vec![crate::source::MadIndexedBinding {
            id: "metadata".into(),
            consumer: "test".into(),
            reference_file_offsets: vec![0],
            string_ids: Vec::new(),
            content_kind: crate::source::MadIndexedBindingContentKind::TextLines,
            display_text: None,
            contains_japanese: false,
        }],
        unbound_string_ids: Vec::new(),
    };

    let pointers = collect_pointer_replacements(&bytes, &source, &relocated).unwrap();

    assert_eq!(pointers.len(), 1);
    assert!(!pointers[0].machine_code);
    assert_eq!(pointers[0].expected_runtime_address, 0x7ebe);
}
