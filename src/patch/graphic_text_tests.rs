use super::*;

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn text_that_cannot_fit_is_rejected() {
    let mut pixels = vec![0_u8; 32 * 32];
    let error = draw_centered_text(
        &mut pixels,
        32,
        32,
        Rect::new(0, 0, 32, 32),
        "대마도전략",
        TextStyle::plain(2, 5),
    )
    .unwrap_err();

    assert!(error.to_string().contains("does not fit"));
}

#[test]
fn text_renderer_rejects_a_mismatched_canvas() {
    let error = draw_centered_text(
        &mut [0; 15],
        4,
        4,
        Rect::new(0, 0, 4, 4),
        "가",
        TextStyle::plain(1, 1),
    )
    .unwrap_err();

    assert!(error.to_string().contains("canvas"));
}
