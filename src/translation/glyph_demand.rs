use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use encoding_rs::SHIFT_JIS;

use crate::font::{verify_ending_credit_repertoire, verify_pc98_repertoire};
use crate::localization::LocalizationRenderPath;
use crate::source::LocalizationSourceCatalog;

use super::corpus::load_reviewed_translation_corpus;
use super::{
    TranslationGlyphAuditReport, TranslationGlyphDemand, TranslationGlyphFamilyReport,
    TranslationGlyphRenderPathReport, TranslationGlyphSegmentReport,
};

pub fn audit_translation_glyphs(
    source_path: &Path,
    drafts_directory: &Path,
) -> Result<TranslationGlyphAuditReport> {
    let corpus = load_reviewed_translation_corpus(source_path, drafts_directory)?;
    let mut measured_segments = Vec::with_capacity(corpus.segments.len());
    for segment in &corpus.segments {
        let mut demand = DemandAccumulator::default();
        for entry in &segment.draft.entries {
            demand.entry_count += 1;
            for text in entry.korean_text.iter().flatten() {
                demand.add_span(text);
            }
        }
        measured_segments.push(MeasuredSegment {
            id: segment.protected.id.clone(),
            source_catalog: segment.protected.source_catalog,
            render_path: segment.protected.render_path,
            demand,
        });
    }

    let capacity_by_path = corpus
        .source
        .localization_strategy
        .render_paths
        .iter()
        .map(|summary| (summary.path, summary.verified_glyph_capacity))
        .collect::<Vec<_>>();
    let embedded_font_characters = measured_segments
        .iter()
        .flat_map(|segment| segment.demand.embedded_font_characters(segment.render_path))
        .collect::<BTreeSet<_>>();
    let font = verify_pc98_repertoire(embedded_font_characters)?;
    let ending_credit_font_characters = measured_segments
        .iter()
        .filter(|segment| segment.render_path == LocalizationRenderPath::EndingCreditFont)
        .flat_map(|segment| segment.demand.characters.iter().copied())
        .filter(|character| *character != ' ')
        .collect::<BTreeSet<_>>();
    let ending_credit_font = verify_ending_credit_repertoire(ending_credit_font_characters)?;

    let render_paths = capacity_by_path
        .iter()
        .map(|(render_path, capacity)| {
            let demand = combine_demands(
                measured_segments
                    .iter()
                    .filter(|segment| segment.render_path == *render_path),
            );
            TranslationGlyphRenderPathReport {
                render_path: *render_path,
                entry_count: demand.entry_count,
                text_span_count: demand.text_span_count,
                demand: demand.report(*render_path, *capacity),
            }
        })
        .collect();

    let families = corpus
        .source
        .localization_strategy
        .families
        .iter()
        .map(|family| {
            let demand = combine_demands(
                measured_segments
                    .iter()
                    .filter(|segment| segment.source_catalog == family.source_catalog),
            );
            TranslationGlyphFamilyReport {
                source_catalog: family.source_catalog,
                render_path: family.render_path,
                entry_count: demand.entry_count,
                text_span_count: demand.text_span_count,
                demand: demand.report(
                    family.render_path,
                    capacity_for(family.render_path, &capacity_by_path),
                ),
            }
        })
        .collect();

    let segments = measured_segments
        .iter()
        .map(|segment| TranslationGlyphSegmentReport {
            id: segment.id.clone(),
            source_catalog: segment.source_catalog,
            render_path: segment.render_path,
            entry_count: segment.demand.entry_count,
            text_span_count: segment.demand.text_span_count,
            demand: segment.demand.report(
                segment.render_path,
                capacity_for(segment.render_path, &capacity_by_path),
            ),
        })
        .collect();

    Ok(TranslationGlyphAuditReport {
        source_sha256: corpus.validation.source_sha256,
        target_item_count: corpus.validation.target_item_count,
        segment_count: corpus.validation.segment_count,
        human_review_ready: corpus.validation.human_review_ready,
        font,
        ending_credit_font,
        render_paths,
        families,
        segments,
    })
}

