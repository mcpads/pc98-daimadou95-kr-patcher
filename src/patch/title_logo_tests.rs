use super::*;

#[test]
#[ignore = "requires the title PNGs in assets/title/"]
fn generated_logo_is_hash_bound_and_paints_only_pc98_palette_roles() {
    let mut canvas = vec![9; 128 * 48];
    let painted =
        paint_imagegen_title_logo(&mut canvas, 128, 48, Rect::new(4, 4, 124, 44), EXACT_TEXT)
            .unwrap();

    assert_eq!(imagegen_title_logo_sha256(), ASSET_SHA256);
    assert!(painted > 1_000);
    assert!(canvas.contains(&BLACK));
    assert!(canvas.contains(&DARK_BLUE));
    assert!(canvas.contains(&RED));
    assert!(
        canvas
            .iter()
            .all(|pixel| [BLACK, DARK_BLUE, RED, 9].contains(pixel))
    );
}

#[test]
fn chroma_key_and_logo_colors_have_distinct_roles() {
    assert_eq!(classify_rgb([0, 255, 0]), None);
    assert_eq!(classify_rgb([12, 220, 18]), None);
    assert_eq!(classify_rgb([220, 30, 40]), Some(RED));
    assert_eq!(classify_rgb([20, 40, 220]), Some(DARK_BLUE));
    assert_eq!(classify_rgb([16, 16, 16]), Some(BLACK));
}

#[test]
fn title_text_change_requires_a_matching_generated_asset() {
    let mut canvas = vec![9; 64 * 32];
    let error =
        paint_imagegen_title_logo(&mut canvas, 64, 32, Rect::new(0, 0, 64, 32), "다른 제목")
            .unwrap_err();

    assert!(error.to_string().contains("is bound to"));
}
