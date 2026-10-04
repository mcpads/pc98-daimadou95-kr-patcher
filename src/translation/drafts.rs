use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::source::{disk, survey_source_path};

use super::workspace::build_translation_workspace;
use super::{
    TranslationContentPart, TranslationDraftEntry, TranslationDraftIndex,
    TranslationDraftPreparationReport, TranslationDraftSegment, TranslationDraftSegmentIndex,
    TranslationDraftValidationReport, TranslationSegment, TranslationStatus,
};

const INDEX_FILENAME: &str = "index.json";
const SEGMENTS_DIRECTORY: &str = "segments";

pub fn prepare_translation_drafts(
    source_path: &Path,
    output_directory: &Path,
) -> Result<TranslationDraftPreparationReport> {
    let source = survey_source_path(source_path)?;
    let protected = build_translation_workspace(&source)?;
    let drafts = build_translation_drafts(&protected.segments, &source.source_sha256);
    write_translation_drafts(output_directory, &drafts)?;
    Ok(TranslationDraftPreparationReport {
        source_sha256: drafts.index.source_sha256.clone(),
        output_directory: output_directory.to_owned(),
        target_item_count: drafts.index.target_item_count,
        segment_count: drafts.index.segment_count,
    })
}

pub fn validate_translation_drafts(
    source_path: &Path,
    drafts_directory: &Path,
    require_human_review_ready: bool,
) -> Result<TranslationDraftValidationReport> {
    let source = survey_source_path(source_path)?;
    let protected = build_translation_workspace(&source)?;
    let expected = build_translation_drafts(&protected.segments, &source.source_sha256);
    let actual_index: TranslationDraftIndex = read_json(&drafts_directory.join(INDEX_FILENAME))?;
    ensure!(
        actual_index == expected.index,
        "translation draft index differs from the current supported-source population"
    );

    let expected_paths = actual_index
        .segments
        .iter()
        .map(|segment| segment.path.clone())
        .collect::<BTreeSet<_>>();
    let actual_paths = list_segment_paths(drafts_directory)?;
    ensure!(
        actual_paths == expected_paths,
        "translation draft segment files differ from the index: expected {expected_paths:?}, got {actual_paths:?}"
    );

    let protected_by_id = protected
        .segments
        .iter()
        .map(|segment| (segment.id.as_str(), segment))
        .collect::<BTreeMap<_, _>>();
    let mut target_ids = BTreeSet::new();
    let mut counts = DraftStatusCounts::default();
    let mut reviewed_segment_count = 0;
    for indexed in &actual_index.segments {
        let actual: TranslationDraftSegment = read_json(&drafts_directory.join(&indexed.path))?;
        let protected_segment = protected_by_id
            .get(indexed.id.as_str())
            .with_context(|| format!("index names unknown translation segment {}", indexed.id))?;
        validate_draft_segment(&actual, protected_segment, &mut target_ids, &mut counts)?;
        if actual.review.is_some() {
            reviewed_segment_count += 1;
        }
    }

    ensure!(
        target_ids.len() == actual_index.target_item_count,
        "translation drafts do not cover the indexed target population"
    );
    let human_review_ready = counts.needs_human_review == actual_index.target_item_count
        && reviewed_segment_count == actual_index.segment_count;
    if require_human_review_ready {
        ensure!(
            human_review_ready,
            "translation drafts are not ready for human review: {}/{} entries and {}/{} segments passed independent review",
            counts.needs_human_review,
            actual_index.target_item_count,
            reviewed_segment_count,
            actual_index.segment_count
        );
    }

    Ok(TranslationDraftValidationReport {
        source_sha256: actual_index.source_sha256,
        target_item_count: target_ids.len(),
        segment_count: actual_index.segment_count,
        untranslated_item_count: counts.untranslated,
        in_progress_item_count: counts.in_progress,
        needs_review_item_count: counts.needs_review,
        needs_human_review_item_count: counts.needs_human_review,
        reviewed_segment_count,
        human_review_ready,
    })
}

struct TranslationDrafts {
    index: TranslationDraftIndex,
    segments: Vec<TranslationDraftSegment>,
}