fn capacity_for(
    render_path: LocalizationRenderPath,
    capacities: &[(LocalizationRenderPath, Option<usize>)],
) -> Option<usize> {
    capacities
        .iter()
        .find(|(candidate, _)| *candidate == render_path)
        .and_then(|(_, capacity)| *capacity)
}

fn combine_demands<'a>(segments: impl Iterator<Item = &'a MeasuredSegment>) -> DemandAccumulator {
    let mut combined = DemandAccumulator::default();
    for segment in segments {
        combined.entry_count += segment.demand.entry_count;
        combined.text_span_count += segment.demand.text_span_count;
        combined
            .characters
            .extend(segment.demand.characters.iter().copied());
    }
    combined
}

struct MeasuredSegment {
    id: String,
    source_catalog: LocalizationSourceCatalog,
    render_path: LocalizationRenderPath,
    demand: DemandAccumulator,
}

#[derive(Default)]
struct DemandAccumulator {
    entry_count: usize,
    text_span_count: usize,
    characters: BTreeSet<char>,
}

impl DemandAccumulator {
    fn add_span(&mut self, text: &str) {
        self.text_span_count += 1;
        self.characters.extend(text.chars());
    }

    fn report(
        &self,
        render_path: LocalizationRenderPath,
        verified_physical_capacity: Option<usize>,
    ) -> TranslationGlyphDemand {
        let slot_characters = match render_path {
            LocalizationRenderPath::SharedIndexedFont => Some(self.characters.clone()),
            LocalizationRenderPath::EndingCreditFont => Some(
                self.characters
                    .iter()
                    .copied()
                    .filter(|character| *character != ' ')
                    .collect(),
            ),
            LocalizationRenderPath::DosSystemText => Some(
                self.characters
                    .iter()
                    .copied()
                    .filter(|character| !is_shift_jis_encodable(*character))
                    .collect(),
            ),
            LocalizationRenderPath::PlanarGraphics => None,
        };
        let slot_glyph_count = slot_characters.as_ref().map(BTreeSet::len);
        let physical_capacity_headroom = verified_physical_capacity
            .zip(slot_glyph_count)
            .map(|(capacity, demand)| capacity as isize - demand as isize);
        let physical_capacity_fit = physical_capacity_headroom.map(|headroom| headroom >= 0);

        TranslationGlyphDemand {
            unique_rendered_character_count: self.characters.len(),
            unique_hangul_syllable_count: self
                .characters
                .iter()
                .filter(|character| is_hangul_syllable(**character))
                .count(),
            slot_glyph_count,
            verified_physical_capacity,
            physical_capacity_headroom,
            physical_capacity_fit,
            rendered_characters: strings_for(&self.characters),
            slot_glyph_characters: slot_characters
                .as_ref()
                .map(strings_for)
                .unwrap_or_default(),
        }
    }

    fn embedded_font_characters(&self, render_path: LocalizationRenderPath) -> BTreeSet<char> {
        match render_path {
            LocalizationRenderPath::SharedIndexedFont | LocalizationRenderPath::PlanarGraphics => {
                self.characters.clone()
            }
            LocalizationRenderPath::EndingCreditFont => BTreeSet::new(),
            LocalizationRenderPath::DosSystemText => self
                .characters
                .iter()
                .copied()
                .filter(|character| !is_shift_jis_encodable(*character))
                .collect(),
        }
    }
}

fn is_shift_jis_encodable(character: char) -> bool {
    let mut encoded = [0_u8; 4];
    let value = character.encode_utf8(&mut encoded);
    let (_, _, had_errors) = SHIFT_JIS.encode(value);
    !had_errors
}

fn is_hangul_syllable(character: char) -> bool {
    matches!(character as u32, 0xAC00..=0xD7A3)
}

fn strings_for(characters: &BTreeSet<char>) -> Vec<String> {
    characters
        .iter()
        .map(|character| character.to_string())
        .collect()
}

#[cfg(test)]
#[path = "glyph_demand_tests.rs"]
mod glyph_demand_tests;
