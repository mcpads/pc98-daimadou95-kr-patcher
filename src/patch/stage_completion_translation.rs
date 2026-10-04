use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::stage_completion_graphic::{
    MADDAT_ENTRY_ID, TILE_IDS, decode_stage_completion_graphic, encode_stage_completion_graphic,
};
use crate::translation::ReviewedTranslationCorpus;

use super::maddat::apply_repacked_maddat;
use super::stage_completion_compositor::compose_stage_completion_graphic;

const SEGMENT_ID: &str = "graphic-stage-completion-legend";
const ENTRY_ID: &str = "graphic-text:stage-completion-legend:01";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct StageCompletionTranslationFiles {
    pub maddat: Vec<u8>,
    pub report: StageCompletionTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct StageCompletionTranslationReport {
    pub segment_id: String,
    pub maddat_entry_id: usize,
    pub tile_ids: [usize; 6],
    pub korean_text: String,
    pub packed_input_size: usize,
    pub packed_output_size: usize,
    pub changed_pixel_count: usize,
    pub protected_tile_count: usize,
}

pub(crate) fn compile_stage_completion_translation(
    maddat: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<StageCompletionTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "stage-completion build requires a human-review-ready translation corpus"
    );
    let segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == SEGMENT_ID)
        .context("translation corpus is missing graphic-stage-completion-legend")?;
    let korean_text = segment.reinsertion_text(ENTRY_ID)?;

    let archive = parse_flink_archive(maddat)?;
    let source_entry = archive
        .entries
        .get(MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing stage-completion graphic entry 152")?;
    ensure!(
        source_entry.id == MADDAT_ENTRY_ID,
        "stage-completion graphic entry ID differs from its table position"
    );
    let source = decode_stage_completion_graphic(source_entry.bytes)?;
    let composition = compose_stage_completion_graphic(&source.pixels, &korean_text)?;
    let changed_pixel_count = source
        .pixels
        .iter()
        .zip(&composition.pixels)
        .filter(|(source, output)| source != output)
        .count();
    let packed = encode_stage_completion_graphic(&source, &composition.pixels)?;
    ensure!(
        packed.len() <= usize::from(u16::MAX),
        "translated stage-completion graphic is {} bytes and exceeds the FLINK u16 size field",
        packed.len()
    );

    let replacements = BTreeMap::from([(MADDAT_ENTRY_ID, packed.clone())]);
    let repacked = repack_contiguous_flink_archive(maddat, &replacements, &[])?;
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "stage-completion-maddat-repacker",
        "replace only the six stage-completion legend tiles with the reviewed Korean composition",
    )?;

    Ok(StageCompletionTranslationFiles {
        maddat: checked_maddat,
        report: StageCompletionTranslationReport {
            segment_id: SEGMENT_ID.into(),
            maddat_entry_id: MADDAT_ENTRY_ID,
            tile_ids: TILE_IDS,
            korean_text,
            packed_input_size: source_entry.bytes.len(),
            packed_output_size: packed.len(),
            changed_pixel_count,
            protected_tile_count: 128 - TILE_IDS.len(),
        },
    })
}
