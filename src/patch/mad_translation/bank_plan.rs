use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::font::rasterize_pc98_character;
use crate::game_data::indexed_text::{FONT_GLYPH_SIZE, verify_indexed_font};
use crate::source::MadIndexedTextCatalog;

use super::MadTranslation;
use crate::patch::indexed_font_bank::{
    IndexedFontBank, IndexedFontGlyph, compile_mapped_indexed_font_bank,
};

pub(super) const MAD_FONT_FIRST_ENTRY_ID: usize = 166;
const TERMINATOR_INDEX: u8 = 0xff;
const USABLE_GLYPH_COUNT: usize = TERMINATOR_INDEX as usize;

pub(super) struct MadFontPlan {
    pub base_codebook: BTreeMap<char, u8>,
    pub unit_codebooks: BTreeMap<u8, BTreeMap<char, u8>>,
    pub unit_font_entry_ids: BTreeMap<u8, usize>,
    pub result_banner_top: Vec<u8>,
    pub result_banner_bottom: Vec<u8>,
    pub result_banner_tile_columns: usize,
    pub banks: Vec<MadCompiledFontBank>,
}

pub(super) struct MadCompiledFontBank {
    pub entry_id: usize,
    pub bank_id: String,
    pub unit_numbers: Vec<u8>,
    pub slot_count: usize,
    pub compiled: IndexedFontBank,
}

struct UnitDemand {
    unit_number: u8,
    characters: BTreeSet<char>,
}

struct UnitGroup {
    unit_numbers: Vec<u8>,
    characters: BTreeSet<char>,
}

struct CompositeBanner {
    glyphs: BTreeMap<u8, IndexedFontGlyph>,
    top: Vec<u8>,
    bottom: Vec<u8>,
    tile_columns: usize,
}

pub(super) fn compile_mad_font_plan(
    source_font: &[u8],
    source: &MadIndexedTextCatalog,
    translations: &[MadTranslation],
) -> Result<MadFontPlan> {
    verify_indexed_font(source_font)?;
    let result_banner = translations
        .iter()
        .find(|translation| translation.record_id == "result-banner")
        .context("MAD translations are missing the result banner")?;
    ensure!(
        translations
            .iter()
            .filter(|translation| translation.record_id == "result-banner")
            .count()
            == 1,
        "MAD translations contain more than one result banner"
    );

    let mut base_characters = source
        .strings
        .iter()
        .filter(|string| string.bound_to_consumer && !string.contains_japanese)
        .filter_map(|string| string.text.as_deref())
        .flat_map(str::chars)
        .collect::<BTreeSet<_>>();
    let mut unit_characters = BTreeMap::<u8, BTreeSet<char>>::new();
    for translation in translations {
        if translation.record_id == "result-banner" {
            continue;
        }
        if let Some(unit_number) = battle_unit_number(&translation.segment_id)? {
            unit_characters
                .entry(unit_number)
                .or_default()
                .extend(translation.text.chars());
        } else {
            base_characters.extend(translation.text.chars());
        }
    }
    base_characters.insert(' ');

    let base_codebook = sequential_codebook(&base_characters, 0)?;
    let result_banner = compile_result_banner(&result_banner.text, &base_codebook)?;
    let custom_glyphs = result_banner.glyphs;
    let result_banner_top = result_banner.top;
    let result_banner_bottom = result_banner.bottom;
    let result_banner_tile_columns = result_banner.tile_columns;
    let custom_end = base_codebook
        .len()
        .checked_add(custom_glyphs.len())
        .context("MAD base font slot count overflow")?;
    ensure!(
        custom_end <= USABLE_GLYPH_COUNT,
        "MAD base text and composite banner require {custom_end} of {USABLE_GLYPH_COUNT} usable glyphs"
    );

    let unit_demands = unit_characters
        .into_iter()
        .map(|(unit_number, characters)| UnitDemand {
            unit_number,
            characters,
        })
        .collect();
    let groups = pack_unit_demands(unit_demands, &base_characters, custom_glyphs.len())?;
    let preserved = BTreeSet::from([TERMINATOR_INDEX]);
    let mut banks = Vec::with_capacity(groups.len() + 1);
    let base_compiled = compile_mapped_indexed_font_bank(
        source_font,
        base_codebook.clone(),
        &custom_glyphs,
        &preserved,
        "mad-base",
    )?;
    banks.push(MadCompiledFontBank {
        entry_id: MAD_FONT_FIRST_ENTRY_ID,
        bank_id: "mad-base".to_owned(),
        unit_numbers: Vec::new(),
        slot_count: custom_end,
        compiled: base_compiled,
    });

    let mut unit_codebooks = BTreeMap::new();
    let mut unit_font_entry_ids = BTreeMap::new();
    for (group_index, group) in groups.into_iter().enumerate() {
        let entry_id = MAD_FONT_FIRST_ENTRY_ID + group_index + 1;
        let mut codebook = base_codebook.clone();
        let extras = group
            .characters
            .difference(&base_characters)
            .copied()
            .collect::<BTreeSet<_>>();
        codebook.extend(sequential_codebook(&extras, custom_end)?);
        let slot_count = codebook.len() + custom_glyphs.len();
        ensure!(
            slot_count <= USABLE_GLYPH_COUNT,
            "MAD battle font for units {:?} requires {slot_count} of {USABLE_GLYPH_COUNT} usable glyphs",
            group.unit_numbers
        );
        let bank_id = format!(
            "mad-battle-units-{}",
            group
                .unit_numbers
                .iter()
                .map(|unit| format!("{unit:02}"))
                .collect::<Vec<_>>()
                .join("-")
        );
        let compiled = compile_mapped_indexed_font_bank(
            source_font,
            codebook.clone(),
            &custom_glyphs,
            &preserved,
            &bank_id,
        )?;
        for unit_number in &group.unit_numbers {
            ensure!(
                unit_codebooks
                    .insert(*unit_number, codebook.clone())
                    .is_none()
                    && unit_font_entry_ids.insert(*unit_number, entry_id).is_none(),
                "MAD unit {unit_number:02} was assigned to more than one font bank"
            );
        }
        banks.push(MadCompiledFontBank {
            entry_id,
            bank_id,
            unit_numbers: group.unit_numbers,
            slot_count,
            compiled,
        });
    }

    Ok(MadFontPlan {
        base_codebook,
        unit_codebooks,
        unit_font_entry_ids,
        result_banner_top,
        result_banner_bottom,
        result_banner_tile_columns,
        banks,
    })
}

