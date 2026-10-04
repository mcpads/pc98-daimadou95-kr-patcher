use super::*;

#[test]
fn frame_pointer_table_selects_eight_three_by_three_maps() {
    let mut bytes = vec![0; FRAME_MAP_RUNTIME_ADDRESSES[7] - COM_ORIGIN + FRAME_TILE_COUNT];
    for (frame_index, runtime_address) in FRAME_MAP_RUNTIME_ADDRESSES.into_iter().enumerate() {
        let pointer_offset = FRAME_POINTER_TABLE_FILE_OFFSET + frame_index * 2;
        bytes[pointer_offset..pointer_offset + 2]
            .copy_from_slice(&(runtime_address as u16).to_le_bytes());
        let map_offset = runtime_address - COM_ORIGIN;
        for tile_position in 0..FRAME_TILE_COUNT {
            bytes[map_offset + tile_position] =
                (frame_index * FRAME_TILE_COUNT + tile_position) as u8;
        }
    }

    let frames = parse_frame_maps(&bytes, 256).unwrap();

    assert_eq!(frames.len(), 8);
    assert!(frames.iter().all(|frame| frame.tile_ids.len() == 9));
    assert_eq!(frames[0].tile_ids, (0..9).collect::<Vec<_>>());
    assert_eq!(frames[7].tile_ids, (63..72).collect::<Vec<_>>());
}

#[test]
fn frame_map_rejects_a_tile_id_outside_the_masked_bank() {
    let mut bytes = vec![0; FRAME_MAP_RUNTIME_ADDRESSES[7] - COM_ORIGIN + FRAME_TILE_COUNT];
    for (frame_index, runtime_address) in FRAME_MAP_RUNTIME_ADDRESSES.into_iter().enumerate() {
        let pointer_offset = FRAME_POINTER_TABLE_FILE_OFFSET + frame_index * 2;
        bytes[pointer_offset..pointer_offset + 2]
            .copy_from_slice(&(runtime_address as u16).to_le_bytes());
    }
    bytes[FRAME_MAP_RUNTIME_ADDRESSES[0] - COM_ORIGIN] = 7;

    let error = parse_frame_maps(&bytes, 7).unwrap_err();

    assert!(error.to_string().contains("outside the 7-tile bank"));
}

