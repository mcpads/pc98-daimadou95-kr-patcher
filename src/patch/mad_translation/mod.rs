mod bank_plan;
mod consumer;
mod result_banner_window;
mod text;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::font::FontProvenance;
use crate::game_data::compile_lz::decode_complete_compile_lz;
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::indexed_text;
use crate::source::LocalizationSourceCatalog;
use crate::translation::{ReviewedTranslationCorpus, TranslationStatus};

use self::bank_plan::{MAD_FONT_FIRST_ENTRY_ID, MadFontPlan, compile_mad_font_plan};
pub use self::result_banner_window::MadResultBannerWindowReport;
use super::maddat::apply_repacked_maddat;

const MAD_TRANSLATION_COUNT: usize = 240;

pub(crate) struct MadTranslation {
    pub entry_id: String,
    pub record_id: String,
    pub segment_id: String,
    pub text: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MadTranslationFiles {
    pub maddat: Vec<u8>,
    pub mad_com: Vec<u8>,
    pub report: MadTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MadTranslationReport {
    pub translated_entry_count: usize,
    pub source_font_entry_id: usize,
    pub first_font_entry_id: usize,
    pub last_font_entry_id: usize,
    pub font_banks: Vec<MadFontBankReport>,
    pub unit_font_ranges: Vec<MadUnitFontRangeReport>,
    pub text_pools: Vec<MadTextPoolReport>,
    pub result_banner_window: MadResultBannerWindowReport,
    pub pointer_reference_count: usize,
    pub changed_pointer_reference_count: usize,
    pub mad_com_input_size: usize,
    pub mad_com_output_size: usize,
    pub hook_site_file_offset: usize,
    pub hook_file_offset: usize,
    pub hook_runtime_address: usize,
    pub hook_byte_size: usize,
    pub font: FontProvenance,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MadFontBankReport {
    pub entry_id: usize,
    pub bank_id: String,
    pub unit_numbers: Vec<u8>,
    pub slot_count: usize,
    pub packed_size: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MadTextPoolReport {
    pub id: String,
    pub file_offset: usize,
    pub capacity: usize,
    pub used: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MadUnitFontRangeReport {
    pub unit_number: u8,
    pub runtime_start: usize,
    pub runtime_end: usize,
    pub font_entry_id: usize,
}

pub(crate) fn compile_mad_translation(
    maddat: &[u8],
    mad_com: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<MadTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "MAD build requires a human-review-ready translation corpus"
    );
    let translations = load_mad_translations(corpus)?;
    ensure!(
        translations.len() == MAD_TRANSLATION_COUNT,
        "MAD translation corpus has {} entries instead of {MAD_TRANSLATION_COUNT}",
        translations.len()
    );

    let archive = parse_flink_archive(maddat)?;
    ensure!(
        archive.entries.len() + 1 == MAD_FONT_FIRST_ENTRY_ID,
        "MAD font banks require the cumulative MADDAT entry {MAD_FONT_FIRST_ENTRY_ID} slot"
    );
    let source_font_entry = archive
        .entries
        .get(indexed_text::FONT_ENTRY_ID - 1)
        .context("MADDAT is missing the shared indexed font")?;
    let source_font = decode_complete_compile_lz(source_font_entry.bytes)?;
    indexed_text::verify_indexed_font(&source_font.output)?;
    let font_plan = compile_mad_font_plan(
        &source_font.output,
        &corpus.source.mad_indexed_text,
        &translations,
    )?;

    let patched_text = text::replace_mad_indexed_text(
        mad_com,
        &corpus.source.mad_indexed_text,
        &translations,
        &font_plan,
    )?;
    let patched_result_banner_window = result_banner_window::fit_result_banner_window(
        &patched_text.bytes,
        font_plan.result_banner_tile_columns,
    )?;
    let patched_consumer = consumer::install_mad_font_bank_loader(
        &patched_result_banner_window.bytes,
        &patched_text.unit_ranges,
        &font_plan.unit_font_entry_ids,
    )?;

    let appended_fonts = font_plan
        .banks
        .iter()
        .map(|bank| bank.compiled.packed.clone())
        .collect::<Vec<_>>();
    let last_font_entry_id = MAD_FONT_FIRST_ENTRY_ID + appended_fonts.len() - 1;
    let repacked = repack_contiguous_flink_archive(maddat, &BTreeMap::new(), &appended_fonts)?;
    ensure!(
        repacked.replaced_entry_ids.is_empty()
            && repacked.appended_entry_ids
                == (MAD_FONT_FIRST_ENTRY_ID..=last_font_entry_id).collect::<BTreeSet<_>>(),
        "MADDAT repack ownership differs from the MAD translation plan"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "mad-font-maddat-repacker",
        "append the state-specific MAD Hangul font banks",
    )?;
    verify_appended_fonts(&archive, &checked_maddat, &font_plan)?;

    let font = font_plan
        .banks
        .first()
        .context("MAD font compiler produced no font banks")?
        .compiled
        .font
        .clone();
    let font_banks = font_plan
        .banks
        .iter()
        .map(|bank| MadFontBankReport {
            entry_id: bank.entry_id,
            bank_id: bank.bank_id.clone(),
            unit_numbers: bank.unit_numbers.clone(),
            slot_count: bank.slot_count,
            packed_size: bank.compiled.packed.len(),
        })
        .collect();
    let unit_font_ranges = patched_text
        .unit_ranges
        .iter()
        .map(|(&unit_number, range)| MadUnitFontRangeReport {
            unit_number,
            runtime_start: usize::from(range.start),
            runtime_end: usize::from(range.end),
            font_entry_id: font_plan.unit_font_entry_ids[&unit_number],
        })
        .collect();
    let mad_com_output_size = patched_consumer.bytes.len();

    Ok(MadTranslationFiles {
        maddat: checked_maddat,
        mad_com: patched_consumer.bytes,
        report: MadTranslationReport {
            translated_entry_count: translations.len(),
            source_font_entry_id: indexed_text::FONT_ENTRY_ID,
            first_font_entry_id: MAD_FONT_FIRST_ENTRY_ID,
            last_font_entry_id,
            font_banks,
            unit_font_ranges,
            text_pools: patched_text.pool_reports,
            result_banner_window: patched_result_banner_window.report,
            pointer_reference_count: patched_text.pointer_reference_count,
            changed_pointer_reference_count: patched_text.changed_pointer_reference_count,
            mad_com_input_size: mad_com.len(),
            mad_com_output_size,
            hook_site_file_offset: patched_consumer.hook_site_file_offset,
            hook_file_offset: patched_consumer.hook_file_offset,
            hook_runtime_address: patched_consumer.hook_runtime_address,
            hook_byte_size: patched_consumer.hook_byte_size,
            font,
        },
    })
}

fn load_mad_translations(corpus: &ReviewedTranslationCorpus) -> Result<Vec<MadTranslation>> {
    let mut translations = Vec::new();
    let mut record_ids = BTreeSet::new();
    for segment in corpus.segments.iter().filter(|segment| {
        segment.protected.source_catalog == LocalizationSourceCatalog::MadIndexedText
    }) {
        for (protected, draft) in segment.protected.entries.iter().zip(&segment.draft.entries) {
            ensure!(
                protected.id == draft.id,
                "MAD draft entry order changed in {}",
                segment.protected.id
            );
            ensure!(
                matches!(
                    draft.status,
                    TranslationStatus::NeedsHumanReview | TranslationStatus::Complete
                ),
                "MAD translation {} is not ready for development reinsertion",
                draft.id
            );
            ensure!(
                record_ids.insert(protected.source_record_id.clone()),
                "MAD translation corpus duplicates source record {}",
                protected.source_record_id
            );
            ensure!(
                draft.korean_text.len() == 1,
                "MAD translation {} must contain one Korean text span",
                draft.id
            );
            translations.push(MadTranslation {
                entry_id: draft.id.clone(),
                record_id: protected.source_record_id.clone(),
                segment_id: segment.protected.id.clone(),
                text: draft.korean_text[0]
                    .clone()
                    .with_context(|| format!("MAD translation {} has no Korean text", draft.id))?,
            });
        }
    }
    Ok(translations)
}

fn verify_appended_fonts(
    source: &crate::game_data::flink::FlinkArchive<'_>,
    candidate: &[u8],
    font_plan: &MadFontPlan,
) -> Result<()> {
    let candidate = parse_flink_archive(candidate)?;
    ensure!(
        candidate.entries[..source.entries.len()]
            .iter()
            .zip(&source.entries)
            .all(|(candidate, source)| candidate.bytes == source.bytes),
        "MAD translation changed a pre-existing MADDAT payload"
    );
    for bank in &font_plan.banks {
        let decoded = decode_complete_compile_lz(candidate.entries[bank.entry_id - 1].bytes)?;
        ensure!(
            decoded.output == bank.compiled.decoded,
            "MAD font bank {} did not survive MADDAT repack",
            bank.bank_id
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "mad_translation_tests.rs"]
mod mad_translation_tests;
