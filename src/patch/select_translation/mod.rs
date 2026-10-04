mod consumer;
mod text;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::font::FontProvenance;
use crate::game_data::compile_lz::decode_complete_compile_lz;
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::indexed_text;
use crate::translation::{ReviewedTranslationCorpus, TranslationStatus};

use super::indexed_font_bank::{IndexedFontBank, compile_indexed_font_bank};
use super::maddat::apply_repacked_maddat;
use text::{SelectScriptTranslation, SelectStageTranslation};

const STAGE_COUNT: usize = 9;
const SCRIPTS_PER_STAGE: usize = 6;
const SELECT_FONT_FIRST_ENTRY_ID: usize = 157;
const SELECT_FONT_LAST_ENTRY_ID: usize = SELECT_FONT_FIRST_ENTRY_ID + STAGE_COUNT - 1;
const SELECT_USABLE_GLYPH_COUNT: usize = 240;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SelectTranslationFiles {
    pub maddat: Vec<u8>,
    pub select_com: Vec<u8>,
    pub report: SelectTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct SelectTranslationReport {
    pub translated_entry_count: usize,
    pub source_font_entry_id: usize,
    pub first_font_entry_id: usize,
    pub last_font_entry_id: usize,
    pub usable_glyph_count: usize,
    pub stages: Vec<SelectStageFontReport>,
    pub maddat_input_size: usize,
    pub maddat_output_size: usize,
    pub pointer_table_file_offset: usize,
    pub script_pool_file_offset: usize,
    pub script_pool_capacity: usize,
    pub script_pool_used: usize,
    pub select_com_input_size: usize,
    pub select_com_output_size: usize,
    pub hook_site_file_offset: usize,
    pub resume_hook_site_file_offset: usize,
    pub hook_file_offset: usize,
    pub hook_runtime_address: usize,
    pub resume_hook_runtime_address: usize,
    pub shared_loader_runtime_address: usize,
    pub hook_byte_size: usize,
    pub font: FontProvenance,
}

#[derive(Debug, Eq, PartialEq)]
pub struct SelectStageFontReport {
    pub stage_number: usize,
    pub segment_id: String,
    pub font_entry_id: usize,
    pub unique_character_count: usize,
    pub packed_font_size: usize,
}

pub(crate) fn compile_select_translation(
    maddat: &[u8],
    select_com: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<SelectTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "SELECT build requires a human-review-ready translation corpus"
    );
    let translations = load_stage_translations(corpus)?;
    let archive = parse_flink_archive(maddat)?;
    ensure!(
        archive.entries.len() + 1 == SELECT_FONT_FIRST_ENTRY_ID,
        "SELECT font banks require the cumulative MADDAT entry {SELECT_FONT_FIRST_ENTRY_ID} slot"
    );
    let source_font_entry = archive
        .entries
        .get(indexed_text::FONT_ENTRY_ID - 1)
        .context("MADDAT is missing the shared indexed font")?;
    let source_font = decode_complete_compile_lz(source_font_entry.bytes)?;
    indexed_text::verify_indexed_font(&source_font.output)?;
    let reserved_indices: BTreeSet<u8> = (SELECT_USABLE_GLYPH_COUNT as u8..=u8::MAX).collect();
    let font_banks = translations
        .iter()
        .map(|stage| {
            compile_indexed_font_bank(
                &source_font.output,
                stage
                    .scripts
                    .iter()
                    .flat_map(|script| script.spans.iter())
                    .flat_map(|span| span.chars()),
                &reserved_indices,
                &format!("select-stage-{:02}", stage.stage_number),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        font_banks.iter().all(|bank| bank
            .codebook
            .values()
            .all(|index| usize::from(*index) < SELECT_USABLE_GLYPH_COUNT)),
        "SELECT font codebook exceeds the 240 glyphs copied by its consumer"
    );

    let codebooks = font_banks
        .iter()
        .map(|bank| bank.codebook.clone())
        .collect::<Vec<_>>();
    let patched_text = text::replace_select_dialogue(select_com, &translations, &codebooks)?;
    let patched_consumer = consumer::install_select_stage_font_loader(&patched_text.bytes)?;
    ensure!(
        patched_consumer.copied_glyph_count == SELECT_USABLE_GLYPH_COUNT,
        "SELECT typed font loader copies {} glyphs instead of {SELECT_USABLE_GLYPH_COUNT}",
        patched_consumer.copied_glyph_count
    );

    let appended_fonts = font_banks
        .iter()
        .map(|bank| bank.packed.clone())
        .collect::<Vec<_>>();
    let repacked = repack_contiguous_flink_archive(maddat, &BTreeMap::new(), &appended_fonts)?;
    ensure!(
        repacked.replaced_entry_ids.is_empty()
            && repacked.appended_entry_ids
                == (SELECT_FONT_FIRST_ENTRY_ID..=SELECT_FONT_LAST_ENTRY_ID).collect(),
        "MADDAT repack ownership differs from the SELECT translation plan"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "select-maddat-repacker",
        "append nine isolated Hangul font banks for SELECT.COM",
    )?;
    verify_appended_fonts(&archive, &checked_maddat, &font_banks)?;

    let stages = font_banks
        .iter()
        .enumerate()
        .map(|(index, bank)| SelectStageFontReport {
            stage_number: index + 1,
            segment_id: format!("select-stage-{:02}", index + 1),
            font_entry_id: SELECT_FONT_FIRST_ENTRY_ID + index,
            unique_character_count: bank.codebook.len(),
            packed_font_size: bank.packed.len(),
        })
        .collect();
    let font = font_banks
        .first()
        .context("SELECT font compiler produced no stage banks")?
        .font
        .clone();
    let select_com_output_size = patched_consumer.bytes.len();

    Ok(SelectTranslationFiles {
        maddat: checked_maddat,
        select_com: patched_consumer.bytes,
        report: SelectTranslationReport {
            translated_entry_count: STAGE_COUNT * SCRIPTS_PER_STAGE,
            source_font_entry_id: indexed_text::FONT_ENTRY_ID,
            first_font_entry_id: SELECT_FONT_FIRST_ENTRY_ID,
            last_font_entry_id: SELECT_FONT_LAST_ENTRY_ID,
            usable_glyph_count: SELECT_USABLE_GLYPH_COUNT,
            stages,
            maddat_input_size: repacked.input_size,
            maddat_output_size: repacked.output_size,
            pointer_table_file_offset: patched_text.pointer_table_file_offset,
            script_pool_file_offset: patched_text.script_pool_file_offset,
            script_pool_capacity: patched_text.script_pool_capacity,
            script_pool_used: patched_text.script_pool_used,
            select_com_input_size: select_com.len(),
            select_com_output_size,
            hook_site_file_offset: patched_consumer.hook_site_file_offset,
            resume_hook_site_file_offset: patched_consumer.resume_hook_site_file_offset,
            hook_file_offset: patched_consumer.hook_file_offset,
            hook_runtime_address: patched_consumer.hook_runtime_address,
            resume_hook_runtime_address: patched_consumer.resume_hook_runtime_address,
            shared_loader_runtime_address: patched_consumer.shared_loader_runtime_address,
            hook_byte_size: patched_consumer.hook_byte_size,
            font,
        },
    })
}

fn load_stage_translations(
    corpus: &ReviewedTranslationCorpus,
) -> Result<Vec<SelectStageTranslation>> {
    (1..=STAGE_COUNT)
        .map(|stage_number| {
            let segment_id = format!("select-stage-{stage_number:02}");
            let segment = corpus
                .segments
                .iter()
                .find(|segment| segment.protected.id == segment_id)
                .with_context(|| format!("translation corpus is missing {segment_id}"))?;
            ensure!(
                segment.protected.entries.len() == SCRIPTS_PER_STAGE,
                "{segment_id} has {} entries instead of {SCRIPTS_PER_STAGE}",
                segment.protected.entries.len()
            );
            let scripts = segment
                .protected
                .entries
                .iter()
                .zip(&segment.draft.entries)
                .map(|(protected, draft)| {
                    ensure!(
                        draft.id == protected.id,
                        "{segment_id} draft entry order changed"
                    );
                    ensure!(
                        matches!(
                            draft.status,
                            TranslationStatus::NeedsHumanReview | TranslationStatus::Complete
                        ),
                        "SELECT translation {} is not ready for reinsertion review",
                        draft.id
                    );
                    let spans = draft
                        .korean_text
                        .iter()
                        .enumerate()
                        .map(|(span_index, text)| {
                            text.clone().with_context(|| {
                                format!(
                                    "SELECT translation {} span {} has no Korean text",
                                    draft.id,
                                    span_index + 1
                                )
                            })
                        })
                        .collect::<Result<Vec<_>>>()?;
                    Ok(SelectScriptTranslation {
                        entry_id: draft.id.clone(),
                        spans,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(SelectStageTranslation {
                stage_number,
                scripts,
            })
        })
        .collect()
}

fn verify_appended_fonts(
    source: &crate::game_data::flink::FlinkArchive<'_>,
    candidate: &[u8],
    font_banks: &[IndexedFontBank],
) -> Result<()> {
    let candidate = parse_flink_archive(candidate)?;
    ensure!(
        candidate.entries[..source.entries.len()]
            .iter()
            .zip(&source.entries)
            .all(|(candidate, source)| candidate.bytes == source.bytes),
        "SELECT translation changed a pre-existing MADDAT payload"
    );
    for (index, bank) in font_banks.iter().enumerate() {
        let entry_id = SELECT_FONT_FIRST_ENTRY_ID + index;
        let decoded = decode_complete_compile_lz(candidate.entries[entry_id - 1].bytes)?;
        ensure!(
            decoded.output == bank.decoded,
            "SELECT stage {} font bank did not survive MADDAT repack",
            index + 1
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "select_translation_tests.rs"]
mod select_translation_tests;
