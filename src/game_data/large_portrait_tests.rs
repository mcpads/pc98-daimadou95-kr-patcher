use super::*;

#[test]
fn shared_portrait_frames_round_trip_through_one_expanded_bank_and_four_maps() {
    let mut frames = std::array::from_fn(|_| vec![4; FRAME_WIDTH * FRAME_HEIGHT]);
    paint_tile(&mut frames[0], 3, 3, 6);
    frames[1] = frames[0].clone();
    frames[2] = frames[0].clone();
    frames[3] = frames[0].clone();
    paint_tile(&mut frames[1], 3, 3, 8);
    paint_tile(&mut frames[2], 3, 3, 9);
    paint_tile(&mut frames[3], 3, 3, 0);

    let encoded = encode_large_portrait_set(&frames, 4).unwrap();
    let decoded =
        decode_large_portrait_set(&encoded.packed_tile_bank, &encoded.packed_tile_maps).unwrap();

    assert_eq!(decoded.frames, frames);
    assert_eq!(encoded.used_tile_count, 5);
    assert_eq!(
        decoded.decoded_tile_bank.len(),
        TILE_BANK_CAPACITY * BYTES_PER_TILE
    );
    assert_eq!(
        decoded.decoded_tile_maps.len(),
        FRAME_COUNT * MAP_RECORD_SIZE
    );
}

#[test]
fn independent_portrait_frames_round_trip_through_all_tile_ids() {
    let mut frames = std::array::from_fn(|_| vec![0; FRAME_WIDTH * FRAME_HEIGHT]);
    for (frame_index, frame) in frames.iter_mut().enumerate() {
        for tile_position in 0..TILES_PER_FRAME {
            let identity = frame_index * TILES_PER_FRAME + tile_position;
            paint_identity_tile(frame, tile_position, identity);
        }
    }

    let encoded = encode_large_portrait_set(&frames, 4).unwrap();
    let decoded =
        decode_large_portrait_set(&encoded.packed_tile_bank, &encoded.packed_tile_maps).unwrap();

    assert_eq!(encoded.used_tile_count, TILE_BANK_CAPACITY);
    assert_eq!(decoded.frames, frames);
    assert_eq!(decoded.used_tile_ids.len(), TILE_BANK_CAPACITY);
}

#[test]
fn decoder_rejects_a_map_that_changes_the_verified_geometry() {
    let tiles = encode_compile_lz(&vec![0; SOURCE_TILE_COUNT * BYTES_PER_TILE]).unwrap();
    let mut maps = Vec::new();
    for _ in 0..FRAME_COUNT {
        maps.extend_from_slice(&[8, 8]);
        maps.extend_from_slice(&[0; TILES_PER_FRAME]);
    }
    maps[0] = 7;
    let maps = encode_compile_lz(&maps).unwrap();

    let error = decode_large_portrait_set(&tiles, &maps).unwrap_err();

    assert!(error.to_string().contains("declares 7x8 tiles"));
}

fn paint_tile(frame: &mut [u8], tile_x: usize, tile_y: usize, color: u8) {
    for y in 0..TILE_HEIGHT {
        let start = (tile_y * TILE_HEIGHT + y) * FRAME_WIDTH + tile_x * TILE_WIDTH;
        frame[start..start + TILE_WIDTH].fill(color);
    }
}

fn paint_identity_tile(frame: &mut [u8], tile_position: usize, identity: usize) {
    let tile_x = tile_position % TILES_PER_ROW;
    let tile_y = tile_position / TILES_PER_ROW;
    paint_tile(frame, tile_x, tile_y, 0);
    for bit in 0..8 {
        let x = tile_x * TILE_WIDTH + bit;
        let y = tile_y * TILE_HEIGHT;
        frame[y * FRAME_WIDTH + x] = u8::from(identity & (1 << bit) != 0);
    }
    frame[tile_y * TILE_HEIGHT * FRAME_WIDTH + tile_x * TILE_WIDTH + 8] = 2;
}
