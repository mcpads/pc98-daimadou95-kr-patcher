use super::*;

fn decode_tiny(input: &[u8], previous_planes: &[Vec<u8>]) -> DecodedPlane {
    decode_plane(
        input,
        0,
        previous_planes,
        PlaneGeometry {
            stride: 2,
            height: 2,
        },
    )
    .unwrap()
}

#[test]
fn commands_follow_the_games_column_major_screen_order() {
    let decoded = decode_tiny(&[0x24, 1, 2, 3, 4], &[]);

    assert_eq!(decoded.bytes, vec![1, 3, 2, 4]);
    assert_eq!(decoded.end_offset, 5);
    assert_eq!(decoded.command_count, 1);
}

#[test]
fn same_plane_and_prior_plane_commands_use_screen_offsets() {
    let same_plane = decode_tiny(&[0x22, 0x00, 0xff, 0x32], &[]);
    assert_eq!(same_plane.bytes, vec![0, 0, 0xff, 0xff]);

    let previous = vec![vec![0x10, 0x20, 0x30, 0x40]];
    let copied = decode_tiny(&[0x44], &previous);
    assert_eq!(copied.bytes, previous[0]);
    let inverted = decode_tiny(&[0x54], &previous);
    assert_eq!(inverted.bytes, vec![0xef, 0xdf, 0xcf, 0xbf]);
}

#[test]
fn paired_rotating_and_pattern_commands_preserve_the_decoder_semantics() {
    let paired = decode_tiny(&[0xB2, 0x11, 0x22, 0x33], &[]);
    assert_eq!(paired.bytes, vec![0x11, 0x11, 0x22, 0x33]);

    let rotating = decode_tiny(&[0xD4, 0x81], &[]);
    assert_eq!(rotating.bytes, vec![0x81, 0x18, 0x06, 0x60]);

    let pattern = decode_tiny(&[0xE2, 0xAA, 0x55], &[]);
    assert_eq!(pattern.bytes, vec![0xAA, 0xAA, 0x55, 0x55]);
}

#[test]
fn screen_decoder_requires_the_exact_header_and_four_complete_planes() {
    let too_short = decode_gcs(b"gcs v1.4\0").unwrap_err();
    assert!(too_short.to_string().contains("64-byte header"));

    let mut missing_planes = vec![0; HEADER_SIZE];
    missing_planes[..SIGNATURE.len()].copy_from_slice(SIGNATURE);
    let error = decode_gcs(&missing_planes).unwrap_err();
    assert!(error.to_string().contains("GCS command at 0x40"));
}

#[test]
fn encoder_preserves_pixels_across_runs_patterns_and_prior_plane_copies() {
    let mut header = [0_u8; HEADER_SIZE];
    header[..SIGNATURE.len()].copy_from_slice(SIGNATURE);
    header[0x20] = 0x5a;
    let first = (0..PLANE_SIZE)
        .map(|offset| [0x00, 0xff, 0x55, 0xaa][(offset / 97) % 4])
        .collect::<Vec<_>>();
    let second = (0..PLANE_SIZE)
        .map(|offset| [0x12, 0x34][offset % 2])
        .collect::<Vec<_>>();
    let third = first.clone();
    let fourth = first.iter().map(|byte| !byte).collect();
    let planes = [first, second, third, fourth];

    let packed = encode_gcs(&header, &planes).unwrap();
    let decoded = decode_gcs(&packed).unwrap();

    assert_eq!(decoded.header, header);
    assert_eq!(decoded.planes, planes);
    assert!(packed.len() < 65_536);
}

#[test]
fn encoder_rejects_an_incomplete_plane() {
    let mut header = [0_u8; HEADER_SIZE];
    header[..SIGNATURE.len()].copy_from_slice(SIGNATURE);
    let mut planes: [Vec<u8>; PLANE_COUNT] = std::array::from_fn(|_| vec![0; PLANE_SIZE]);
    planes[2].pop();

    let error = encode_gcs(&header, &planes).unwrap_err();

    assert!(error.to_string().contains("plane 3"));
}
