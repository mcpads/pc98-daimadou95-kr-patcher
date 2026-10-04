use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::game_data::ending_ingredient_labels::BLANK_GLYPH_INDEX;
use crate::game_data::ending_meal_captions::parse_ending_meal_captions;

#[derive(Debug, Eq, PartialEq)]
pub struct EndingMealCaptionPatchReport {
    pub record_file_offsets: Vec<usize>,
    pub record_count: usize,
    pub span_count: usize,
    pub glyph_slot_count: usize,
    pub translated_glyph_count: usize,
}

#[derive(Debug)]
pub(super) struct PatchedEndingMealCaptions {
    pub bytes: Vec<u8>,
    pub report: EndingMealCaptionPatchReport,
}

pub(super) fn replace_ending_meal_captions(
    bytes: &[u8],
    translations: &BTreeMap<String, Vec<String>>,
    codebook: &BTreeMap<char, u8>,
) -> Result<PatchedEndingMealCaptions> {
    let catalog = parse_ending_meal_captions(bytes)?;
    ensure!(
        translations.len() == catalog.records.len(),
        "ENDING meal-caption translation supplies {} records for {} fixed command streams",
        translations.len(),
        catalog.records.len()
    );

    let mut plan = WritePlan::new();
    let mut replacements = Vec::with_capacity(catalog.records.len());
    let mut span_count = 0;
    let mut glyph_slot_count = 0;
    let mut translated_glyph_count = 0;
    for record in &catalog.records {
        let translated_spans = translations
            .get(record.id)
            .with_context(|| format!("ENDING meal-caption translation is missing {}", record.id))?;
        ensure!(
            translated_spans.len() == record.spans.len(),
            "ENDING meal caption {} supplies {} text spans for {} fixed phases",
            record.id,
            translated_spans.len(),
            record.spans.len()
        );
        let mut replacement = record.bytes.to_vec();
        for (span_number, (span, translation)) in
            record.spans.iter().zip(translated_spans).enumerate()
        {
            let encoded = encode_centered_span(
                translation,
                span.slot_file_offsets.len(),
                codebook,
                record.id,
                span_number,
            )?;
            for (file_offset, glyph_index) in span.slot_file_offsets.iter().zip(encoded) {
                replacement[*file_offset - record.file_offset] = glyph_index;
            }
            span_count += 1;
            glyph_slot_count += span.slot_file_offsets.len();
            translated_glyph_count += translation.chars().count();
        }

        let end = record.file_offset + record.bytes.len();
        let region_id = format!("ending-meal-caption-{}-commands", record.id);
        plan = plan
            .region(ImageRegion {
                id: region_id,
                range: record.file_offset..end,
                kind: RegionKind::Data,
                reason: "fixed progressive ending caption command stream".into(),
            })
            .write(ExpectedWrite {
                id: format!("write-ending-meal-caption-{}", record.id),
                owner: "ending-meal-caption-compiler".into(),
                purpose: "replace visible indexed glyph operands while preserving animation commands and timing"
                    .into(),
                offset: record.file_offset,
                expected_original: record.bytes.to_vec(),
                replacement: replacement.clone(),
                intent: WriteIntent::Data,
            });
        replacements.push((record.file_offset, replacement));
    }

    let output = plan.apply(bytes, None)?;
    for (file_offset, replacement) in &replacements {
        ensure!(
            output[*file_offset..*file_offset + replacement.len()] == *replacement,
            "ENDING meal-caption command stream at {file_offset:#x} did not survive its Expected Write"
        );
    }
    Ok(PatchedEndingMealCaptions {
        bytes: output,
        report: EndingMealCaptionPatchReport {
            record_file_offsets: catalog
                .records
                .iter()
                .map(|record| record.file_offset)
                .collect(),
            record_count: catalog.records.len(),
            span_count,
            glyph_slot_count,
            translated_glyph_count,
        },
    })
}

fn encode_centered_span(
    text: &str,
    slot_count: usize,
    codebook: &BTreeMap<char, u8>,
    record_id: &str,
    span_number: usize,
) -> Result<Vec<u8>> {
    let glyph_count = text.chars().count();
    ensure!(
        glyph_count > 0 && glyph_count <= slot_count,
        "ENDING meal caption {record_id} span {} uses {glyph_count} glyphs but its command stream requires 1..={slot_count}",
        span_number + 1
    );
    let leading_padding = (slot_count - glyph_count).div_ceil(2);
    let mut encoded = vec![BLANK_GLYPH_INDEX; slot_count];
    for (cell, character) in text.chars().enumerate() {
        let index = codebook.get(&character).copied().with_context(|| {
            format!(
                "ENDING meal caption {record_id} span {} codebook is missing {character:?}",
                span_number + 1
            )
        })?;
        ensure!(
            index != BLANK_GLYPH_INDEX && index != 0xfe && index != 0xff,
            "ENDING meal caption {record_id} maps {character:?} to a reserved glyph"
        );
        encoded[leading_padding + cell] = index;
    }
    Ok(encoded)
}

#[cfg(test)]
#[path = "meal_captions_tests.rs"]
mod meal_captions_tests;
