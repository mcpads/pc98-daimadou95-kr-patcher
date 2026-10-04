use super::*;

#[test]
fn planar_indices_round_trip_every_bit_position_and_palette_index() {
    let pixels = (0_u8..16).cycle().take(16 * 3).collect::<Vec<_>>();

    let planes = encode_planar_indices(&pixels, 16, 3, 4).unwrap();
    let decoded = decode_planar_indices(&planes, 16, 3).unwrap();

    assert_eq!(decoded, pixels);
}

#[test]
fn planar_encoding_rejects_geometry_and_palette_overflow() {
    assert!(encode_planar_indices(&[0; 7], 7, 1, 4).is_err());
    assert!(encode_planar_indices(&[16; 8], 8, 1, 4).is_err());
    assert!(decode_planar_indices(&[vec![0]], 16, 1).is_err());
}