fn pack_unit_demands(
    mut demands: Vec<UnitDemand>,
    base_characters: &BTreeSet<char>,
    custom_glyph_count: usize,
) -> Result<Vec<UnitGroup>> {
    demands.sort_by(|left, right| {
        right
            .characters
            .len()
            .cmp(&left.characters.len())
            .then(left.unit_number.cmp(&right.unit_number))
    });
    let mut groups: Vec<UnitGroup> = Vec::new();
    for demand in demands {
        let mut best = None;
        for (index, group) in groups.iter().enumerate() {
            let merged = group
                .characters
                .union(&demand.characters)
                .copied()
                .collect::<BTreeSet<_>>();
            let slot_count = base_characters.union(&merged).count() + custom_glyph_count;
            if slot_count <= USABLE_GLYPH_COUNT
                && best.is_none_or(|(_, best_size)| slot_count < best_size)
            {
                best = Some((index, slot_count));
            }
        }
        if let Some((index, _)) = best {
            groups[index].unit_numbers.push(demand.unit_number);
            groups[index].characters.extend(demand.characters);
        } else {
            let slot_count = base_characters.union(&demand.characters).count() + custom_glyph_count;
            ensure!(
                slot_count <= USABLE_GLYPH_COUNT,
                "MAD unit {:02} alone requires {slot_count} of {USABLE_GLYPH_COUNT} usable glyphs",
                demand.unit_number
            );
            groups.push(UnitGroup {
                unit_numbers: vec![demand.unit_number],
                characters: demand.characters,
            });
        }
    }
    for group in &mut groups {
        group.unit_numbers.sort_unstable();
    }
    Ok(groups)
}

