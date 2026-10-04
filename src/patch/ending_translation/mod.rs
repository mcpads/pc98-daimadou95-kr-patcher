mod consumer;
mod credits;
mod curry_result;
mod ingredients;
mod meal_captions;
mod meal_reaction;
mod text;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::font::FontProvenance;
use crate::game_data::compile_lz::decode_complete_compile_lz;
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::{
    ending_curry_result::DECIMAL_GLYPH_INDICES, ending_ingredient_labels::BLANK_GLYPH_INDEX,
    indexed_text,
};
use crate::translation::ReviewedTranslationCorpus;

use super::indexed_font_bank::compile_indexed_font_bank;
use super::maddat::apply_repacked_maddat;

pub use credits::EndingCreditsPatchReport;
pub use curry_result::EndingCurryResultPatchReport;
pub use meal_captions::EndingMealCaptionPatchReport;
pub use meal_reaction::EndingMealReactionPatchReport;

const INTERLUDE_SEGMENT_ID: &str = "ending-interlude";
const INTERLUDE_ENTRY_ID: &str = "ending-interlude-caption";
const CURRY_RESULT_SEGMENT_ID: &str = "ending-curry-result";
const CREDITS_SEGMENT_ID: &str = "ending-credits";
const MEAL_CAPTIONS_SEGMENT_ID: &str = "ending-meal-captions";
const MEAL_REACTION_SEGMENT_ID: &str = "ending-meal-reaction";
const PASSWORD_SEGMENT_ID: &str = "ending-passwords";
const INGREDIENT_SEGMENT_ID: &str = "ending-interlude-ingredients";
const ENDING_FONT_ENTRY_ID: usize = 156;
const TERMINATOR: u8 = 0xff;
const LINE_BREAK: u8 = 0xfe;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EndingTranslationFiles {
    pub maddat: Vec<u8>,
    pub ending_com: Vec<u8>,
    pub report: EndingTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct EndingTranslationReport {
    pub segment_ids: [String; 7],
    pub translated_entry_count: usize,
    pub source_font_entry_id: usize,
    pub ending_font_entry_id: usize,
    pub unique_character_count: usize,
    pub packed_font_size: usize,
    pub maddat_input_size: usize,
    pub maddat_output_size: usize,
    pub interlude_file_offset: usize,
    pub ingredient_table_file_offset: usize,
    pub ingredient_label_count: usize,
    pub credits: credits::EndingCreditsPatchReport,
    pub curry_result: curry_result::EndingCurryResultPatchReport,
    pub meal_captions: meal_captions::EndingMealCaptionPatchReport,
    pub meal_reaction: meal_reaction::EndingMealReactionPatchReport,
    pub password_pointer_table_file_offset: usize,
    pub password_pool_file_offset: usize,
    pub password_pool_capacity: usize,
    pub password_pool_used: usize,
    pub font_load_file_offsets: [usize; 2],
    pub font: FontProvenance,
}

pub(crate) fn compile_ending_translation(
    maddat: &[u8],
    ending_com: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<EndingTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "ending build requires a human-review-ready translation corpus"
    );
    let interlude_segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == INTERLUDE_SEGMENT_ID)
        .context("translation corpus is missing ending-interlude")?;
    let interlude_text = interlude_segment.reinsertion_text(INTERLUDE_ENTRY_ID)?;
    let curry_result_segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == CURRY_RESULT_SEGMENT_ID)
        .context("translation corpus is missing ending-curry-result")?;
    let curry_result_texts = curry_result_segment
        .protected
        .entries
        .iter()
        .map(|entry| {
            Ok((
                entry.source_record_id.clone(),
                curry_result_segment.reinsertion_text_spans(&entry.id)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let credits_segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == CREDITS_SEGMENT_ID)
        .context("translation corpus is missing ending-credits")?;
    let credit_texts = credits_segment
        .protected
        .entries
        .iter()
        .map(|entry| {
            Ok((
                entry.id.clone(),
                credits_segment.reinsertion_text(&entry.id)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let meal_reaction_segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == MEAL_REACTION_SEGMENT_ID)
        .context("translation corpus is missing ending-meal-reaction")?;
    let meal_reaction_text =
        meal_reaction_segment.reinsertion_text("ending-meal-reaction:bite-sound-effect")?;
    let meal_captions_segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == MEAL_CAPTIONS_SEGMENT_ID)
        .context("translation corpus is missing ending-meal-captions")?;
    let meal_caption_texts = meal_captions_segment
        .protected
        .entries
        .iter()
        .map(|entry| {
            Ok((
                entry.source_record_id.clone(),
                meal_captions_segment.reinsertion_text_spans(&entry.id)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let password_segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == PASSWORD_SEGMENT_ID)
        .context("translation corpus is missing ending-passwords")?;
    let password_texts = password_segment
        .protected
        .entries
        .iter()
        .map(|entry| password_segment.reinsertion_text(&entry.id))
        .collect::<Result<Vec<_>>>()?;
    let ingredient_segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == INGREDIENT_SEGMENT_ID)
        .context("translation corpus is missing ending-interlude-ingredients")?;
    let ingredient_texts = ingredient_segment
        .protected
        .entries
        .iter()
        .map(|entry| {
            Ok((
                entry.source_record_id.clone(),
                ingredient_segment.reinsertion_text(&entry.id)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;

    let archive = parse_flink_archive(maddat)?;
    ensure!(
        archive.entries.len() + 1 == ENDING_FONT_ENTRY_ID,
        "ending font bank requires the cumulative MADDAT entry {ENDING_FONT_ENTRY_ID} slot"
    );
    let source_font_entry = archive
        .entries
        .get(indexed_text::FONT_ENTRY_ID - 1)
        .context("MADDAT is missing the shared indexed font")?;
    let source_font = decode_complete_compile_lz(source_font_entry.bytes)?;
    indexed_text::verify_indexed_font(&source_font.output)?;
    let source_credit_font_entry = archive
        .entries
        .get(crate::game_data::ending_credits::FONT_ENTRY_ID - 1)
        .context("MADDAT is missing the ending-credit font")?;
    let source_credit_text_entry = archive
        .entries
        .get(crate::game_data::ending_credits::TEXT_ENTRY_ID - 1)
        .context("MADDAT is missing the ending-credit text grid")?;
    let credits = credits::compile_ending_credits(
        source_credit_font_entry.bytes,
        source_credit_text_entry.bytes,
        &credit_texts,
    )?;
    let characters = std::iter::once(interlude_text.as_str())
        .chain(std::iter::once(meal_reaction_text.as_str()))
        .chain(
            meal_caption_texts
                .values()
                .flat_map(|spans| spans.iter().map(String::as_str)),
        )
        .chain(password_texts.iter().map(String::as_str))
        .chain(ingredient_texts.values().map(String::as_str))
        .chain(
            curry_result_texts
                .values()
                .flat_map(|spans| spans.iter().map(String::as_str)),
        )
        .flat_map(str::chars);
    let reserved_indices = DECIMAL_GLYPH_INDICES
        .into_iter()
        .chain([BLANK_GLYPH_INDEX, LINE_BREAK, TERMINATOR])
        .collect::<BTreeSet<_>>();
    let font_bank =
        compile_indexed_font_bank(&source_font.output, characters, &reserved_indices, "ending")?;

    let replacements = BTreeMap::from([
        (
            crate::game_data::ending_credits::FONT_ENTRY_ID,
            credits.packed_font.clone(),
        ),
        (
            crate::game_data::ending_credits::TEXT_ENTRY_ID,
            credits.text_grid.clone(),
        ),
    ]);
    let repacked = repack_contiguous_flink_archive(
        maddat,
        &replacements,
        std::slice::from_ref(&font_bank.packed),
    )?;
    ensure!(
        repacked.replaced_entry_ids
            == BTreeSet::from([
                crate::game_data::ending_credits::FONT_ENTRY_ID,
                crate::game_data::ending_credits::TEXT_ENTRY_ID,
            ])
            && repacked.appended_entry_ids == BTreeSet::from([ENDING_FONT_ENTRY_ID]),
        "MADDAT repack ownership differs from the ending translation plan"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "ending-maddat-repacker",
        "replace localized ending-credit assets and append the isolated Hangul font bank for ENDING.COM",
    )?;
    let candidate_archive = parse_flink_archive(&checked_maddat)?;
    ensure!(
        candidate_archive.entries[..archive.entries.len()]
            .iter()
            .zip(&archive.entries)
            .all(|(candidate, source)| {
                replacements
                    .get(&source.id)
                    .map_or(candidate.bytes == source.bytes, |replacement| {
                        candidate.bytes == replacement
                    })
            }),
        "ending translation changed a non-target MADDAT payload"
    );
    let decoded_credit_font = decode_complete_compile_lz(
        candidate_archive.entries[crate::game_data::ending_credits::FONT_ENTRY_ID - 1].bytes,
    )?;
    ensure!(
        decoded_credit_font.output == credits.decoded_font
            && candidate_archive.entries[crate::game_data::ending_credits::TEXT_ENTRY_ID - 1].bytes
                == credits.text_grid,
        "ending-credit assets did not survive MADDAT repack"
    );
    let decoded_ending_font =
        decode_complete_compile_lz(candidate_archive.entries[ENDING_FONT_ENTRY_ID - 1].bytes)?;
    ensure!(
        decoded_ending_font.output == font_bank.decoded,
        "ending font bank did not survive MADDAT repack"
    );

    let patched_text = text::replace_ending_text(
        ending_com,
        &interlude_text,
        &password_texts,
        &font_bank.codebook,
    )?;
    let patched_ingredients = ingredients::replace_ending_ingredient_labels(
        &patched_text.bytes,
        &ingredient_texts,
        &font_bank.codebook,
    )?;
    let patched_curry_result = curry_result::replace_ending_curry_result(
        &patched_ingredients.bytes,
        &curry_result_texts,
        &font_bank.codebook,
    )?;
    let patched_meal_captions = meal_captions::replace_ending_meal_captions(
        &patched_curry_result.bytes,
        &meal_caption_texts,
        &font_bank.codebook,
    )?;
    let patched_meal_reaction = meal_reaction::replace_ending_meal_reaction(
        &patched_meal_captions.bytes,
        &meal_reaction_text,
        &font_bank.codebook,
    )?;
    let patched_consumer = consumer::select_ending_font_bank(&patched_meal_reaction.bytes)?;

    Ok(EndingTranslationFiles {
        maddat: checked_maddat,
        ending_com: patched_consumer.bytes,
        report: EndingTranslationReport {
            segment_ids: [
                INTERLUDE_SEGMENT_ID.into(),
                CURRY_RESULT_SEGMENT_ID.into(),
                CREDITS_SEGMENT_ID.into(),
                INGREDIENT_SEGMENT_ID.into(),
                MEAL_CAPTIONS_SEGMENT_ID.into(),
                MEAL_REACTION_SEGMENT_ID.into(),
                PASSWORD_SEGMENT_ID.into(),
            ],
            translated_entry_count: 1
                + curry_result_texts.len()
                + credit_texts.len()
                + ingredient_texts.len()
                + meal_caption_texts.len()
                + 1
                + password_texts.len(),
            source_font_entry_id: indexed_text::FONT_ENTRY_ID,
            ending_font_entry_id: ENDING_FONT_ENTRY_ID,
            unique_character_count: font_bank.codebook.len(),
            packed_font_size: font_bank.packed.len(),
            maddat_input_size: repacked.input_size,
            maddat_output_size: repacked.output_size,
            interlude_file_offset: patched_text.interlude_file_offset,
            ingredient_table_file_offset: patched_ingredients.table_file_offset,
            ingredient_label_count: patched_ingredients.label_count,
            credits: credits.report,
            curry_result: patched_curry_result.report,
            meal_captions: patched_meal_captions.report,
            meal_reaction: patched_meal_reaction.report,
            password_pointer_table_file_offset: patched_text.pointer_table_file_offset,
            password_pool_file_offset: patched_text.password_pool_file_offset,
            password_pool_capacity: patched_text.password_pool_capacity,
            password_pool_used: patched_text.password_pool_used,
            font_load_file_offsets: patched_consumer.font_load_file_offsets,
            font: font_bank.font,
        },
    })
}

#[cfg(test)]
#[path = "ending_translation_tests.rs"]
mod ending_translation_tests;
