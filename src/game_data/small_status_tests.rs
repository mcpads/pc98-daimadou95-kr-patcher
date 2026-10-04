use super::*;

#[test]
fn four_independent_status_frames_round_trip_within_the_runtime_bank() {
    let mut frames = std::array::from_fn(|frame_index| {
        vec![u8::try_from(frame_index + 1).unwrap(); FRAME_WIDTH * FRAME_HEIGHT]
    });
    for (frame_index, frame) in frames.iter_mut().enumerate() {
        for tile_position in 0..TILES_PER_FRAME {
            let x = tile_position % TILES_PER_ROW * TILE_WIDTH;
            let y = tile_position / TILES_PER_ROW * TILE_HEIGHT;
            frame[y * FRAME_WIDTH + x] =
                u8::try_from((frame_index * 4 + tile_position) % 16).unwrap();
        }
    }

    let encoded = encode_small_status_set(&frames, 4).unwrap();
    let decoded = decode_small_status_set(
        &encoded.packed_tile_bank,
        std::array::from_fn(|index| encoded.map_records[index].as_slice()),
    )
    .unwrap();

    assert_eq!(decoded.frames, frames);
    assert!(encoded.used_tile_count <= TILE_BANK_CAPACITY);
    assert_eq!(decoded.used_tile_ids.len(), encoded.used_tile_count);
}

#[test]
fn decoder_rejects_a_status_map_with_the_wrong_geometry() {
    let packed = encode_compile_lz(&vec![0; SOURCE_TILE_COUNT * BYTES_PER_TILE]).unwrap();
    let mut maps: [Vec<u8>; FRAME_COUNT] = std::array::from_fn(|_| vec![4; MAP_RECORD_SIZE]);
    for map in &mut maps {
        map[0] = 4;
        map[1] = 4;
        map[2..].fill(0);
    }
    maps[2][0] = 3;

    let error =
        decode_small_status_set(&packed, std::array::from_fn(|index| maps[index].as_slice()))
            .unwrap_err();

    assert!(error.to_string().contains("declares 3x4 tiles"));
}
