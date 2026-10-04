use super::*;

#[test]
fn game_over_graphic_round_trips_all_planes_and_palette_indices() {
    let pixels = (0..WIDTH * HEIGHT)
        .map(|index| ((index / 31 + index % 13) % 16) as u8)
        .collect::<Vec<_>>();

    let packed = encode_game_over_graphic(&pixels).unwrap();
    let decoded = decode_game_over_graphic(&packed).unwrap();

    assert_eq!(decoded.pixels, pixels);
    assert_eq!(decoded.planar_bytes.len(), WIDTH / 8 * HEIGHT * PLANE_COUNT);
    assert_eq!(decoded.packed_size, packed.len());
}

#[test]
fn game_over_graphic_rejects_the_wrong_decoded_geometry() {
    let packed = encode_compile_lz(&[0; 32]).unwrap();
    let error = decode_game_over_graphic(&packed).unwrap_err();

    assert!(error.to_string().contains("decoded bytes"));
}
