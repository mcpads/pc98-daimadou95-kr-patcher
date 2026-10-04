use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::source::{
    LocalizationTextDisposition, LocalizationTextItem, SourceStructureReport, disk,
    survey_source_path,
};

use super::projection::translation_entry;
use super::segments::{ensure_safe_segment_id, segment_id_for};
use super::{
    TranslationContentPart, TranslationEntry, TranslationPreparationReport, TranslationSegment,
    TranslationSegmentIndex, TranslationStatus, TranslationValidationReport,
    TranslationWorkspaceIndex,
};

const INDEX_FILENAME: &str = "index.json";
const SEGMENTS_DIRECTORY: &str = "segments";

pub fn prepare_translation_workspace(
    source_path: &Path,
    output_directory: &Path,
) -> Result<TranslationPreparationReport> {
    let source = survey_source_path(source_path)?;
    let workspace = build_translation_workspace(&source)?;
    write_translation_workspace(output_directory, &workspace)?;
    Ok(TranslationPreparationReport {
        source_sha256: workspace.index.source_sha256.clone(),
        output_directory: output_directory.to_owned(),
        target_item_count: workspace.index.target_item_count,
        segment_count: workspace.index.segment_count,
    })
}

pub fn validate_translation_workspace(
    source_path: &Path,
    workspace_directory: &Path,
) -> Result<TranslationValidationReport> {
    let source = survey_source_path(source_path)?;
    let expected = build_translation_workspace(&source)?;
    let index_path = workspace_directory.join(INDEX_FILENAME);
    let actual_index: TranslationWorkspaceIndex = read_json(&index_path)?;
    ensure!(
        actual_index == expected.index,
        "translation workspace index differs from the current protected source baseline"
    );

    let segments_directory = workspace_directory.join(SEGMENTS_DIRECTORY);
    let expected_paths = actual_index
        .segments
        .iter()
        .map(|segment| segment.path.clone())
        .collect::<BTreeSet<_>>();
    let actual_paths = list_segment_paths(workspace_directory, &segments_directory)?;
    ensure!(
        actual_paths == expected_paths,
        "translation workspace segment files differ from the index: expected {expected_paths:?}, got {actual_paths:?}"
    );

    let expected_by_id = expected
        .segments
        .iter()
        .map(|segment| (segment.id.as_str(), segment))
        .collect::<BTreeMap<_, _>>();
    let mut target_ids = BTreeSet::new();
    let mut counts = TranslationStatusCounts::default();
    for indexed in &actual_index.segments {
        let actual_path = workspace_directory.join(&indexed.path);
        let actual: TranslationSegment = read_json(&actual_path)?;
        let expected_segment = expected_by_id
            .get(indexed.id.as_str())
            .with_context(|| format!("index names unknown segment {}", indexed.id))?;
        ensure!(
            protected_segment_bytes(&actual)? == protected_segment_bytes(expected_segment)?,
            "{} protected translation fields differ from the current source",
            indexed.id
        );
        ensure!(
            disk::sha256_hex(&protected_segment_bytes(&actual)?) == indexed.protected_sha256,
            "{} protected hash differs from the index",
            indexed.id
        );
        ensure!(
            actual.entries.len() == indexed.entry_count,
            "{} entry count differs from the index",
            indexed.id
        );
        for entry in &actual.entries {
            ensure!(
                target_ids.insert(entry.id.clone()),
                "translation workspace contains duplicate target ID {}",
                entry.id
            );
            validate_editable_fields(entry)?;
            counts.add(entry.status)?;
        }
    }

    let expected_target_ids = source
        .localization
        .source_text
        .iter()
        .filter(|item| item.disposition == LocalizationTextDisposition::Target)
        .map(|item| item.id.clone())
        .collect::<BTreeSet<_>>();
    ensure!(
        target_ids == expected_target_ids,
        "translation workspace target IDs differ from the current source population"
    );

    Ok(TranslationValidationReport {
        source_sha256: actual_index.source_sha256,
        target_item_count: target_ids.len(),
        segment_count: actual_index.segment_count,
        untranslated_item_count: counts.untranslated,
        in_progress_item_count: counts.in_progress,
        needs_review_item_count: counts.needs_review,
        needs_human_review_item_count: counts.needs_human_review,
    })
}

