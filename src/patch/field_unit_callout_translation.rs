use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use anyhow::{Context, Result, ensure};

use crate::game_data::compile_lz::encode_compile_lz;
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::masked_sprite::{
    BYTES_PER_TILE, MaskedSpriteBank, MaskedSpriteTile, TILE_HEIGHT, TILE_WIDTH,
    decode_masked_sprite_bank, encode_masked_sprite_tile,
};
use crate::source::{FieldUnitCallout, catalog_state_graphics, disk};
use crate::translation::ReviewedTranslationCorpus;

use super::field_unit_callout_compositor::{
    FieldUnitCalloutComposition, HEIGHT, TRANSPARENT_PALETTE_INDEX, WIDTH,
    compose_field_unit_callout,
};
use super::maddat::apply_repacked_maddat;

const FIRST_TRANSLATED_UNIT_CLASS_ID: usize = 1;
const LAST_TRANSLATED_UNIT_CLASS_ID: usize = 19;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FieldUnitCalloutTranslationFiles {
    pub maddat: Vec<u8>,
    pub report: FieldUnitCalloutTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct FieldUnitCalloutTranslationReport {
    pub callouts: Vec<FieldUnitCalloutTranslationEntryReport>,
    pub changed_bank_count: usize,
    pub changed_pixel_count: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct FieldUnitCalloutTranslationEntryReport {
    pub segment_id: String,
    pub translation_entry_id: String,
    pub maddat_entry_id: usize,
    pub runtime_unit_class_id: usize,
    pub tile_ids: [usize; 3],
    pub korean_text: String,
    pub packed_input_size: usize,
    pub packed_output_size: usize,
    pub changed_pixel_count: usize,
    pub opaque_pixel_count: usize,
    pub transparent_pixel_count: usize,
    pub opaque_palette_indices: Vec<u8>,
    pub transparent_palette_indices: Vec<u8>,
}

pub(crate) fn compile_field_unit_callout_translation(
    maddat: &[u8],
    mad_com: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<FieldUnitCalloutTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "field-unit callout build requires a human-review-ready translation corpus"
    );

    let archive = parse_flink_archive(maddat)?;
    let source_catalog = catalog_state_graphics(maddat, mad_com)?.field_unit_callouts;
    let translated_callouts = source_catalog
        .callouts
        .iter()
        .filter(|callout| callout.source_text.is_some())
        .collect::<Vec<_>>();
    ensure!(
        translated_callouts
            .iter()
            .map(|callout| callout.runtime_unit_class_id)
            .collect::<BTreeSet<_>>()
            == (FIRST_TRANSLATED_UNIT_CLASS_ID..=LAST_TRANSLATED_UNIT_CLASS_ID).collect(),
        "translated field-unit callout population differs from runtime classes 1..19"
    );

    let mut source_banks = BTreeMap::new();
    let mut planned_tiles: BTreeMap<usize, BTreeMap<usize, MaskedSpriteTile>> = BTreeMap::new();
    let mut callouts = Vec::with_capacity(translated_callouts.len());

    for source_callout in translated_callouts {
        let runtime_unit_class_id = source_callout.runtime_unit_class_id;
        let segment_id = format!("graphic-field-unit-callout-{runtime_unit_class_id:02}");
        let translation_entry_id =
            format!("graphic-text:field-unit-callout-{runtime_unit_class_id:02}:01");
        let maddat_entry_id = source_callout.maddat_entry_id;
        let tile_ids = callout_tile_ids(source_callout)?;
        let segment = corpus
            .segments
            .iter()
            .find(|segment| segment.protected.id == segment_id)
            .with_context(|| format!("translation corpus is missing {segment_id}"))?;
        let korean_text = segment.reinsertion_text(&translation_entry_id)?;
        let protected_entry = segment
            .protected
            .entries
            .iter()
            .find(|entry| entry.id == translation_entry_id)
            .with_context(|| {
                format!("protected translation segment is missing {translation_entry_id}")
            })?;
        let graphic_asset = protected_entry
            .evidence
            .graphic_asset
            .as_ref()
            .with_context(|| format!("{translation_entry_id} has no protected graphic asset"))?;
        ensure!(
            protected_entry.source_record_id
                == format!("field-unit-callout-{runtime_unit_class_id:02}:01")
                && graphic_asset.maddat_entry_id == maddat_entry_id
                && graphic_asset.width == WIDTH
                && graphic_asset.height == HEIGHT
                && graphic_asset.plane_count == 5,
            "{translation_entry_id} differs from the adopted field-unit callout identity"
        );

        let source_entry = archive.entries.get(maddat_entry_id - 1).with_context(|| {
            format!("MADDAT is missing field-unit callout bank {maddat_entry_id}")
        })?;
        ensure!(
            source_entry.id == maddat_entry_id,
            "field-unit callout bank {maddat_entry_id} differs from its table position"
        );
        ensure!(
            source_entry.bytes.len() == graphic_asset.packed_size
                && disk::sha256_hex(source_entry.bytes) == graphic_asset.packed_sha256,
            "MADDAT {maddat_entry_id} packed bytes differ from the protected callout source"
        );
        if let Entry::Vacant(slot) = source_banks.entry(maddat_entry_id) {
            let source_bank = decode_masked_sprite_bank(source_entry.bytes)?;
            validate_source_bank(
                maddat_entry_id,
                &source_bank,
                graphic_asset.decoded_size,
                &graphic_asset.decoded_sha256,
            )?;
            slot.insert(source_bank);
        }
        let source_bank = source_banks
            .get(&maddat_entry_id)
            .expect("field-unit callout source bank was inserted");

        let composition = compose_field_unit_callout(&korean_text)?;
        let changed_pixel_count =
            count_changed_callout_pixels(source_bank, tile_ids, &composition)?;
        ensure!(
            changed_pixel_count > 0,
            "field-unit callout {runtime_unit_class_id} translation made no pixel changes"
        );
        let bank_plan = planned_tiles.entry(maddat_entry_id).or_default();
        for (column, tile_id) in tile_ids.into_iter().enumerate() {
            let tile = composition_tile(&composition, column)?;
            match bank_plan.entry(tile_id) {
                Entry::Vacant(slot) => {
                    slot.insert(tile);
                }
                Entry::Occupied(slot) => ensure!(
                    slot.get() == &tile,
                    "field-unit callout class {runtime_unit_class_id} conflicts with another translation at MADDAT {maddat_entry_id} tile {tile_id}"
                ),
            }
        }

        let opaque_pixel_count = composition.opaque.iter().filter(|pixel| **pixel).count();
        let transparent_pixel_count = composition.opaque.len() - opaque_pixel_count;
        let opaque_palette_indices = palette_indices_for_mask(&composition, true);
        let transparent_palette_indices = palette_indices_for_mask(&composition, false);
        callouts.push(FieldUnitCalloutTranslationEntryReport {
            segment_id,
            translation_entry_id,
            maddat_entry_id,
            runtime_unit_class_id,
            tile_ids,
            korean_text,
            packed_input_size: source_entry.bytes.len(),
            packed_output_size: 0,
            changed_pixel_count,
            opaque_pixel_count,
            transparent_pixel_count,
            opaque_palette_indices,
            transparent_palette_indices,
        });
    }

    let mut replacements = BTreeMap::new();
    let mut changed_pixel_count = 0;
    for (maddat_entry_id, tile_plan) in &planned_tiles {
        let source_bank = source_banks
            .get(maddat_entry_id)
            .expect("planned field-unit callout bank has a decoded source");
        changed_pixel_count += count_changed_planned_pixels(source_bank, tile_plan)?;
        let decoded = replace_planned_callout_tiles(&source_bank.decoded, tile_plan)?;
        let packed = encode_compile_lz(&decoded)?;
        ensure!(
            packed.len() <= usize::from(u16::MAX),
            "translated MADDAT {maddat_entry_id} is {} bytes and exceeds the FLINK u16 size field",
            packed.len()
        );
        let output_bank = decode_masked_sprite_bank(&packed)?;
        validate_output_bank(source_bank, &output_bank, tile_plan)?;
        for report in callouts
            .iter_mut()
            .filter(|report| report.maddat_entry_id == *maddat_entry_id)
        {
            report.packed_output_size = packed.len();
        }
        ensure!(
            replacements.insert(*maddat_entry_id, packed).is_none(),
            "duplicate field-unit callout replacement for MADDAT {maddat_entry_id}"
        );
    }

    let replaced_entry_ids = replacements.keys().copied().collect::<BTreeSet<_>>();
    let expected_replaced_entry_ids = std::iter::once(8).chain(111..=119).collect::<BTreeSet<_>>();
    ensure!(
        replaced_entry_ids == expected_replaced_entry_ids,
        "field-unit callout replacement population differs from MADDAT 8 and 111..119"
    );
    let repacked = repack_contiguous_flink_archive(maddat, &replacements, &[])?;
    ensure!(
        repacked.replaced_entry_ids == replaced_entry_ids,
        "field-unit callout MADDAT repack ownership differs from entries 8 and 111..119"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "field-unit-callout-maddat-repacker",
        "replace only reviewed fixed-bank and dynamic-bank callout tiles in MADDAT 8 and 111..119 with Korean compositions",
    )?;
    let checked_archive = parse_flink_archive(&checked_maddat)?;
    ensure!(
        checked_archive.entries.len() == archive.entries.len(),
        "translated field-unit callouts changed the MADDAT entry population"
    );
    for (source_entry, checked_entry) in archive.entries.iter().zip(&checked_archive.entries) {
        if let Some(expected) = replacements.get(&source_entry.id) {
            ensure!(
                checked_entry.bytes == expected,
                "translated field-unit callout bank {} did not survive MADDAT repack",
                source_entry.id
            );
        } else {
            ensure!(
                checked_entry.bytes == source_entry.bytes,
                "field-unit callout repack changed protected MADDAT entry {}",
                source_entry.id
            );
        }
    }

    Ok(FieldUnitCalloutTranslationFiles {
        maddat: checked_maddat,
        report: FieldUnitCalloutTranslationReport {
            changed_bank_count: replacements.len(),
            changed_pixel_count,
            callouts,
        },
    })
}

fn validate_source_bank(
    maddat_entry_id: usize,
    source: &MaskedSpriteBank,
    protected_decoded_size: usize,
    protected_decoded_sha256: &str,
) -> Result<()> {
    ensure!(
        source.tiles.len() == 128 && source.decoded.len() == 128 * BYTES_PER_TILE,
        "MADDAT {maddat_entry_id} does not contain 128 masked-sprite records"
    );
    ensure!(
        source.decoded.len() == protected_decoded_size
            && disk::sha256_hex(&source.decoded) == protected_decoded_sha256,
        "MADDAT {maddat_entry_id} decoded bytes differ from the protected callout source"
    );
    Ok(())
}

fn palette_indices_for_mask(
    composition: &FieldUnitCalloutComposition,
    opaque_value: bool,
) -> Vec<u8> {
    composition
        .opaque
        .iter()
        .zip(&composition.palette_indices)
        .filter_map(|(opaque, palette_index)| (*opaque == opaque_value).then_some(*palette_index))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn callout_tile_ids(callout: &FieldUnitCallout) -> Result<[usize; 3]> {
    callout
        .tile_ids
        .iter()
        .map(|tile_id| usize::from(*tile_id))
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|tile_ids: Vec<usize>| {
            anyhow::anyhow!(
                "field-unit callout class {} selects {} tiles instead of 3",
                callout.runtime_unit_class_id,
                tile_ids.len()
            )
        })
}

fn replace_planned_callout_tiles(
    source_decoded: &[u8],
    planned_tiles: &BTreeMap<usize, MaskedSpriteTile>,
) -> Result<Vec<u8>> {
    ensure!(
        source_decoded.len() == 128 * BYTES_PER_TILE,
        "field-unit callout source bank has {} bytes instead of {}",
        source_decoded.len(),
        128 * BYTES_PER_TILE
    );
    ensure!(
        !planned_tiles.is_empty(),
        "field-unit callout bank has no planned tile replacements"
    );
    let mut output = source_decoded.to_vec();
    for (&tile_id, tile) in planned_tiles {
        ensure!(
            tile_id < 128,
            "field-unit callout replacement tile {tile_id} lies outside the 128-tile bank"
        );
        let encoded = encode_masked_sprite_tile(tile)?;
        let start = tile_id * BYTES_PER_TILE;
        output[start..start + BYTES_PER_TILE].copy_from_slice(&encoded);
    }
    for tile_id in 0..128 {
        if planned_tiles.contains_key(&tile_id) {
            continue;
        }
        let start = tile_id * BYTES_PER_TILE;
        ensure!(
            output[start..start + BYTES_PER_TILE] == source_decoded[start..start + BYTES_PER_TILE],
            "field-unit callout replacement changed protected tile {tile_id}"
        );
    }
    Ok(output)
}

fn composition_tile(
    composition: &FieldUnitCalloutComposition,
    column: usize,
) -> Result<MaskedSpriteTile> {
    ensure!(column < 3, "field-unit callout column is outside 0..3");
    ensure!(
        composition.opaque.len() == WIDTH * HEIGHT
            && composition.palette_indices.len() == WIDTH * HEIGHT,
        "field-unit callout composition has an incomplete 48x16 canvas"
    );
    let mut opaque = vec![false; TILE_WIDTH * TILE_HEIGHT];
    let mut palette_indices = vec![TRANSPARENT_PALETTE_INDEX; TILE_WIDTH * TILE_HEIGHT];
    for y in 0..TILE_HEIGHT {
        for x in 0..TILE_WIDTH {
            let source = y * WIDTH + column * TILE_WIDTH + x;
            let target = y * TILE_WIDTH + x;
            opaque[target] = composition.opaque[source];
            palette_indices[target] = composition.palette_indices[source];
        }
    }
    Ok(MaskedSpriteTile {
        opaque,
        palette_indices,
    })
}

fn validate_output_bank(
    source: &MaskedSpriteBank,
    output: &MaskedSpriteBank,
    planned_tiles: &BTreeMap<usize, MaskedSpriteTile>,
) -> Result<()> {
    ensure!(
        output.tiles.len() == source.tiles.len(),
        "translated field-unit callout changed the bank tile population"
    );
    for tile_id in 0..source.tiles.len() {
        if let Some(planned) = planned_tiles.get(&tile_id) {
            ensure!(
                &output.tiles[tile_id] == planned,
                "translated field-unit callout tile {tile_id} does not match its planned composition"
            );
        } else {
            ensure!(
                output.tiles[tile_id] == source.tiles[tile_id],
                "translated field-unit callout changed protected tile {tile_id}"
            );
        }
    }
    Ok(())
}

fn count_changed_callout_pixels(
    source: &MaskedSpriteBank,
    tile_ids: [usize; 3],
    composition: &FieldUnitCalloutComposition,
) -> Result<usize> {
    ensure!(
        tile_ids.iter().all(|tile_id| *tile_id < source.tiles.len()),
        "field-unit callout source bank is missing one of tiles {tile_ids:?}"
    );
    let mut changed = 0;
    for (column, tile_id) in tile_ids.into_iter().enumerate() {
        let tile = &source.tiles[tile_id];
        for y in 0..TILE_HEIGHT {
            for x in 0..TILE_WIDTH {
                let source_pixel = y * TILE_WIDTH + x;
                let target_pixel = y * WIDTH + column * TILE_WIDTH + x;
                if tile.opaque[source_pixel] != composition.opaque[target_pixel]
                    || tile.palette_indices[source_pixel]
                        != composition.palette_indices[target_pixel]
                {
                    changed += 1;
                }
            }
        }
    }
    Ok(changed)
}

fn count_changed_planned_pixels(
    source: &MaskedSpriteBank,
    planned_tiles: &BTreeMap<usize, MaskedSpriteTile>,
) -> Result<usize> {
    let mut changed = 0;
    for (&tile_id, planned) in planned_tiles {
        let source_tile = source.tiles.get(tile_id).with_context(|| {
            format!("field-unit callout source bank is missing planned tile {tile_id}")
        })?;
        ensure!(
            source_tile.opaque.len() == planned.opaque.len()
                && source_tile.palette_indices.len() == planned.palette_indices.len(),
            "field-unit callout tile {tile_id} has incompatible source and planned dimensions"
        );
        changed += source_tile
            .opaque
            .iter()
            .zip(&source_tile.palette_indices)
            .zip(planned.opaque.iter().zip(&planned.palette_indices))
            .filter(
                |((source_opaque, source_palette), (planned_opaque, planned_palette))| {
                    source_opaque != planned_opaque || source_palette != planned_palette
                },
            )
            .count();
    }
    Ok(changed)
}

#[cfg(test)]
#[path = "field_unit_callout_translation_tests.rs"]
mod field_unit_callout_translation_tests;
