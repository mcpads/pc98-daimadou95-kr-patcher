use super::*;

#[test]
fn decoder_matches_the_games_literal_overlap_and_prefill_behavior() {
    let stream = decode_complete_compile_lz(&[
        2, b'A', b'B', // literal AB
        0x82, 1, // copy five bytes from distance two
        0,
    ])
    .unwrap();

    assert_eq!(stream.output, b"ABABABA");
    assert_eq!(stream.packed_size, 6);
    assert_eq!(stream.command_count, 3);

    let prefill = decode_complete_compile_lz(&[0x80, 0, 0]).unwrap();
    assert_eq!(prefill.output, vec![0; 3]);
}

#[test]
fn malformed_or_concatenated_entries_are_not_accepted_as_one_asset() {
    let truncated = decode_complete_compile_lz(&[3, b'A', b'B']).unwrap_err();
    assert!(truncated.to_string().contains("literal at 0x0"));

    let trailing = decode_complete_compile_lz(&[1, b'A', 0, 0]).unwrap_err();
    assert!(trailing.to_string().contains("leaving 1 trailing bytes"));
}

#[test]
fn encoder_round_trips_literals_long_runs_and_overlapping_patterns() {
    let mut source = (0_u8..=255).collect::<Vec<_>>();
    source.extend(std::iter::repeat_n(0x5a, 300));
    source.extend_from_slice(b"ABABABABABABABABABAB");

    let packed = encode_compile_lz(&source).unwrap();
    let decoded = decode_complete_compile_lz(&packed).unwrap();

    assert_eq!(decoded.output, source);
    assert_eq!(decoded.packed_size, packed.len());
}

#[test]
fn encoder_uses_back_references_for_repeated_bitmap_data() {
    let source = vec![0_u8; 8192];

    let packed = encode_compile_lz(&source).unwrap();

    assert!(packed.len() < 200);
    assert!(packed.iter().any(|command| *command >= 0x80));
    assert_eq!(decode_complete_compile_lz(&packed).unwrap().output, source);
}
