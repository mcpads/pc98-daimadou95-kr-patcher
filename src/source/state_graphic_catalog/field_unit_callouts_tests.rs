use super::*;

#[test]
fn field_callouts_keep_their_event_time_trigger_proof_open() {
    let observation = field_unit_callout_runtime_observation(vec![
        "field-unit-callout-11".to_owned(),
        "field-unit-callout-12".to_owned(),
    ]);

    assert!(observation.requires_runtime_diagnosis);
    assert_eq!(
        observation.affected_surface_ids,
        ["field-unit-callout-11", "field-unit-callout-12"]
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

#[test]
fn unit_eleven_through_nineteen_select_the_real_7d_7e_7f_callout_tiles() {
    let last_map_file_offset = SHARED_DYNAMIC_CALLOUT_MAP_RUNTIME_ADDRESS - COM_ORIGIN;
    let mut bytes = vec![0; last_map_file_offset + FIELD_UNIT_CALLOUT_TILE_COUNT];
    for unit_id in 1..=10 {
        let runtime_address = 0x7480 + (unit_id - 1) * FIELD_UNIT_CALLOUT_TILE_COUNT;
        let pointer_offset = FIELD_UNIT_CALLOUT_POINTER_TABLE_FILE_OFFSET + (unit_id - 1) * 2;
        bytes[pointer_offset..pointer_offset + 2]
            .copy_from_slice(&(runtime_address as u16).to_le_bytes());
        let map_offset = runtime_address - COM_ORIGIN;
        bytes[map_offset..map_offset + FIELD_UNIT_CALLOUT_TILE_COUNT].copy_from_slice(&[0, 1, 2]);
    }
    for unit_id in 11..=FIELD_UNIT_CALLOUT_COUNT {
        let pointer_offset = FIELD_UNIT_CALLOUT_POINTER_TABLE_FILE_OFFSET + (unit_id - 1) * 2;
        bytes[pointer_offset..pointer_offset + 2]
            .copy_from_slice(&(SHARED_DYNAMIC_CALLOUT_MAP_RUNTIME_ADDRESS as u16).to_le_bytes());
    }
    bytes[last_map_file_offset..last_map_file_offset + FIELD_UNIT_CALLOUT_TILE_COUNT]
        .copy_from_slice(&SHARED_DYNAMIC_CALLOUT_TILE_IDS);
    let bank_tile_counts = FIELD_UNIT_CALLOUT_BANK_ENTRY_IDS
        .into_iter()
        .map(|entry_id| (entry_id, 128))
        .collect::<Vec<_>>();

    let selections = parse_field_unit_callout_selections(&bytes, &bank_tile_counts).unwrap();

    assert_eq!(selections.len(), FIELD_UNIT_CALLOUT_COUNT);
    for selection in &selections[10..] {
        assert_eq!(
            selection.maddat_entry_id,
            100 + selection.runtime_unit_class_id
        );
        assert_eq!(
            selection.map_runtime_address,
            SHARED_DYNAMIC_CALLOUT_MAP_RUNTIME_ADDRESS
        );
        assert_eq!(selection.tile_ids, SHARED_DYNAMIC_CALLOUT_TILE_IDS);
    }
}

#[test]
fn transcribed_callout_identities_bind_every_runtime_class_to_its_character_roar() {
    assert_eq!(
        FIELD_UNIT_CALLOUT_IDENTITIES
            .iter()
            .map(|identity| (identity.source_character_name, identity.source_text))
            .collect::<Vec<_>>(),
        [
            ("ぷよぷよ", Some("ぷよっ")),
            ("コドモドラゴン", Some("じゃけ")),
            ("アーちゃん", Some("べろっ")),
            ("てっぽううお", Some("ぎょ～")),
            ("ハニービー", Some("ござる")),
            ("ガスト", Some("フフフー")),
            ("ぷよぷよ", Some("ぷよっ")),
            ("ぷよぷよ", Some("ぷよっ")),
            ("ぷよぷよ", Some("ぷよっ")),
            ("アルル", Some("はふぅ")),
            ("スケルトンT", Some("おちゃー")),
            ("ドラコ", Some("がおー")),
            ("ナスグレイブ", Some("なーす")),
            ("ウィッチ", Some("おいっす")),
            ("すけとうだら", Some("ふぃー")),
            ("サムライモウル", Some("もぐう")),
            ("ミノタウロス", Some("ごどお")),
            ("シェゾ", Some("とりゃ")),
            ("ルルー", Some("てやー!")),
        ]
    );
}
