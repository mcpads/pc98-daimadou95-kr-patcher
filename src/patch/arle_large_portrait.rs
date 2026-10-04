use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::large_portrait::{
    DECODED_TILE_BANK_SIZE, TILE_BANK_CAPACITY, TILE_BANK_ENTRY_ID, TILE_MAP_ENTRY_ID,
    decode_large_portrait_set, encode_large_portrait_set,
};

use super::arle_large_portrait_asset::{BACKGROUND_PALETTE_INDEX, load_arle_large_portrait_asset};
use super::arle_large_portrait_consumer::expand_large_portrait_consumers;
use super::maddat::apply_repacked_maddat;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ArleLargePortraitFiles {
    pub maddat: Vec<u8>,
    pub select_com: Vec<u8>,
    pub mad_com: Vec<u8>,
    pub report: ArleLargePortraitReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArleLargePortraitReport {
    pub asset_sha256: String,
    pub tile_bank_entry_id: usize,
    pub tile_map_entry_id: usize,
    pub frame_count: usize,
    pub source_used_tile_count: usize,
    pub output_used_tile_count: usize,
    pub output_tile_bank_capacity: usize,
    pub source_tile_bank_packed_size: usize,
    pub output_tile_bank_packed_size: usize,
    pub source_tile_map_packed_size: usize,
    pub output_tile_map_packed_size: usize,
    pub changed_pixel_count: usize,
    pub select_com_input_size: usize,
    pub select_com_output_size: usize,
    pub select_hook_file_offset: usize,
    pub select_hook_runtime_address: usize,
    pub select_hook_byte_size: usize,
    pub select_portrait_segment_byte_size: usize,
    pub mad_com_input_size: usize,
    pub mad_com_output_size: usize,
    pub mad_bank_allocation_file_offset: usize,
}

pub(crate) fn compile_arle_large_portrait(
    maddat: &[u8],
    select_com: &[u8],
    mad_com: &[u8],
    asset_input: &ResolvedArleAsset,
) -> Result<ArleLargePortraitFiles> {
    let archive = parse_flink_archive(maddat)?;
    let source_tile_bank = archive
        .entries
        .get(TILE_BANK_ENTRY_ID - 1)
        .context("MADDAT is missing Arle large portrait tile bank entry 101")?;
    let source_tile_maps = archive
        .entries
        .get(TILE_MAP_ENTRY_ID - 1)
        .context("MADDAT is missing Arle large portrait map entry 129")?;
    ensure!(
        source_tile_bank.id == TILE_BANK_ENTRY_ID && source_tile_maps.id == TILE_MAP_ENTRY_ID,
        "Arle large portrait entry IDs differ from their table positions"
    );
    let source = decode_large_portrait_set(source_tile_bank.bytes, source_tile_maps.bytes)?;
    ensure!(
        source.used_tile_ids.len() == 88,
        "supported Arle large portrait maps use {} tiles instead of the verified 88",
        source.used_tile_ids.len()
    );

    let asset = load_arle_large_portrait_asset(asset_input)?;
    let changed_pixel_count = source
        .frames
        .iter()
        .zip(&asset.frames)
        .flat_map(|(source, output)| source.iter().zip(output))
        .filter(|(source, output)| source != output)
        .count();
    ensure!(
        changed_pixel_count > 0,
        "Arle large portrait asset makes no pixel changes"
    );

    let encoded = encode_large_portrait_set(&asset.frames, BACKGROUND_PALETTE_INDEX)?;
    ensure!(
        encoded.used_tile_count > source.used_tile_ids.len(),
        "independent Arle portraits unexpectedly fit the source shared-tile population"
    );
    ensure!(
        encoded.packed_tile_bank.len() <= usize::from(u16::MAX)
            && encoded.packed_tile_maps.len() <= usize::from(u16::MAX),
        "Arle large portrait replacement exceeds a FLINK u16 entry size"
    );
    let replacements = BTreeMap::from([
        (TILE_BANK_ENTRY_ID, encoded.packed_tile_bank.clone()),
        (TILE_MAP_ENTRY_ID, encoded.packed_tile_maps.clone()),
    ]);
    let repacked = repack_contiguous_flink_archive(maddat, &replacements, &[])?;
    ensure!(
        repacked.replaced_entry_ids == BTreeSet::from([TILE_BANK_ENTRY_ID, TILE_MAP_ENTRY_ID]),
        "Arle large portrait repack ownership differs from entries 101 and 129"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "arle-large-portrait-maddat-repacker",
        "replace four state-preserving Arle portraits inside the runtime tile bank",
    )?;

    let candidate_archive = parse_flink_archive(&checked_maddat)?;
    let candidate = decode_large_portrait_set(
        candidate_archive.entries[TILE_BANK_ENTRY_ID - 1].bytes,
        candidate_archive.entries[TILE_MAP_ENTRY_ID - 1].bytes,
    )?;
    ensure!(
        candidate.frames == asset.frames,
        "Arle large portrait pixels did not survive the MADDAT repack"
    );
    ensure!(
        candidate.decoded_tile_bank.len() == DECODED_TILE_BANK_SIZE,
        "Arle large portrait output did not retain the expanded 256-tile bank"
    );
    for (source_entry, candidate_entry) in archive.entries.iter().zip(&candidate_archive.entries) {
        if ![TILE_BANK_ENTRY_ID, TILE_MAP_ENTRY_ID].contains(&source_entry.id) {
            ensure!(
                source_entry.bytes == candidate_entry.bytes,
                "Arle large portrait repack changed protected MADDAT entry {}",
                source_entry.id
            );
        }
    }

    let consumers = expand_large_portrait_consumers(
        select_com,
        mad_com,
        encoded.packed_tile_bank.len(),
        encoded.packed_tile_maps.len(),
    )?;
    let select_com_output_size = consumers.select_com.len();
    let mad_com_output_size = consumers.mad_com.len();

    Ok(ArleLargePortraitFiles {
        maddat: checked_maddat,
        select_com: consumers.select_com,
        mad_com: consumers.mad_com,
        report: ArleLargePortraitReport {
            asset_sha256: asset.sha256,
            tile_bank_entry_id: TILE_BANK_ENTRY_ID,
            tile_map_entry_id: TILE_MAP_ENTRY_ID,
            frame_count: asset.frames.len(),
            source_used_tile_count: source.used_tile_ids.len(),
            output_used_tile_count: encoded.used_tile_count,
            output_tile_bank_capacity: TILE_BANK_CAPACITY,
            source_tile_bank_packed_size: source_tile_bank.bytes.len(),
            output_tile_bank_packed_size: encoded.packed_tile_bank.len(),
            source_tile_map_packed_size: source_tile_maps.bytes.len(),
            output_tile_map_packed_size: encoded.packed_tile_maps.len(),
            changed_pixel_count,
            select_com_input_size: select_com.len(),
            select_com_output_size,
            select_hook_file_offset: consumers.select_hook_file_offset,
            select_hook_runtime_address: consumers.select_hook_runtime_address,
            select_hook_byte_size: consumers.select_hook_byte_size,
            select_portrait_segment_byte_size: consumers.select_portrait_segment_byte_size,
            mad_com_input_size: mad_com.len(),
            mad_com_output_size,
            mad_bank_allocation_file_offset: consumers.mad_bank_allocation_file_offset,
        },
    })
}

#[cfg(test)]
#[path = "arle_large_portrait_tests.rs"]
mod arle_large_portrait_tests;
