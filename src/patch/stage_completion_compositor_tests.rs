use super::*;

fn reviewed_palette_fixture() -> Vec<u8> {
    let mut pixels = vec![BACKGROUND; WIDTH * HEIGHT];
    for (offset, color) in SOURCE_PALETTE.into_iter().skip(1).enumerate() {
        pixels[WIDTH + 1 + offset] = color;
    }
    pixels
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn korean_legend_uses_the_reviewed_source_palette_and_canvas() {
    let source = reviewed_palette_fixture();

    let composition = compose_stage_completion_graphic(&source, "전설").unwrap();

    assert_eq!(composition.pixels.len(), WIDTH * HEIGHT);
    assert_ne!(composition.pixels, source);
    assert!(
        composition
            .pixels
            .iter()
            .all(|pixel| SOURCE_PALETTE.contains(pixel))
    );
    for color in SOURCE_PALETTE {
        assert!(composition.pixels.contains(&color));
    }
}

#[test]
fn compositor_rejects_source_palette_drift_and_overlong_text() {
    let mut source = reviewed_palette_fixture();
    source[WIDTH + 1] = 7;
    assert!(compose_stage_completion_graphic(&source, "전설").is_err());

    let source = reviewed_palette_fixture();
    assert!(compose_stage_completion_graphic(&source, "새로운 전설").is_err());
}
