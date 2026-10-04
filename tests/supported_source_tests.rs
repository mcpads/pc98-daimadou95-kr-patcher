//! User-owned supported-source integration gate.

use std::env;
use std::fs;

use ds8_daimadou_builder::{
    LocalizationRenderPath, build_hangul_probe_image, build_standalone_image, survey_source_path,
};

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and NeoDunggeunmo.ttf with its OFL in assets/fonts/"]
fn supported_source_build_is_repeatable_and_does_not_change_the_input() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let original = fs::read(&source).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("first.hdm");
    let second_path = directory.path().join("second.hdm");

    let first_report = build_standalone_image(source.as_ref(), &first_path).unwrap();
    let second_report = build_standalone_image(source.as_ref(), &second_path).unwrap();

    assert_eq!(fs::read(&source).unwrap(), original);
    assert_eq!(
        fs::read(first_path).unwrap(),
        fs::read(second_path).unwrap()
    );
    assert_eq!(first_report, second_report);
    assert_eq!(first_report.file_count, 17);

    let survey = survey_source_path(source.as_ref()).unwrap();
    assert_eq!(survey.localization.summary.declared_runtime_file_count, 17);
    assert_eq!(survey.localization.summary.audited_runtime_file_count, 17);
    assert_eq!(survey.localization.summary.declared_maddat_entry_count, 154);
    assert_eq!(survey.localization.summary.audited_maddat_entry_count, 154);
    assert_eq!(
        survey
            .localization
            .summary
            .identified_source_text_item_count,
        456
    );
    assert_eq!(survey.localization.summary.target_text_item_count, 384);
    assert_eq!(
        survey.localization_strategy.summary.target_item_count,
        survey.localization.summary.target_text_item_count
    );
    assert_eq!(
        survey.localization_strategy.summary.assigned_item_count,
        384
    );
    assert_eq!(
        survey.localization_strategy.summary.unassigned_item_count,
        0
    );
    let render_path_count = |expected| {
        survey
            .localization_strategy
            .render_paths
            .iter()
            .find(|path| path.path == expected)
            .map(|path| path.target_item_count)
    };
    assert_eq!(
        render_path_count(LocalizationRenderPath::SharedIndexedFont),
        Some(346)
    );
    assert_eq!(
        render_path_count(LocalizationRenderPath::EndingCreditFont),
        Some(10)
    );
    assert_eq!(
        render_path_count(LocalizationRenderPath::DosSystemText),
        Some(3)
    );
    assert_eq!(
        render_path_count(LocalizationRenderPath::PlanarGraphics),
        Some(25)
    );
    assert_eq!(survey.localization.summary.excluded_text_item_count, 72);
    assert_eq!(survey.localization.summary.unresolved_text_item_count, 0);
    assert!(survey.localization.files.iter().all(|file| !matches!(
        file.status,
        ds8_daimadou_builder::LocalizationFileStatus::Unresolved
    )));
    assert!(survey.localization.maddat_entries.iter().all(|entry| {
        !matches!(
            entry.status,
            ds8_daimadou_builder::LocalizationMaddatStatus::Unresolved
        )
    }));
    assert_eq!(
        survey
            .localization
            .maddat_entries
            .iter()
            .filter(|entry| matches!(
                entry.status,
                ds8_daimadou_builder::LocalizationMaddatStatus::TargetData
            ))
            .map(|entry| entry.entry_id)
            .collect::<Vec<_>>(),
        [
            5, 8, 111, 112, 113, 114, 115, 116, 117, 118, 119, 140, 142, 148, 152, 153,
        ]
    );
    let outside_gameplay_scope = survey
        .localization
        .source_text
        .iter()
        .filter(|item| {
            item.reason
                == ds8_daimadou_builder::LocalizationTextReason::OutsideGameplayLocalizationScope
        })
        .collect::<Vec<_>>();
    assert_eq!(outside_gameplay_scope.len(), 60);
    for (filename, expected_count) in [
        ("MST.COM", 6),
        ("FPLAY.COM", 20),
        ("BPLAY.COM", 20),
        ("BSAMP.COM", 3),
        ("NMOUSE.COM", 11),
    ] {
        assert_eq!(
            outside_gameplay_scope
                .iter()
                .filter(|item| item.source_filename == filename)
                .count(),
            expected_count
        );
    }
    assert_eq!(
        survey
            .localization
            .source_text
            .iter()
            .filter(|item| item.reason
                == ds8_daimadou_builder::LocalizationTextReason::OriginalPresentationPreserved)
            .count(),
        2
    );
    assert_eq!(
        survey
            .localization
            .source_text
            .iter()
            .filter(|item| item.reason
                == ds8_daimadou_builder::LocalizationTextReason::DormantNoConsumerReference)
            .count(),
        10
    );
    assert_eq!(survey.maddat.entries.len(), 154);
    assert_eq!(survey.maddat.table_end_offset, 0x3ac);
    assert_eq!(survey.maddat.first_entry_offset, Some(0x3ac));
    assert_eq!(survey.maddat.last_entry_end_offset, Some(919_585));
    assert!(survey.maddat.entries_are_contiguous);
    assert_eq!(
        survey
            .maddat
            .entries
            .iter()
            .filter(|entry| matches!(
                entry.storage,
                ds8_daimadou_builder::MaddatStorage::CompileLz { .. }
            ))
            .count(),
        132
    );
    assert_eq!(
        survey
            .maddat
            .entries
            .iter()
            .filter_map(|entry| match entry.storage {
                ds8_daimadou_builder::MaddatStorage::MaskedSpriteBank { tile_count, .. } =>
                    Some((entry.id, tile_count)),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            (7, 256),
            (8, 128),
            (9, 256),
            (111, 128),
            (112, 128),
            (113, 128),
            (114, 128),
            (115, 128),
            (116, 128),
            (117, 128),
            (118, 128),
            (119, 128),
        ]
    );
    assert_eq!(
        survey
            .maddat
            .entries
            .iter()
            .filter(|entry| matches!(
                entry.storage,
                ds8_daimadou_builder::MaddatStorage::GcsScreen { .. }
            ))
            .count(),
        5
    );
    assert_eq!(
        survey
            .maddat
            .entries
            .iter()
            .filter(|entry| matches!(
                entry.storage,
                ds8_daimadou_builder::MaddatStorage::OpeningTextScript { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        survey
            .maddat
            .entries
            .iter()
            .filter(|entry| matches!(
                entry.storage,
                ds8_daimadou_builder::MaddatStorage::EndingCreditTextGrid { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        survey
            .maddat
            .entries
            .iter()
            .filter(|entry| matches!(
                entry.storage,
                ds8_daimadou_builder::MaddatStorage::EndingCreditMotionTrack { .. }
            ))
            .count(),
        3
    );
    assert_eq!(
        survey
            .maddat
            .entries
            .iter()
            .filter(|entry| matches!(
                entry.storage,
                ds8_daimadou_builder::MaddatStorage::Unresolved { .. }
            ))
            .count(),
        0
    );
    assert_eq!(survey.gaiji.glyphs.len(), 175);
    assert_eq!(survey.gaiji.first_character_code, Some(0x7622));
    assert_eq!(survey.gaiji.last_character_code, Some(0x7772));
    assert_eq!(survey.gaiji.first_shift_jis_code, Some(0xeba0));
    assert_eq!(survey.gaiji.uniform_glyph_size, Some(34));
    assert_eq!(survey.mad_text.renderer.file_offset, 0x80e0);
    assert_eq!(survey.mad_text.renderer.runtime_address, 0x81e0);
    assert_eq!(survey.mad_text.fixed_slots.len(), 5);
    assert!(
        survey
            .mad_text
            .fixed_slots
            .iter()
            .all(|slot| slot.byte_size == 0x60)
    );
    assert_eq!(survey.indexed_font.maddat_entry_id, 6);
    assert_eq!(survey.indexed_font.decoded_size, 8192);
    assert_eq!(survey.indexed_font.glyph_count, 256);
    assert_eq!(survey.indexed_font.glyph_size, 32);
    assert_eq!(survey.select_dialogue.interpreter.file_offset, 0x2082);
    assert_eq!(survey.select_dialogue.interpreter.runtime_address, 0x2182);
    assert_eq!(survey.select_dialogue.pointer_table_file_offset, 0x42a8);
    assert_eq!(survey.select_dialogue.stages.len(), 9);
    assert!(
        survey
            .select_dialogue
            .stages
            .iter()
            .all(|stage| stage.scripts.len() == 6)
    );
    assert_eq!(
        survey.select_dialogue.stages[0].scripts[0].runtime_address,
        0x4414
    );
    assert!(
        survey
            .select_dialogue
            .stages
            .iter()
            .flat_map(|stage| &stage.scripts)
            .any(|script| script.text.contains("こしひかり"))
    );
    assert!(
        survey
            .select_dialogue
            .stages
            .iter()
            .flat_map(|stage| &stage.scripts)
            .any(|script| script.text.contains("カレ～ル～"))
    );
    assert_eq!(survey.opening_text.maddat_entry_id, 142);
    assert_eq!(survey.opening_text.byte_size, 469);
    assert_eq!(
        survey.opening_text.consumer.load_reference_file_offset,
        0x2890
    );
    assert_eq!(survey.opening_text.consumer.call_site_file_offset, 0x2b2c);
    assert_eq!(survey.opening_text.consumer.interpreter_file_offset, 0x2d5e);
    assert_eq!(
        survey.opening_text.consumer.interpreter_runtime_address,
        0x2e5e
    );
    assert!(survey.opening_text.text.contains("カレ～のざいりょうが"));
    assert!(survey.opening_text.text.contains("がんばるぞ！ うん！"));
    assert_eq!(survey.graphic_text.surfaces.len(), 24);
    let callouts = survey
        .graphic_text
        .surfaces
        .iter()
        .filter(|surface| surface.id.starts_with("field-unit-callout-"))
        .collect::<Vec<_>>();
    assert_eq!(
        callouts
            .iter()
            .map(|surface| (surface.maddat_entry_id, surface.reviewed_text[0].as_str()))
            .collect::<Vec<_>>(),
        [
            (8, "ぷよっ"),
            (8, "じゃけ"),
            (8, "べろっ"),
            (8, "ぎょ～"),
            (8, "ござる"),
            (8, "フフフー"),
            (8, "ぷよっ"),
            (8, "ぷよっ"),
            (8, "ぷよっ"),
            (8, "はふぅ"),
            (111, "おちゃー"),
            (112, "がおー"),
            (113, "なーす"),
            (114, "おいっす"),
            (115, "ふぃー"),
            (116, "もぐう"),
            (117, "ごどお"),
            (118, "とりゃ"),
            (119, "てやー!"),
        ]
    );
    assert!(callouts.iter().all(|surface| {
        if surface.width != 48 || surface.height != 16 || surface.plane_count != 5 {
            return false;
        }
        let ds8_daimadou_builder::GraphicTextConsumerEvidence::FieldUnitCallout {
            runtime_unit_class_id,
            map_file_offset,
            map_runtime_address,
            tile_ids,
            renderer_runtime_address: 0x5409,
            pointer_table_runtime_address: 0x546a,
            ..
        } = &surface.consumer
        else {
            return false;
        };
        let (expected_map_file_offset, expected_tile_ids) = match *runtime_unit_class_id {
            1 | 7 | 8 | 9 => (0x740d, &[96, 97, 98][..]),
            2 => (0x7410, &[99, 100, 101][..]),
            3 => (0x7413, &[102, 103, 104][..]),
            4 => (0x7416, &[105, 106, 107][..]),
            5 => (0x7419, &[111, 112, 113][..]),
            6 => (0x741c, &[108, 109, 110][..]),
            _ => (0x741f, &[125, 126, 127][..]),
        };
        (if *runtime_unit_class_id <= 10 {
            8
        } else {
            *runtime_unit_class_id + 100
        }) == surface.maddat_entry_id
            && *map_file_offset == expected_map_file_offset
            && *map_runtime_address == expected_map_file_offset + 0x100
            && tile_ids == expected_tile_ids
    }));
    let arle_callout = callouts
        .iter()
        .find(|surface| {
            matches!(
                surface.consumer,
                ds8_daimadou_builder::GraphicTextConsumerEvidence::FieldUnitCallout {
                    runtime_unit_class_id: 10,
                    ..
                }
            )
        })
        .unwrap();
    assert!(matches!(
        &arle_callout.consumer,
        ds8_daimadou_builder::GraphicTextConsumerEvidence::FieldUnitCallout {
            runtime_unit_class_id: 10,
            bank_load_block_file_offset: 0x1336,
            bank_load_block_runtime_address: 0x1436,
            bank_loader_call_file_offset: 0x1345,
            ..
        }
    ));
    assert!(survey.graphic_text.surfaces.iter().any(|surface| {
        surface.id == "stage-select"
            && surface.maddat_entry_id == 5
            && surface.width == 640
            && surface.height == 400
            && surface.plane_count == 4
            && !surface.contains_japanese
            && surface.requires_localization
            && surface.reviewed_text == ["STAGE SELECT"]
            && matches!(
                &surface.consumer,
                ds8_daimadou_builder::GraphicTextConsumerEvidence::GcsScreen {
                    load_reference_file_offset: 0x1c30,
                    load_reference_runtime_address: 0x1d30,
                    renderer_call_file_offsets,
                    renderer_runtime_address: 0x1689,
                    ..
                } if renderer_call_file_offsets == &[0x1c80, 0x1c8b]
            )
    }));
    assert!(survey.graphic_text.surfaces.iter().any(|surface| {
        surface.id == "title"
            && surface.maddat_entry_id == 140
            && surface.width == 640
            && surface.height == 400
            && surface.reviewed_text
                == [
                    "大魔導戦略物語'95",
                    "だいまどうせんりゃくものがたり'95",
                    "DAIMADOUSENRYAKU MONOGATARI'95",
                ]
    }));
    assert!(survey.graphic_text.surfaces.iter().any(|surface| {
        surface.id == "title-menu"
            && surface.maddat_entry_id == 141
            && surface.width == 24
            && surface.height == 32
            && surface.plane_count == 4
            && surface.reviewed_text == ["START", "CONTINUE"]
            && matches!(
                &surface.consumer,
                ds8_daimadou_builder::GraphicTextConsumerEvidence::MaskedSpriteLabels {
                    load_block_file_offset: 0x1e80,
                    file_read_call_file_offset: 0x1eb8,
                    file_read_runtime_address: 0x03ae,
                    decompressor_call_file_offset: 0x1ed0,
                    decompressor_runtime_address: 0x158c,
                    decoded_copy_word_count_file_offset: 0x1edf,
                    decoded_copy_word_count: 0x870,
                    decoded_copy_byte_count: 4320,
                    decoded_copy_call_file_offset: 0x1ee2,
                    decoded_copy_runtime_address: 0x34ad,
                    precompose_call_file_offset: 0x1fb7,
                    precompose_runtime_address: 0x2683,
                    sequence_file_offsets,
                    label_slot_counts,
                    stored_glyph_count: 12,
                    selectable_glyph_count: 11,
                    color_plane_count: 3,
                    mask_plane_count: 1,
                    ..
                } if sequence_file_offsets == &[0x36be, 0x36c9]
                    && label_slot_counts == &[5, 8]
            )
    }));
    assert!(survey.graphic_text.surfaces.iter().any(|surface| {
        surface.id == "stage-completion-legend"
            && surface.maddat_entry_id == 152
            && surface.width == 48
            && surface.height == 32
            && surface.plane_count == 4
            && surface.reviewed_text == ["伝説"]
            && matches!(
                &surface.consumer,
                ds8_daimadou_builder::GraphicTextConsumerEvidence::TiledPlanarRegion {
                    load_block_file_offset: 0x359b,
                    file_read_call_file_offset: 0x35d3,
                    file_read_runtime_address: 0x03ae,
                    decompressor_call_file_offset: 0x35f1,
                    decompressor_runtime_address: 0x158c,
                    tile_ids,
                    tile_columns: 3,
                    tile_rows: 2,
                    tile_width: 16,
                    tile_height: 16,
                    ..
                } if tile_ids == &[120, 121, 122, 123, 124, 125]
            )
    }));
    assert!(survey.graphic_text.surfaces.iter().any(|surface| {
        surface.id == "game-over"
            && surface.maddat_entry_id == 153
            && surface.width == 288
            && surface.height == 64
            && surface.plane_count == 4
            && surface.reviewed_text == ["ゲームオーバー"]
            && matches!(
                surface.consumer,
                ds8_daimadou_builder::GraphicTextConsumerEvidence::PlanarBlock {
                    load_block_file_offset: 0x5f92,
                    renderer_file_offset: 0x62c9,
                    bytes_per_row: 36,
                    words_per_row: 18,
                    ..
                }
            )
    }));
    assert!(!survey.ending_credits.contains_japanese);
    assert!(survey.ending_credits.requires_localization);
    assert_eq!(survey.ending_credits.font.maddat_entry_id, 147);
    assert_eq!(survey.ending_credits.font.decoded_size, 6240);
    assert_eq!(survey.ending_credits.font.glyph_count, 65);
    assert_eq!(survey.ending_credits.text.maddat_entry_id, 148);
    assert_eq!(survey.ending_credits.text.byte_size, 601);
    assert_eq!(survey.ending_credits.text.screens.len(), 10);
    assert_eq!(
        survey
            .ending_credits
            .text
            .screens
            .iter()
            .flat_map(|screen| &screen.lines)
            .filter(|line| line.requires_localization)
            .count(),
        10
    );
    assert_eq!(
        survey.ending_credits.text.screens[0].lines[1].text,
        "       STAFF"
    );
    assert!(survey.ending_credits.text.screens.iter().any(|screen| {
        screen
            .lines
            .iter()
            .any(|line| line.text.contains("MOO NIITANI"))
    }));
    assert!(survey.ending_credits.text.screens.iter().any(|screen| {
        screen
            .lines
            .iter()
            .any(|line| line.text.contains("[PUYO] KEROL"))
    }));
    assert_eq!(survey.ending_credits.motion_tracks.len(), 3);
    assert!(
        survey
            .ending_credits
            .motion_tracks
            .iter()
            .all(|track| track.points.len() == 190 && track.terminator == 0x0280)
    );
    assert_eq!(
        survey
            .ending_credits
            .consumer
            .font_load_reference_file_offset,
        0x32aa
    );
    assert_eq!(
        survey
            .ending_credits
            .consumer
            .sequence_call_site_file_offset,
        0x34ca
    );
    assert!(survey.ending_interlude_caption.contains_japanese);
    assert_eq!(survey.ending_interlude_caption.text, "ほんじつのカレ～");
    assert_eq!(survey.ending_interlude_caption.file_offset, 0x494d);
    assert_eq!(survey.ending_interlude_caption.consumer.file_offset, 0x2610);
    assert_eq!(
        survey.ending_interlude_caption.consumer.background_entry_id,
        154
    );
    assert!(survey.ending_curry_result.contains_japanese);
    assert_eq!(survey.ending_curry_result.consumer.file_offset, 0x28b7);
    assert_eq!(survey.ending_curry_result.consumer.runtime_address, 0x29b7);
    assert_eq!(
        survey.ending_curry_result.consumer.selector_runtime_address,
        0x4a5f
    );
    assert_eq!(survey.ending_curry_result.month.file_offset, 0x44b7);
    assert_eq!(survey.ending_curry_result.month.suffix_text, "がつ");
    assert_eq!(survey.ending_curry_result.day.file_offset, 0x44bc);
    assert_eq!(survey.ending_curry_result.day.suffix_text, "にち");
    assert_eq!(survey.ending_curry_result.messages.len(), 4);
    assert_eq!(
        survey
            .ending_curry_result
            .messages
            .iter()
            .map(|message| message.result_class)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3]
    );
    assert!(survey.ending_meal_reaction.contains_japanese);
    assert_eq!(survey.ending_meal_reaction.id, "bite-sound-effect");
    assert_eq!(survey.ending_meal_reaction.text, "ぱくっ");
    assert_eq!(survey.ending_meal_reaction.consumer.file_offset, 0x2743);
    assert_eq!(survey.ending_meal_reaction.consumer.runtime_address, 0x2843);
    assert_eq!(
        survey.ending_meal_reaction.consumer.glyph_load_file_offsets,
        [0x2748, 0x2765, 0x2782]
    );
    assert_eq!(survey.ending_meal_reaction.glyph_loads.len(), 3);
    assert_eq!(
        survey
            .ending_curry_result
            .messages
            .iter()
            .map(|message| message.line_break_counts.as_slice())
            .collect::<Vec<_>>(),
        [
            &[1, 2, 1][..],
            &[1, 2, 1, 1][..],
            &[1, 2, 1][..],
            &[1, 2, 1, 1][..],
        ]
    );
    assert!(survey.ending_ingredient_labels.contains_japanese);
    assert_eq!(survey.ending_ingredient_labels.records.len(), 27);
    assert_eq!(survey.ending_ingredient_labels.table_file_offset, 0x45e1);
    assert_eq!(survey.ending_ingredient_labels.table_byte_size, 27 * 16);
    assert_eq!(survey.ending_ingredient_labels.consumer.file_offset, 0x3a28);
    assert_eq!(
        survey.ending_ingredient_labels.consumer.runtime_address,
        0x3b28
    );
    assert_eq!(
        survey.ending_ingredient_labels.consumer.fixed_cell_count,
        16
    );
    assert_eq!(
        survey
            .ending_ingredient_labels
            .records
            .iter()
            .find(|record| record.id == "slot-01-variant-01")
            .map(|record| record.text.as_str()),
        Some("ごはん")
    );
    assert_eq!(
        survey
            .ending_ingredient_labels
            .records
            .iter()
            .find(|record| record.id == "slot-02-variant-01")
            .map(|record| record.text.as_str()),
        Some("イモ")
    );
    assert!(survey.ending_passwords.contains_japanese);
    assert_eq!(survey.ending_passwords.records.len(), 11);
    assert_eq!(survey.ending_passwords.pointer_table_file_offset, 0x4916);
    assert_eq!(survey.ending_passwords.selector_file_offset, 0x4a1c);
    assert_eq!(survey.ending_passwords.consumer.file_offset, 0x3b5a);
    assert_eq!(survey.ending_passwords.consumer.runtime_address, 0x3c5a);
    assert_eq!(
        survey.ending_passwords.consumer.call_site_file_offset,
        0x3632
    );
    assert!(survey.ending_passwords.records.iter().all(|record| {
        record.text.starts_with("ぱすわ～ど  「") && record.text.ends_with('」')
    }));
    assert_eq!(survey.mst_text.records.len(), 8);
    assert_eq!(
        survey
            .mst_text
            .records
            .iter()
            .filter(|record| record.contains_japanese)
            .count(),
        6
    );
    assert!(survey.mst_text.records.iter().any(|record| {
        record.id == "song-data-not-found"
            && record
                .text_segments
                .iter()
                .any(|segment| segment.contains("SONG.DATが見つかりません"))
    }));
    assert_eq!(survey.dos_program_text.programs.len(), 5);
    assert_eq!(survey.dos_program_text.record_count, 83);
    assert_eq!(survey.dos_program_text.japanese_record_count, 66);
    assert_eq!(survey.dos_program_text.bound_japanese_record_count, 57);
    assert_eq!(survey.dos_program_text.unbound_japanese_record_ids.len(), 9);
    assert!(survey.dos_program_text.programs.iter().any(|program| {
        program.filename == "NMOUSE.COM"
            && program.records.iter().any(|record| {
                record.id == "help"
                    && record.bound_to_consumer
                    && record
                        .text_segments
                        .iter()
                        .any(|segment| segment.contains("起動方法"))
            })
    }));
    assert!(survey.dos_program_text.programs.iter().any(|program| {
        program.filename == "MAD.COM"
            && program.records.iter().any(|record| {
                record.id == "disk-write-protected"
                    && record.bound_to_consumer
                    && record
                        .text_segments
                        .iter()
                        .any(|segment| segment.contains("ディスクが書き込み禁止になっています"))
            })
    }));
    assert!(survey.mst_text.records.iter().any(|record| {
        record.id == "memory-allocation-failure"
            && record.text_segments.len() == 5
            && record
                .text_segments
                .iter()
                .any(|segment| segment.contains("MST.COMを実行してみてください"))
    }));
    assert_eq!(survey.mad_indexed_text.renderer.file_offset, 0x4db4);
    assert_eq!(survey.mad_indexed_text.renderer.runtime_address, 0x4eb4);
    assert_eq!(survey.mad_indexed_text.pools.len(), 3);
    assert_eq!(survey.mad_indexed_text.strings.len(), 253);
    assert_eq!(
        survey.mad_indexed_text.unbound_string_ids,
        ["mad-indexed-7d1b"]
    );
    assert!(survey.mad_indexed_text.strings.iter().any(|string| {
        string.bound_to_consumer && string.text.as_deref() == Some("ぷよぷよアタック！")
    }));
    assert!(survey.mad_indexed_text.strings.iter().any(|string| {
        string.bound_to_consumer && string.text.as_deref() == Some("タイトルへもどる")
    }));
    assert!(survey.mad_indexed_text.bindings.iter().any(|binding| {
        binding.id == "result-banner"
            && binding.string_ids.len() == 2
            && binding.display_text.as_deref() == Some("ステージクリア")
            && binding.contains_japanese
            && binding.string_ids.iter().all(|id| {
                survey
                    .mad_indexed_text
                    .strings
                    .iter()
                    .find(|string| &string.id == id)
                    .is_some_and(|string| string.text.is_none())
            })
    }));
    assert!(!survey.gaiji.contains_japanese_text);
    assert!(survey.mad_text.fixed_slots.iter().all(|slot| {
        !slot.contains_japanese_text && slot.row_count == 6 && slot.cells_per_row == 6
    }));

    let first_probe_path = directory.path().join("first-probe.hdm");
    let second_probe_path = directory.path().join("second-probe.hdm");
    let first_probe = build_hangul_probe_image(source.as_ref(), &first_probe_path).unwrap();
    let second_probe = build_hangul_probe_image(source.as_ref(), &second_probe_path).unwrap();
    assert_eq!(
        fs::read(first_probe_path).unwrap(),
        fs::read(second_probe_path).unwrap()
    );
    assert_eq!(first_probe, second_probe);
    assert_eq!(first_probe.probe.character, '가');
    assert_eq!(first_probe.probe.character_code, 0x7622);
    assert_eq!(first_probe.probe.shift_jis_code, 0xeba0);
    assert_eq!(first_probe.probe.gaiji_bitmap_file_offset, 0x1d7);
    assert_eq!(first_probe.probe.mad_text_file_offset, 0xedb);
    assert_eq!(fs::read(&source).unwrap(), original);
}
