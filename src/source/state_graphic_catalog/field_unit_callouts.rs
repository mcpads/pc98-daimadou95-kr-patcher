use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::game_data::{flink, masked_sprite};
use crate::source::disk;

use super::{
    BANK_LOAD_BLOCK_LEN, BANK_LOADER_RUNTIME_ADDRESS, COM_ORIGIN, StateGraphicEvidenceNeed,
    StateGraphicLocalizationStatus, StateGraphicObservation, decode_complete_subroutine,
    decode_exact_block, verify_near_call,
};

const FIXED_CALLOUT_BANK_ENTRY_ID: usize = 8;
const FIXED_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET: usize = 0x1336;
const FIXED_CALLOUT_BANK_LOADER_CALL_FILE_OFFSET: usize = 0x1345;
const DYNAMIC_CALLOUT_BANK_ENTRY_BASE: usize = 100;
const DYNAMIC_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET: usize = 0x13b5;
const DYNAMIC_CALLOUT_BANK_LOAD_BLOCK_LEN: usize = 0x18;
const DYNAMIC_CALLOUT_BANK_LOADER_CALL_FILE_OFFSET: usize = 0x13ca;
const FIELD_UNIT_CALLOUT_COUNT: usize = 19;
const FIELD_UNIT_CALLOUT_TILE_COUNT: usize = 3;
const FIELD_UNIT_CALLOUT_RENDERER_FILE_OFFSET: usize = 0x5309;
const FIELD_UNIT_CALLOUT_RENDERER_RUNTIME_ADDRESS: usize = 0x5409;
const FIELD_UNIT_CALLOUT_RENDERER_LEN: usize = 0x61;
const FIELD_UNIT_CALLOUT_POINTER_TABLE_FILE_OFFSET: usize = 0x536a;
const FIELD_UNIT_CALLOUT_POINTER_TABLE_RUNTIME_ADDRESS: usize = 0x546a;
const FIELD_UNIT_CALLOUT_BANK_SELECTION_FILE_OFFSETS: [usize; 2] = [0x5190, 0x52e7];
const FIELD_UNIT_CALLOUT_BANK_SELECTION_LEN: usize = 0x12;
const FIELD_UNIT_CALLOUT_CALL_FILE_OFFSETS: [usize; 2] = [0x51a2, 0x52f9];
const SHARED_DYNAMIC_CALLOUT_MAP_RUNTIME_ADDRESS: usize = 0x751f;
const SHARED_DYNAMIC_CALLOUT_TILE_IDS: [u8; FIELD_UNIT_CALLOUT_TILE_COUNT] = [0x7d, 0x7e, 0x7f];
const FIELD_UNIT_CALLOUT_BANK_ENTRY_IDS: [usize; 10] =
    [8, 111, 112, 113, 114, 115, 116, 117, 118, 119];
const FIELD_UNIT_CALLOUT_OPAQUE_PALETTE_INDICES: [u8; 2] = [1, 9];
const FIELD_UNIT_CALLOUT_TRANSPARENT_PALETTE_INDICES: [u8; 1] = [0];

#[derive(Clone, Copy)]
struct FieldUnitCalloutIdentity {
    source_character_name: &'static str,
    source_text: Option<&'static str>,
}

