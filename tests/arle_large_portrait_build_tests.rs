//! User-owned supported-source build gate for the tracked Arle large-portrait source asset.

use std::env;
use std::fs;

use ds8_daimadou_builder::build_arle_large_portrait_image;

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the tracked Arle PNG set in assets/characters/arle/"]
fn arle_large_portrait_build_is_repeatable_and_keeps_the_source_immutable() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let original = fs::read(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("arle-portrait-first.hdm");
    let second_path = directory.path().join("arle-portrait-second.hdm");

    let first = build_arle_large_portrait_image(source.as_ref(), &first_path).unwrap();
    let second = build_arle_large_portrait_image(source.as_ref(), &second_path).unwrap();

    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(
        fs::read(first_path).unwrap(),
        fs::read(second_path).unwrap()
    );
    assert_eq!(first, second);
    assert_eq!(first.portrait.tile_bank_entry_id, 101);
    assert_eq!(first.portrait.tile_map_entry_id, 129);
    assert_eq!(first.portrait.frame_count, 4);
    assert_eq!(first.portrait.source_used_tile_count, 88);
    assert_eq!(first.portrait.output_used_tile_count, 197);
    assert_eq!(first.portrait.output_tile_bank_capacity, 256);
    assert!(first.portrait.changed_pixel_count > 0);
    assert_eq!(
        first.portrait.asset_sha256,
        "efdd0537c914a1c31a52c3e879c8b7c89efd1d3b6ecac0aceb2e29186723ecc4"
    );
    assert!(first.portrait.select_com_output_size > first.portrait.select_com_input_size);
    assert_eq!(
        first.portrait.select_hook_file_offset,
        first.portrait.select_com_input_size
    );
    assert!(first.portrait.select_hook_byte_size > 0);
    let required_segment_size = 0x8110
        + first
            .portrait
            .output_tile_bank_packed_size
            .max(first.portrait.output_tile_map_packed_size);
    assert_eq!(first.portrait.select_portrait_segment_byte_size % 16, 0);
    assert!(first.portrait.select_portrait_segment_byte_size >= required_segment_size);
    assert!(first.portrait.select_portrait_segment_byte_size < required_segment_size + 16);
    assert_eq!(
        first.portrait.select_hook_runtime_address,
        first.portrait.select_hook_file_offset + 0x100
    );
    assert_eq!(
        first.portrait.mad_com_output_size,
        first.portrait.mad_com_input_size
    );
    assert_eq!(first.portrait.mad_bank_allocation_file_offset, 0x012f);
}
