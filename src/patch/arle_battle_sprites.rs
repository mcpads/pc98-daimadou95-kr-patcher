use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::battle_sprite::{
    FRAME_COUNT, TILE_BANK_CAPACITY, TILE_BANK_ENTRY_IDS, TILE_MAP_ENTRY_ID,
    decode_battle_sprite_set, encode_battle_sprite_set,
};
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};

use super::arle_battle_sprites_asset::{BACKGROUND_PALETTE_INDEX, load_arle_battle_sprites_asset};
use super::maddat::apply_repacked_maddat;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ArleBattleSpritesFiles {
    pub maddat: Vec<u8>,
    pub report: ArleBattleSpritesReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArleBattleSpritesReport {
    pub asset_sha256: String,
    pub tile_map_entry_id: usize,
    pub tile_bank_entry_ids: [usize; 2],
    pub frame_count: usize,
    pub source_used_tile_counts: [usize; 2],
    pub output_used_tile_counts: [usize; 2],
    pub source_packed_sizes: [usize; 3],
    pub output_packed_sizes: [usize; 3],
    pub changed_pixel_count: usize,
}

pub(crate) fn compile_arle_battle_sprites(
    maddat: &[u8],
    asset_input: &ResolvedArleAsset,
) -> Result<ArleBattleSpritesFiles> {
    let archive = parse_flink_archive(maddat)?;
    let source_maps = archive
        .entries
        .get(TILE_MAP_ENTRY_ID - 1)
        .context("MADDAT is missing Arle battle map entry 16")?;
    let source_banks = TILE_BANK_ENTRY_IDS.map(|entry_id| {
        archive
            .entries
            .get(entry_id - 1)
            .with_context(|| format!("MADDAT is missing Arle battle tile bank entry {entry_id}"))
    });
    let source_banks = source_banks.into_iter().collect::<Result<Vec<_>>>()?;
    let source = decode_battle_sprite_set(
        source_maps.bytes,
        [source_banks[0].bytes, source_banks[1].bytes],
    )?;
    let source_used_tile_counts = source.used_tile_ids_by_bank.each_ref().map(BTreeSet::len);
    ensure!(
        source_used_tile_counts == [TILE_BANK_CAPACITY; 2],
        "supported Arle battle frames use {source_used_tile_counts:?} tiles instead of both verified 256-tile banks"
    );

    let asset = load_arle_battle_sprites_asset(asset_input)?;
    let changed_pixel_count = source
        .frames
        .iter()
        .zip(&asset.frames)
        .flat_map(|(source, output)| source.iter().zip(output))
        .filter(|(source, output)| source != output)
        .count();
    ensure!(
        changed_pixel_count > 0,
        "Arle battle sprite asset makes no pixel changes"
    );

    let encoded = encode_battle_sprite_set(&asset.frames, BACKGROUND_PALETTE_INDEX)?;
    let replacements = BTreeMap::from([
        (TILE_MAP_ENTRY_ID, encoded.packed_tile_maps.clone()),
        (TILE_BANK_ENTRY_IDS[0], encoded.packed_tile_banks[0].clone()),
        (TILE_BANK_ENTRY_IDS[1], encoded.packed_tile_banks[1].clone()),
    ]);
    ensure!(
        replacements
            .values()
            .all(|bytes| bytes.len() <= usize::from(u16::MAX)),
        "Arle battle replacement exceeds a FLINK u16 entry size"
    );
    let repacked = repack_contiguous_flink_archive(maddat, &replacements, &[])?;
    let replaced_entry_ids = BTreeSet::from([
        TILE_MAP_ENTRY_ID,
        TILE_BANK_ENTRY_IDS[0],
        TILE_BANK_ENTRY_IDS[1],
    ]);
    ensure!(
        repacked.replaced_entry_ids == replaced_entry_ids,
        "Arle battle repack ownership differs from entries 16, 38, and 39"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "arle-battle-sprites-maddat-repacker",
        "replace all ten Arle battle frames and their two complete tile banks",
    )?;

    let candidate_archive = parse_flink_archive(&checked_maddat)?;
    let candidate = decode_battle_sprite_set(
        candidate_archive.entries[TILE_MAP_ENTRY_ID - 1].bytes,
        [
            candidate_archive.entries[TILE_BANK_ENTRY_IDS[0] - 1].bytes,
            candidate_archive.entries[TILE_BANK_ENTRY_IDS[1] - 1].bytes,
        ],
    )?;
    ensure!(
        candidate.frames == asset.frames,
        "Arle battle sprite pixels did not survive the MADDAT repack"
    );
    for (source_entry, candidate_entry) in archive.entries.iter().zip(&candidate_archive.entries) {
        if !replaced_entry_ids.contains(&source_entry.id) {
            ensure!(
                source_entry.bytes == candidate_entry.bytes,
                "Arle battle repack changed protected MADDAT entry {}",
                source_entry.id
            );
        }
    }

    Ok(ArleBattleSpritesFiles {
        maddat: checked_maddat,
        report: ArleBattleSpritesReport {
            asset_sha256: asset.sha256,
            tile_map_entry_id: TILE_MAP_ENTRY_ID,
            tile_bank_entry_ids: TILE_BANK_ENTRY_IDS,
            frame_count: FRAME_COUNT,
            source_used_tile_counts,
            output_used_tile_counts: encoded.used_tile_counts,
            source_packed_sizes: [
                source_maps.bytes.len(),
                source_banks[0].bytes.len(),
                source_banks[1].bytes.len(),
            ],
            output_packed_sizes: [
                encoded.packed_tile_maps.len(),
                encoded.packed_tile_banks[0].len(),
                encoded.packed_tile_banks[1].len(),
            ],
            changed_pixel_count,
        },
    })
}

#[cfg(test)]
#[path = "arle_battle_sprites_tests.rs"]
mod arle_battle_sprites_tests;
