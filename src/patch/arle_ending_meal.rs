use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::ending_meal_scene::{
    FRAME_COUNT, MADDAT_ENTRY_ID, MAP_RECORD_SIZE, OUTPUT_BANK_TILE_CAPACITY, SOURCE_TILE_COUNT,
    decode_source_ending_meal_scenes, decode_split_ending_meal_scenes,
    encode_split_ending_meal_scenes,
};
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::source::disk::sha256_hex;

use super::arle_ending_meal_asset::{BACKGROUND_PALETTE_INDEX, load_arle_ending_meal_asset};
use super::arle_ending_meal_consumer::{
    MAP_FILE_OFFSET, TILE_ADDRESS_FILE_OFFSET, replace_ending_meal_maps_and_install_bank_selector,
};
use super::arle_ending_meal_loader::{
    PACKED_DESTINATION_FILE_OFFSET, UNPACK_BLOCK_FILE_OFFSET, install_ending_meal_bank_loader,
};
use super::maddat::apply_repacked_maddat;

const SOURCE_ENTRY_SHA256: &str =
    "4646ec448dea2e88d63420e0411850207db1208b5992b4c4d9818f820299f5c2";
const SOURCE_MAPS_SHA256: &str = "90b6a001f61be1f2e60ae6547e951b3f35d89d65410a4952a0f0225e21b379d4";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ArleEndingMealFiles {
    pub maddat: Vec<u8>,
    pub ending_com: Vec<u8>,
    pub report: ArleEndingMealReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArleEndingMealReport {
    pub asset_sha256: String,
    pub maddat_entry_id: usize,
    pub map_file_offset: usize,
    pub tile_address_hook_file_offset: usize,
    pub hook_file_offset: usize,
    pub hook_runtime_address: usize,
    pub hook_byte_size: usize,
    pub frame_count: usize,
    pub state_bank_indices: [usize; FRAME_COUNT],
    pub source_tile_count: usize,
    pub source_used_tile_count: usize,
    pub output_bank_tile_capacity: usize,
    pub output_used_tile_counts: [usize; 2],
    pub packed_input_size: usize,
    pub packed_output_size: usize,
    pub packed_staging_capacity: usize,
    pub packed_destination_file_offset: usize,
    pub unpack_block_file_offset: usize,
    pub unpack_block_byte_size: usize,
    pub changed_pixel_count: usize,
}

pub(crate) fn compile_arle_ending_meal(
    maddat: &[u8],
    ending_com: &[u8],
    asset_input: &ResolvedArleAsset,
) -> Result<ArleEndingMealFiles> {
    let archive = parse_flink_archive(maddat)?;
    let source_entry = archive
        .entries
        .get(MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing ending meal scene entry 145")?;
    ensure!(
        source_entry.id == MADDAT_ENTRY_ID,
        "ending meal scene entry ID differs from its table position"
    );
    ensure!(
        sha256_hex(source_entry.bytes) == SOURCE_ENTRY_SHA256,
        "ending meal source tile bank differs from the verified MADDAT entry"
    );
    let source_map_records = ending_meal_map_records(ending_com)?;
    ensure!(
        sha256_hex(&source_map_records.concat()) == SOURCE_MAPS_SHA256,
        "ENDING meal scene maps differ from the verified four-state source"
    );
    let source = decode_source_ending_meal_scenes(
        source_entry.bytes,
        std::array::from_fn(|index| source_map_records[index].as_slice()),
    )?;
    ensure!(
        source.used_tile_ids.len() == SOURCE_TILE_COUNT - 1,
        "supported ending meal maps use {} tiles instead of the verified {}",
        source.used_tile_ids.len(),
        SOURCE_TILE_COUNT - 1
    );

    let asset = load_arle_ending_meal_asset(asset_input)?;
    let changed_pixel_count = source
        .frames
        .iter()
        .zip(&asset.frames)
        .flat_map(|(source, output)| source.iter().zip(output))
        .filter(|(source, output)| source != output)
        .count();
    ensure!(
        changed_pixel_count > 0,
        "Arle ending meal asset makes no pixel changes"
    );

    let encoded = encode_split_ending_meal_scenes(&asset.frames, BACKGROUND_PALETTE_INDEX)?;
    ensure!(
        encoded.packed_tile_banks.len() <= usize::from(u16::MAX),
        "Arle ending meal replacement exceeds the FLINK u16 entry size"
    );
    let repacked = repack_contiguous_flink_archive(
        maddat,
        &BTreeMap::from([(MADDAT_ENTRY_ID, encoded.packed_tile_banks.clone())]),
        &[],
    )?;
    ensure!(
        repacked.replaced_entry_ids == BTreeSet::from([MADDAT_ENTRY_ID]),
        "Arle ending meal repack ownership differs from entry 145"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "arle-ending-meal-maddat-repacker",
        "replace all four ending meal composites with two complete state-paired tile banks",
    )?;
    let patched_loader =
        install_ending_meal_bank_loader(ending_com, encoded.packed_tile_banks.len())?;
    let patched_consumer = replace_ending_meal_maps_and_install_bank_selector(
        &patched_loader.bytes,
        &encoded.map_records,
    )?;

    let candidate_archive = parse_flink_archive(&checked_maddat)?;
    let candidate_maps = ending_meal_map_records(&patched_consumer.bytes)?;
    let candidate = decode_split_ending_meal_scenes(
        candidate_archive.entries[MADDAT_ENTRY_ID - 1].bytes,
        std::array::from_fn(|index| candidate_maps[index].as_slice()),
    )?;
    ensure!(
        candidate.frames == asset.frames,
        "Arle ending meal pixels did not survive the MADDAT and ENDING.COM writes"
    );
    ensure!(
        candidate
            .used_tile_ids_by_bank
            .each_ref()
            .map(BTreeSet::len)
            == encoded.used_tile_counts,
        "Arle ending meal bank selection changed the used tile population"
    );
    for (source, candidate) in archive.entries.iter().zip(&candidate_archive.entries) {
        if source.id != MADDAT_ENTRY_ID {
            ensure!(
                source.bytes == candidate.bytes,
                "Arle ending meal repack changed protected MADDAT entry {}",
                source.id
            );
        }
    }

    Ok(ArleEndingMealFiles {
        maddat: checked_maddat,
        ending_com: patched_consumer.bytes,
        report: ArleEndingMealReport {
            asset_sha256: asset.sha256,
            maddat_entry_id: MADDAT_ENTRY_ID,
            map_file_offset: MAP_FILE_OFFSET,
            tile_address_hook_file_offset: TILE_ADDRESS_FILE_OFFSET,
            hook_file_offset: patched_consumer.hook_file_offset,
            hook_runtime_address: patched_consumer.hook_runtime_address,
            hook_byte_size: patched_consumer.hook_byte_size,
            frame_count: FRAME_COUNT,
            state_bank_indices: [0, 0, 1, 1],
            source_tile_count: SOURCE_TILE_COUNT,
            source_used_tile_count: source.used_tile_ids.len(),
            output_bank_tile_capacity: OUTPUT_BANK_TILE_CAPACITY,
            output_used_tile_counts: encoded.used_tile_counts,
            packed_input_size: source_entry.bytes.len(),
            packed_output_size: encoded.packed_tile_banks.len(),
            packed_staging_capacity: patched_loader.packed_staging_capacity,
            packed_destination_file_offset: PACKED_DESTINATION_FILE_OFFSET,
            unpack_block_file_offset: UNPACK_BLOCK_FILE_OFFSET,
            unpack_block_byte_size: patched_loader.unpack_block_byte_size,
            changed_pixel_count,
        },
    })
}

fn ending_meal_map_records(bytes: &[u8]) -> Result<[Vec<u8>; FRAME_COUNT]> {
    let maps = bytes
        .get(MAP_FILE_OFFSET..MAP_FILE_OFFSET + FRAME_COUNT * MAP_RECORD_SIZE)
        .context("ENDING.COM is shorter than its four ending meal maps")?;
    Ok(std::array::from_fn(|frame_index| {
        let start = frame_index * MAP_RECORD_SIZE;
        maps[start..start + MAP_RECORD_SIZE].to_vec()
    }))
}

#[cfg(test)]
#[path = "arle_ending_meal_tests.rs"]
mod arle_ending_meal_tests;
