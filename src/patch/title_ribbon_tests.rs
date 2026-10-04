use super::*;

#[test]
#[ignore = "requires the title PNGs in assets/title/"]
fn generated_ribbon_is_hash_bound_and_paints_only_pc98_palette_roles() {
    let mut canvas = vec![8; 160 * 56];
    let painted =
        paint_imagegen_title_ribbon(&mut canvas, 160, 56, Rect::new(4, 4, 156, 52), EXACT_TEXT)
            .unwrap();

    assert_eq!(imagegen_title_ribbon_sha256(), ASSET_SHA256);
    assert!(painted > 2_000);
    assert!(canvas.contains(&BLACK));
    assert!(canvas.contains(&LIGHT_BLUE));
    assert!(canvas.contains(&RED));
    assert!(canvas.contains(&YELLOW));
    assert!(
        canvas
            .iter()
            .all(|pixel| [RED, BLACK, LIGHT_BLUE, YELLOW, 8].contains(pixel))
    );
}

#[test]
fn ribbon_chroma_key_and_artwork_colors_have_distinct_roles() {
    assert_eq!(classify_rgb([0, 255, 0]), None);
    assert_eq!(classify_rgb([12, 220, 18]), None);
    assert_eq!(classify_rgb([255, 64, 20]), Some(RED));
    assert_eq!(classify_rgb([64, 190, 255]), Some(LIGHT_BLUE));
    assert_eq!(classify_rgb([16, 16, 16]), Some(BLACK));
    assert_eq!(classify_rgb([255, 240, 150]), Some(YELLOW));
}

#[test]
fn ribbon_text_change_requires_a_matching_generated_asset() {
    let mut canvas = vec![9; 64 * 32];
    let error =
        paint_imagegen_title_ribbon(&mut canvas, 64, 32, Rect::new(0, 0, 64, 32), "다른 제목")
            .unwrap_err();

    assert!(error.to_string().contains("is bound to"));
}
