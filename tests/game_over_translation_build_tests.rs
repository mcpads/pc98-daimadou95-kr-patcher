//! User-owned supported-source build gate for the tracked Korean game-over graphic.

use std::env;
use std::fs;
use std::path::Path;

use ds8_daimadou_builder::build_game_over_translation_image;

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn reviewed_game_over_build_is_repeatable_and_keeps_the_source_immutable() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let original = fs::read(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("game-over-first.hdm");
    let second_path = directory.path().join("game-over-second.hdm");

    let first = build_game_over_translation_image(
        source.as_ref(),
        Path::new("assets/translations"),
        &first_path,
    )
    .unwrap();
    let second = build_game_over_translation_image(
        source.as_ref(),
        Path::new("assets/translations"),
        &second_path,
    )
    .unwrap();

    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(
        fs::read(first_path).unwrap(),
        fs::read(second_path).unwrap()
    );
    assert_eq!(first, second);
    assert_eq!(first.game_over.segment_id, "graphic-game-over");
    assert_eq!(first.game_over.maddat_entry_id, 153);
    assert!(first.game_over.packed_output_size <= usize::from(u16::MAX));
    assert!(first.game_over.changed_pixel_count > 0);
}
