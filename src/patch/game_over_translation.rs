use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::game_over_graphic::{
    MADDAT_ENTRY_ID, decode_game_over_graphic, encode_game_over_graphic,
};
use crate::translation::ReviewedTranslationCorpus;

use super::game_over_compositor::compose_game_over_graphic;
use super::maddat::apply_repacked_maddat;

const SEGMENT_ID: &str = "graphic-game-over";
const ENTRY_ID: &str = "graphic-text:game-over:01";

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct GameOverTranslationFiles {
    pub maddat: Vec<u8>,
    pub report: GameOverTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GameOverTranslationReport {
    pub segment_id: String,
    pub maddat_entry_id: usize,
    pub korean_text: String,
    pub packed_input_size: usize,
    pub packed_output_size: usize,
    pub changed_pixel_count: usize,
    pub background_color: u8,
    pub primary_color: u8,
    pub outline_color: Option<u8>,
    pub shadow_color: Option<u8>,
}

pub(crate) fn compile_game_over_translation(
    maddat: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<GameOverTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "game-over build requires a human-review-ready translation corpus"
    );
    let segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == SEGMENT_ID)
        .context("translation corpus is missing graphic-game-over")?;
    let korean_text = segment.reinsertion_text(ENTRY_ID)?;

    let archive = parse_flink_archive(maddat)?;
    let source_entry = archive
        .entries
        .get(MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing game-over graphic entry 153")?;
    ensure!(
        source_entry.id == MADDAT_ENTRY_ID,
        "game-over graphic entry ID differs from its table position"
    );
    let source = decode_game_over_graphic(source_entry.bytes)?;
    let composition = compose_game_over_graphic(&source.pixels, &korean_text)?;
    let changed_pixel_count = source
        .pixels
        .iter()
        .zip(&composition.pixels)
        .filter(|(source, output)| source != output)
        .count();
    let packed = encode_game_over_graphic(&composition.pixels)?;
    ensure!(
        packed.len() <= usize::from(u16::MAX),
        "translated game-over graphic is {} bytes and exceeds the FLINK u16 size field",
        packed.len()
    );
    let replacements = BTreeMap::from([(MADDAT_ENTRY_ID, packed.clone())]);
    let repacked = repack_contiguous_flink_archive(maddat, &replacements, &[])?;
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "game-over-maddat-repacker",
        "replace the game-over planar block with the tracked Korean composition",
    )?;

    Ok(GameOverTranslationFiles {
        maddat: checked_maddat,
        report: GameOverTranslationReport {
            segment_id: SEGMENT_ID.into(),
            maddat_entry_id: MADDAT_ENTRY_ID,
            korean_text,
            packed_input_size: source_entry.bytes.len(),
            packed_output_size: packed.len(),
            changed_pixel_count,
            background_color: composition.palette.background,
            primary_color: composition.palette.primary,
            outline_color: composition.palette.outline,
            shadow_color: composition.palette.shadow,
        },
    })
}
