use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::stage_select_graphic::{
    MADDAT_ENTRY_ID, decode_stage_select_graphic, encode_stage_select_graphic,
};
use crate::translation::ReviewedTranslationCorpus;

use super::maddat::apply_repacked_maddat;
use super::stage_select_compositor::{
    EXPECTED_SOURCE_LABEL_PIXEL_COUNT, compose_stage_select_graphic,
};

const SEGMENT_ID: &str = "graphic-stage-select";
const ENTRY_ID: &str = "graphic-text:stage-select:01";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct StageSelectTranslationFiles {
    pub maddat: Vec<u8>,
    pub report: StageSelectTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StageSelectTranslationReport {
    pub segment_id: String,
    pub maddat_entry_id: usize,
    pub korean_text: String,
    pub packed_input_size: usize,
    pub packed_output_size: usize,
    pub changed_pixel_count: usize,
    pub erased_source_label_pixel_count: usize,
}

pub(crate) fn compile_stage_select_translation(
    maddat: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<StageSelectTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "stage-select build requires a human-review-ready translation corpus"
    );
    let segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == SEGMENT_ID)
        .context("translation corpus is missing graphic-stage-select")?;
    let korean_text = segment.reinsertion_text(ENTRY_ID)?;

    let archive = parse_flink_archive(maddat)?;
    let source_entry = archive
        .entries
        .get(MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing stage-select graphic entry 5")?;
    ensure!(
        source_entry.id == MADDAT_ENTRY_ID,
        "stage-select graphic entry ID differs from its table position"
    );
    let source = decode_stage_select_graphic(source_entry.bytes)?;
    let composition = compose_stage_select_graphic(&source.pixels, &korean_text)?;
    ensure!(
        composition.erased_source_label_pixel_count == EXPECTED_SOURCE_LABEL_PIXEL_COUNT,
        "stage-select source label has {} white/cyan pixels instead of the reviewed {EXPECTED_SOURCE_LABEL_PIXEL_COUNT}",
        composition.erased_source_label_pixel_count
    );
    let changed_pixel_count = source
        .pixels
        .iter()
        .zip(&composition.pixels)
        .filter(|(source, output)| source != output)
        .count();
    ensure!(
        changed_pixel_count > 0,
        "stage-select translation made no pixel changes"
    );

    let packed = encode_stage_select_graphic(&source, &composition.pixels)?;
    ensure!(
        packed.len() <= usize::from(u16::MAX),
        "translated stage-select GCS is {} bytes and exceeds the FLINK u16 size field",
        packed.len()
    );
    let replacements = BTreeMap::from([(MADDAT_ENTRY_ID, packed.clone())]);
    let repacked = repack_contiguous_flink_archive(maddat, &replacements, &[])?;
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "stage-select-maddat-repacker",
        "replace the baked STAGE SELECT labels with the reviewed Korean composition",
    )?;

    Ok(StageSelectTranslationFiles {
        maddat: checked_maddat,
        report: StageSelectTranslationReport {
            segment_id: SEGMENT_ID.into(),
            maddat_entry_id: MADDAT_ENTRY_ID,
            korean_text,
            packed_input_size: source_entry.bytes.len(),
            packed_output_size: packed.len(),
            changed_pixel_count,
            erased_source_label_pixel_count: composition.erased_source_label_pixel_count,
        },
    })
}

#[cfg(test)]
#[path = "stage_select_translation_tests.rs"]
mod stage_select_translation_tests;
