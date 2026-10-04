use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::font::{FontProvenance, rasterize_pc98_character, verify_pc98_repertoire};
use crate::game_data::compile_lz::{decode_complete_compile_lz, encode_compile_lz};
use crate::game_data::indexed_text::{FONT_GLYPH_COUNT, FONT_GLYPH_SIZE, verify_indexed_font};

pub(super) struct IndexedFontBank {
    pub decoded: Vec<u8>,
    pub packed: Vec<u8>,
    pub codebook: BTreeMap<char, u8>,
    pub font: FontProvenance,
}

pub(super) struct IndexedFontGlyph {
    pub label: String,
    pub bitmap: [u8; FONT_GLYPH_SIZE],
}

pub(super) fn compile_indexed_font_bank(
    source_font: &[u8],
    characters: impl IntoIterator<Item = char>,
    reserved_indices: &BTreeSet<u8>,
    bank_id: &str,
) -> Result<IndexedFontBank> {
    verify_indexed_font(source_font)?;
    ensure!(
        !bank_id.is_empty(),
        "indexed font bank ID must not be empty"
    );

    let unique_characters: BTreeSet<char> = characters.into_iter().collect();
    let available_indices = (0_u8..=u8::MAX).filter(|index| !reserved_indices.contains(index));
    let codebook: BTreeMap<char, u8> = unique_characters
        .iter()
        .copied()
        .zip(available_indices)
        .collect();
    ensure!(
        codebook.len() == unique_characters.len(),
        "{bank_id} requires {} glyphs but only {} non-reserved slots are available",
        unique_characters.len(),
        FONT_GLYPH_COUNT - reserved_indices.len()
    );

    compile_mapped_indexed_font_bank(
        source_font,
        codebook,
        &BTreeMap::new(),
        reserved_indices,
        bank_id,
    )
}

pub(super) fn compile_mapped_indexed_font_bank(
    source_font: &[u8],
    codebook: BTreeMap<char, u8>,
    custom_glyphs: &BTreeMap<u8, IndexedFontGlyph>,
    preserved_indices: &BTreeSet<u8>,
    bank_id: &str,
) -> Result<IndexedFontBank> {
    verify_indexed_font(source_font)?;
    ensure!(
        !bank_id.is_empty(),
        "indexed font bank ID must not be empty"
    );
    let codebook_indices = codebook.values().copied().collect::<BTreeSet<_>>();
    ensure!(
        codebook_indices.len() == codebook.len(),
        "{bank_id} codebook assigns more than one character to a glyph slot"
    );
    ensure!(
        codebook_indices.is_disjoint(&custom_glyphs.keys().copied().collect()),
        "{bank_id} character and custom glyph slots overlap"
    );
    ensure!(
        codebook_indices.is_disjoint(preserved_indices)
            && custom_glyphs
                .keys()
                .all(|index| !preserved_indices.contains(index)),
        "{bank_id} assigns a preserved glyph slot"
    );

    let font = verify_pc98_repertoire(codebook.keys().copied())?.provenance;
    let owner = format!("{bank_id}-font-bank-compiler");
    let mut plan = WritePlan::new();
    for (&character, &index) in &codebook {
        let offset = usize::from(index) * FONT_GLYPH_SIZE;
        let bitmap = rasterize_pc98_character(character)
            .with_context(|| format!("rasterize {bank_id} character {character:?}"))?;
        plan = plan
            .region(ImageRegion {
                id: format!("{bank_id}-font-slot-{index:02x}"),
                range: offset..offset + FONT_GLYPH_SIZE,
                kind: RegionKind::Data,
                reason: format!("isolated {bank_id} font slot for {character:?}"),
            })
            .write(ExpectedWrite {
                id: format!("write-{bank_id}-glyph-{index:02x}"),
                owner: owner.clone(),
                purpose: format!("render {character:?} into its {bank_id} glyph slot"),
                offset,
                expected_original: source_font[offset..offset + FONT_GLYPH_SIZE].to_vec(),
                replacement: bitmap.to_vec(),
                intent: WriteIntent::Data,
            });
    }
    for (&index, glyph) in custom_glyphs {
        let offset = usize::from(index) * FONT_GLYPH_SIZE;
        plan = plan
            .region(ImageRegion {
                id: format!("{bank_id}-custom-font-slot-{index:02x}"),
                range: offset..offset + FONT_GLYPH_SIZE,
                kind: RegionKind::Data,
                reason: format!("isolated {bank_id} custom font slot for {}", glyph.label),
            })
            .write(ExpectedWrite {
                id: format!("write-{bank_id}-custom-glyph-{index:02x}"),
                owner: owner.clone(),
                purpose: format!("write {} into its {bank_id} glyph slot", glyph.label),
                offset,
                expected_original: source_font[offset..offset + FONT_GLYPH_SIZE].to_vec(),
                replacement: glyph.bitmap.to_vec(),
                intent: WriteIntent::Data,
            });
    }
    let decoded = plan.apply(source_font, None)?;
    for &reserved in preserved_indices {
        let offset = usize::from(reserved) * FONT_GLYPH_SIZE;
        ensure!(
            decoded[offset..offset + FONT_GLYPH_SIZE]
                == source_font[offset..offset + FONT_GLYPH_SIZE],
            "{bank_id} changed reserved font slot {reserved:02X}"
        );
    }
    let packed = encode_compile_lz(&decoded)?;
    ensure!(
        decode_complete_compile_lz(&packed)?.output == decoded,
        "{bank_id} font bank failed its packed round trip"
    );

    Ok(IndexedFontBank {
        decoded,
        packed,
        codebook,
        font,
    })
}

#[cfg(test)]
#[path = "indexed_font_bank_tests.rs"]
mod indexed_font_bank_tests;
