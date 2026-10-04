mod consumer;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use crate::font::FontProvenance;
use crate::game_data::compile_lz::decode_complete_compile_lz;
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::{indexed_text, opening_script};
use crate::translation::ReviewedTranslationCorpus;
use crate::translation::TranslationStatus;

use super::indexed_font_bank::compile_indexed_font_bank;
use super::maddat::apply_repacked_maddat;

const SEGMENT_ID: &str = "opening-sequence";
const ENTRY_ID: &str = "opening-text";
const OPENING_FONT_ENTRY_ID: usize = 155;
const VERIFIED_DISPLAY_RUN_LENGTHS: [usize; 5] = [19, 19, 10, 19, 11];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OpeningTranslationFiles {
    pub maddat: Vec<u8>,
    pub opening_com: Vec<u8>,
    pub report: OpeningTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct OpeningTranslationReport {
    pub segment_id: String,
    pub translation_entry_id: String,
    pub script_entry_id: usize,
    pub source_font_entry_id: usize,
    pub opening_font_entry_id: usize,
    pub glyph_command_count: usize,
    pub unique_character_count: usize,
    pub packed_font_size: usize,
    pub maddat_input_size: usize,
    pub maddat_output_size: usize,
    pub font_load_file_offset: usize,
    pub font: FontProvenance,
}

pub(crate) fn compile_opening_translation(
    maddat: &[u8],
    opening_com: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<OpeningTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "opening build requires a human-review-ready translation corpus"
    );
    let segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == SEGMENT_ID)
        .context("translation corpus is missing opening-sequence")?;
    let protected_entry_index = segment
        .protected
        .entries
        .iter()
        .position(|entry| entry.id == ENTRY_ID)
        .context("protected opening segment is missing opening-text")?;
    let protected_entry = &segment.protected.entries[protected_entry_index];
    let draft_entry = segment
        .draft
        .entries
        .get(protected_entry_index)
        .context("draft opening segment is missing opening-text")?;
    ensure!(
        draft_entry.id == protected_entry.id,
        "draft opening entry order differs from its protected segment"
    );
    ensure!(
        matches!(
            draft_entry.status,
            TranslationStatus::NeedsHumanReview | TranslationStatus::Complete
        ),
        "opening translation is not ready for reinsertion review"
    );
    let archive = parse_flink_archive(maddat)?;
    ensure!(
        archive.entries.len() + 1 == OPENING_FONT_ENTRY_ID,
        "opening font bank must be appended as MADDAT entry {OPENING_FONT_ENTRY_ID}"
    );
    let source_font_entry = archive
        .entries
        .get(indexed_text::FONT_ENTRY_ID - 1)
        .context("MADDAT is missing the shared indexed font")?;
    let source_font = decode_complete_compile_lz(source_font_entry.bytes)?;
    indexed_text::verify_indexed_font(&source_font.output)?;
    let script_entry = archive
        .entries
        .get(opening_script::MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing the opening script")?;
    let script = opening_script::parse_opening_text_script(script_entry.bytes)?;
    let display_runs = opening_script::opening_display_runs(&script.tokens)?;
    let characters = opening_characters(&draft_entry.korean_text)?;
    validate_opening_display_runs(&characters, &display_runs)?;
    let reserved_indices: BTreeSet<u8> = script
        .tokens
        .iter()
        .filter_map(|token| match token {
            opening_script::OpeningScriptToken::Sprite { glyph_index, .. } => Some(*glyph_index),
            _ => None,
        })
        .collect();

    let font_bank = compile_indexed_font_bank(
        &source_font.output,
        characters.iter().copied(),
        &reserved_indices,
        "opening",
    )?;
    let replacement_indices = characters
        .iter()
        .map(|character| {
            font_bank
                .codebook
                .get(character)
                .copied()
                .with_context(|| format!("opening font codebook is missing {character:?}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let translated_script =
        opening_script::replace_opening_glyph_indices(script_entry.bytes, &replacement_indices)?;

    let replacements =
        BTreeMap::from([(opening_script::MADDAT_ENTRY_ID, translated_script.clone())]);
    let repacked = repack_contiguous_flink_archive(
        maddat,
        &replacements,
        std::slice::from_ref(&font_bank.packed),
    )?;
    ensure!(
        repacked.replaced_entry_ids == BTreeSet::from([opening_script::MADDAT_ENTRY_ID])
            && repacked.appended_entry_ids == BTreeSet::from([OPENING_FONT_ENTRY_ID]),
        "MADDAT repack ownership differs from the opening translation plan"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "opening-maddat-repacker",
        "replace the opening script and append its isolated Hangul font bank",
    )?;
    let candidate_archive = parse_flink_archive(&checked_maddat)?;
    ensure!(
        candidate_archive.entries[indexed_text::FONT_ENTRY_ID - 1].bytes == source_font_entry.bytes,
        "opening translation changed the shared indexed font"
    );
    ensure!(
        candidate_archive.entries[opening_script::MADDAT_ENTRY_ID - 1].bytes == translated_script,
        "opening translation script did not survive MADDAT repack"
    );
    let decoded_opening_font =
        decode_complete_compile_lz(candidate_archive.entries[OPENING_FONT_ENTRY_ID - 1].bytes)?;
    ensure!(
        decoded_opening_font.output == font_bank.decoded,
        "opening font bank did not survive MADDAT repack"
    );

    let patched_consumer = consumer::select_opening_font_bank(opening_com)?;
    Ok(OpeningTranslationFiles {
        maddat: checked_maddat,
        opening_com: patched_consumer.bytes,
        report: OpeningTranslationReport {
            segment_id: SEGMENT_ID.to_owned(),
            translation_entry_id: ENTRY_ID.to_owned(),
            script_entry_id: opening_script::MADDAT_ENTRY_ID,
            source_font_entry_id: indexed_text::FONT_ENTRY_ID,
            opening_font_entry_id: OPENING_FONT_ENTRY_ID,
            glyph_command_count: characters.len(),
            unique_character_count: font_bank.codebook.len(),
            packed_font_size: font_bank.packed.len(),
            maddat_input_size: repacked.input_size,
            maddat_output_size: repacked.output_size,
            font_load_file_offset: patched_consumer.file_offset,
            font: font_bank.font,
        },
    })
}

fn opening_characters(korean_text: &[Option<String>]) -> Result<Vec<char>> {
    korean_text
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let text = text
                .as_deref()
                .with_context(|| format!("opening text span {index} has no Korean text"))?;
            let mut characters = text.chars();
            let character = characters
                .next()
                .with_context(|| format!("opening text span {index} is empty"))?;
            ensure!(
                characters.next().is_none(),
                "opening text span {index} must contain exactly one rendered character"
            );
            Ok(character)
        })
        .collect()
}

fn validate_opening_display_runs(
    characters: &[char],
    runs: &[opening_script::OpeningDisplayRun],
) -> Result<()> {
    let run_lengths = runs
        .iter()
        .map(|run| run.glyph_range.len())
        .collect::<Vec<_>>();
    ensure!(
        run_lengths == VERIFIED_DISPLAY_RUN_LENGTHS,
        "opening display runs differ from the verified consumer layout: expected {VERIFIED_DISPLAY_RUN_LENGTHS:?}, got {run_lengths:?}"
    );
    ensure!(
        runs.first().map(|run| run.glyph_range.start) == Some(0)
            && runs.last().map(|run| run.glyph_range.end) == Some(characters.len())
            && runs
                .windows(2)
                .all(|pair| pair[0].glyph_range.end == pair[1].glyph_range.start),
        "opening display runs do not cover the translated glyph stream exactly"
    );

    for (run_index, run) in runs.iter().enumerate() {
        let text = characters
            .get(run.glyph_range.clone())
            .context("opening display run lies outside the translated glyph stream")?;
        let first = *text
            .first()
            .context("opening translation contains an empty display run")?;
        ensure!(
            !first.is_whitespace() && !is_sentence_ending(first),
            "opening display run {run_index} starts with detached padding or punctuation"
        );
        let last_visible = text
            .iter()
            .copied()
            .rfind(|character| !character.is_whitespace())
            .context("opening translation contains a blank display run")?;
        ensure!(
            is_sentence_ending(last_visible),
            "opening display run {run_index} ends mid-sentence with {last_visible:?}"
        );

        let mut line_start = 0;
        for line_end in run
            .line_break_offsets
            .iter()
            .copied()
            .chain(std::iter::once(text.len()))
        {
            ensure!(
                text[line_start..line_end]
                    .iter()
                    .any(|character| !character.is_whitespace()),
                "opening display run {run_index} contains a blank rendered line"
            );
            line_start = line_end;
        }
    }
    Ok(())
}

fn is_sentence_ending(character: char) -> bool {
    matches!(character, '!' | '?' | '.' | '…' | '~')
}

#[cfg(test)]
#[path = "opening_translation_tests.rs"]
mod opening_translation_tests;
