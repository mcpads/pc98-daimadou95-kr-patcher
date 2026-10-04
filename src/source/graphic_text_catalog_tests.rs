use super::*;

#[test]
#[ignore = "requires GRAPHIC_TEXT_PAYLOAD_DIR with user-owned extracted game files"]
fn extracted_payload_catalogs_every_reviewed_graphic_surface() {
    let directory = std::path::PathBuf::from(
        std::env::var_os("GRAPHIC_TEXT_PAYLOAD_DIR").expect("set GRAPHIC_TEXT_PAYLOAD_DIR"),
    );
    let maddat = std::fs::read(directory.join("MADDAT")).unwrap();
    let opening = std::fs::read(directory.join("OPENING.COM")).unwrap();
    let select = std::fs::read(directory.join("SELECT.COM")).unwrap();
    let mad = std::fs::read(directory.join("MAD.COM")).unwrap();
    let state_graphics =
        crate::source::state_graphic_catalog::catalog_state_graphics(&maddat, &mad).unwrap();

    let catalog = catalog_graphic_text(&maddat, &opening, &select, &mad, &state_graphics).unwrap();
    assert_eq!(catalog.surfaces.len(), 24);
    let stage_select = catalog
        .surfaces
        .iter()
        .find(|surface| surface.id == "stage-select")
        .unwrap();
    assert_eq!(stage_select.maddat_entry_id, 5);
    assert_eq!(stage_select.decoded_sha256, STAGE_SELECT_DECODED_SHA256);
    assert_eq!(stage_select.reviewed_text, ["STAGE SELECT"]);
    assert!(!stage_select.contains_japanese);
    assert!(stage_select.requires_localization);
    let title = catalog
        .surfaces
        .iter()
        .find(|surface| surface.id == "title")
        .unwrap();
    assert_eq!(
        title.reviewed_text,
        [
            "大魔導戦略物語'95",
            "だいまどうせんりゃくものがたり'95",
            "DAIMADOUSENRYAKU MONOGATARI'95",
        ]
    );
    let title_menu = catalog
        .surfaces
        .iter()
        .find(|surface| surface.id == "title-menu")
        .unwrap();
    assert_eq!(title_menu.maddat_entry_id, 141);
    assert_eq!(title_menu.decoded_sha256, TITLE_MENU_DECODED_SHA256);
    assert_eq!(title_menu.reviewed_text, ["START", "CONTINUE"]);
    assert!(!title_menu.requires_localization);
    assert!(matches!(
        &title_menu.consumer,
        GraphicTextConsumerEvidence::MaskedSpriteLabels {
            source_glyph_sequences,
            label_slot_counts,
            decoded_copy_word_count_file_offset: 0x1edf,
            decoded_copy_word_count: 0x870,
            decoded_copy_byte_count: 4320,
            decoded_copy_call_file_offset: 0x1ee2,
            decoded_copy_runtime_address: 0x34ad,
            stored_glyph_count: 12,
            selectable_glyph_count: 11,
            color_plane_count: 3,
            mask_plane_count: 1,
            ..
        } if source_glyph_sequences == &[vec![0, 1, 2, 3, 1], vec![4, 5, 6, 1, 7, 6, 8, 9]]
            && label_slot_counts == &[5, 8]
    ));

    let callouts = catalog
        .surfaces
        .iter()
        .filter(|surface| surface.id.starts_with("field-unit-callout-"))
        .collect::<Vec<_>>();
    assert_eq!(callouts.len(), 19);
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
    assert!(callouts.iter().all(|callout| {
        (callout.width, callout.height, callout.plane_count) == (48, 16, 5)
            && matches!(
                &callout.consumer,
                GraphicTextConsumerEvidence::FieldUnitCallout {
                    runtime_unit_class_id,
                    tile_ids,
                    renderer_runtime_address: 0x5409,
                    pointer_table_runtime_address: 0x546a,
                    ..
                } if (if *runtime_unit_class_id <= 10 {
                    8
                } else {
                    *runtime_unit_class_id + 100
                }) == callout.maddat_entry_id
                    && tile_ids == match *runtime_unit_class_id {
                        1 | 7 | 8 | 9 => &[96, 97, 98][..],
                        2 => &[99, 100, 101][..],
                        3 => &[102, 103, 104][..],
                        4 => &[105, 106, 107][..],
                        5 => &[111, 112, 113][..],
                        6 => &[108, 109, 110][..],
                        _ => &[125, 126, 127][..],
                    }
            )
    }));
    let arle_callout = callouts
        .iter()
        .find(|callout| {
            matches!(
                callout.consumer,
                GraphicTextConsumerEvidence::FieldUnitCallout {
                    runtime_unit_class_id: 10,
                    ..
                }
            )
        })
        .unwrap();
    assert!(matches!(
        &arle_callout.consumer,
        GraphicTextConsumerEvidence::FieldUnitCallout {
            runtime_unit_class_id: 10,
            bank_load_block_file_offset: 0x1336,
            bank_load_block_runtime_address: 0x1436,
            bank_loader_call_file_offset: 0x1345,
            ..
        }
    ));

    let mut drifted_opening = opening.clone();
    drifted_opening[TITLE_MENU_DECODED_COPY_WORD_COUNT_FILE_OFFSET + 1] ^= 1;
    let error = catalog_graphic_text(&maddat, &drifted_opening, &select, &mad, &state_graphics)
        .unwrap_err()
        .to_string();
    assert!(error.contains("decoded-copy word count"));
}

