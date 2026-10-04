use anyhow::{Context, Result, ensure};
use serde::Serialize;
use v30::{Instruction, decode_bytes};

use crate::game_data::{flink, masked_sprite};
use crate::source::disk;

mod field_unit_callouts;

pub use field_unit_callouts::{
    FieldUnitCallout, FieldUnitCalloutBank, FieldUnitCalloutCatalog,
    FieldUnitCalloutConsumerEvidence, FieldUnitCalloutPaletteUsage,
};

use field_unit_callouts::{catalog_field_unit_callouts, field_unit_callout_runtime_observation};

const COM_ORIGIN: usize = 0x100;

const RECOVERY_EFFECT_MADDAT_ENTRY_ID: usize = 9;
const RECOVERY_EFFECT_PACKED_SHA256: &str =
    "83c0dd61e77c55215330e8228887bada58473dc6787e1bf23ba0b93daa187ce7";
const RECOVERY_EFFECT_DECODED_SHA256: &str =
    "2951a9559ca8c316fd0721fd78e94eeb65d1983a605abb96224f6164e76a7835";

const BANK_LOAD_BLOCK_FILE_OFFSET: usize = 0x1348;
const BANK_LOAD_BLOCK_LEN: usize = 0x12;
const BANK_LOADER_CALL_FILE_OFFSET: usize = 0x1357;
const BANK_LOADER_RUNTIME_ADDRESS: usize = 0xb400;

const SHARED_RECOVERY_ROUTINE_FILE_OFFSET: usize = 0x395e;
const SHARED_RECOVERY_ROUTINE_LEN: usize = 0x16;
const SHARED_RECOVERY_ROUTINE_RUNTIME_ADDRESS: usize = 0x3a5e;
const EFFECT_CALL_FILE_OFFSET: usize = 0x3966;
const EFFECT_RUNTIME_ADDRESS: usize = 0x4fb0;
const UNIT_REDRAW_CALL_FILE_OFFSET: usize = 0x396f;
const UNIT_REDRAW_RUNTIME_ADDRESS: usize = 0x525f;

const MAGIC_RECOVERY_CALL_FILE_OFFSETS: [usize; 2] = [0x5dc2, 0x5dcf];
const HEALTH_RECOVERY_CALL_FILE_OFFSETS: [usize; 1] = [0x5ea2];