pub(super) struct TranslationWorkspace {
    pub(super) index: TranslationWorkspaceIndex,
    pub(super) segments: Vec<TranslationSegment>,
}

pub(super) fn build_translation_workspace(
    source: &SourceStructureReport,
) -> Result<TranslationWorkspace> {
    let targets = source
        .localization
        .source_text
        .iter()
        .filter(|item| item.disposition == LocalizationTextDisposition::Target)
        .collect::<Vec<_>>();
    ensure!(
        targets.len() == source.localization_strategy.summary.assigned_item_count,
        "translation workspace population differs from the localization strategy"
    );

    let mut grouped: BTreeMap<String, Vec<&LocalizationTextItem>> = BTreeMap::new();
    for item in targets {
        grouped
            .entry(segment_id_for(item, source)?)
            .or_default()
            .push(item);
    }

    let mut segments = Vec::with_capacity(grouped.len());
    let mut assigned_ids = BTreeSet::new();
    for (segment_id, mut items) in grouped {
        ensure_safe_segment_id(&segment_id)?;
        items.sort_by(|left, right| left.id.cmp(&right.id));
        let source_catalog = items
            .first()
            .context("translation segment has no entries")?
            .source_catalog;
        ensure!(
            items
                .iter()
                .all(|item| item.source_catalog == source_catalog),
            "translation segment {segment_id} mixes source catalogs"
        );
        let family = source
            .localization_strategy
            .families
            .iter()
            .find(|family| family.source_catalog == source_catalog)
            .with_context(|| format!("{segment_id} has no localization family plan"))?;
        let mut entries = Vec::with_capacity(items.len());
        for item in items {
            ensure!(
                assigned_ids.insert(item.id.as_str()),
                "translation target {} was assigned more than once",
                item.id
            );
            let entry = translation_entry(item, source)?;
            validate_editable_fields(&entry)?;
            entries.push(entry);
        }
        segments.push(TranslationSegment {
            id: segment_id,
            source_sha256: source.source_sha256.clone(),
            source_catalog,
            context: family.context,
            render_path: family.render_path,
            glyph_supply: family.glyph_supply,
            source_layout: family.source_layout,
            glyph_gate: family.glyph_gate,
            reinsertion_gate: family.reinsertion_gate,
            entries,
        });
    }
    ensure!(
        assigned_ids.len() == source.localization_strategy.summary.assigned_item_count,
        "translation workspace did not assign the complete strategy population"
    );

    let mut index_segments = Vec::with_capacity(segments.len());
    for segment in &segments {
        index_segments.push(TranslationSegmentIndex {
            id: segment.id.clone(),
            path: format!("{SEGMENTS_DIRECTORY}/{}.json", segment.id),
            entry_count: segment.entries.len(),
            protected_sha256: disk::sha256_hex(&protected_segment_bytes(segment)?),
        });
    }
    Ok(TranslationWorkspace {
        index: TranslationWorkspaceIndex {
            source_sha256: source.source_sha256.clone(),
            target_item_count: assigned_ids.len(),
            segment_count: segments.len(),
            segments: index_segments,
        },
        segments,
    })
}

