use super::*;

fn source_fixture() -> (Vec<u8>, usize) {
    let mut pixels = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    for rect in [TOP_LABEL, BOTTOM_LABEL] {
        for y in rect.top..rect.bottom {
            for x in rect.left..rect.right {
                pixels[y * SCREEN_WIDTH + x] = if (x / 8 + y / 8).is_multiple_of(2) {
                    4
                } else {
                    5
                };
            }
        }
    }
    let mut source_label_pixel_count = 0;
    for rect in [TOP_LABEL, BOTTOM_LABEL] {
        for y in rect.top + 8..rect.bottom - 8 {
            for x in rect.left + 16..rect.left + 64 {
                if (x + y).is_multiple_of(3) {
                    pixels[y * SCREEN_WIDTH + x] = if x.is_multiple_of(2) {
                        PRIMARY
                    } else {
                        OUTLINE
                    };
                    source_label_pixel_count += 1;
                }
            }
        }
    }
    (pixels, source_label_pixel_count)
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn replaces_both_source_labels_without_touching_the_rest_of_the_screen() {
    let (source, source_label_pixel_count) = source_fixture();
    let composition = compose_stage_select_graphic(&source, "스테이지 선택").unwrap();

    assert_eq!(
        composition.erased_source_label_pixel_count,
        source_label_pixel_count
    );
    assert!(composition.pixels.contains(&PRIMARY));
    assert!(composition.pixels.contains(&OUTLINE));
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if !TOP_LABEL.contains(x, y) && !BOTTOM_LABEL.contains(x, y) {
                assert_eq!(
                    composition.pixels[y * SCREEN_WIDTH + x],
                    source[y * SCREEN_WIDTH + x]
                );
            }
        }
    }
}

#[test]
fn rejects_a_translation_that_cannot_map_to_the_two_panels() {
    let (source, _) = source_fixture();

    assert!(compose_stage_select_graphic(&source, "스테이지").is_err());
    assert!(compose_stage_select_graphic(&source, "스테이지 선택 화면").is_err());
}

#[test]
fn rejects_a_source_without_the_reviewed_label_colors() {
    let source = vec![4; SCREEN_WIDTH * SCREEN_HEIGHT];

    assert!(compose_stage_select_graphic(&source, "스테이지 선택").is_err());
}
