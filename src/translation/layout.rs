use std::path::Path;

use anyhow::Result;

use crate::source::LocalizationSourceCatalog;

use super::corpus::load_reviewed_translation_corpus;
use super::layout_measurement::{family_hard_limit, measure_translation_entry};
use super::{
    TranslationLayoutAuditReport, TranslationLayoutFamilyReport, TranslationLayoutFinding,
    TranslationLayoutFindingKind, TranslationLayoutOpenGate,
};

pub fn audit_translation_layout(
    source_path: &Path,
    drafts_directory: &Path,
) -> Result<TranslationLayoutAuditReport> {
    let corpus = load_reviewed_translation_corpus(source_path, drafts_directory)?;
    let mut measured = Vec::new();
    let mut findings = Vec::new();
    let mut glyph_token_count = 0;

    for segment in &corpus.segments {
        for (source_entry, draft_entry) in
            segment.protected.entries.iter().zip(&segment.draft.entries)
        {
            let measurement = measure_translation_entry(
                &segment.protected.id,
                segment.protected.source_catalog,
                source_entry,
                draft_entry,
            )?;
            glyph_token_count += measurement.glyph_token_count;
            findings.extend(measurement.structural_findings);
            for (unit_index, unit) in measurement.units.into_iter().enumerate() {
                let hard_overflow = unit
                    .hard_limit_cells
                    .is_some_and(|limit| unit.korean_cells > limit);
                if hard_overflow {
                    findings.push(finding_for_unit(
                        TranslationLayoutFindingKind::HardOverflow,
                        segment.protected.source_catalog,
                        &segment.protected.id,
                        &source_entry.id,
                        unit_index,
                        &unit,
                    ));
                }
                if unit.korean_cells > unit.source_cells {
                    findings.push(finding_for_unit(
                        TranslationLayoutFindingKind::SourceGrowth,
                        segment.protected.source_catalog,
                        &segment.protected.id,
                        &source_entry.id,
                        unit_index,
                        &unit,
                    ));
                }
                measured.push(MeasuredCorpusUnit {
                    source_catalog: segment.protected.source_catalog,
                    source_cells: unit.source_cells,
                    korean_cells: unit.korean_cells,
                    hard_limit_cells: unit.hard_limit_cells,
                });
            }
        }
    }

    let hard_violation_count = findings
        .iter()
        .filter(|finding| finding.kind.is_hard_violation())
        .count();
    let source_growth_advisory_count = findings
        .iter()
        .filter(|finding| finding.kind == TranslationLayoutFindingKind::SourceGrowth)
        .count();
    let hard_limit_unit_count = measured
        .iter()
        .filter(|unit| unit.hard_limit_cells.is_some())
        .count();

    let families = corpus
        .source
        .localization_strategy
        .families
        .iter()
        .map(|family| {
            let units = measured
                .iter()
                .filter(|unit| unit.source_catalog == family.source_catalog)
                .collect::<Vec<_>>();
            let family_findings = findings
                .iter()
                .filter(|finding| finding.source_catalog == family.source_catalog)
                .collect::<Vec<_>>();
            TranslationLayoutFamilyReport {
                source_catalog: family.source_catalog,
                source_layout: family.source_layout,
                entry_count: family.target_ids.len(),
                measured_unit_count: units.len(),
                hard_limit_cells: family_hard_limit(family.source_catalog),
                max_source_cells: units
                    .iter()
                    .map(|unit| unit.source_cells)
                    .max()
                    .unwrap_or(0),
                max_korean_cells: units
                    .iter()
                    .map(|unit| unit.korean_cells)
                    .max()
                    .unwrap_or(0),
                hard_violation_count: family_findings
                    .iter()
                    .filter(|finding| finding.kind.is_hard_violation())
                    .count(),
                source_growth_advisory_count: family_findings
                    .iter()
                    .filter(|finding| finding.kind == TranslationLayoutFindingKind::SourceGrowth)
                    .count(),
                consumer_specific_limit_resolved: consumer_specific_limit_resolved(
                    family.source_catalog,
                ),
            }
        })
        .collect();

    let open_gates = corpus
        .source
        .localization_strategy
        .families
        .iter()
        .filter_map(|family| {
            open_gate_reason(family.source_catalog).map(|reason| TranslationLayoutOpenGate {
                source_catalog: family.source_catalog,
                affected_entry_count: family.target_ids.len(),
                reason: reason.to_owned(),
            })
        })
        .collect();

    Ok(TranslationLayoutAuditReport {
        source_sha256: corpus.validation.source_sha256,
        target_item_count: corpus.validation.target_item_count,
        segment_count: corpus.validation.segment_count,
        human_review_ready: corpus.validation.human_review_ready,
        verified_hard_limits_fit: hard_violation_count == 0,
        measured_unit_count: measured.len(),
        hard_limit_unit_count,
        glyph_token_count,
        hard_violation_count,
        source_growth_advisory_count,
        families,
        open_gates,
        findings,
    })
}