pub(super) fn validate_editable_fields(entry: &TranslationEntry) -> Result<()> {
    if let Some(notes) = &entry.notes {
        ensure!(!notes.trim().is_empty(), "{} has empty notes", entry.id);
    }
    let text_spans = entry
        .content
        .iter()
        .filter_map(|part| match part {
            TranslationContentPart::Text { korean_text, .. } => Some(korean_text),
            _ => None,
        })
        .collect::<Vec<_>>();
    ensure!(!text_spans.is_empty(), "{} has no text spans", entry.id);
    for korean_text in text_spans.iter().filter_map(|text| text.as_deref()) {
        ensure!(
            !korean_text.trim().is_empty(),
            "{} has an empty Korean text span",
            entry.id
        );
    }
    let translated_span_count = text_spans.iter().filter(|text| text.is_some()).count();
    match entry.status {
        TranslationStatus::Untranslated => ensure!(
            translated_span_count == 0,
            "{} is untranslated but contains Korean text",
            entry.id
        ),
        TranslationStatus::InProgress => ensure!(
            translated_span_count > 0,
            "{} is in progress but has no Korean text",
            entry.id
        ),
        TranslationStatus::NeedsReview | TranslationStatus::NeedsHumanReview => ensure!(
            translated_span_count == text_spans.len(),
            "{} is ready for review but has untranslated text spans",
            entry.id
        ),
        TranslationStatus::Complete => bail!(
            "{} claims complete before a human-approval input exists",
            entry.id
        ),
    }
    Ok(())
}

pub(super) fn protected_segment_bytes(segment: &TranslationSegment) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(segment)?;
    let entries = value
        .get_mut("entries")
        .and_then(serde_json::Value::as_array_mut)
        .context("serialized translation segment has no entries")?;
    for entry in entries {
        let entry = entry
            .as_object_mut()
            .context("serialized translation entry is not an object")?;
        entry.remove("status");
        entry.remove("notes");
        let content = entry
            .get_mut("content")
            .and_then(serde_json::Value::as_array_mut)
            .context("serialized translation entry has no content")?;
        for part in content {
            if part.get("kind").and_then(serde_json::Value::as_str) == Some("text") {
                part.as_object_mut()
                    .expect("a tagged content part serializes as an object")
                    .remove("korean_text");
            }
        }
    }
    Ok(serde_json::to_vec(&value)?)
}

fn write_translation_workspace(
    output_directory: &Path,
    workspace: &TranslationWorkspace,
) -> Result<()> {
    if output_directory.exists() {
        bail!(
            "refusing to overwrite existing translation workspace: {}",
            output_directory.display()
        );
    }
    let parent = output_directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create translation workspace parent {}",
            parent.display()
        )
    })?;
    fs::create_dir(output_directory).with_context(|| {
        format!(
            "failed to create translation workspace {}",
            output_directory.display()
        )
    })?;
    let segments_directory = output_directory.join(SEGMENTS_DIRECTORY);
    fs::create_dir(&segments_directory).with_context(|| {
        format!(
            "failed to create translation segments directory {}",
            segments_directory.display()
        )
    })?;

    for (indexed, segment) in workspace.index.segments.iter().zip(&workspace.segments) {
        ensure!(
            indexed.id == segment.id,
            "translation segment order drifted"
        );
        write_new_json(&output_directory.join(&indexed.path), segment)?;
    }
    write_new_json(&output_directory.join(INDEX_FILENAME), &workspace.index)?;
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

fn list_segment_paths(
    workspace_directory: &Path,
    segments_directory: &Path,
) -> Result<BTreeSet<String>> {
    let mut paths = BTreeSet::new();
    for entry in fs::read_dir(segments_directory).with_context(|| {
        format!(
            "failed to read translation segments directory {}",
            segments_directory.display()
        )
    })? {
        let entry = entry.context("failed to read translation segment entry")?;
        ensure!(
            entry
                .file_type()
                .context("failed to read translation segment file type")?
                .is_file(),
            "translation segments directory contains a non-file entry"
        );
        let relative = entry
            .path()
            .strip_prefix(workspace_directory)
            .context("translation segment lies outside its workspace")?
            .to_str()
            .context("translation segment path is not UTF-8")?
            .to_owned();
        paths.insert(relative);
    }
    Ok(paths)
}

#[derive(Default)]
struct TranslationStatusCounts {
    untranslated: usize,
    in_progress: usize,
    needs_review: usize,
    needs_human_review: usize,
}

impl TranslationStatusCounts {
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
