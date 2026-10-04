use super::*;

fn four_frames() -> [Vec<u8>; FRAME_COUNT] {
    std::array::from_fn(|frame_index| {
        let mut frame = vec![1; FRAME_WIDTH * FRAME_HEIGHT];
        for y in 0..FRAME_HEIGHT {
            for x in 0..FRAME_WIDTH {
                if (x / TILE_WIDTH + y / TILE_HEIGHT + frame_index).is_multiple_of(5) {
                    frame[y * FRAME_WIDTH + x] = ((x + y + frame_index) % 15) as u8;
                }
            }
        }
        frame
    })
}

#[test]
fn split_banks_round_trip_all_four_semantic_frames() {
    let frames = four_frames();

    let encoded = encode_split_ending_meal_scenes(&frames, 1).unwrap();
    let decoded = decode_split_ending_meal_scenes(
        &encoded.packed_tile_banks,
        std::array::from_fn(|index| encoded.map_records[index].as_slice()),
    )
    .unwrap();

    assert_eq!(decoded.frames, frames);
    assert_eq!(
        encoded.map_records.each_ref().map(Vec::len),
        [MAP_RECORD_SIZE; 4]
    );
    assert!(
        encoded
            .used_tile_counts
            .iter()
            .all(|count| *count <= OUTPUT_BANK_TILE_CAPACITY)
    );
}

#[test]
fn split_encoder_rejects_a_pair_that_exceeds_its_physical_bank() {
    let mut frames = std::array::from_fn(|_| vec![1; FRAME_WIDTH * FRAME_HEIGHT]);
    for tile_number in 0..=OUTPUT_BANK_TILE_CAPACITY {
        let frame_index = tile_number / MAP_RECORD_SIZE;
        let tile_position = tile_number % MAP_RECORD_SIZE;
        let tile_x = tile_position % TILES_PER_ROW * TILE_WIDTH;
        let tile_y = tile_position / TILES_PER_ROW * TILE_HEIGHT;
        for nibble in 0..3 {
            frames[frame_index][tile_y * FRAME_WIDTH + tile_x + nibble] =
                ((tile_number >> (nibble * 4)) & 0x0f) as u8;
        }
    }

    let error = encode_split_ending_meal_scenes(&frames, 1).unwrap_err();

    assert!(error.to_string().contains("more than 248 unique tiles"));
}

#[test]
fn decoder_rejects_a_tile_record_without_its_runtime_header() {
    let frames = four_frames();
    let encoded = encode_split_ending_meal_scenes(&frames, 1).unwrap();
    let decoded = decode_complete_compile_lz(&encoded.packed_tile_banks).unwrap();
    let mut invalid = decoded.output;
    invalid[0] ^= 0xff;
    let packed = encode_compile_lz(&invalid).unwrap();

    let error = decode_split_ending_meal_scenes(
        &packed,
        std::array::from_fn(|index| encoded.map_records[index].as_slice()),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("invalid 2-byte-wide, 16-row header")
    );
}
