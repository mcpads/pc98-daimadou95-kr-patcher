use super::*;

#[test]
fn replacement_round_trips_only_the_six_consumer_tiles() {
    let mut decoded_tile_bank = vec![0; DECODED_SIZE];
    for (tile_id, tile) in decoded_tile_bank
        .chunks_exact_mut(BYTES_PER_TILE)
        .enumerate()
    {
        tile.fill((tile_id & 0xff) as u8);
    }
    let packed_source = encode_compile_lz(&decoded_tile_bank).unwrap();
    let source = decode_stage_completion_graphic(&packed_source).unwrap();
    let pixels = (0..WIDTH * HEIGHT)
        .map(|offset| ((offset / WIDTH / 4 + offset % WIDTH / 4) % 16) as u8)
        .collect::<Vec<_>>();

    let packed_output = encode_stage_completion_graphic(&source, &pixels).unwrap();
    let output = decode_stage_completion_graphic(&packed_output).unwrap();

    assert_eq!(output.pixels, pixels);
    for tile_id in 0..TILE_COUNT {
        if TILE_IDS.contains(&tile_id) {
            continue;
        }
        let start = tile_id * BYTES_PER_TILE;
        assert_eq!(
            &output.decoded_tile_bank[start..start + BYTES_PER_TILE],
            &source.decoded_tile_bank[start..start + BYTES_PER_TILE]
        );
    }
}

#[test]
fn decoder_rejects_a_tile_bank_with_the_wrong_population() {
    let packed = encode_compile_lz(&vec![0; DECODED_SIZE - BYTES_PER_TILE]).unwrap();

    let error = decode_stage_completion_graphic(&packed).unwrap_err();

    assert!(error.to_string().contains("decoded bytes instead of"));
}
