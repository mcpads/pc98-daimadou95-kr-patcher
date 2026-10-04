use super::*;

#[test]
#[ignore = "requires the title PNGs in assets/title/ and NeoDunggeunmo.ttf with its OFL in assets/fonts/"]
fn title_composition_changes_only_its_declared_graphic_regions() {
    let mut source = vec![9_u8; SCREEN_WIDTH * SCREEN_HEIGHT];
    source[264 * SCREEN_WIDTH + 256] = 11;
    let composition = compose_title_screen(
        &source,
        "대마도전략 이야기 '95",
        "대마도전략 이야기 '95",
        "대마도전략",
    )
    .unwrap();

    assert!(
        composition
            .pixels
            .iter()
            .zip(&source)
            .any(|(candidate, source)| candidate != source)
    );
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if !MAIN_EDITABLE.contains(x, y)
                && !SUBTITLE_EDITABLE.contains(x, y)
                && !is_compact_title_pixel(x, y)
            {
                assert_eq!(
                    composition.pixels[y * SCREEN_WIDTH + x],
                    source[y * SCREEN_WIDTH + x]
                );
            }
        }
    }
}

#[test]
fn malformed_source_pixel_population_is_rejected() {
    let error =
        compose_title_screen(&[0; 16], "대마도전략", "대마도전략", "대마도전략").unwrap_err();

    assert!(error.to_string().contains("instead of"));
}
