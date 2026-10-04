use std::collections::BTreeSet;

use super::*;

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn mool_callout_fits_without_compression_and_keeps_mask_palette_roles() {
    let composition = compose_field_unit_callout("모올~!").unwrap();

    assert_eq!(composition.opaque.len(), WIDTH * HEIGHT);
    assert_eq!(composition.palette_indices.len(), WIDTH * HEIGHT);
    assert_eq!(composition.interior_width, 40);
    assert!(composition.interior_height <= MAX_INTERIOR_HEIGHT);
    assert_eq!(
        composition
            .palette_indices
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        [
            TRANSPARENT_PALETTE_INDEX,
            OUTLINE_PALETTE_INDEX,
            INTERIOR_PALETTE_INDEX,
        ]
        .into_iter()
        .collect()
    );
    assert!(
        composition
            .opaque
            .iter()
            .zip(&composition.palette_indices)
            .all(|(opaque, palette_index)| *opaque || *palette_index == TRANSPARENT_PALETTE_INDEX)
    );
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn every_adopted_unit_callout_fits_the_shared_canvas_without_squeezing() {
    for text in [
        "뿌요!",
        "그니까",
        "메롱!",
        "뻐끔~",
        "하오!",
        "후후후",
        "후우",
        "차아~!",
        "가오~!",
        "가~지!",
        "오잇스!",
        "피~쉬!",
        "모올~!",
        "쿠웅!",
        "이얏!",
        "얍~!",
    ] {
        let composition = compose_field_unit_callout(text).unwrap();

        assert!(composition.interior_width <= MAX_INTERIOR_WIDTH, "{text}");
        assert!(composition.interior_height <= MAX_INTERIOR_HEIGHT, "{text}");
        assert!(composition.opaque.iter().any(|pixel| *pixel), "{text}");
        assert!(composition.opaque.iter().any(|pixel| !*pixel), "{text}");
    }
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn overwide_callout_is_rejected_instead_of_being_silently_squeezed() {
    let error = compose_field_unit_callout("우물우물!").unwrap_err();

    assert!(error.to_string().contains("only 46x14 is available"));
}
