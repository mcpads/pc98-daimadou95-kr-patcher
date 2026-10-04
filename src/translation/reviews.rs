use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Serialize, de::DeserializeOwned};

use super::drafts::reviewable_draft_sha256;
use super::{
    RecordedTranslationReview, TranslationDraftIndex, TranslationDraftSegment,
    TranslationReviewOutcome, TranslationSegmentReview, TranslationStatus,
    validate_translation_drafts,
};

const INDEX_FILENAME: &str = "index.json";

pub fn record_translation_segment_review(
    source_path: &Path,
    drafts_directory: &Path,
    segment_id: &str,
    outcome: TranslationReviewOutcome,
    notes: Vec<String>,
) -> Result<RecordedTranslationReview> {
    validate_translation_drafts(source_path, drafts_directory, false)?;

    let index: TranslationDraftIndex = read_json(&drafts_directory.join(INDEX_FILENAME))?;
    let indexed = index
        .segments
        .iter()
        .find(|indexed| indexed.id == segment_id)
        .with_context(|| format!("translation draft index has no segment {segment_id}"))?;
    let segment_path = drafts_directory.join(&indexed.path);
    let mut segment: TranslationDraftSegment = read_json(&segment_path)?;
    ensure!(
        segment.id == segment_id,
        "translation draft segment ID differs from the requested segment"
    );

    let recorded = bind_translation_segment_review(&mut segment, outcome, notes)?;
    replace_json(&segment_path, &segment)?;
    validate_translation_drafts(source_path, drafts_directory, false).with_context(|| {
        format!("recorded review did not validate for translation segment {segment_id}")
    })?;
    Ok(recorded)
}

fn bind_translation_segment_review(
    segment: &mut TranslationDraftSegment,
    outcome: TranslationReviewOutcome,
    notes: Vec<String>,
) -> Result<RecordedTranslationReview> {
    ensure!(
        segment.review.is_none(),
        "{} already has an independent review",
        segment.id
    );
    ensure!(
        !notes.is_empty(),
        "{} independent review requires at least one note",
        segment.id
    );
    ensure!(
        notes.iter().all(|note| !note.trim().is_empty()),
        "{} independent review contains an empty note",
        segment.id
    );
    ensure!(
        segment
            .entries
            .iter()
            .all(|entry| entry.status == TranslationStatus::NeedsReview),
        "{} independent review requires every entry to need review",
        segment.id
    );

    for entry in &mut segment.entries {
        entry.status = TranslationStatus::NeedsHumanReview;
    }
    let reviewed_draft_sha256 = reviewable_draft_sha256(segment)?;
    segment.review = Some(TranslationSegmentReview {
        reviewed_draft_sha256: reviewed_draft_sha256.clone(),
        outcome,
        notes,
    });
    Ok(RecordedTranslationReview {
        segment_id: segment.id.clone(),
        entry_count: segment.entries.len(),
        reviewed_draft_sha256,
        outcome,
    })
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(
        &fs::read(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))
}

fn replace_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("translation draft segment filename is not UTF-8")?;
    let temporary_path =
        path.with_file_name(format!(".{filename}.review-{}.tmp", std::process::id()));
    write_new_file(&temporary_path, &bytes)?;
    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error).with_context(|| format!("failed to replace {}", path.display()));
    }
    Ok(())
}

fn write_new_file(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("refusing to overwrite temporary file {}", path.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("failed to write {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to flush {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
#[path = "reviews_tests.rs"]
mod reviews_tests;