#[test]
fn consumer_evidence_requires_typed_calls_to_the_expected_targets() {
    let mut bytes = vec![0; GAME_OVER_RENDERER_CALL_FILE_OFFSET + 3];
    let next_runtime = GAME_OVER_RENDERER_CALL_FILE_OFFSET + COM_ORIGIN + 3;
    let displacement = GAME_OVER_RENDERER_FILE_OFFSET + COM_ORIGIN - next_runtime;
    bytes[GAME_OVER_RENDERER_CALL_FILE_OFFSET] = 0xe8;
    bytes[GAME_OVER_RENDERER_CALL_FILE_OFFSET + 1..GAME_OVER_RENDERER_CALL_FILE_OFFSET + 3]
        .copy_from_slice(&(displacement as i16).to_le_bytes());

    verify_near_call(
        &bytes,
        GAME_OVER_RENDERER_CALL_FILE_OFFSET,
        GAME_OVER_RENDERER_FILE_OFFSET + COM_ORIGIN,
        "fixture",
    )
    .unwrap();
    bytes[GAME_OVER_RENDERER_CALL_FILE_OFFSET] = 0x90;
    assert!(
        verify_near_call(
            &bytes,
            GAME_OVER_RENDERER_CALL_FILE_OFFSET,
            GAME_OVER_RENDERER_FILE_OFFSET + COM_ORIGIN,
            "fixture",
        )
        .is_err()
    );
}

#[test]
fn stage_completion_consumer_rejects_untyped_file_and_decompress_calls() {
    let mut bytes = vec![0; STAGE_COMPLETION_DECOMPRESS_CALL_FILE_OFFSET + 3];
    bytes[STAGE_COMPLETION_LOAD_BLOCK_FILE_OFFSET..STAGE_COMPLETION_LOAD_BLOCK_FILE_OFFSET + 3]
        .copy_from_slice(&[0xb8, stage_completion_graphic::MADDAT_ENTRY_ID as u8, 0x00]);
    write_near_call(
        &mut bytes,
        STAGE_COMPLETION_FILE_READ_CALL_FILE_OFFSET,
        STAGE_COMPLETION_FILE_READ_RUNTIME_ADDRESS,
    );
    write_near_call(
        &mut bytes,
        STAGE_COMPLETION_DECOMPRESS_CALL_FILE_OFFSET,
        STAGE_COMPLETION_DECOMPRESS_RUNTIME_ADDRESS,
    );

    verify_near_call(
        &bytes,
        STAGE_COMPLETION_FILE_READ_CALL_FILE_OFFSET,
        STAGE_COMPLETION_FILE_READ_RUNTIME_ADDRESS,
        "fixture file reader",
    )
    .unwrap();
    verify_near_call(
        &bytes,
        STAGE_COMPLETION_DECOMPRESS_CALL_FILE_OFFSET,
        STAGE_COMPLETION_DECOMPRESS_RUNTIME_ADDRESS,
        "fixture decompressor",
    )
    .unwrap();

    bytes[STAGE_COMPLETION_DECOMPRESS_CALL_FILE_OFFSET] = 0x90;
    assert!(
        verify_near_call(
            &bytes,
            STAGE_COMPLETION_DECOMPRESS_CALL_FILE_OFFSET,
            STAGE_COMPLETION_DECOMPRESS_RUNTIME_ADDRESS,
            "fixture decompressor",
        )
        .is_err()
    );
}

