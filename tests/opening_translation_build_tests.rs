//! User-owned supported-source build gate for the tracked Korean opening.

use std::env;
use std::fs;
use std::path::Path;

use ds8_daimadou_builder::build_opening_translation_image;

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn reviewed_opening_build_is_repeatable_and_keeps_the_source_immutable() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let original = fs::read(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("opening-first.hdm");
    let second_path = directory.path().join("opening-second.hdm");

    let first = build_opening_translation_image(
        source.as_ref(),
        Path::new("assets/translations"),
        &first_path,
    )
    .unwrap();
    let second = build_opening_translation_image(
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
    assert_eq!(first.opening.segment_id, "opening-sequence");
    assert_eq!(first.opening.translation_entry_id, "opening-text");
    assert_eq!(first.opening.glyph_command_count, 78);
    assert_eq!(first.opening.opening_font_entry_id, 155);
    assert_eq!(first.opening.source_font_entry_id, 6);
}
