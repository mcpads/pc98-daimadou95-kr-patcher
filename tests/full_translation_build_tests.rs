//! User-owned supported-source gate for the full pre-review Korean build.

use std::env;
use std::fs;
use std::path::Path;

use ds8_daimadou_builder::{
    ARLE_ASSET_FILENAMES, ArleAssetSelection, ArleAssetSet, build_full_translation_image,
    build_full_translation_image_with_arle_asset_selection,
    build_full_translation_image_with_arle_assets,
};

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn full_translation_build_combines_every_target_repeatably() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let original = fs::read(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("full-first.hdm");
    let second_path = directory.path().join("full-second.hdm");
    let selected_assets_path = directory.path().join("arle-assets");
    let selected_assets_output_path = directory.path().join("full-selected-assets.hdm");
    let original_arle_output_path = directory.path().join("full-original-arle.hdm");
    fs::create_dir(&selected_assets_path).unwrap();
    for filename in ARLE_ASSET_FILENAMES {
        fs::copy(
            Path::new("assets/characters/arle").join(filename),
            selected_assets_path.join(filename),
        )
        .unwrap();
    }

    let first = build_full_translation_image(
        source.as_ref(),
        Path::new("assets/translations"),
        &first_path,
    )
    .unwrap();
    let second = build_full_translation_image(
        source.as_ref(),
        Path::new("assets/translations"),
        &second_path,
    )
    .unwrap();
    let selected_assets = build_full_translation_image_with_arle_assets(
        source.as_ref(),
        Path::new("assets/translations"),
        &selected_assets_output_path,
        &ArleAssetSet::from_directory(&selected_assets_path),
    )
    .unwrap();
    let original_arle = build_full_translation_image_with_arle_asset_selection(
        source.as_ref(),
        Path::new("assets/translations"),
        &original_arle_output_path,
        &ArleAssetSelection::preserve_original(),
    )
    .unwrap();

    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(
        fs::read(&first_path).unwrap(),
        fs::read(&second_path).unwrap()
    );
    assert_eq!(first, second);
    assert_eq!(first, selected_assets);
    assert_eq!(
        fs::read(&first_path).unwrap(),
        fs::read(&selected_assets_output_path).unwrap()
    );
    assert_ne!(
        fs::read(&first_path).unwrap(),
        fs::read(&original_arle_output_path).unwrap()
    );
    assert!(original_arle.arle_assets.original_is_preserved());
    assert_ne!(first.image.output_sha256, original_arle.image.output_sha256);
    assert_eq!(first.image.file_count, original_arle.image.file_count);
    assert_eq!(first.image.file_count, 17);

    let translated_target_count = first.media_errors.translated_entry_count
        + 1
        + first.ending.translated_entry_count
        + first.select.translated_entry_count
        + first.mad.translated_entry_count
        + 25;
    assert_eq!(translated_target_count, 384);
    assert_eq!(
        first.changed_runtime_files,
        [
            "ENDING.COM",
            "MAD.COM",
            "MADDAT",
            "OPENING.COM",
            "SELECT.COM"
        ]
    );
    assert_eq!(
        original_arle.changed_runtime_files,
        first.changed_runtime_files
    );
    assert_eq!(first.opening.opening_font_entry_id, 155);
    assert_eq!(first.mad.last_font_entry_id, 171);
    assert_eq!(
        (
            first.mad.result_banner_window.width_instruction_file_offset,
            first
                .mad
                .result_banner_window
                .window_origin_instruction_file_offset,
            first
                .mad
                .result_banner_window
                .text_origin_instruction_file_offset,
            first.mad.result_banner_window.source_inner_tile_columns,
            first.mad.result_banner_window.required_inner_tile_columns,
            first.mad.result_banner_window.output_inner_tile_columns,
            first.mad.result_banner_window.horizontal_shift_tile_columns,
            first.mad.result_banner_window.horizontal_shift_pixels,
        ),
        (0x6165, 0x616b, 0x617d, 14, 16, 16, 1, 16)
    );
    assert_eq!(first.field_unit_callout.changed_bank_count, 10);
    assert_eq!(
        first
            .field_unit_callout
            .callouts
            .iter()
            .map(|callout| (
                callout.maddat_entry_id,
                callout.runtime_unit_class_id,
                callout.tile_ids,
                callout.korean_text.as_str(),
            ))
            .collect::<Vec<_>>(),
        [
            (8, 1, [96, 97, 98], "뿌요!"),
            (8, 2, [99, 100, 101], "그니까"),
            (8, 3, [102, 103, 104], "메롱!"),
            (8, 4, [105, 106, 107], "뻐끔~"),
            (8, 5, [111, 112, 113], "하오!"),
            (8, 6, [108, 109, 110], "후후후"),
            (8, 7, [96, 97, 98], "뿌요!"),
            (8, 8, [96, 97, 98], "뿌요!"),
            (8, 9, [96, 97, 98], "뿌요!"),
            (8, 10, [125, 126, 127], "후우"),
            (111, 11, [125, 126, 127], "차아~!"),
            (112, 12, [125, 126, 127], "가오~!"),
            (113, 13, [125, 126, 127], "가~지!"),
            (114, 14, [125, 126, 127], "오잇스!"),
            (115, 15, [125, 126, 127], "피~쉬!"),
            (116, 16, [125, 126, 127], "모올~!"),
            (117, 17, [125, 126, 127], "쿠웅!"),
            (118, 18, [125, 126, 127], "이얏!"),
            (119, 19, [125, 126, 127], "얍~!"),
        ]
    );
    assert!(
        first
            .field_unit_callout
            .callouts
            .iter()
            .all(|callout| callout.opaque_palette_indices == [1, 9]
                && callout.transparent_palette_indices == [0]
                && callout.changed_pixel_count > 0)
    );
    assert!(first.field_unit_callout.changed_pixel_count > 0);
    assert!(first.title.changed_pixel_count > 0);
    assert_eq!(
        first.title.original_menu.packed_input_sha256,
        first.title.original_menu.packed_output_sha256
    );
    assert_eq!(
        first.title.original_menu.opening_input_sha256,
        first.title.original_menu.opening_output_sha256
    );
    assert!(first.game_over.changed_pixel_count > 0);
    assert_eq!(first.stage_completion.maddat_entry_id, 152);
    assert_eq!(
        first.stage_completion.tile_ids,
        [120, 121, 122, 123, 124, 125]
    );
    assert_eq!(first.stage_completion.protected_tile_count, 122);
    assert!(first.stage_completion.changed_pixel_count > 0);
    assert_eq!(first.stage_select.maddat_entry_id, 5);
    assert_eq!(first.stage_select.korean_text, "스테이지 선택");
    assert_eq!(first.stage_select.erased_source_label_pixel_count, 11_138);
    assert!(first.stage_select.changed_pixel_count > 0);
    let arle = first.arle_assets.replacement().unwrap();
    assert_eq!(
        (
            arle.portrait.tile_bank_entry_id,
            arle.portrait.tile_map_entry_id,
            arle.portrait.frame_count,
        ),
        (101, 129, 4)
    );
    assert_eq!(arle.portrait.output_used_tile_count, 197);
    assert_eq!(arle.portrait.output_tile_bank_capacity, 256);
    assert_eq!(
        arle.portrait.asset_sha256,
        "efdd0537c914a1c31a52c3e879c8b7c89efd1d3b6ecac0aceb2e29186723ecc4"
    );
    assert!(arle.portrait.select_com_output_size > arle.portrait.select_com_input_size);
    let required_segment_size = 0x8110
        + arle
            .portrait
            .output_tile_bank_packed_size
            .max(arle.portrait.output_tile_map_packed_size);
    assert_eq!(arle.portrait.select_portrait_segment_byte_size % 16, 0);
    assert!(arle.portrait.select_portrait_segment_byte_size >= required_segment_size);
    assert!(arle.portrait.select_portrait_segment_byte_size < required_segment_size + 16);
    assert_eq!(
        arle.portrait.mad_com_output_size,
        arle.portrait.mad_com_input_size
    );
    assert_eq!(
        (
            arle.opening_poses.maddat_entry_id,
            arle.opening_poses.pose_count,
            arle.opening_poses.protected_accessory_pixel_count,
        ),
        (143, 3, 16 * 32)
    );
    assert_eq!(arle.opening_poses.packed_input_size, 14_118);
    assert!(arle.opening_poses.packed_output_size <= usize::from(u16::MAX));
    assert!(arle.opening_poses.changed_pixel_count > 0);
    assert_eq!(
        arle.opening_poses.asset_sha256,
        "ffd941b0d5bb7f7d3ddd475b2ff670473f6544591984c02126aa5d23293c8e04"
    );
    assert_eq!(
        (
            arle.small_status.tile_bank_entry_id,
            arle.small_status.map_file_offsets,
            arle.small_status.frame_count,
            arle.small_status.source_used_tile_count,
        ),
        (73, [0x7129, 0x713b, 0x714d, 0x715f], 4, 29)
    );
    assert!(arle.small_status.output_used_tile_count <= 64);
    assert!(arle.small_status.changed_pixel_count > 0);
    assert_eq!(
        arle.small_status.asset_sha256,
        "a6644d7868d6ad60488db7f6bbf62381d99f73dfb039398d5b80cf929b5d20b0"
    );
    assert_eq!(
        (
            arle.battle_sprites.tile_map_entry_id,
            arle.battle_sprites.tile_bank_entry_ids,
            arle.battle_sprites.frame_count,
            arle.battle_sprites.source_used_tile_counts,
        ),
        (16, [38, 39], 10, [256, 256])
    );
    assert!(
        arle.battle_sprites
            .output_used_tile_counts
            .iter()
            .all(|count| *count <= 256)
    );
    assert!(arle.battle_sprites.changed_pixel_count > 0);
    assert_eq!(
        arle.battle_sprites.asset_sha256,
        "abc6f393708efa54511b7a8cb1404979d576fce03c31f8e813475293be347cc7"
    );

    let media_errors = &first.media_errors;
    assert_eq!(media_errors.translated_entry_count, 3);
    assert_eq!(media_errors.input_size, first.mad.mad_com_output_size);
    assert!(media_errors.text_output_size > media_errors.input_size);
    assert_eq!(
        media_errors.installer_hook_file_offset,
        media_errors.text_output_size
    );
    assert!(media_errors.output_size > media_errors.text_output_size);
    assert!(media_errors.output_size + 0x100 <= usize::from(u16::MAX) + 1);
}
