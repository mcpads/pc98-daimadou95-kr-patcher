use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::small_status::{
    FRAME_COUNT, MAP_RECORD_SIZE, TILE_BANK_ENTRY_ID, decode_small_status_set,
    encode_small_status_set,
};

use super::arle_large_portrait_asset::BACKGROUND_PALETTE_INDEX;
use super::arle_small_status_asset::load_arle_small_status_asset;
use super::maddat::apply_repacked_maddat;

const MAD_MAP_FILE_OFFSETS: [usize; FRAME_COUNT] = [0x7129, 0x713b, 0x714d, 0x715f];
const SOURCE_MAP_RECORDS: [[u8; MAP_RECORD_SIZE]; FRAME_COUNT] = [
    [4, 4, 0, 1, 2, 3, 5, 6, 7, 8, 11, 12, 13, 14, 15, 16, 17, 18],
    [4, 4, 0, 1, 2, 3, 5, 6, 7, 8, 11, 12, 13, 14, 15, 26, 27, 18],
    [
        4, 4, 0, 1, 2, 3, 5, 9, 10, 8, 11, 12, 13, 14, 15, 19, 20, 18,
    ],
    [
        4, 4, 0, 1, 2, 3, 5, 21, 22, 8, 11, 23, 24, 25, 15, 28, 4, 18,
    ],
];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ArleSmallStatusFiles {
    pub maddat: Vec<u8>,
    pub mad_com: Vec<u8>,
    pub report: ArleSmallStatusReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArleSmallStatusReport {
    pub asset_sha256: String,
    pub tile_bank_entry_id: usize,
    pub map_file_offsets: [usize; FRAME_COUNT],
    pub frame_count: usize,
    pub source_used_tile_count: usize,
    pub output_used_tile_count: usize,
    pub source_tile_bank_packed_size: usize,
    pub output_tile_bank_packed_size: usize,
    pub changed_pixel_count: usize,
}

pub(crate) fn compile_arle_small_status(
    maddat: &[u8],
    mad_com: &[u8],
    asset_input: &ResolvedArleAsset,
) -> Result<ArleSmallStatusFiles> {
    let archive = parse_flink_archive(maddat)?;
    let source_tile_bank = archive
        .entries
        .get(TILE_BANK_ENTRY_ID - 1)
        .context("MADDAT is missing Arle small status tile bank entry 73")?;
    ensure!(
        source_tile_bank.id == TILE_BANK_ENTRY_ID,
        "Arle small status entry ID differs from its table position"
    );
    let original_map_records = source_map_records(mad_com)?;
    let source = decode_small_status_set(
        source_tile_bank.bytes,
        std::array::from_fn(|index| original_map_records[index].as_slice()),
    )?;
    ensure!(
        source.used_tile_ids.len() == 29,
        "supported Arle small status maps use {} tiles instead of the verified 29",
        source.used_tile_ids.len()
    );

    let asset = load_arle_small_status_asset(asset_input)?;
    let changed_pixel_count = source
        .frames
        .iter()
        .zip(&asset.frames)
        .flat_map(|(source, output)| source.iter().zip(output))
        .filter(|(source, output)| source != output)
        .count();
    ensure!(
        changed_pixel_count > 0,
        "Arle small status asset makes no pixel changes"
    );

    let encoded = encode_small_status_set(&asset.frames, BACKGROUND_PALETTE_INDEX)?;
    ensure!(
        encoded.packed_tile_bank.len() <= usize::from(u16::MAX),
        "Arle small status replacement exceeds a FLINK u16 entry size"
    );
    let repacked = repack_contiguous_flink_archive(
        maddat,
        &BTreeMap::from([(TILE_BANK_ENTRY_ID, encoded.packed_tile_bank.clone())]),
        &[],
    )?;
    ensure!(
        repacked.replaced_entry_ids == BTreeSet::from([TILE_BANK_ENTRY_ID]),
        "Arle small status repack ownership differs from entry 73"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "arle-small-status-maddat-repacker",
        "replace the four-state lower-right Arle status tile bank",
    )?;
    let checked_mad_com = replace_small_status_maps(mad_com, &encoded.map_records)?;

    let candidate_archive = parse_flink_archive(&checked_maddat)?;
    let candidate_maps = source_map_records(&checked_mad_com)?;
    let candidate = decode_small_status_set(
        candidate_archive.entries[TILE_BANK_ENTRY_ID - 1].bytes,
        std::array::from_fn(|index| candidate_maps[index].as_slice()),
    )?;
    ensure!(
        candidate.frames == asset.frames,
        "Arle small status pixels did not survive the MADDAT and MAD.COM repack"
    );
    for (source_entry, candidate_entry) in archive.entries.iter().zip(&candidate_archive.entries) {
        if source_entry.id != TILE_BANK_ENTRY_ID {
            ensure!(
                source_entry.bytes == candidate_entry.bytes,
                "Arle small status repack changed protected MADDAT entry {}",
                source_entry.id
            );
        }
    }

    Ok(ArleSmallStatusFiles {
        maddat: checked_maddat,
        mad_com: checked_mad_com,
        report: ArleSmallStatusReport {
            asset_sha256: asset.sha256,
            tile_bank_entry_id: TILE_BANK_ENTRY_ID,
            map_file_offsets: MAD_MAP_FILE_OFFSETS,
            frame_count: FRAME_COUNT,
            source_used_tile_count: source.used_tile_ids.len(),
            output_used_tile_count: encoded.used_tile_count,
            source_tile_bank_packed_size: source_tile_bank.bytes.len(),
            output_tile_bank_packed_size: encoded.packed_tile_bank.len(),
            changed_pixel_count,
        },
    })
}

fn source_map_records(bytes: &[u8]) -> Result<[Vec<u8>; FRAME_COUNT]> {
    MAD_MAP_FILE_OFFSETS
        .map(|offset| {
            let record = bytes
                .get(offset..offset + MAP_RECORD_SIZE)
                .context("MAD.COM is shorter than an Arle small status map")?;
            Ok(record.to_vec())
        })
        .into_iter()
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .map_err(|_| anyhow::anyhow!("four map offsets must produce four status records"))
}

fn replace_small_status_maps(
    baseline: &[u8],
    replacements: &[Vec<u8>; FRAME_COUNT],
) -> Result<Vec<u8>> {
    let mut plan = WritePlan::new();
    for (frame_index, (&offset, replacement)) in
        MAD_MAP_FILE_OFFSETS.iter().zip(replacements).enumerate()
    {
        ensure!(
            replacement.len() == MAP_RECORD_SIZE,
            "Arle small status map {frame_index} has {} bytes instead of {MAP_RECORD_SIZE}",
            replacement.len()
        );
        plan = plan
            .region(ImageRegion {
                id: format!("arle-small-status-map-{frame_index}"),
                range: offset..offset + MAP_RECORD_SIZE,
                kind: RegionKind::Data,
                reason: "MAD.COM unit-10 lower-right status tile map".into(),
            })
            .write(ExpectedWrite {
                id: format!("replace-arle-small-status-map-{frame_index}"),
                owner: "arle-small-status-map-patch".into(),
                purpose: "bind one authored status expression to its verified MAD.COM consumer"
                    .into(),
                offset,
                expected_original: SOURCE_MAP_RECORDS[frame_index].to_vec(),
                replacement: replacement.clone(),
                intent: WriteIntent::Data,
            });
    }
    Ok(plan.apply(baseline, None)?)
}

#[cfg(test)]
#[path = "arle_small_status_tests.rs"]
mod arle_small_status_tests;
