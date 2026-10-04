use super::*;

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn compact_title_changes_only_its_declared_region() {
    let mut source = vec![BACKGROUND; SCREEN_WIDTH * SCREEN_HEIGHT];
    source[(COMPACT_TITLE.top + 1) * SCREEN_WIDTH + COMPACT_TITLE.left + 1] = TEXT;
    let mut output = source.clone();

    let report = compose_compact_title(&source, &mut output, "대마도전략").unwrap();

    assert_eq!(report.erased_source_pixel_count, 1);
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if !is_compact_title_pixel(x, y) {
                assert_eq!(output[y * SCREEN_WIDTH + x], source[y * SCREEN_WIDTH + x]);
            }
        }
    }
}

#[test]
fn compact_title_rejects_an_unknown_source_palette_index() {
    let source = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut output = source.clone();

    assert!(compose_compact_title(&source, &mut output, "대마도전략").is_err());
}

#[test]
#[ignore = "requires TITLE_COMPACT_MADDAT pointing to a user-owned extracted MADDAT"]
fn extracted_compact_title_binds_the_exact_source_pixel_population() {
    let maddat = std::fs::read(std::env::var("TITLE_COMPACT_MADDAT").unwrap()).unwrap();
    let archive = crate::game_data::flink::parse_flink_archive(&maddat).unwrap();
    let gcs = crate::game_data::gcs::decode_gcs(archive.entries[139].bytes).unwrap();
    let source =
        crate::game_data::planar::decode_planar_indices(&gcs.planes, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();
    let mut output = source.clone();

    let report = compose_compact_title(&source, &mut output, "대마도전략").unwrap();

    assert_eq!(report.erased_source_pixel_count, 889);
}