const EFFECT_ROUTINE_FILE_OFFSET: usize = 0x4eb0;
const EFFECT_ROUTINE_LEN: usize = 0x44;
const EFFECT_POSITION_CALL_FILE_OFFSET: usize = 0x4ec3;
const EFFECT_POSITION_RUNTIME_ADDRESS: usize = 0x54b5;
const FRAME_POINTER_TABLE_FILE_OFFSET: usize = 0x4ef4;
const FRAME_TILE_COLUMNS: usize = 3;
const FRAME_TILE_ROWS: usize = 3;
const FRAME_TILE_COUNT: usize = FRAME_TILE_COLUMNS * FRAME_TILE_ROWS;
const FRAME_MAP_RUNTIME_ADDRESSES: [usize; 8] = [
    0x78c1, 0x78ca, 0x78d3, 0x78dc, 0x78e5, 0x78ee, 0x78f7, 0x7900,
];

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StateGraphicCatalog {
    pub surfaces: Vec<StateGraphicSurface>,
    pub field_unit_callouts: FieldUnitCalloutCatalog,
    pub unresolved_observations: Vec<StateGraphicObservation>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StateGraphicSurface {
    pub id: String,
    pub maddat_entry_id: usize,
    pub packed_size: usize,
    pub packed_sha256: String,
    pub decoded_size: usize,
    pub decoded_sha256: String,
    pub tile_count: usize,
    pub tile_width: usize,
    pub tile_height: usize,
    pub mask_plane_count: usize,
    pub color_plane_count: usize,
    pub frame_tile_columns: usize,
    pub frame_tile_rows: usize,
    pub source_text: Option<String>,
    pub requires_localization: bool,
    pub localization_status: StateGraphicLocalizationStatus,
    pub frames: Vec<StateGraphicFrame>,
    pub consumer: StateGraphicConsumerEvidence,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StateGraphicLocalizationStatus {
    SourceFramesReviewedNonText,
    SourceGraphicTextFoundUntranscribed,
    SourceGraphicTextTranscribed,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StateGraphicObservation {
    pub id: String,
    pub display_condition: String,
    pub screen_region: String,
    pub reported_symptom: String,
    pub requires_runtime_diagnosis: bool,
    pub affected_surface_ids: Vec<String>,
    pub required_evidence: Vec<StateGraphicEvidenceNeed>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StateGraphicEvidenceNeed {
    EventTimeFrameSequence,
    EventTimeEffectBankHash,
    EventTimeCalloutBankHash,
    EventTimeRenderingConsumerTrace,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StateGraphicFrame {
    pub frame_number: usize,
    pub map_file_offset: usize,
    pub map_runtime_address: usize,
    pub tile_ids: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StateGraphicConsumerEvidence {
    pub program_filename: String,
    pub program_sha256: String,
    pub bank_load_block_file_offset: usize,
    pub bank_load_block_runtime_address: usize,
    pub bank_load_decoded_instruction_count: usize,
    pub bank_loader_call_file_offset: usize,
    pub bank_loader_runtime_address: usize,
    pub shared_recovery_routine_file_offset: usize,
    pub shared_recovery_routine_runtime_address: usize,
    pub shared_recovery_decoded_instruction_count: usize,
    pub effect_call_file_offset: usize,
    pub effect_runtime_address: usize,
    pub effect_decoded_instruction_count: usize,
    pub effect_position_call_file_offset: usize,
    pub effect_position_runtime_address: usize,
    pub unit_redraw_call_file_offset: usize,
    pub unit_redraw_runtime_address: usize,
    pub frame_pointer_table_file_offset: usize,
    pub triggers: Vec<StateGraphicTrigger>,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct StateGraphicTrigger {
    pub id: String,
    pub call_site_file_offsets: Vec<usize>,
}

pub(crate) fn catalog_state_graphics(maddat: &[u8], mad_com: &[u8]) -> Result<StateGraphicCatalog> {
    let archive = flink::parse_flink_archive(maddat)?;
    let entry = archive
        .entries
        .get(RECOVERY_EFFECT_MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing the unit-recovery effect bank")?;
    ensure!(
        entry.id == RECOVERY_EFFECT_MADDAT_ENTRY_ID,
        "MADDAT unit-recovery effect entry does not match its table position"
    );
    ensure!(
        disk::sha256_hex(entry.bytes) == RECOVERY_EFFECT_PACKED_SHA256,
        "MADDAT unit-recovery effect packed bytes differ from the reviewed source"
    );
    let bank = masked_sprite::decode_masked_sprite_bank(entry.bytes)?;
    ensure!(
        disk::sha256_hex(&bank.decoded) == RECOVERY_EFFECT_DECODED_SHA256,
        "MADDAT unit-recovery effect decoded bytes differ from the reviewed source"
    );
    let (frames, consumer) = verify_recovery_effect_consumer(mad_com, bank.tiles.len())?;
    let field_unit_callouts = catalog_field_unit_callouts(&archive, mad_com)?;

    let surface_id = "unit-recovery-light-effect".to_owned();
    let affected_callout_ids = field_unit_callouts
        .callouts
        .iter()
        .map(|callout| callout.id.clone())
        .collect();

    Ok(StateGraphicCatalog {
        surfaces: vec![StateGraphicSurface {
            id: surface_id.clone(),
            maddat_entry_id: entry.id,
            packed_size: bank.packed_size,
            packed_sha256: disk::sha256_hex(entry.bytes),
            decoded_size: bank.decoded.len(),
            decoded_sha256: disk::sha256_hex(&bank.decoded),
            tile_count: bank.tiles.len(),
            tile_width: masked_sprite::TILE_WIDTH,
            tile_height: masked_sprite::TILE_HEIGHT,
            mask_plane_count: masked_sprite::MASK_PLANE_COUNT,
            color_plane_count: masked_sprite::COLOR_PLANE_COUNT,
            frame_tile_columns: FRAME_TILE_COLUMNS,
            frame_tile_rows: FRAME_TILE_ROWS,
            source_text: None,
            requires_localization: false,
            localization_status: StateGraphicLocalizationStatus::SourceFramesReviewedNonText,
            frames,
            consumer,
        }],
        field_unit_callouts,
        unresolved_observations: vec![field_unit_callout_runtime_observation(affected_callout_ids)],
    })
}

fn verify_recovery_effect_consumer(
    bytes: &[u8],
    bank_tile_count: usize,
) -> Result<(Vec<StateGraphicFrame>, StateGraphicConsumerEvidence)> {
    let load = bytes
        .get(BANK_LOAD_BLOCK_FILE_OFFSET..BANK_LOAD_BLOCK_FILE_OFFSET + BANK_LOAD_BLOCK_LEN)
        .context("MAD recovery-effect bank load block is truncated")?;
    ensure!(
        load.starts_with(&[0xb9, RECOVERY_EFFECT_MADDAT_ENTRY_ID as u8, 0x00]),
        "MAD recovery-effect load no longer selects MADDAT entry {RECOVERY_EFFECT_MADDAT_ENTRY_ID}"
    );
    let bank_load_decoded_instruction_count =
        decode_exact_block(load, "MAD recovery-effect bank load")?;
    verify_near_call(
        bytes,
        BANK_LOADER_CALL_FILE_OFFSET,
        BANK_LOADER_RUNTIME_ADDRESS,
        "MAD recovery-effect bank loader",
    )?;

    let shared_recovery = bytes
        .get(
            SHARED_RECOVERY_ROUTINE_FILE_OFFSET
                ..SHARED_RECOVERY_ROUTINE_FILE_OFFSET + SHARED_RECOVERY_ROUTINE_LEN,
        )
        .context("MAD shared recovery routine is truncated")?;
    let shared_recovery_decoded_instruction_count =
        decode_complete_subroutine(shared_recovery, "MAD shared recovery routine")?;
    verify_near_call(
        bytes,
        EFFECT_CALL_FILE_OFFSET,
        EFFECT_RUNTIME_ADDRESS,
        "MAD unit-recovery light effect",
    )?;
    verify_near_call(
        bytes,
        UNIT_REDRAW_CALL_FILE_OFFSET,
        UNIT_REDRAW_RUNTIME_ADDRESS,
        "MAD post-recovery unit redraw",
    )?;
    for file_offset in MAGIC_RECOVERY_CALL_FILE_OFFSETS
        .into_iter()
        .chain(HEALTH_RECOVERY_CALL_FILE_OFFSETS)
    {
        verify_near_call(
            bytes,
            file_offset,
            SHARED_RECOVERY_ROUTINE_RUNTIME_ADDRESS,
            "MAD recovery trigger",
        )?;
    }

    let effect = bytes
        .get(EFFECT_ROUTINE_FILE_OFFSET..EFFECT_ROUTINE_FILE_OFFSET + EFFECT_ROUTINE_LEN)
        .context("MAD unit-recovery effect routine is truncated")?;
    let effect_decoded_instruction_count =
        decode_complete_subroutine(effect, "MAD unit-recovery effect routine")?;
    verify_near_call(
        bytes,
        EFFECT_POSITION_CALL_FILE_OFFSET,
        EFFECT_POSITION_RUNTIME_ADDRESS,
        "MAD recovery-effect field-unit position",
    )?;
    let frames = parse_frame_maps(bytes, bank_tile_count)?;

    Ok((
        frames,
        StateGraphicConsumerEvidence {
            program_filename: "MAD.COM".to_owned(),
            program_sha256: disk::sha256_hex(bytes),
            bank_load_block_file_offset: BANK_LOAD_BLOCK_FILE_OFFSET,
            bank_load_block_runtime_address: BANK_LOAD_BLOCK_FILE_OFFSET + COM_ORIGIN,
            bank_load_decoded_instruction_count,
            bank_loader_call_file_offset: BANK_LOADER_CALL_FILE_OFFSET,
            bank_loader_runtime_address: BANK_LOADER_RUNTIME_ADDRESS,
            shared_recovery_routine_file_offset: SHARED_RECOVERY_ROUTINE_FILE_OFFSET,
            shared_recovery_routine_runtime_address: SHARED_RECOVERY_ROUTINE_RUNTIME_ADDRESS,
            shared_recovery_decoded_instruction_count,
            effect_call_file_offset: EFFECT_CALL_FILE_OFFSET,
            effect_runtime_address: EFFECT_RUNTIME_ADDRESS,
            effect_decoded_instruction_count,
            effect_position_call_file_offset: EFFECT_POSITION_CALL_FILE_OFFSET,
            effect_position_runtime_address: EFFECT_POSITION_RUNTIME_ADDRESS,
            unit_redraw_call_file_offset: UNIT_REDRAW_CALL_FILE_OFFSET,
            unit_redraw_runtime_address: UNIT_REDRAW_RUNTIME_ADDRESS,
            frame_pointer_table_file_offset: FRAME_POINTER_TABLE_FILE_OFFSET,
            triggers: vec![
                StateGraphicTrigger {
                    id: "magic-recovery".to_owned(),
                    call_site_file_offsets: MAGIC_RECOVERY_CALL_FILE_OFFSETS.to_vec(),
                },
                StateGraphicTrigger {
                    id: "health-recovery".to_owned(),
                    call_site_file_offsets: HEALTH_RECOVERY_CALL_FILE_OFFSETS.to_vec(),
                },
            ],
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
    ))
}

fn parse_frame_maps(bytes: &[u8], bank_tile_count: usize) -> Result<Vec<StateGraphicFrame>> {
    let pointers = bytes
        .get(
            FRAME_POINTER_TABLE_FILE_OFFSET
                ..FRAME_POINTER_TABLE_FILE_OFFSET + FRAME_MAP_RUNTIME_ADDRESSES.len() * 2,
        )
        .context("MAD recovery-effect frame pointer table is truncated")?;
    let runtime_addresses = pointers
        .chunks_exact(2)
        .map(|pair| usize::from(u16::from_le_bytes([pair[0], pair[1]])))
        .collect::<Vec<_>>();
    ensure!(
        runtime_addresses == FRAME_MAP_RUNTIME_ADDRESSES,
        "MAD recovery-effect frame pointers differ from the reviewed eight-frame sequence"
    );

    runtime_addresses
        .into_iter()
        .enumerate()
        .map(|(frame_index, runtime_address)| {
            let file_offset = runtime_address
                .checked_sub(COM_ORIGIN)
                .context("MAD recovery-effect frame address precedes the COM origin")?;
            let tile_ids = bytes
                .get(file_offset..file_offset + FRAME_TILE_COUNT)
                .with_context(|| {
                    format!("MAD recovery-effect frame {} is truncated", frame_index + 1)
                })?
                .to_vec();
            ensure!(
                tile_ids.iter().all(|tile_id| usize::from(*tile_id) < bank_tile_count),
                "MAD recovery-effect frame {} selects a tile outside the {bank_tile_count}-tile bank",
                frame_index + 1
            );
            Ok(StateGraphicFrame {
                frame_number: frame_index + 1,
                map_file_offset: file_offset,
                map_runtime_address: runtime_address,
                tile_ids,
            })
        })
        .collect()
}

fn verify_near_call(
    bytes: &[u8],
    file_offset: usize,
    expected_runtime_target: usize,
    label: &str,
) -> Result<()> {
    let encoded = bytes
        .get(file_offset..file_offset + 3)
        .with_context(|| format!("{label} call is truncated"))?;
    let decoded = decode_bytes(encoded).with_context(|| format!("decode {label} call"))?;
    ensure!(
        decoded.byte_len == 3 && matches!(decoded.instruction, Instruction::Call { .. }),
        "{label} is not a typed V30 near CALL"
    );
    let displacement = i16::from_le_bytes([encoded[1], encoded[2]]);
    let next_runtime = u16::try_from(file_offset + COM_ORIGIN + decoded.byte_len)
        .context("near CALL return address exceeds the V30 offset space")?;
    let runtime_target = usize::from(next_runtime.wrapping_add_signed(displacement));
    ensure!(
        runtime_target == expected_runtime_target,
        "{label} targets {runtime_target:#x}, expected {expected_runtime_target:#x}"
    );
    Ok(())
}

fn decode_complete_subroutine(bytes: &[u8], label: &str) -> Result<usize> {
    let instruction_count = decode_exact_block(bytes, label)?;
    let mut cursor = 0;
    let mut last = None;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..])
            .with_context(|| format!("decode V30 instruction at {label} byte {cursor:#x}"))?;
        cursor += decoded.byte_len;
        last = Some(decoded.instruction);
    }
    ensure!(
        matches!(last, Some(Instruction::Ret { pop: 0 })),
        "{label} does not end in a typed V30 RET"
    );
    Ok(instruction_count)
}

fn decode_exact_block(bytes: &[u8], label: &str) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..])
            .with_context(|| format!("decode V30 instruction at {label} byte {cursor:#x}"))?;
        cursor += decoded.byte_len;
        instruction_count += 1;
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed {label} boundary"
    );
    Ok(instruction_count)
}

#[cfg(test)]
#[path = "state_graphic_catalog_tests.rs"]
mod state_graphic_catalog_tests;
