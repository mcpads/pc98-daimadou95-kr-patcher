use super::*;

#[test]
fn ten_frames_keep_the_verified_four_and_six_bank_split() {
    let mut frames = std::array::from_fn(|_| vec![0; FRAME_WIDTH * FRAME_HEIGHT]);
    for (frame_index, frame) in frames.iter_mut().enumerate() {
        let left = 48 + frame_index;
        let top = 32 + frame_index;
        for y in top..top + 64 {
            for x in left..left + 64 {
                frame[y * FRAME_WIDTH + x] = u8::try_from(frame_index % 15 + 1).unwrap();
            }
        }
    }

    let encoded = encode_battle_sprite_set(&frames, 0).unwrap();
    let decoded = decode_battle_sprite_set(
        &encoded.packed_tile_maps,
        [&encoded.packed_tile_banks[0], &encoded.packed_tile_banks[1]],
    )
    .unwrap();

    assert_eq!(decoded.frames, frames);
    assert!(encoded.used_tile_counts.iter().all(|count| *count <= 256));
    assert_eq!(
        decoded.used_tile_ids_by_bank.each_ref().map(BTreeSet::len),
        encoded.used_tile_counts
    );
}

#[test]
fn decoder_rejects_a_frame_that_selects_the_wrong_bank() {
    let frames = std::array::from_fn(|_| vec![0; FRAME_WIDTH * FRAME_HEIGHT]);
    let encoded = encode_battle_sprite_set(&frames, 0).unwrap();
    let mut decoded_maps = decode_complete_compile_lz(&encoded.packed_tile_maps)
        .unwrap()
        .output;
    decoded_maps[2] = 1;
    let invalid_maps = encode_compile_lz(&decoded_maps).unwrap();

    let error = decode_battle_sprite_set(
        &invalid_maps,
        [&encoded.packed_tile_banks[0], &encoded.packed_tile_banks[1]],
    )
    .unwrap_err();

    assert!(error.to_string().contains("selects bank 1 instead of 0"));
}
