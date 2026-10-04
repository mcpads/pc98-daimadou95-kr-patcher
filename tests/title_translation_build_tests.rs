//! User-owned supported-source build gate for the tracked Korean title graphic.

use std::env;
use std::fs;
use std::path::Path;

use ds8_daimadou_builder::build_title_translation_image;

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn reviewed_title_build_is_repeatable_and_keeps_the_source_immutable() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let original = fs::read(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("title-first.hdm");
    let second_path = directory.path().join("title-second.hdm");

    let first = build_title_translation_image(
        source.as_ref(),
        Path::new("assets/translations"),
        &first_path,
    )
    .unwrap();
    let second = build_title_translation_image(
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
    assert_eq!(first.title.title_segment_id, "graphic-title");
    assert_eq!(first.title.maddat_entry_id, 140);
    assert_eq!(first.title.compact_title_text, "대마도전략");
    assert_eq!(first.title.erased_compact_title_pixel_count, 889);
    assert_eq!(first.title.original_menu.maddat_entry_id, 141);
    assert_eq!(
        first.title.original_menu.labels,
        ["START".to_owned(), "CONTINUE".to_owned()]
    );
    assert_eq!(
        first.title.original_menu.sequence_file_offsets,
        [0x36be, 0x36c9]
    );
    assert_eq!(
        first.title.original_menu.packed_input_sha256,
        "12a0de87b545e9c9d762556acfb754e0ffdcea4a5fe3ffbf9e4833574c51e7e5"
    );
    assert_eq!(
        first.title.original_menu.packed_input_sha256,
        first.title.original_menu.packed_output_sha256
    );
    assert_eq!(
        first.title.original_menu.packed_input_size,
        first.title.original_menu.packed_output_size
    );
    assert_eq!(
        first.title.original_menu.opening_input_sha256,
        first.title.original_menu.opening_output_sha256
    );
    assert!(first.title.packed_output_size <= usize::from(u16::MAX));
    assert!(first.title.changed_pixel_count > 0);
    assert_eq!(
        first.title.imagegen_logo_sha256,
        "b0dc12c96a15184dab7ebcb788918e0c80486f47f1564d6fc093ad3153c79722"
    );
    assert_eq!(
        first.title.imagegen_ribbon_sha256,
        "26d49834c571e5f3d23229711bdcc2cbb048dee7c8195f748103e303dcb8ae16"
    );
}