#[test]
#[ignore = "requires STATE_GRAPHIC_PAYLOAD_DIR with user-owned MADDAT and MAD.COM"]
fn extracted_payload_binds_recovery_effect_and_field_unit_callouts_to_their_consumers() {
    let directory = std::path::PathBuf::from(
        std::env::var_os("STATE_GRAPHIC_PAYLOAD_DIR").expect("set STATE_GRAPHIC_PAYLOAD_DIR"),
    );
    let maddat = std::fs::read(directory.join("MADDAT")).unwrap();
    let mad = std::fs::read(directory.join("MAD.COM")).unwrap();

    let catalog = catalog_state_graphics(&maddat, &mad).unwrap();
    let recovery = &catalog.surfaces[0];

    assert_eq!(recovery.id, "unit-recovery-light-effect");
    assert_eq!(recovery.maddat_entry_id, 9);
    assert_eq!(recovery.tile_count, 256);
    assert_eq!(recovery.frames.len(), 8);
    assert_eq!(recovery.frames[0].tile_ids, [3, 4, 5, 23, 24, 25, 0, 0, 41]);
    assert_eq!(recovery.frames[7].tile_ids, [0, 0, 0, 0, 0, 0, 70, 70, 70]);
    assert!(!recovery.requires_localization);
    assert_eq!(
        recovery.localization_status,
        StateGraphicLocalizationStatus::SourceFramesReviewedNonText
    );
    assert_eq!(recovery.consumer.effect_position_call_file_offset, 0x4ec3);
    assert_eq!(recovery.consumer.effect_position_runtime_address, 0x54b5);
    assert_eq!(
        recovery.consumer.triggers,
        [
            StateGraphicTrigger {
                id: "magic-recovery".to_owned(),
                call_site_file_offsets: vec![0x5dc2, 0x5dcf],
            },
            StateGraphicTrigger {
                id: "health-recovery".to_owned(),
                call_site_file_offsets: vec![0x5ea2],
            },
        ]
    );

    let callouts = &catalog.field_unit_callouts;
    assert_eq!(callouts.banks.len(), 10);
    assert_eq!(callouts.callouts.len(), 19);
    assert_eq!(
        callouts
            .banks
            .iter()
            .map(|bank| bank.maddat_entry_id)
            .collect::<Vec<_>>(),
        [8, 111, 112, 113, 114, 115, 116, 117, 118, 119]
    );
    for callout in &callouts.callouts[10..] {
        assert_eq!(callout.maddat_entry_id, 100 + callout.runtime_unit_class_id);
        assert_eq!(
            callout.unit_name_lookup_id,
            format!("unit-name-{:02}", callout.runtime_unit_class_id - 1)
        );
        assert_eq!(callout.map_file_offset, 0x741f);
        assert_eq!(callout.map_runtime_address, 0x751f);
        assert_eq!(callout.tile_ids, [0x7d, 0x7e, 0x7f]);
        assert_eq!(callout.palette_usage.opaque_palette_indices, [1, 9]);
        assert_eq!(callout.palette_usage.transparent_palette_indices, [0]);
        assert_eq!(
            callout.palette_usage.opaque_pixel_count
                + callout.palette_usage.transparent_pixel_count,
            48 * 16
        );
        assert!(callout.requires_localization);
        assert_eq!(
            callout.localization_status,
            StateGraphicLocalizationStatus::SourceGraphicTextTranscribed
        );
    }
    let arle_callout = &callouts.callouts[9];
    assert_eq!(arle_callout.maddat_entry_id, 8);
    assert_eq!(arle_callout.runtime_unit_class_id, 10);
    assert_eq!(arle_callout.unit_name_lookup_id, "unit-name-09");
    assert_eq!(arle_callout.source_character_name, "アルル");
    assert_eq!(arle_callout.source_text.as_deref(), Some("はふぅ"));
    assert_eq!(arle_callout.tile_ids, [125, 126, 127]);
    assert_eq!(
        arle_callout.localization_status,
        StateGraphicLocalizationStatus::SourceGraphicTextTranscribed
    );
    assert_eq!(
        callouts.callouts[..10]
            .iter()
            .map(|callout| {
                (
                    callout.runtime_unit_class_id,
                    callout.source_character_name.as_str(),
                    callout.source_text.as_deref(),
                    callout.tile_ids.as_slice(),
                )
            })
            .collect::<Vec<_>>(),
        [
            (1, "ぷよぷよ", Some("ぷよっ"), [96, 97, 98].as_slice()),
            (
                2,
                "コドモドラゴン",
                Some("じゃけ"),
                [99, 100, 101].as_slice()
            ),
            (3, "アーちゃん", Some("べろっ"), [102, 103, 104].as_slice()),
            (
                4,
                "てっぽううお",
                Some("ぎょ～"),
                [105, 106, 107].as_slice()
            ),
            (5, "ハニービー", Some("ござる"), [111, 112, 113].as_slice()),
            (6, "ガスト", Some("フフフー"), [108, 109, 110].as_slice()),
            (7, "ぷよぷよ", Some("ぷよっ"), [96, 97, 98].as_slice()),
            (8, "ぷよぷよ", Some("ぷよっ"), [96, 97, 98].as_slice()),
            (9, "ぷよぷよ", Some("ぷよっ"), [96, 97, 98].as_slice()),
            (10, "アルル", Some("はふぅ"), [125, 126, 127].as_slice()),
        ]
    );
    assert!(callouts.callouts[..10].iter().all(|callout| {
        callout.requires_localization
            && callout.localization_status
                == StateGraphicLocalizationStatus::SourceGraphicTextTranscribed
    }));
    assert_eq!(
        callouts.callouts[10..]
            .iter()
            .map(|callout| {
                (
                    callout.maddat_entry_id,
                    callout.source_character_name.as_str(),
                    callout.source_text.as_deref(),
                )
            })
            .collect::<Vec<_>>(),
        [
            (111, "スケルトンT", Some("おちゃー")),
            (112, "ドラコ", Some("がおー")),
            (113, "ナスグレイブ", Some("なーす")),
            (114, "ウィッチ", Some("おいっす")),
            (115, "すけとうだら", Some("ふぃー")),
            (116, "サムライモウル", Some("もぐう")),
            (117, "ミノタウロス", Some("ごどお")),
            (118, "シェゾ", Some("とりゃ")),
            (119, "ルルー", Some("てやー!")),
        ]
    );
    assert_eq!(callouts.consumer.renderer_file_offset, 0x5309);
    assert_eq!(callouts.consumer.renderer_runtime_address, 0x5409);
    assert_eq!(callouts.consumer.pointer_table_file_offset, 0x536a);
    assert_eq!(callouts.consumer.call_site_file_offsets, [0x51a2, 0x52f9]);

    assert_eq!(catalog.unresolved_observations.len(), 1);
    let observation = &catalog.unresolved_observations[0];
    assert_eq!(observation.id, "field-unit-callout-runtime-trigger");
    assert!(observation.requires_runtime_diagnosis);
    assert_eq!(observation.affected_surface_ids.len(), 19);
    assert_eq!(
        observation.affected_surface_ids[10],
        "field-unit-callout-11"
    );
    assert_eq!(
        observation.required_evidence,
        [
            StateGraphicEvidenceNeed::EventTimeFrameSequence,
            StateGraphicEvidenceNeed::EventTimeCalloutBankHash,
            StateGraphicEvidenceNeed::EventTimeRenderingConsumerTrace,
        ]
    );
}