const FIELD_UNIT_CALLOUT_IDENTITIES: [FieldUnitCalloutIdentity; FIELD_UNIT_CALLOUT_COUNT] = [
    FieldUnitCalloutIdentity {
        source_character_name: "ぷよぷよ",
        source_text: Some("ぷよっ"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "コドモドラゴン",
        source_text: Some("じゃけ"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "アーちゃん",
        source_text: Some("べろっ"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "てっぽううお",
        source_text: Some("ぎょ～"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ハニービー",
        source_text: Some("ござる"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ガスト",
        source_text: Some("フフフー"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ぷよぷよ",
        source_text: Some("ぷよっ"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ぷよぷよ",
        source_text: Some("ぷよっ"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ぷよぷよ",
        source_text: Some("ぷよっ"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "アルル",
        source_text: Some("はふぅ"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "スケルトンT",
        source_text: Some("おちゃー"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ドラコ",
        source_text: Some("がおー"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ナスグレイブ",
        source_text: Some("なーす"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ウィッチ",
        source_text: Some("おいっす"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "すけとうだら",
        source_text: Some("ふぃー"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "サムライモウル",
        source_text: Some("もぐう"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ミノタウロス",
        source_text: Some("ごどお"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "シェゾ",
        source_text: Some("とりゃ"),
    },
    FieldUnitCalloutIdentity {
        source_character_name: "ルルー",
        source_text: Some("てやー!"),
    },
];

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct FieldUnitCalloutCatalog {
    pub banks: Vec<FieldUnitCalloutBank>,
    pub callouts: Vec<FieldUnitCallout>,
    pub consumer: FieldUnitCalloutConsumerEvidence,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct FieldUnitCalloutBank {
    pub maddat_entry_id: usize,
    pub packed_size: usize,
    pub packed_sha256: String,
    pub decoded_size: usize,
    pub decoded_sha256: String,
    pub tile_count: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct FieldUnitCallout {
    pub id: String,
    pub runtime_unit_class_id: usize,
    pub unit_name_lookup_id: String,
    pub source_character_name: String,
    pub maddat_entry_id: usize,
    pub map_file_offset: usize,
    pub map_runtime_address: usize,
    pub tile_ids: Vec<u8>,
    pub source_tile_strip_sha256: String,
    pub palette_usage: FieldUnitCalloutPaletteUsage,
    pub source_text: Option<String>,
    pub requires_localization: bool,
    pub localization_status: StateGraphicLocalizationStatus,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct FieldUnitCalloutPaletteUsage {
    pub opaque_pixel_count: usize,
    pub opaque_palette_indices: Vec<u8>,
    pub transparent_pixel_count: usize,
    pub transparent_palette_indices: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct FieldUnitCalloutConsumerEvidence {
    pub program_filename: String,
    pub program_sha256: String,
    pub fixed_bank_load_block_file_offset: usize,
    pub fixed_bank_load_block_runtime_address: usize,
    pub fixed_bank_load_decoded_instruction_count: usize,
    pub fixed_bank_loader_call_file_offset: usize,
    pub dynamic_bank_load_block_file_offset: usize,
    pub dynamic_bank_load_block_runtime_address: usize,
    pub dynamic_bank_load_decoded_instruction_count: usize,
    pub dynamic_bank_loader_call_file_offset: usize,
    pub bank_loader_runtime_address: usize,
    pub bank_selection_file_offsets: Vec<usize>,
    pub bank_selection_decoded_instruction_count: usize,
    pub renderer_file_offset: usize,
    pub renderer_runtime_address: usize,
    pub renderer_decoded_instruction_count: usize,
    pub pointer_table_file_offset: usize,
    pub pointer_table_runtime_address: usize,
    pub call_site_file_offsets: Vec<usize>,
    pub isa_profile_id: String,
}

struct FieldUnitCalloutSelection {
    runtime_unit_class_id: usize,
    maddat_entry_id: usize,
    map_file_offset: usize,
    map_runtime_address: usize,
    tile_ids: Vec<u8>,
}

pub(super) fn catalog_field_unit_callouts(
    archive: &flink::FlinkArchive<'_>,
    mad_com: &[u8],
) -> Result<FieldUnitCalloutCatalog> {
    let decoded_banks = FIELD_UNIT_CALLOUT_BANK_ENTRY_IDS
        .into_iter()
        .map(|entry_id| {
            let entry = archive
                .entries
                .get(entry_id - 1)
                .with_context(|| format!("MADDAT is missing field-unit callout bank {entry_id}"))?;
            ensure!(
                entry.id == entry_id,
                "MADDAT field-unit callout bank {entry_id} does not match its table position"
            );
            let bank = masked_sprite::decode_masked_sprite_bank(entry.bytes)?;
            ensure!(
                bank.tiles.len() == 128,
                "MADDAT field-unit callout bank {entry_id} has {} tiles instead of 128",
                bank.tiles.len()
            );
            Ok((entry, bank))
        })
        .collect::<Result<Vec<_>>>()?;
    let bank_tile_counts = decoded_banks
        .iter()
        .map(|(entry, bank)| (entry.id, bank.tiles.len()))
        .collect::<Vec<_>>();
    let (selections, consumer) = verify_field_unit_callout_consumer(mad_com, &bank_tile_counts)?;

    let callouts = selections
        .into_iter()
        .map(|selection| {
            let (_, bank) = decoded_banks
                .iter()
                .find(|(entry, _)| entry.id == selection.maddat_entry_id)
                .expect("verified field-unit callout selection uses a decoded bank");
            let mut strip_bytes =
                Vec::with_capacity(FIELD_UNIT_CALLOUT_TILE_COUNT * masked_sprite::BYTES_PER_TILE);
            let mut opaque_pixel_count = 0;
            let mut opaque_palette_indices = BTreeSet::new();
            let mut transparent_pixel_count = 0;
            let mut transparent_palette_indices = BTreeSet::new();
            for &tile_id in &selection.tile_ids {
                let start = usize::from(tile_id) * masked_sprite::BYTES_PER_TILE;
                strip_bytes
                    .extend_from_slice(&bank.decoded[start..start + masked_sprite::BYTES_PER_TILE]);
                let tile = &bank.tiles[usize::from(tile_id)];
                for (&opaque, &palette_index) in
                    tile.opaque.iter().zip(&tile.palette_indices)
                {
                    if opaque {
                        opaque_pixel_count += 1;
                        opaque_palette_indices.insert(palette_index);
                    } else {
                        transparent_pixel_count += 1;
                        transparent_palette_indices.insert(palette_index);
                    }
                }
            }
            let palette_usage = FieldUnitCalloutPaletteUsage {
                opaque_pixel_count,
                opaque_palette_indices: opaque_palette_indices.into_iter().collect(),
                transparent_pixel_count,
                transparent_palette_indices: transparent_palette_indices.into_iter().collect(),
            };
            ensure!(
                palette_usage.opaque_palette_indices
                    == FIELD_UNIT_CALLOUT_OPAQUE_PALETTE_INDICES,
                "field-unit callout {} uses opaque palette indices {:?} instead of {:?}",
                selection.runtime_unit_class_id,
                palette_usage.opaque_palette_indices,
                FIELD_UNIT_CALLOUT_OPAQUE_PALETTE_INDICES
            );
            ensure!(
                palette_usage.transparent_palette_indices
                    == FIELD_UNIT_CALLOUT_TRANSPARENT_PALETTE_INDICES,
                "field-unit callout {} stores transparent pixels with palette indices {:?} instead of {:?}",
                selection.runtime_unit_class_id,
                palette_usage.transparent_palette_indices,
                FIELD_UNIT_CALLOUT_TRANSPARENT_PALETTE_INDICES
            );
            let identity = FIELD_UNIT_CALLOUT_IDENTITIES
                .get(selection.runtime_unit_class_id - 1)
                .expect("verified field-unit class ID has a source identity");
            Ok(FieldUnitCallout {
                id: format!(
                    "field-unit-callout-{:02}",
                    selection.runtime_unit_class_id
                ),
                runtime_unit_class_id: selection.runtime_unit_class_id,
                unit_name_lookup_id: format!(
                    "unit-name-{:02}",
                    selection.runtime_unit_class_id - 1
                ),
                source_character_name: identity.source_character_name.to_owned(),
                maddat_entry_id: selection.maddat_entry_id,
                map_file_offset: selection.map_file_offset,
                map_runtime_address: selection.map_runtime_address,
                tile_ids: selection.tile_ids,
                source_tile_strip_sha256: disk::sha256_hex(&strip_bytes),
                palette_usage,
                source_text: identity.source_text.map(str::to_owned),
                requires_localization: true,
                localization_status: if identity.source_text.is_some() {
                    StateGraphicLocalizationStatus::SourceGraphicTextTranscribed
                } else {
                    StateGraphicLocalizationStatus::SourceGraphicTextFoundUntranscribed
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let banks = decoded_banks
        .into_iter()
        .map(|(entry, bank)| FieldUnitCalloutBank {
            maddat_entry_id: entry.id,
            packed_size: bank.packed_size,
            packed_sha256: disk::sha256_hex(entry.bytes),
            decoded_size: bank.decoded.len(),
            decoded_sha256: disk::sha256_hex(&bank.decoded),
            tile_count: bank.tiles.len(),
        })
        .collect();

    Ok(FieldUnitCalloutCatalog {
        banks,
        callouts,
        consumer,
    })
}

pub(super) fn field_unit_callout_runtime_observation(
    affected_surface_ids: Vec<String>,
) -> StateGraphicObservation {
    StateGraphicObservation {
        id: "field-unit-callout-runtime-trigger".to_owned(),
        display_condition:
            "reported during health or magic recovery; complete field-action trigger set unresolved"
                .to_owned(),
        screen_region: "directly above the affected field unit".to_owned(),
        reported_symptom:
            "a source-language three-tile graphical callout appears above the field unit".to_owned(),
        requires_runtime_diagnosis: true,
        affected_surface_ids,
        required_evidence: vec![
            StateGraphicEvidenceNeed::EventTimeFrameSequence,
            StateGraphicEvidenceNeed::EventTimeCalloutBankHash,
            StateGraphicEvidenceNeed::EventTimeRenderingConsumerTrace,
        ],
    }
}

fn verify_field_unit_callout_consumer(
    bytes: &[u8],
    bank_tile_counts: &[(usize, usize)],
) -> Result<(
    Vec<FieldUnitCalloutSelection>,
    FieldUnitCalloutConsumerEvidence,
)> {
    let fixed_load = bytes
        .get(
            FIXED_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET
                ..FIXED_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET + BANK_LOAD_BLOCK_LEN,
        )
        .context("MAD fixed field-unit callout bank load block is truncated")?;
    ensure!(
        fixed_load.starts_with(&[0xb9, FIXED_CALLOUT_BANK_ENTRY_ID as u8, 0x00]),
        "MAD fixed field-unit callout load no longer selects MADDAT entry {FIXED_CALLOUT_BANK_ENTRY_ID}"
    );
    let fixed_bank_load_decoded_instruction_count =
        decode_exact_block(fixed_load, "MAD fixed field-unit callout bank load")?;
    verify_near_call(
        bytes,
        FIXED_CALLOUT_BANK_LOADER_CALL_FILE_OFFSET,
        BANK_LOADER_RUNTIME_ADDRESS,
        "MAD fixed field-unit callout bank loader",
    )?;

    let dynamic_load = bytes
        .get(
            DYNAMIC_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET
                ..DYNAMIC_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET + DYNAMIC_CALLOUT_BANK_LOAD_BLOCK_LEN,
        )
        .context("MAD dynamic field-unit callout bank load block is truncated")?;
    ensure!(
        dynamic_load.starts_with(&[0x32, 0xed, 0x80, 0xe1, 0x1f, 0x51, 0x83, 0xc1, 0x64]),
        "MAD dynamic field-unit callout load no longer selects unit ID plus 100"
    );
    ensure!(
        dynamic_load[9..14] == [0x2e, 0x8b, 0x1e, 0x18, 0xc0],
        "MAD dynamic field-unit callout load no longer targets its reviewed bank segment"
    );
    let dynamic_bank_load_decoded_instruction_count =
        decode_exact_block(dynamic_load, "MAD dynamic field-unit callout bank load")?;
    verify_near_call(
        bytes,
        DYNAMIC_CALLOUT_BANK_LOADER_CALL_FILE_OFFSET,
        BANK_LOADER_RUNTIME_ADDRESS,
        "MAD dynamic field-unit callout bank loader",
    )?;

    let mut bank_selection_decoded_instruction_count = None;
    for file_offset in FIELD_UNIT_CALLOUT_BANK_SELECTION_FILE_OFFSETS {
        let selection = bytes
            .get(file_offset..file_offset + FIELD_UNIT_CALLOUT_BANK_SELECTION_LEN)
            .context("MAD field-unit callout bank selection block is truncated")?;
        ensure!(
            selection
                == [
                    0x2e, 0x8e, 0x1e, 0x16, 0xc0, 0x2e, 0x80, 0x3e, 0x51, 0xc1, 0x0a, 0x76, 0x05,
                    0x2e, 0x8e, 0x1e, 0x18, 0xc0,
                ],
            "MAD field-unit callout bank selection differs from the reviewed unit-10 boundary"
        );
        let count = decode_exact_block(selection, "MAD field-unit callout bank selection")?;
        ensure!(
            bank_selection_decoded_instruction_count.is_none_or(|expected| expected == count),
            "MAD field-unit callout bank selection paths decode differently"
        );
        bank_selection_decoded_instruction_count = Some(count);
    }

    let renderer = bytes
        .get(
            FIELD_UNIT_CALLOUT_RENDERER_FILE_OFFSET
                ..FIELD_UNIT_CALLOUT_RENDERER_FILE_OFFSET + FIELD_UNIT_CALLOUT_RENDERER_LEN,
        )
        .context("MAD field-unit callout renderer is truncated")?;
    let renderer_decoded_instruction_count =
        decode_complete_subroutine(renderer, "MAD field-unit callout renderer")?;
    for file_offset in FIELD_UNIT_CALLOUT_CALL_FILE_OFFSETS {
        verify_near_call(
            bytes,
            file_offset,
            FIELD_UNIT_CALLOUT_RENDERER_RUNTIME_ADDRESS,
            "MAD field-unit callout renderer",
        )?;
    }

    let selections = parse_field_unit_callout_selections(bytes, bank_tile_counts)?;

    Ok((
        selections,
        FieldUnitCalloutConsumerEvidence {
            program_filename: "MAD.COM".to_owned(),
            program_sha256: disk::sha256_hex(bytes),
            fixed_bank_load_block_file_offset: FIXED_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET,
            fixed_bank_load_block_runtime_address: FIXED_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET
                + COM_ORIGIN,
            fixed_bank_load_decoded_instruction_count,
            fixed_bank_loader_call_file_offset: FIXED_CALLOUT_BANK_LOADER_CALL_FILE_OFFSET,
            dynamic_bank_load_block_file_offset: DYNAMIC_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET,
            dynamic_bank_load_block_runtime_address: DYNAMIC_CALLOUT_BANK_LOAD_BLOCK_FILE_OFFSET
                + COM_ORIGIN,
            dynamic_bank_load_decoded_instruction_count,
            dynamic_bank_loader_call_file_offset: DYNAMIC_CALLOUT_BANK_LOADER_CALL_FILE_OFFSET,
            bank_loader_runtime_address: BANK_LOADER_RUNTIME_ADDRESS,
            bank_selection_file_offsets: FIELD_UNIT_CALLOUT_BANK_SELECTION_FILE_OFFSETS.to_vec(),
            bank_selection_decoded_instruction_count: bank_selection_decoded_instruction_count
                .expect("field-unit callout has two bank selection paths"),
            renderer_file_offset: FIELD_UNIT_CALLOUT_RENDERER_FILE_OFFSET,
            renderer_runtime_address: FIELD_UNIT_CALLOUT_RENDERER_RUNTIME_ADDRESS,
            renderer_decoded_instruction_count,
            pointer_table_file_offset: FIELD_UNIT_CALLOUT_POINTER_TABLE_FILE_OFFSET,
            pointer_table_runtime_address: FIELD_UNIT_CALLOUT_POINTER_TABLE_RUNTIME_ADDRESS,
            call_site_file_offsets: FIELD_UNIT_CALLOUT_CALL_FILE_OFFSETS.to_vec(),
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
    ))
}

fn parse_field_unit_callout_selections(
    bytes: &[u8],
    bank_tile_counts: &[(usize, usize)],
) -> Result<Vec<FieldUnitCalloutSelection>> {
    let pointers = bytes
        .get(
            FIELD_UNIT_CALLOUT_POINTER_TABLE_FILE_OFFSET
                ..FIELD_UNIT_CALLOUT_POINTER_TABLE_FILE_OFFSET + FIELD_UNIT_CALLOUT_COUNT * 2,
        )
        .context("MAD field-unit callout pointer table is truncated")?;
    pointers
        .chunks_exact(2)
        .enumerate()
        .map(|(unit_index, pair)| {
            let runtime_unit_class_id = unit_index + 1;
            let map_runtime_address = usize::from(u16::from_le_bytes([pair[0], pair[1]]));
            let map_file_offset = map_runtime_address
                .checked_sub(COM_ORIGIN)
                .context("MAD field-unit callout map precedes the COM origin")?;
            let tile_ids = bytes
                .get(map_file_offset..map_file_offset + FIELD_UNIT_CALLOUT_TILE_COUNT)
                .with_context(|| {
                    format!(
                        "MAD field-unit callout map {runtime_unit_class_id} is truncated"
                    )
                })?
                .to_vec();
            let maddat_entry_id = if runtime_unit_class_id <= 10 {
                FIXED_CALLOUT_BANK_ENTRY_ID
            } else {
                DYNAMIC_CALLOUT_BANK_ENTRY_BASE + runtime_unit_class_id
            };
            let bank_tile_count = bank_tile_counts
                .iter()
                .find_map(|(entry_id, tile_count)| {
                    (*entry_id == maddat_entry_id).then_some(*tile_count)
                })
                .with_context(|| {
                    format!(
                        "MAD field-unit callout {runtime_unit_class_id} has no decoded bank"
                    )
                })?;
            ensure!(
                tile_ids
                    .iter()
                    .all(|tile_id| usize::from(*tile_id) < bank_tile_count),
                "MAD field-unit callout {runtime_unit_class_id} selects a tile outside MADDAT {maddat_entry_id}'s {bank_tile_count}-tile bank"
            );
            if runtime_unit_class_id > 10 {
                ensure!(
                    map_runtime_address == SHARED_DYNAMIC_CALLOUT_MAP_RUNTIME_ADDRESS
                        && tile_ids == SHARED_DYNAMIC_CALLOUT_TILE_IDS,
                    "MAD field-unit callout {runtime_unit_class_id} no longer selects the shared 7D 7E 7F map"
                );
            }
            Ok(FieldUnitCalloutSelection {
                runtime_unit_class_id,
                maddat_entry_id,
                map_file_offset,
                map_runtime_address,
                tile_ids,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "field_unit_callouts_tests.rs"]
mod field_unit_callouts_tests;
