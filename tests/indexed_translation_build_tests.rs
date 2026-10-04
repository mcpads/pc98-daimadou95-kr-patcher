//! User-owned supported-source build gate for cumulative indexed translations.

use std::env;
use std::fs;
use std::path::Path;

use ds8_daimadou_builder::build_indexed_translation_image;

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn indexed_translation_build_is_repeatable_and_keeps_the_source_immutable() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let original = fs::read(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("indexed-first.hdm");
    let second_path = directory.path().join("indexed-second.hdm");

    let first = build_indexed_translation_image(
        source.as_ref(),
        Path::new("assets/translations"),
        &first_path,
    )
    .unwrap();
    let second = build_indexed_translation_image(
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
    assert_eq!(first.opening.opening_font_entry_id, 155);
    assert_eq!(first.ending.ending_font_entry_id, 156);
    assert_eq!(first.ending.translated_entry_count, 56);
    assert_eq!(first.ending.credits.translated_role_count, 10);
    assert_eq!(first.ending.credits.unique_character_count, 23);
    assert_eq!(first.ending.credits.available_source_slot_count, 30);
    assert_eq!(first.ending.ingredient_table_file_offset, 0x45e1);
    assert_eq!(first.ending.ingredient_label_count, 27);
    assert_eq!(first.ending.curry_result.date_templates_file_offset, 0x44b7);
    assert_eq!(first.ending.curry_result.date_templates_byte_size, 10);
    assert_eq!(first.ending.curry_result.message_pool_file_offset, 0x3c9c);
    assert_eq!(first.ending.curry_result.message_count, 4);
    assert!(
        first.ending.curry_result.message_pool_used
            <= first.ending.curry_result.message_pool_capacity
    );
    assert_eq!(
        first.ending.meal_reaction.glyph_load_file_offsets,
        [0x2748, 0x2765, 0x2782]
    );
    assert_eq!(first.ending.meal_reaction.glyph_slot_count, 3);
    assert_eq!(first.ending.meal_reaction.translated_glyph_count, 2);
    assert!(first.ending.password_pool_used <= first.ending.password_pool_capacity);
    assert_eq!(first.select.first_font_entry_id, 157);
    assert_eq!(first.select.last_font_entry_id, 165);
    assert_eq!(first.select.translated_entry_count, 54);
    assert_eq!(first.select.usable_glyph_count, 240);
    assert_eq!(first.select.hook_site_file_offset, 0x3fc7);
    assert_eq!(first.select.resume_hook_site_file_offset, 0x4017);
    assert_eq!(first.select.hook_file_offset, 0x4eec);
    assert_eq!(first.select.hook_runtime_address, 0x4fec);
    assert_eq!(
        first.select.resume_hook_runtime_address,
        first.select.hook_runtime_address + 11
    );
    assert_eq!(
        first.select.shared_loader_runtime_address,
        first.select.resume_hook_runtime_address + 10
    );
    assert!(first.select.hook_byte_size > 21);
    assert!(first.select.script_pool_used <= first.select.script_pool_capacity);
    assert_eq!(first.mad.translated_entry_count, 240);
    assert_eq!(first.mad.first_font_entry_id, 166);
    assert_eq!(first.mad.last_font_entry_id, 171);
    assert_eq!(
        first
            .mad
            .font_banks
            .iter()
            .map(|bank| (bank.entry_id, bank.unit_numbers.as_slice(), bank.slot_count))
            .collect::<Vec<_>>(),
        vec![
            (166, &[][..], 209),
            (167, &[5, 13, 14][..], 255),
            (168, &[6, 18][..], 250),
            (169, &[3, 4][..], 254),
            (170, &[2, 10, 16][..], 251),
            (171, &[11, 12, 15, 17, 19][..], 237),
        ]
    );
    assert_eq!(
        first
            .mad
            .text_pools
            .iter()
            .map(|pool| (pool.id.as_str(), pool.used, pool.capacity))
            .collect::<Vec<_>>(),
        vec![
            ("numeric-glyphs", 20, 20),
            ("battle-and-menu-text", 1_631, 1_910),
            ("system-and-terrain-text", 149, 164),
        ]
    );
    assert_eq!(
        first
            .mad
            .unit_font_ranges
            .iter()
            .map(|range| (
                range.unit_number,
                range.runtime_start,
                range.runtime_end,
                range.font_entry_id,
            ))
            .collect::<Vec<_>>(),
        vec![
            (2, 0x7af3, 0x7b83, 170),
            (3, 0x7b83, 0x7c19, 169),
            (4, 0x7c19, 0x7c90, 169),
            (5, 0x7c9c, 0x7d45, 167),
            (6, 0x7d45, 0x7dfe, 168),
            (10, 0x7dfe, 0x7e72, 170),
            (11, 0x7e72, 0x7e97, 171),
            (12, 0x7e97, 0x7ebc, 171),
            (13, 0x7ebc, 0x7ef4, 167),
            (14, 0x7ef4, 0x7f16, 167),
            (15, 0x7f16, 0x7f36, 171),
            (16, 0x7f36, 0x7f61, 170),
            (17, 0x7f61, 0x7f86, 171),
            (18, 0x7f86, 0x7fb1, 168),
            (19, 0x7fb1, 0x7fd4, 171),
        ]
    );
    assert_eq!(first.mad.hook_site_file_offset, 0x4db4);
    assert_eq!(first.mad.hook_file_offset, 0xf9a2);
    assert_eq!(first.mad.hook_runtime_address, 0xfaa2);
    assert!(first.mad.hook_byte_size > 0);
    assert!(first.mad.mad_com_output_size + 0x100 <= 0x10000);
}
