use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::font::{
    FontProvenance, rasterize_ending_credit_character, verify_ending_credit_repertoire,
};
use crate::game_data::compile_lz::{decode_complete_compile_lz, encode_compile_lz};
use crate::game_data::ending_credits::{
    CELLS_PER_LINE, FONT_GLYPH_COUNT, FONT_GLYPH_SIZE, ROLE_LINE_SPECS, ending_credit_glyph_offset,
    ending_credit_line_offset, parse_ending_credit_grid, verify_ending_credit_font,
};

#[derive(Debug, Eq, PartialEq)]
pub(super) struct EndingCreditsPatch {
    pub decoded_font: Vec<u8>,
    pub packed_font: Vec<u8>,
    pub text_grid: Vec<u8>,
    pub report: EndingCreditsPatchReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct EndingCreditsPatchReport {
    pub translated_role_count: usize,
    pub unique_character_count: usize,
    pub available_source_slot_count: usize,
    pub preserved_source_glyph_count: usize,
    pub packed_font_size: usize,
    pub font: FontProvenance,
}

pub(super) fn compile_ending_credits(
    packed_source_font: &[u8],
    source_text_grid: &[u8],
    translations: &BTreeMap<String, String>,
) -> Result<EndingCreditsPatch> {
    let decoded_source_font = decode_complete_compile_lz(packed_source_font)?.output;
    verify_ending_credit_font(&decoded_source_font)?;
    let source_grid = parse_ending_credit_grid(source_text_grid)?;

    let expected_ids = ROLE_LINE_SPECS
        .iter()
        .map(|spec| spec.target_id())
        .collect::<BTreeSet<_>>();
    ensure!(
        translations.keys().cloned().collect::<BTreeSet<_>>() == expected_ids,
        "ending-credit translations do not cover the ten reviewed role lines"
    );

    let used_source_indices = source_grid
        .screens
        .iter()
        .flat_map(|screen| &screen.lines)
        .flat_map(|line| line.glyph_indices)
        .filter(|index| *index != 0)
        .collect::<BTreeSet<_>>();
    let available_indices = (1..=FONT_GLYPH_COUNT)
        .map(|index| u8::try_from(index).expect("ending-credit glyph index fits u8"))
        .filter(|index| !used_source_indices.contains(index))
        .collect::<Vec<_>>();
    let unique_characters = translations
        .values()
        .flat_map(|text| text.chars())
        .filter(|character| *character != ' ')
        .collect::<BTreeSet<_>>();
    ensure!(
        unique_characters.len() <= available_indices.len(),
        "ending-credit translation requires {} glyphs but only {} source-unused slots exist",
        unique_characters.len(),
        available_indices.len()
    );
    let font = verify_ending_credit_repertoire(unique_characters.iter().copied())?.provenance;
    let codebook = unique_characters
        .iter()
        .copied()
        .zip(available_indices.iter().copied())
        .collect::<BTreeMap<_, _>>();

    let mut font_plan = WritePlan::new();
    for (&character, &index) in &codebook {
        let offset = ending_credit_glyph_offset(index)?;
        let replacement = rasterize_ending_credit_character(character)?;
        font_plan = font_plan
            .region(ImageRegion {
                id: format!("ending-credit-font-slot-{index:02x}"),
                range: offset..offset + FONT_GLYPH_SIZE,
                kind: RegionKind::Data,
                reason: format!("source-unused 24x32 ending-credit glyph slot for {character:?}"),
            })
            .write(ExpectedWrite {
                id: format!("write-ending-credit-glyph-{index:02x}"),
                owner: "ending-credit-font-compiler".into(),
                purpose: format!("render {character:?} into an unused ending-credit glyph slot"),
                offset,
                expected_original: decoded_source_font[offset..offset + FONT_GLYPH_SIZE].to_vec(),
                replacement: replacement.to_vec(),
                intent: WriteIntent::Data,
            });
    }
    let decoded_font = font_plan.apply(&decoded_source_font, None)?;
    for &index in &used_source_indices {
        let offset = ending_credit_glyph_offset(index)?;
        ensure!(
            decoded_font[offset..offset + FONT_GLYPH_SIZE]
                == decoded_source_font[offset..offset + FONT_GLYPH_SIZE],
            "ending-credit font compiler changed source-used glyph {index:02X}"
        );
    }
    let packed_font = encode_compile_lz(&decoded_font)?;
    ensure!(
        decode_complete_compile_lz(&packed_font)?.output == decoded_font,
        "ending-credit font failed its Compile LZ round trip"
    );

    let mut grid_plan = WritePlan::new();
    for spec in ROLE_LINE_SPECS {
        let line = &source_grid.screens[spec.screen_number - 1].lines[spec.line_number - 1];
        ensure!(
            line.text.trim() == spec.source_text,
            "ending-credit role {} contains {:?} instead of {:?}",
            spec.record_id(),
            line.text,
            spec.source_text
        );
        let source_start_column = line
            .glyph_indices
            .iter()
            .position(|index| *index != 0)
            .with_context(|| {
                format!(
                    "ending-credit role {} has no source glyph",
                    spec.record_id()
                )
            })?;
        let source_end_column = line
            .glyph_indices
            .iter()
            .rposition(|index| *index != 0)
            .expect("a source glyph was found")
            + 1;
        let translation = translations
            .get(&spec.target_id())
            .expect("the exact translation key set was verified");
        let encoded = translation
            .chars()
            .map(|character| {
                if character == ' ' {
                    Ok(0)
                } else {
                    codebook.get(&character).copied().with_context(|| {
                        format!("ending-credit translation has unmapped character {character:?}")
                    })
                }
            })
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            encoded.len() <= CELLS_PER_LINE,
            "ending-credit role {} needs {} cells in a {CELLS_PER_LINE}-cell line",
            spec.record_id(),
            encoded.len()
        );
        let source_width = source_end_column - source_start_column;
        let start_column = if source_start_column == (CELLS_PER_LINE - source_width) / 2 {
            (CELLS_PER_LINE - encoded.len()) / 2
        } else {
            source_start_column
        };
        ensure!(
            start_column + encoded.len() <= CELLS_PER_LINE,
            "ending-credit role {} needs {} cells from preserved column {} in a {CELLS_PER_LINE}-cell line",
            spec.record_id(),
            encoded.len(),
            start_column
        );
        let mut replacement = [0_u8; CELLS_PER_LINE];
        replacement[start_column..start_column + encoded.len()].copy_from_slice(&encoded);
        let offset = ending_credit_line_offset(spec.screen_number, spec.line_number)?;
        grid_plan = grid_plan
            .region(ImageRegion {
                id: format!("ending-credit-role-{}", spec.record_id()),
                range: offset..offset + CELLS_PER_LINE,
                kind: RegionKind::Data,
                reason: "one consumer-selected fixed ending-credit role line".into(),
            })
            .write(ExpectedWrite {
                id: format!("write-ending-credit-role-{}", spec.record_id()),
                owner: "ending-credit-role-compiler".into(),
                purpose: format!("replace {:?} with {translation:?}", spec.source_text),
                offset,
                expected_original: source_text_grid[offset..offset + CELLS_PER_LINE].to_vec(),
                replacement: replacement.to_vec(),
                intent: WriteIntent::Data,
            });
    }
    let text_grid = grid_plan.apply(source_text_grid, None)?;
    ensure!(
        text_grid.len() == source_text_grid.len()
            && source_text_grid.last() == text_grid.last()
            && text_grid[..text_grid.len() - 1].iter().all(|index| {
                *index == 0
                    || used_source_indices.contains(index)
                    || codebook.values().any(|candidate| candidate == index)
            }),
        "ending-credit translation produced an invalid grid index or changed the DOS EOF marker"
    );
    let packed_font_size = packed_font.len();

    Ok(EndingCreditsPatch {
        decoded_font,
        packed_font,
        text_grid,
        report: EndingCreditsPatchReport {
            translated_role_count: ROLE_LINE_SPECS.len(),
            unique_character_count: codebook.len(),
            available_source_slot_count: available_indices.len(),
            preserved_source_glyph_count: used_source_indices.len(),
            packed_font_size,
            font,
        },
    })
}

#[cfg(test)]
#[path = "credits_tests.rs"]
mod credits_tests;
