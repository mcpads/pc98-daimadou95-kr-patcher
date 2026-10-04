use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::corpus::load_reviewed_translation_corpus;
use super::{
    TranslationContentPart, TranslationReviewOutcome, TranslationReviewQueueEntry,
    TranslationReviewQueueReport, TranslationReviewQueueSegment, TranslationReviewTextPair,
};

pub fn report_translation_review(
    source_path: &Path,
    drafts_directory: &Path,
) -> Result<TranslationReviewQueueReport> {
    let corpus = load_reviewed_translation_corpus(source_path, drafts_directory)?;
    let mut segments = Vec::with_capacity(corpus.segments.len());

    for segment in &corpus.segments {
        let review = segment
            .draft
            .review
            .as_ref()
            .with_context(|| format!("{} has no bound independent review", segment.draft.id))?;
        let mut entries = Vec::with_capacity(segment.protected.entries.len());
        for (source_entry, draft_entry) in
            segment.protected.entries.iter().zip(&segment.draft.entries)
        {
            let source_text = source_entry
                .content
                .iter()
                .filter_map(|part| match part {
                    TranslationContentPart::Text { source_text, .. } => Some(source_text),
                    _ => None,
                })
                .collect::<Vec<_>>();
            ensure!(
                source_text.len() == draft_entry.korean_text.len(),
                "{} review queue span count differs from the protected source",
                source_entry.id
            );
            let text = source_text
                .into_iter()
                .zip(&draft_entry.korean_text)
                .map(|(source, korean)| {
                    Ok(TranslationReviewTextPair {
                        source: source.clone(),
                        korean: korean
                            .as_ref()
                            .with_context(|| {
                                format!(
                                    "{} review queue contains an untranslated span",
                                    source_entry.id
                                )
                            })?
                            .clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            entries.push(TranslationReviewQueueEntry {
                id: source_entry.id.clone(),
                source_filename: source_entry.source_filename.clone(),
                source_record_id: source_entry.source_record_id.clone(),
                consumer_roles: source_entry
                    .consumer_contexts
                    .iter()
                    .map(|context| context.role.clone())
                    .collect(),
                text,
                notes: draft_entry.notes.clone(),
            });
        }
        segments.push(TranslationReviewQueueSegment {
            id: segment.protected.id.clone(),
            outcome: review.outcome,
            notes: review.notes.clone(),
            entries,
        });
    }

    let human_decision_segment_count = segments
        .iter()
        .filter(|segment| segment.outcome == TranslationReviewOutcome::HumanDecisionRequired)
        .count();
    let human_decision_entry_count = segments
        .iter()
        .filter(|segment| segment.outcome == TranslationReviewOutcome::HumanDecisionRequired)
        .map(|segment| segment.entries.len())
        .sum();
    let ready_segment_count = segments.len() - human_decision_segment_count;
    let ready_entry_count = corpus.validation.target_item_count - human_decision_entry_count;

    Ok(TranslationReviewQueueReport {
        source_sha256: corpus.validation.source_sha256,
        target_item_count: corpus.validation.target_item_count,
        segment_count: corpus.validation.segment_count,
        human_review_ready: corpus.validation.human_review_ready,
        human_decision_segment_count,
        human_decision_entry_count,
        ready_segment_count,
        ready_entry_count,
        segments,
    })
}
