use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use encoding_rs::SHIFT_JIS;

use crate::game_data::gaiji::jis_row_cell_to_shift_jis;
use crate::source::{GaijiCatalog, MadTextCatalog, MadTextToken};

use super::{DosOutputPart, DosTranslation};

const FIRST_EXTERNAL_CHARACTER_CODE: u16 = 0x7621;
const FIRST_SOURCE_CHARACTER_CODE: u16 = 0x7622;
const FIRST_SECOND_ROW_CHARACTER_CODE: u16 = 0x7721;
const LAST_SOURCE_CHARACTER_CODE: u16 = 0x7772;
const LAST_EXTERNAL_CHARACTER_CODE: u16 = 0x777e;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GaijiSlot {
    pub character_code: u16,
    pub shift_jis_code: u16,
}

pub(super) type GaijiCodebook = BTreeMap<char, GaijiSlot>;

pub(super) struct MadMediaErrorFontPlan {
    pub codebook: GaijiCodebook,
    pub referenced_source_gaiji_count: usize,
    pub available_external_slot_count: usize,
    pub blocked_external_slot_count: usize,
}

pub(super) fn compile_mad_media_error_font_plan(
    gaiji: &GaijiCatalog,
    mad_text: &MadTextCatalog,
    mad_com: &[u8],
    persistent_consumer_programs: &[&[u8]],
    translations: &[DosTranslation],
) -> Result<MadMediaErrorFontPlan> {
    ensure!(
        gaiji.glyphs.len() == 175,
        "MAD media-error font plan requires the verified 175-slot GAIJI table"
    );
    let fixed_slot_glyph_indices = mad_text
        .fixed_slots
        .iter()
        .flat_map(|slot| &slot.tokens)
        .filter_map(|token| match token {
            MadTextToken::Gaiji { glyph_index, .. } => Some(*glyph_index),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    ensure!(
        fixed_slot_glyph_indices
            .iter()
            .all(|index| *index < gaiji.glyphs.len()),
        "MAD fixed graphic references a GAIJI slot outside the verified table"
    );

    let mad_referenced_character_codes = referenced_source_gaiji_codes(mad_com, gaiji);
    ensure!(
        fixed_slot_glyph_indices
            .iter()
            .all(|index| mad_referenced_character_codes
                .contains(&gaiji.glyphs[*index].character_code)),
        "MAD source scan no longer covers every verified fixed-art GAIJI slot"
    );
    let uninstalled_slots = uninstalled_external_slots(gaiji)?;
    let persistent_referenced_codes = referenced_slot_codes(
        persistent_consumer_programs.iter().copied(),
        &uninstalled_slots,
    );
    let mad_slots = uninstalled_slots
        .iter()
        .copied()
        .filter(|slot| !persistent_referenced_codes.contains(&slot.character_code))
        .collect::<Vec<_>>();
    let characters = unencodable_characters(translations.iter());
    let codebook = assign_slots(&characters, &mad_slots, "MAD.COM media errors")?;

    Ok(MadMediaErrorFontPlan {
        codebook,
        referenced_source_gaiji_count: mad_referenced_character_codes.len(),
        available_external_slot_count: mad_slots.len(),
        blocked_external_slot_count: persistent_referenced_codes.len(),
    })
}

fn referenced_source_gaiji_codes(bytes: &[u8], gaiji: &GaijiCatalog) -> BTreeSet<u16> {
    gaiji
        .glyphs
        .iter()
        .filter(|glyph| {
            let encoded = glyph.shift_jis_code.to_be_bytes();
            bytes.windows(encoded.len()).any(|window| window == encoded)
        })
        .map(|glyph| glyph.character_code)
        .collect()
}

fn uninstalled_external_slots(gaiji: &GaijiCatalog) -> Result<Vec<GaijiSlot>> {
    ensure!(
        gaiji.first_character_code == Some(FIRST_SOURCE_CHARACTER_CODE)
            && gaiji.last_character_code == Some(LAST_SOURCE_CHARACTER_CODE),
        "MAD external-slot plan requires the verified GAIJI character-code range"
    );
    let source_codes = gaiji
        .glyphs
        .iter()
        .map(|glyph| glyph.character_code)
        .collect::<BTreeSet<_>>();
    let mut character_codes = vec![FIRST_EXTERNAL_CHARACTER_CODE];
    character_codes.extend(FIRST_SECOND_ROW_CHARACTER_CODE..=LAST_EXTERNAL_CHARACTER_CODE);
    character_codes.retain(|code| !source_codes.contains(code));
    character_codes
        .into_iter()
        .map(|character_code| {
            Ok(GaijiSlot {
                character_code,
                shift_jis_code: jis_row_cell_to_shift_jis(character_code)?,
            })
        })
        .collect()
}

fn referenced_slot_codes<'a>(
    programs: impl Iterator<Item = &'a [u8]>,
    slots: &[GaijiSlot],
) -> BTreeSet<u16> {
    let programs = programs.collect::<Vec<_>>();
    slots
        .iter()
        .filter(|slot| {
            let encoded = slot.shift_jis_code.to_be_bytes();
            programs
                .iter()
                .any(|bytes| bytes.windows(encoded.len()).any(|window| window == encoded))
        })
        .map(|slot| slot.character_code)
        .collect()
}

fn unencodable_characters<'a>(
    translations: impl Iterator<Item = &'a DosTranslation>,
) -> BTreeSet<char> {
    translations
        .flat_map(|translation| &translation.parts)
        .filter_map(|part| match part {
            DosOutputPart::Text(text) => Some(text.chars()),
            DosOutputPart::Bytes(_) | DosOutputPart::Terminator => None,
        })
        .flatten()
        .filter(|character| !is_shift_jis_encodable(*character))
        .collect()
}

fn assign_slots(
    characters: &BTreeSet<char>,
    available_slots: &[GaijiSlot],
    bank_id: &str,
) -> Result<GaijiCodebook> {
    ensure!(
        characters.len() <= available_slots.len(),
        "external-character bank {bank_id} requires {} glyphs but has {} slots",
        characters.len(),
        available_slots.len()
    );
    Ok(characters
        .iter()
        .copied()
        .zip(available_slots.iter().copied())
        .collect())
}

pub(super) fn is_shift_jis_encodable(character: char) -> bool {
    let mut utf8 = [0_u8; 4];
    let text = character.encode_utf8(&mut utf8);
    let (_, _, had_errors) = SHIFT_JIS.encode(text);
    !had_errors
}

#[cfg(test)]
#[path = "bank_plan_tests.rs"]
mod bank_plan_tests;
