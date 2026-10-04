use super::*;

#[test]
fn stage_select_codec_preserves_every_indexed_pixel() {
    let pixels = (0..SCREEN_WIDTH * SCREEN_HEIGHT)
        .map(|offset| ((offset / SCREEN_WIDTH + offset) % 16) as u8)
        .collect::<Vec<_>>();
    let mut header = [0; HEADER_SIZE];
    header[..8].copy_from_slice(b"gcs v1.4");
    let source = StageSelectGraphic {
        header,
        pixels: pixels.clone(),
    };

    let packed = encode_stage_select_graphic(&source, &pixels).unwrap();
    let decoded = decode_stage_select_graphic(&packed).unwrap();

    assert_eq!(decoded.header, source.header);
    assert_eq!(decoded.pixels, pixels);
}

#[test]
fn stage_select_codec_rejects_an_incomplete_canvas() {
    let source = StageSelectGraphic {
        header: [0; HEADER_SIZE],
        pixels: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT],
    };

    assert!(encode_stage_select_graphic(&source, &[0; 16]).is_err());
}