#[test]
fn stage_select_consumer_rejects_an_untyped_file_read_call() {
    let mut bytes = vec![0; STAGE_SELECT_RENDER_CALL_FILE_OFFSETS[1] + 3];
    bytes[STAGE_SELECT_LOAD_FILE_OFFSET..STAGE_SELECT_LOAD_FILE_OFFSET + 3].copy_from_slice(&[
        0xb8,
        stage_select_graphic::MADDAT_ENTRY_ID as u8,
        0x00,
    ]);
    write_near_call(
        &mut bytes,
        STAGE_SELECT_FILE_READ_CALL_FILE_OFFSET,
        STAGE_SELECT_FILE_READ_RUNTIME_ADDRESS,
    );
    for file_offset in STAGE_SELECT_RENDER_CALL_FILE_OFFSETS {
        write_near_call(&mut bytes, file_offset, STAGE_SELECT_RENDER_RUNTIME_ADDRESS);
    }

    verify_near_call(
        &bytes,
        STAGE_SELECT_FILE_READ_CALL_FILE_OFFSET,
        STAGE_SELECT_FILE_READ_RUNTIME_ADDRESS,
        "fixture stage-select file reader",
    )
    .unwrap();
    bytes[STAGE_SELECT_FILE_READ_CALL_FILE_OFFSET] = 0x90;
    assert!(
        verify_near_call(
            &bytes,
            STAGE_SELECT_FILE_READ_CALL_FILE_OFFSET,
            STAGE_SELECT_FILE_READ_RUNTIME_ADDRESS,
            "fixture stage-select file reader",
        )
        .is_err()
    );
}

#[test]
fn reviewed_stage_select_load_block_is_fully_typed_v30() {
    let load = [
        0xb8, 0x05, 0x00, 0x48, 0x8b, 0xd8, 0xc1, 0xe0, 0x02, 0xd1, 0xe3, 0x03, 0xd8, 0x8b, 0x07,
        0x2e, 0xa3, 0x73, 0x08, 0x8b, 0x47, 0x02, 0x2e, 0xa3, 0x71, 0x08, 0x8b, 0x47, 0x04, 0x2e,
        0xa3, 0x6f, 0x08, 0x8c, 0xc8, 0x8e, 0xd8, 0xb8, 0xbf, 0x42, 0xa3, 0x6d, 0x08, 0xa1, 0x83,
        0x42, 0xa3, 0x69, 0x08, 0xb8, 0x00, 0x00, 0xa3, 0x6b, 0x08, 0xe8, 0x44, 0xe6,
    ];

    assert_eq!(load.len(), STAGE_SELECT_LOAD_BLOCK_LEN);
    assert_eq!(
        decode_exact_block(&load, "reviewed SELECT stage-select load").unwrap(),
        21
    );
}

#[test]
fn reviewed_stage_completion_load_block_is_fully_typed_v30() {
    let load = [
        0xb8, 0x98, 0x00, 0x48, 0x8b, 0xd8, 0xc1, 0xe0, 0x02, 0xd1, 0xe3, 0x03, 0xd8, 0x8b, 0x07,
        0x2e, 0xa3, 0x73, 0x08, 0x8b, 0x47, 0x02, 0x2e, 0xa3, 0x71, 0x08, 0x8b, 0x47, 0x04, 0x2e,
        0xa3, 0x6f, 0x08, 0x8c, 0xc8, 0x8e, 0xd8, 0xb8, 0xbf, 0x42, 0xa3, 0x6d, 0x08, 0x2e, 0xa1,
        0x83, 0x42, 0xa3, 0x69, 0x08, 0xb8, 0x00, 0x00, 0xa3, 0x6b, 0x08, 0xe8, 0xd8, 0xcc,
    ];

    assert_eq!(load.len(), STAGE_COMPLETION_LOAD_BLOCK_LEN);
    assert_eq!(
        decode_exact_block(&load, "reviewed SELECT stage-completion load").unwrap(),
        21
    );
}

fn write_near_call(bytes: &mut [u8], file_offset: usize, runtime_target: usize) {
    let next_runtime = file_offset + COM_ORIGIN + 3;
    let displacement = runtime_target as isize - next_runtime as isize;
    bytes[file_offset] = 0xe8;
    bytes[file_offset + 1..file_offset + 3].copy_from_slice(&(displacement as i16).to_le_bytes());
}
