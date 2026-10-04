use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::game_data::ending_ingredient_labels::{
    BLANK_GLYPH_INDEX, RECORD_CELL_COUNT, TABLE_BYTE_SIZE, parse_ending_ingredient_labels,
};

#[derive(Debug)]
pub(super) struct PatchedEndingIngredientLabels {
    pub bytes: Vec<u8>,
    pub table_file_offset: usize,
    pub label_count: usize,
}

pub(super) fn replace_ending_ingredient_labels(
    bytes: &[u8],
    translations: &BTreeMap<String, String>,
    codebook: &BTreeMap<char, u8>,
) -> Result<PatchedEndingIngredientLabels> {
    let catalog = parse_ending_ingredient_labels(bytes)?;
    ensure!(
        translations.len() == catalog.records.len(),
        "ENDING ingredient translation supplies {} labels for {} fixed records",
        translations.len(),
        catalog.records.len()
    );
    let mut replacement = Vec::with_capacity(TABLE_BYTE_SIZE);
    for record in &catalog.records {
        let id = format!(
            "slot-{:02}-variant-{:02}",
            record.group_index + 1,
            record.variant_index + 1
        );
        let text = translations
            .get(&id)
            .with_context(|| format!("ENDING ingredient translation is missing {id}"))?;
        replacement.extend_from_slice(&encode_centered_label(text, codebook, &id)?);
    }
    ensure!(
        replacement.len() == TABLE_BYTE_SIZE,
        "ENDING ingredient replacement table has the wrong byte size"
    );

    let table_end = catalog.table_file_offset + catalog.table_byte_size;
    let plan = WritePlan::new()
        .region(ImageRegion {
            id: "ending-ingredient-label-table".into(),
            range: catalog.table_file_offset..table_end,
            kind: RegionKind::Data,
            reason: "input-selected fixed 16-cell ending ingredient labels".into(),
        })
        .write(ExpectedWrite {
            id: "write-ending-ingredient-label-table".into(),
            owner: "ending-ingredient-label-compiler".into(),
            purpose:
                "replace all consumer-reachable ending ingredient labels with tracked Korean text"
                    .into(),
            offset: catalog.table_file_offset,
            expected_original: bytes[catalog.table_file_offset..table_end].to_vec(),
            replacement: replacement.clone(),
            intent: WriteIntent::Data,
        });
    let output = plan.apply(bytes, None)?;
    ensure!(
        output[catalog.table_file_offset..table_end] == replacement,
        "ENDING ingredient table did not survive its Expected Write"
    );

    Ok(PatchedEndingIngredientLabels {
        bytes: output,
        table_file_offset: catalog.table_file_offset,
        label_count: catalog.records.len(),
    })
}

fn encode_centered_label(
    text: &str,
    codebook: &BTreeMap<char, u8>,
    id: &str,
) -> Result<[u8; RECORD_CELL_COUNT]> {
    let cell_count = text.chars().count();
    ensure!(cell_count > 0, "ENDING ingredient {id} is blank");
    ensure!(
        cell_count <= RECORD_CELL_COUNT,
        "ENDING ingredient {id} uses {cell_count} cells but its consumer requires at most {RECORD_CELL_COUNT}"
    );
    let left_padding = (RECORD_CELL_COUNT - cell_count) / 2;
    let mut encoded = [BLANK_GLYPH_INDEX; RECORD_CELL_COUNT];
    for (cell, character) in text.chars().enumerate() {
        let index = codebook
            .get(&character)
            .copied()
            .with_context(|| format!("ENDING ingredient {id} codebook is missing {character:?}"))?;
        ensure!(
            index != BLANK_GLYPH_INDEX && index != 0xff,
            "ENDING ingredient {id} maps {character:?} to a reserved glyph"
        );
        encoded[left_padding + cell] = index;
    }
    Ok(encoded)
}

#[cfg(test)]
#[path = "ingredients_tests.rs"]
mod ingredients_tests;