fn compile_result_banner(
    text: &str,
    base_codebook: &BTreeMap<char, u8>,
) -> Result<CompositeBanner> {
    let banner_characters = text
        .chars()
        .filter(|character| *character != ' ')
        .collect::<BTreeSet<_>>();
    let first_custom_index = base_codebook.len();
    let custom_count = banner_characters
        .len()
        .checked_mul(4)
        .context("MAD composite banner glyph count overflow")?;
    ensure!(
        first_custom_index + custom_count <= USABLE_GLYPH_COUNT,
        "MAD composite banner exceeds the indexed glyph space"
    );

    let mut custom_glyphs = BTreeMap::new();
    let mut tile_indices = BTreeMap::<char, [u8; 4]>::new();
    for (character_index, character) in banner_characters.into_iter().enumerate() {
        let tiles = scale_character_tiles(character)?;
        let first_index = first_custom_index + character_index * 4;
        let indices = [
            u8::try_from(first_index).context("MAD banner top-left index overflow")?,
            u8::try_from(first_index + 1).context("MAD banner top-right index overflow")?,
            u8::try_from(first_index + 2).context("MAD banner bottom-left index overflow")?,
            u8::try_from(first_index + 3).context("MAD banner bottom-right index overflow")?,
        ];
        for (tile_index, (&index, bitmap)) in indices.iter().zip(tiles).enumerate() {
            custom_glyphs.insert(
                index,
                IndexedFontGlyph {
                    label: format!("result banner {character:?} tile {tile_index}"),
                    bitmap,
                },
            );
        }
        tile_indices.insert(character, indices);
    }

    let space_index = *base_codebook
        .get(&' ')
        .context("MAD base codebook has no space for the composite banner")?;
    let mut top = Vec::with_capacity(text.chars().count() * 2 + 1);
    let mut bottom = Vec::with_capacity(text.chars().count() * 2 + 1);
    for character in text.chars() {
        if character == ' ' {
            top.extend_from_slice(&[space_index, space_index]);
            bottom.extend_from_slice(&[space_index, space_index]);
        } else {
            let indices = tile_indices
                .get(&character)
                .with_context(|| format!("MAD banner has no tiles for {character:?}"))?;
            top.extend_from_slice(&indices[..2]);
            bottom.extend_from_slice(&indices[2..]);
        }
    }
    top.push(TERMINATOR_INDEX);
    bottom.push(TERMINATOR_INDEX);
    let tile_columns = top.len() - 1;
    ensure!(
        tile_columns > 0 && tile_columns.is_multiple_of(2) && bottom.len() == tile_columns + 1,
        "MAD composite banner rows have inconsistent tile geometry"
    );
    Ok(CompositeBanner {
        glyphs: custom_glyphs,
        top,
        bottom,
        tile_columns,
    })
}

fn sequential_codebook(
    characters: &BTreeSet<char>,
    first_index: usize,
) -> Result<BTreeMap<char, u8>> {
    characters
        .iter()
        .copied()
        .enumerate()
        .map(|(offset, character)| {
            let index = first_index
                .checked_add(offset)
                .context("MAD font codebook index overflow")?;
            ensure!(
                index < USABLE_GLYPH_COUNT,
                "MAD font codebook requires reserved index {index:#x}"
            );
            Ok((
                character,
                u8::try_from(index).context("MAD font codebook exceeds 8-bit indices")?,
            ))
        })
        .collect()
}

fn scale_character_tiles(character: char) -> Result<[[u8; FONT_GLYPH_SIZE]; 4]> {
    let source = rasterize_pc98_character(character)
        .with_context(|| format!("rasterize MAD composite character {character:?}"))?;
    let mut tiles = [[0_u8; FONT_GLYPH_SIZE]; 4];
    for source_y in 0..16 {
        for source_x in 0..16 {
            let source_byte = source[source_y * 2 + source_x / 8];
            if source_byte & (0x80 >> (source_x % 8)) == 0 {
                continue;
            }
            for scaled_y in source_y * 2..source_y * 2 + 2 {
                for scaled_x in source_x * 2..source_x * 2 + 2 {
                    let tile = (scaled_y / 16) * 2 + scaled_x / 16;
                    let tile_x = scaled_x % 16;
                    let tile_y = scaled_y % 16;
                    tiles[tile][tile_y * 2 + tile_x / 8] |= 0x80 >> (tile_x % 8);
                }
            }
        }
    }
    Ok(tiles)
}

pub(super) fn battle_unit_number(segment_id: &str) -> Result<Option<u8>> {
    segment_id
        .strip_prefix("mad-battle-unit-")
        .map(|unit| {
            unit.parse::<u8>()
                .with_context(|| format!("MAD segment {segment_id} has an invalid unit number"))
        })
        .transpose()
}

#[cfg(test)]
#[path = "bank_plan_tests.rs"]
mod bank_plan_tests;
