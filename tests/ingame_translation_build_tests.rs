//! User-owned supported-source gate for the cumulative in-game Korean build.

use std::env;
use std::fs;
use std::path::Path;

use ds8_daimadou_builder::build_ingame_translation_image;

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn ingame_translation_build_combines_every_game_surface_repeatably() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let original = fs::read(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("ingame-first.hdm");
    let second_path = directory.path().join("ingame-second.hdm");

    let first = build_ingame_translation_image(
        source.as_ref(),
        Path::new("assets/translations"),
        &first_path,
    )
    .unwrap();
    let second = build_ingame_translation_image(
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
    assert_eq!(first.image.file_count, 17);

    assert_eq!(first.opening.opening_font_entry_id, 155);
    assert_eq!(first.ending.ending_font_entry_id, 156);
    assert_eq!(first.ending.translated_entry_count, 56);
    assert_eq!(first.ending.credits.translated_role_count, 10);
    assert_eq!(first.ending.ingredient_table_file_offset, 0x45e1);
    assert_eq!(first.ending.ingredient_label_count, 27);
    assert_eq!(first.ending.curry_result.date_templates_file_offset, 0x44b7);
    assert_eq!(first.ending.curry_result.message_pool_file_offset, 0x3c9c);
    assert_eq!(first.ending.curry_result.message_count, 4);
    assert_eq!(
        first.ending.meal_reaction.glyph_load_file_offsets,
        [0x2748, 0x2765, 0x2782]
    );
    assert_eq!(
        (
            first.select.first_font_entry_id,
            first.select.last_font_entry_id,
            first.select.translated_entry_count,
        ),
        (157, 165, 54)
    );
    assert_eq!(
        (
            first.mad.first_font_entry_id,
            first.mad.last_font_entry_id,
            first.mad.translated_entry_count,
        ),
        (166, 171, 240)
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

    assert_eq!(first.title.maddat_entry_id, 140);
    assert!(first.title.changed_pixel_count > 0);
    assert_eq!(
        first.title.original_menu.packed_input_sha256,
        first.title.original_menu.packed_output_sha256
    );
    assert_eq!(
        first.title.original_menu.opening_input_sha256,
        first.title.original_menu.opening_output_sha256
    );
    assert_eq!(first.game_over.maddat_entry_id, 153);
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
}