fn build_translation_drafts(
    protected_segments: &[TranslationSegment],
    source_sha256: &str,
) -> TranslationDrafts {
    let segments = protected_segments
        .iter()
        .map(|segment| TranslationDraftSegment {
            id: segment.id.clone(),
            entries: segment
                .entries
                .iter()
                .map(|entry| TranslationDraftEntry {
                    id: entry.id.clone(),
                    korean_text: entry
                        .content
                        .iter()
                        .filter(|part| matches!(part, TranslationContentPart::Text { .. }))
                        .map(|_| None)
                        .collect(),
                    status: TranslationStatus::Untranslated,
                    notes: None,
                })
                .collect(),
            review: None,
        })
        .collect::<Vec<_>>();
    let index_segments = segments
        .iter()
        .map(|segment| TranslationDraftSegmentIndex {
            id: segment.id.clone(),
            path: format!("{SEGMENTS_DIRECTORY}/{}.json", segment.id),
            entry_count: segment.entries.len(),
        })
        .collect::<Vec<_>>();
    let target_item_count = segments.iter().map(|segment| segment.entries.len()).sum();
    TranslationDrafts {
        index: TranslationDraftIndex {
            source_sha256: source_sha256.to_owned(),
            target_item_count,
            segment_count: segments.len(),
            segments: index_segments,
        },
        segments,
    }
}

fn validate_draft_segment(
    actual: &TranslationDraftSegment,
    protected: &TranslationSegment,
    target_ids: &mut BTreeSet<String>,
    counts: &mut DraftStatusCounts,
) -> Result<()> {
    ensure!(
        actual.id == protected.id,
        "translation draft segment ID differs from the protected segment"
    );
    ensure!(
        actual.entries.len() == protected.entries.len(),
        "{} entry count differs from the protected segment",
        actual.id
    );
    for (draft, source) in actual.entries.iter().zip(&protected.entries) {
        ensure!(
            draft.id == source.id,
            "{} translation entry order or ID differs from the protected segment",
            actual.id
        );
        ensure!(
            target_ids.insert(draft.id.clone()),
            "translation drafts contain duplicate target ID {}",
            draft.id
        );
        validate_draft_entry(draft, source)?;
        counts.add(draft.status)?;
    }

    match &actual.review {
        Some(review) => {
            ensure!(
                review.reviewed_draft_sha256 == reviewable_draft_sha256(actual)?,
                "{} independent-review hash does not match its current draft",
                actual.id
            );
            ensure!(
                actual
                    .entries
                    .iter()
                    .all(|entry| entry.status == TranslationStatus::NeedsHumanReview),
                "{} has an independent review but not every entry needs human review",
                actual.id
            );
            for note in &review.notes {
                ensure!(
                    !note.trim().is_empty(),
                    "{} independent review contains an empty note",
                    actual.id
                );
            }
        }
        None => ensure!(
            actual
                .entries
                .iter()
                .all(|entry| entry.status != TranslationStatus::NeedsHumanReview),
            "{} claims human-review status without an independent review",
            actual.id
        ),
    }
    Ok(())
}

fn validate_draft_entry(
    draft: &TranslationDraftEntry,
    source: &super::TranslationEntry,
) -> Result<()> {
    if let Some(notes) = &draft.notes {
        ensure!(!notes.trim().is_empty(), "{} has empty notes", draft.id);
    }
    let expected_span_count = source
        .content
        .iter()
        .filter(|part| matches!(part, TranslationContentPart::Text { .. }))
        .count();
    ensure!(
        draft.korean_text.len() == expected_span_count,
        "{} Korean span count differs from the protected source",
        draft.id
    );
    for text in draft.korean_text.iter().filter_map(|text| text.as_deref()) {
        validate_korean_text(&draft.id, text)?;
    }
    let translated = draft
        .korean_text
        .iter()
        .filter(|text| text.is_some())
        .count();
    match draft.status {
        TranslationStatus::Untranslated => ensure!(
            translated == 0,
            "{} is untranslated but contains Korean text",
            draft.id
        ),
        TranslationStatus::InProgress => ensure!(
            translated > 0,
            "{} is in progress but has no Korean text",
            draft.id
        ),
        TranslationStatus::NeedsReview | TranslationStatus::NeedsHumanReview => ensure!(
            translated == expected_span_count,
            "{} is ready for review but has untranslated text spans",
            draft.id
        ),
        TranslationStatus::Complete => bail!(
            "{} claims complete before a human-approval input exists",
            draft.id
        ),
    }
    Ok(())
}

