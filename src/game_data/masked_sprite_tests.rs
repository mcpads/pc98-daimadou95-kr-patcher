use super::*;
use crate::game_data::compile_lz::encode_compile_lz;

#[test]
fn mask_plane_zero_bits_select_the_pixels_consumed_by_the_sprite_renderer() {
    let mut record = vec![0xff; BYTES_PER_TILE];
    for plane in 0..COLOR_PLANE_COUNT {
        record[PLANE_SIZE + plane * PLANE_SIZE..PLANE_SIZE + (plane + 1) * PLANE_SIZE].fill(0);
    }
    let x = 3;
    let y = 4;
    let byte_offset = y * (TILE_WIDTH / 8) + x / 8;
    let bit = 1 << (7 - x % 8);
    record[byte_offset] &= !bit;
    record[PLANE_SIZE + byte_offset] |= bit;
    record[PLANE_SIZE * 3 + byte_offset] |= bit;

    let packed = encode_compile_lz(&record).unwrap();
    let bank = decode_masked_sprite_bank(&packed).unwrap();
    let tile = &bank.tiles[0];
    let pixel_offset = y * TILE_WIDTH + x;

    assert!(tile.opaque[pixel_offset]);
    assert_eq!(tile.palette_indices[pixel_offset], 0b0101);
    assert!(!tile.opaque[0]);
}

#[test]
fn decoder_rejects_a_planar_128_byte_slice_as_a_masked_sprite_record() {
    let packed = encode_compile_lz(&[0; 128]).unwrap();

    let error = decode_masked_sprite_bank(&packed).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("160-byte mask-plus-color record")
    );
}

#[test]
fn encoded_mask_and_palette_planes_round_trip_every_pixel_role() {
    let mut tile = MaskedSpriteTile {
        opaque: vec![false; TILE_WIDTH * TILE_HEIGHT],
        palette_indices: vec![0; TILE_WIDTH * TILE_HEIGHT],
    };
    for y in 0..TILE_HEIGHT {
        for x in 0..TILE_WIDTH {
            let offset = y * TILE_WIDTH + x;
            tile.opaque[offset] = (x + y) % 3 != 0;
            tile.palette_indices[offset] = ((x + y * 3) % 16) as u8;
        }
    }

    let record = encode_masked_sprite_tile(&tile).unwrap();
    let packed = encode_compile_lz(&record).unwrap();
    let decoded = decode_masked_sprite_bank(&packed).unwrap();

    assert_eq!(decoded.tiles, [tile]);
    assert_eq!(decoded.decoded, record);
}

#[test]
fn encoder_rejects_incomplete_pixels_and_out_of_plane_palette_indices() {
    let incomplete = MaskedSpriteTile {
        opaque: vec![false; TILE_WIDTH * TILE_HEIGHT - 1],
        palette_indices: vec![0; TILE_WIDTH * TILE_HEIGHT],
    };
    assert!(encode_masked_sprite_tile(&incomplete).is_err());

    let mut out_of_range = MaskedSpriteTile {
        opaque: vec![true; TILE_WIDTH * TILE_HEIGHT],
        palette_indices: vec![0; TILE_WIDTH * TILE_HEIGHT],
    };
    out_of_range.palette_indices[17] = 16;
    assert!(encode_masked_sprite_tile(&out_of_range).is_err());
}
