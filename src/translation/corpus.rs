use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::source::{SourceStructureReport, survey_source_path};

use super::workspace::build_translation_workspace;
use super::{
    TranslationDraftIndex, TranslationDraftSegment, TranslationDraftValidationReport,
    TranslationSegment, TranslationStatus, validate_translation_drafts,
};

pub(crate) struct ReviewedTranslationCorpus {
    pub(crate) source: SourceStructureReport,
    pub(crate) validation: TranslationDraftValidationReport,
    pub(crate) segments: Vec<ReviewedTranslationSegment>,
}

pub(crate) struct ReviewedTranslationSegment {
    pub(crate) protected: TranslationSegment,
    pub(crate) draft: TranslationDraftSegment,
}

impl ReviewedTranslationSegment {
    pub(crate) fn reinsertion_text(&self, entry_id: &str) -> Result<String> {
        let mut spans = self.reinsertion_text_spans(entry_id)?;
        ensure!(
            spans.len() == 1,
            "translation entry {entry_id} must contain one Korean text span"
        );
        Ok(spans.remove(0))
    }

    pub(crate) fn reinsertion_text_spans(&self, entry_id: &str) -> Result<Vec<String>> {
        let index = self
            .protected
            .entries
            .iter()
            .position(|entry| entry.id == entry_id)
            .with_context(|| {
                format!(
                    "protected translation segment {} is missing {entry_id}",
                    self.protected.id
                )
            })?;
        let draft = self.draft.entries.get(index).with_context(|| {
            format!(
                "draft translation segment {} is missing {entry_id}",
                self.draft.id
            )
        })?;
        ensure!(
            draft.id == entry_id,
            "draft translation entry order changed"
        );
        ensure!(
            matches!(
                draft.status,
                TranslationStatus::NeedsHumanReview | TranslationStatus::Complete
            ),
            "translation entry {entry_id} is not ready for reinsertion review"
        );
        draft
            .korean_text
            .iter()
            .enumerate()
            .map(|(span_index, text)| {
                text.clone().with_context(|| {
                    format!("translation entry {entry_id} has no Korean text in span {span_index}")
                })
            })
            .collect()
    }
}

pub(crate) fn load_reviewed_translation_corpus(
    source_path: &Path,
    drafts_directory: &Path,
) -> Result<ReviewedTranslationCorpus> {
    let validation = validate_translation_drafts(source_path, drafts_directory, true)?;
    let source = survey_source_path(source_path)?;
    let protected = build_translation_workspace(&source)?;
    let draft_index: TranslationDraftIndex = read_json(&drafts_directory.join("index.json"))?;

    let mut draft_segments = BTreeMap::new();
    for indexed in &draft_index.segments {
        let segment: TranslationDraftSegment = read_json(&drafts_directory.join(&indexed.path))?;
        ensure!(
            draft_segments.insert(segment.id.clone(), segment).is_none(),
            "reviewed translation corpus found duplicate segment {}",
            indexed.id
        );
    }

    let mut segments = Vec::with_capacity(protected.segments.len());
    for protected in protected.segments {
        let draft = draft_segments
            .remove(&protected.id)
            .with_context(|| format!("reviewed translation corpus is missing {}", protected.id))?;
        ensure!(
            protected.entries.len() == draft.entries.len(),
            "{} entry count changed after draft validation",
            protected.id
        );
        segments.push(ReviewedTranslationSegment { protected, draft });
    }
    ensure!(
        draft_segments.is_empty(),
        "reviewed translation corpus contains segments outside the protected population"
    );

    Ok(ReviewedTranslationCorpus {
        source,
        validation,
        segments,
    })
}

fn read_json<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Result<T> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))
}