struct MeasuredCorpusUnit {
    source_catalog: LocalizationSourceCatalog,
    source_cells: usize,
    korean_cells: usize,
    hard_limit_cells: Option<usize>,
}

fn finding_for_unit(
    kind: TranslationLayoutFindingKind,
    source_catalog: LocalizationSourceCatalog,
    segment_id: &str,
    entry_id: &str,
    unit_index: usize,
    unit: &super::layout_measurement::MeasuredLayoutUnit,
) -> TranslationLayoutFinding {
    TranslationLayoutFinding {
        kind,
        source_catalog,
        segment_id: segment_id.to_owned(),
        entry_id: entry_id.to_owned(),
        unit_index,
        start_column: unit.start_column,
        source_cells: unit.source_cells,
        korean_cells: unit.korean_cells,
        hard_limit_cells: unit.hard_limit_cells,
        source_text: unit.source_text.clone(),
        korean_text: unit.korean_text.clone(),
    }
}

fn consumer_specific_limit_resolved(source_catalog: LocalizationSourceCatalog) -> bool {
    matches!(
        source_catalog,
        LocalizationSourceCatalog::GraphicText
            | LocalizationSourceCatalog::SelectDialogue
            | LocalizationSourceCatalog::OpeningText
            | LocalizationSourceCatalog::EndingCredits
            | LocalizationSourceCatalog::EndingCurryResult
            | LocalizationSourceCatalog::EndingMealCaptions
            | LocalizationSourceCatalog::EndingMealReaction
            | LocalizationSourceCatalog::EndingInterludeCaption
            | LocalizationSourceCatalog::EndingIngredientLabels
            | LocalizationSourceCatalog::EndingPasswords
    )
}

fn open_gate_reason(source_catalog: LocalizationSourceCatalog) -> Option<&'static str> {
    match source_catalog {
        LocalizationSourceCatalog::MstText | LocalizationSourceCatalog::DosProgramText => Some(
            "the 80-column screen bound is checked, but the cursor position at every DOS AH=09 call still needs consumer-path proof",
        ),
        LocalizationSourceCatalog::MadIndexedText => Some(
            "the 40-cell indexed screen bound is checked, but each menu, status, and battle consumer box still needs position-specific geometry",
        ),
        LocalizationSourceCatalog::GraphicText
        | LocalizationSourceCatalog::SelectDialogue
        | LocalizationSourceCatalog::OpeningText
        | LocalizationSourceCatalog::EndingCredits
        | LocalizationSourceCatalog::EndingCurryResult
        | LocalizationSourceCatalog::EndingMealCaptions
        | LocalizationSourceCatalog::EndingMealReaction
        | LocalizationSourceCatalog::EndingInterludeCaption
        | LocalizationSourceCatalog::EndingIngredientLabels
        | LocalizationSourceCatalog::EndingPasswords => None,
    }
}

impl TranslationLayoutFindingKind {
    fn is_hard_violation(self) -> bool {
        matches!(
            self,
            Self::HardOverflow | Self::GlyphTokenWidth | Self::EmbeddedLineBreak
        )
    }
}