fn validate_korean_text(id: &str, text: &str) -> Result<()> {
    ensure!(!text.is_empty(), "{id} has an empty Korean text span");
    ensure!(
        !contains_japanese_script(text),
        "{id} Korean draft retains Japanese script"
    );
    Ok(())
}

pub(super) fn reviewable_draft_sha256(segment: &TranslationDraftSegment) -> Result<String> {
    let value = serde_json::json!({
        "id": segment.id,
        "entries": segment.entries,
    });
    Ok(disk::sha256_hex(&serde_json::to_vec(&value)?))
}

fn contains_japanese_script(value: &str) -> bool {
    value.chars().any(|character| {
        matches!(
            character as u32,
            0x2E80..=0x2FDF
                | 0x3005..=0x3007
                | 0x3040..=0x30FF
                | 0x31F0..=0x31FF
                | 0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
        )
    })
}

fn write_translation_drafts(output_directory: &Path, drafts: &TranslationDrafts) -> Result<()> {
    if output_directory.exists() {
        bail!(
            "refusing to overwrite existing translation drafts: {}",
            output_directory.display()
        );
    }
    let parent = output_directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create translation draft parent {}",
            parent.display()
        )
    })?;
    fs::create_dir(output_directory).with_context(|| {
        format!(
            "failed to create translation draft directory {}",
            output_directory.display()
        )
    })?;
    fs::create_dir(output_directory.join(SEGMENTS_DIRECTORY)).with_context(|| {
        format!(
            "failed to create translation draft segments in {}",
            output_directory.display()
        )
    })?;
    for (indexed, segment) in drafts.index.segments.iter().zip(&drafts.segments) {
        write_new_json(&output_directory.join(&indexed.path), segment)?;
    }
    write_new_json(&output_directory.join(INDEX_FILENAME), &drafts.index)?;
    Ok(())
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("refusing to overwrite {}", path.display()))?;
    file.write_all(&bytes)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to flush {}", path.display()))?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))
}

fn list_segment_paths(drafts_directory: &Path) -> Result<BTreeSet<String>> {
    let segments_directory = drafts_directory.join(SEGMENTS_DIRECTORY);
    let mut paths = BTreeSet::new();
    for entry in fs::read_dir(&segments_directory).with_context(|| {
        format!(
            "failed to read translation draft segments {}",
            segments_directory.display()
        )
    })? {
        let entry = entry.context("failed to read translation draft segment entry")?;
        ensure!(
            entry
                .file_type()
                .context("failed to read translation draft segment file type")?
                .is_file(),
            "translation draft segments directory contains a non-file entry"
        );
        paths.insert(
            entry
                .path()
                .strip_prefix(drafts_directory)
                .context("translation draft segment lies outside its directory")?
                .to_str()
                .context("translation draft segment path is not UTF-8")?
                .to_owned(),
        );
    }
    Ok(paths)
}

#[derive(Default)]
struct DraftStatusCounts {
    untranslated: usize,
    in_progress: usize,
    needs_review: usize,
    needs_human_review: usize,
}

impl DraftStatusCounts {
    fn add(&mut self, status: TranslationStatus) -> Result<()> {
        match status {
            TranslationStatus::Untranslated => self.untranslated += 1,
            TranslationStatus::InProgress => self.in_progress += 1,
            TranslationStatus::NeedsReview => self.needs_review += 1,
            TranslationStatus::NeedsHumanReview => self.needs_human_review += 1,
            TranslationStatus::Complete => {
                bail!("complete translation status requires a human-approval input")
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "drafts_tests.rs"]
mod drafts_tests;
