use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use super::disk;
use super::graphic_text_catalog::GraphicTextCatalog;
use super::mad_indexed_catalog::{MadIndexedBindingContentKind, MadIndexedTextCatalog};
use super::survey::{
    DosProgramTextCatalog, EndingCreditsCatalog, EndingCurryResultCatalog,
    EndingIngredientLabelCatalog, EndingInterludeCaption, EndingMealCaptionCatalog,
    EndingMealReactionCatalog, EndingPasswordCatalog, MaddatCatalog, MstTextCatalog,
    OpeningTextCatalog, SelectDialogueCatalog,
};

const DECLARED_RUNTIME_FILE_COUNT: usize = 17;
const DECLARED_MADDAT_ENTRY_COUNT: usize = 154;

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationCatalog {
    pub summary: LocalizationSummary,
    pub files: Vec<LocalizationFileCoverage>,
    pub maddat_entries: Vec<LocalizationMaddatEntryCoverage>,
    pub source_text: Vec<LocalizationTextItem>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationSummary {
    pub declared_runtime_file_count: usize,
    pub audited_runtime_file_count: usize,
    pub declared_maddat_entry_count: usize,
    pub audited_maddat_entry_count: usize,
    pub identified_source_text_item_count: usize,
    pub target_text_item_count: usize,
    pub excluded_text_item_count: usize,
    pub unresolved_text_item_count: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationFileCoverage {
    pub filename: String,
    pub file_size: usize,
    pub sha256: String,
    pub status: LocalizationFileStatus,
    pub reason: LocalizationFileReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationFileStatus {
    TargetData,
    ConsumerSupport,
    Excluded,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationFileReason {
    EmbeddedDosText,
    EmbeddedIndexedText,
    MaddatAssetArchive,
    TextConsumerProgram,
    GlyphResource,
    AuxiliaryProgramOutput,
    PlatformSystemFile,
    RuntimeConfiguration,
    RuntimeData,
    LauncherScript,
    AudioData,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationMaddatEntryCoverage {
    pub entry_id: usize,
    pub file_offset: usize,
    pub stored_size: usize,
    pub stored_sha256: String,
    pub status: LocalizationMaddatStatus,
    pub reason: LocalizationMaddatReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationMaddatStatus {
    TargetData,
    ConsumerSupport,
    Excluded,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationMaddatReason {
    LocalizedText,
    IndexedTextFont,
    EndingCreditFont,
    NonTextMotionData,
    ReviewedUnitSpriteGraphics,
    ReviewedNoLocalizedText,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationTextItem {
    pub id: String,
    pub source_filename: String,
    pub source_catalog: LocalizationSourceCatalog,
    pub source_record_id: String,
    pub text: String,
    pub disposition: LocalizationTextDisposition,
    pub reason: LocalizationTextReason,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationSourceCatalog {
    SelectDialogue,
    OpeningText,
    GraphicText,
    EndingCredits,
    EndingCurryResult,
    EndingMealCaptions,
    EndingMealReaction,
    EndingInterludeCaption,
    EndingIngredientLabels,
    EndingPasswords,
    MstText,
    DosProgramText,
    MadIndexedText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationTextDisposition {
    Target,
    Excluded,
    Unresolved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationTextReason {
    ConsumerReachable,
    GraphicConsumerReachable,
    OriginalPresentationPreserved,
    OutsideGameplayLocalizationScope,
    DormantNoConsumerReference,
}

pub(super) struct LocalizationInputs<'a> {
    pub runtime_files: BTreeMap<String, &'a [u8]>,
    pub maddat: &'a MaddatCatalog,
    pub select_dialogue: &'a SelectDialogueCatalog,
    pub opening_text: &'a OpeningTextCatalog,
    pub graphic_text: &'a GraphicTextCatalog,
    pub ending_credits: &'a EndingCreditsCatalog,
    pub ending_curry_result: &'a EndingCurryResultCatalog,
    pub ending_meal_captions: &'a EndingMealCaptionCatalog,
    pub ending_meal_reaction: &'a EndingMealReactionCatalog,
    pub ending_interlude_caption: &'a EndingInterludeCaption,
    pub ending_ingredient_labels: &'a EndingIngredientLabelCatalog,
    pub ending_passwords: &'a EndingPasswordCatalog,
    pub mst_text: &'a MstTextCatalog,
    pub dos_program_text: &'a DosProgramTextCatalog,
    pub mad_indexed_text: &'a MadIndexedTextCatalog,
}

pub(super) fn catalog_localization(inputs: LocalizationInputs<'_>) -> Result<LocalizationCatalog> {
    let files = catalog_runtime_files(&inputs.runtime_files)?;
    let maddat_entries = catalog_maddat_entries(&inputs)?;
    let source_text = catalog_source_text(&inputs)?;

    let target_text_item_count = source_text
        .iter()
        .filter(|item| item.disposition == LocalizationTextDisposition::Target)
        .count();
    let excluded_text_item_count = source_text
        .iter()
        .filter(|item| item.disposition == LocalizationTextDisposition::Excluded)
        .count();
    let unresolved_text_item_count = source_text
        .iter()
        .filter(|item| item.disposition == LocalizationTextDisposition::Unresolved)
        .count();
    ensure!(
        unresolved_text_item_count == 0,
        "localization text audit still has unresolved items"
    );
    ensure!(
        files
            .iter()
            .all(|file| file.status != LocalizationFileStatus::Unresolved),
        "runtime-file localization audit is incomplete"
    );
    ensure!(
        maddat_entries
            .iter()
            .all(|entry| entry.status != LocalizationMaddatStatus::Unresolved),
        "MADDAT localization audit is incomplete"
    );

    Ok(LocalizationCatalog {
        summary: LocalizationSummary {
            declared_runtime_file_count: DECLARED_RUNTIME_FILE_COUNT,
            audited_runtime_file_count: files.len(),
            declared_maddat_entry_count: DECLARED_MADDAT_ENTRY_COUNT,
            audited_maddat_entry_count: maddat_entries.len(),
            identified_source_text_item_count: source_text.len(),
            target_text_item_count,
            excluded_text_item_count,
            unresolved_text_item_count,
        },
        files,
        maddat_entries,
        source_text,
    })
}

#[derive(Clone, Copy)]
struct FileCoverageSpec {
    filename: &'static str,
    status: LocalizationFileStatus,
    reason: LocalizationFileReason,
}

const FILE_COVERAGE: [FileCoverageSpec; DECLARED_RUNTIME_FILE_COUNT] = [
    FileCoverageSpec {
        filename: "BPLAY.COM",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::AuxiliaryProgramOutput,
    },
    FileCoverageSpec {
        filename: "BSAMP.COM",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::AuxiliaryProgramOutput,
    },
    FileCoverageSpec {
        filename: "CONFIG.SYS",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::RuntimeConfiguration,
    },
    FileCoverageSpec {
        filename: "DATAFILE",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::RuntimeData,
    },
    FileCoverageSpec {
        filename: "ENDING.COM",
        status: LocalizationFileStatus::TargetData,
        reason: LocalizationFileReason::EmbeddedIndexedText,
    },
    FileCoverageSpec {
        filename: "FPLAY.COM",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::AuxiliaryProgramOutput,
    },
    FileCoverageSpec {
        filename: "GAIJI.COM",
        status: LocalizationFileStatus::ConsumerSupport,
        reason: LocalizationFileReason::GlyphResource,
    },
    FileCoverageSpec {
        filename: "IO98.SYS",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::PlatformSystemFile,
    },
    FileCoverageSpec {
        filename: "MAD.COM",
        status: LocalizationFileStatus::TargetData,
        reason: LocalizationFileReason::EmbeddedIndexedText,
    },
    FileCoverageSpec {
        filename: "MADDAT",
        status: LocalizationFileStatus::TargetData,
        reason: LocalizationFileReason::MaddatAssetArchive,
    },
    FileCoverageSpec {
        filename: "MADO95.BAT",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::LauncherScript,
    },
    FileCoverageSpec {
        filename: "MEGDOS.SYS",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::PlatformSystemFile,
    },
    FileCoverageSpec {
        filename: "MST.COM",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::AuxiliaryProgramOutput,
    },
    FileCoverageSpec {
        filename: "NMOUSE.COM",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::AuxiliaryProgramOutput,
    },
    FileCoverageSpec {
        filename: "OPENING.COM",
        status: LocalizationFileStatus::ConsumerSupport,
        reason: LocalizationFileReason::TextConsumerProgram,
    },
    FileCoverageSpec {
        filename: "SELECT.COM",
        status: LocalizationFileStatus::TargetData,
        reason: LocalizationFileReason::EmbeddedIndexedText,
    },
    FileCoverageSpec {
        filename: "SONG.DAT",
        status: LocalizationFileStatus::Excluded,
        reason: LocalizationFileReason::AudioData,
    },
];

fn catalog_runtime_files(
    runtime_files: &BTreeMap<String, &[u8]>,
) -> Result<Vec<LocalizationFileCoverage>> {
    let declared: BTreeSet<_> = FILE_COVERAGE
        .iter()
        .map(|spec| spec.filename.to_owned())
        .collect();
    ensure!(
        runtime_files.keys().cloned().collect::<BTreeSet<_>>() == declared,
        "runtime-file localization population differs from the declared 17-file set: expected {declared:?}, got {:?}",
        runtime_files.keys().collect::<Vec<_>>()
    );
    Ok(FILE_COVERAGE
        .iter()
        .map(|spec| {
            let bytes = runtime_files
                .get(spec.filename)
                .expect("runtime file set was checked above");
            LocalizationFileCoverage {
                filename: spec.filename.to_owned(),
                file_size: bytes.len(),
                sha256: disk::sha256_hex(bytes),
                status: spec.status,
                reason: spec.reason,
            }
        })
        .collect())
}

fn catalog_maddat_entries(
    inputs: &LocalizationInputs<'_>,
) -> Result<Vec<LocalizationMaddatEntryCoverage>> {
    ensure!(
        inputs.maddat.entries.len() == DECLARED_MADDAT_ENTRY_COUNT,
        "MADDAT localization population has {} entries instead of the declared {DECLARED_MADDAT_ENTRY_COUNT}",
        inputs.maddat.entries.len()
    );
    ensure!(
        inputs
            .maddat
            .entries
            .iter()
            .enumerate()
            .all(|(index, entry)| entry.id == index + 1),
        "MADDAT localization population is not a complete 1-based sequence"
    );

    let mut target_entry_ids: BTreeSet<_> = inputs
        .graphic_text
        .surfaces
        .iter()
        .filter(|surface| surface.requires_localization)
        .map(|surface| surface.maddat_entry_id)
        .collect();
    if contains_japanese(&inputs.opening_text.text) {
        target_entry_ids.insert(inputs.opening_text.maddat_entry_id);
    }
    if inputs.ending_credits.contains_japanese || inputs.ending_credits.requires_localization {
        target_entry_ids.insert(inputs.ending_credits.text.maddat_entry_id);
    }

    Ok(inputs
        .maddat
        .entries
        .iter()
        .map(|entry| {
            let (status, reason) = if target_entry_ids.contains(&entry.id) {
                (
                    LocalizationMaddatStatus::TargetData,
                    LocalizationMaddatReason::LocalizedText,
                )
            } else {
                match entry.id {
                    6 => (
                        LocalizationMaddatStatus::ConsumerSupport,
                        LocalizationMaddatReason::IndexedTextFont,
                    ),
                    147 => (
                        LocalizationMaddatStatus::ConsumerSupport,
                        LocalizationMaddatReason::EndingCreditFont,
                    ),
                    149..=151 => (
                        LocalizationMaddatStatus::Excluded,
                        LocalizationMaddatReason::NonTextMotionData,
                    ),
                    7..=9 | 111..=119 => (
                        LocalizationMaddatStatus::Excluded,
                        LocalizationMaddatReason::ReviewedUnitSpriteGraphics,
                    ),
                    _ => (
                        LocalizationMaddatStatus::Excluded,
                        LocalizationMaddatReason::ReviewedNoLocalizedText,
                    ),
                }
            };
            LocalizationMaddatEntryCoverage {
                entry_id: entry.id,
                file_offset: entry.offset,
                stored_size: entry.size,
                stored_sha256: entry.sha256.clone(),
                status,
                reason,
            }
        })
        .collect())
}

fn catalog_source_text(inputs: &LocalizationInputs<'_>) -> Result<Vec<LocalizationTextItem>> {
    let mut items = Vec::new();

    for script in inputs
        .select_dialogue
        .stages
        .iter()
        .flat_map(|stage| &stage.scripts)
    {
        add_japanese_item(
            &mut items,
            format!("select-dialogue:{}", script.id),
            "SELECT.COM",
            LocalizationSourceCatalog::SelectDialogue,
            script.id.clone(),
            &script.text,
            LocalizationTextDisposition::Target,
            LocalizationTextReason::ConsumerReachable,
        );
    }

    add_japanese_item(
        &mut items,
        "opening-text".to_owned(),
        "MADDAT",
        LocalizationSourceCatalog::OpeningText,
        format!("maddat-entry-{}", inputs.opening_text.maddat_entry_id),
        &inputs.opening_text.text,
        LocalizationTextDisposition::Target,
        LocalizationTextReason::ConsumerReachable,
    );

    for surface in &inputs.graphic_text.surfaces {
        let (disposition, reason) = if surface.requires_localization {
            (
                LocalizationTextDisposition::Target,
                LocalizationTextReason::GraphicConsumerReachable,
            )
        } else {
            (
                LocalizationTextDisposition::Excluded,
                LocalizationTextReason::OriginalPresentationPreserved,
            )
        };
        for (index, text) in surface.reviewed_text.iter().enumerate() {
            add_source_item(
                &mut items,
                format!("graphic-text:{}:{:02}", surface.id, index + 1),
                "MADDAT",
                LocalizationSourceCatalog::GraphicText,
                format!("{}:{:02}", surface.id, index + 1),
                text,
                disposition,
                reason,
            );
        }
    }

    for screen in &inputs.ending_credits.text.screens {
        for line in &screen.lines {
            let id = format!(
                "ending-credit:screen-{:02}-line-{:02}",
                screen.screen_number, line.line_number
            );
            let record_id = format!(
                "screen-{:02}-line-{:02}",
                screen.screen_number, line.line_number
            );
            if line.requires_localization {
                add_source_item(
                    &mut items,
                    id,
                    "MADDAT",
                    LocalizationSourceCatalog::EndingCredits,
                    record_id,
                    line.reviewed_role_text
                        .as_deref()
                        .expect("a localizable credit role has reviewed source text"),
                    LocalizationTextDisposition::Target,
                    LocalizationTextReason::ConsumerReachable,
                );
            } else {
                add_japanese_item(
                    &mut items,
                    id,
                    "MADDAT",
                    LocalizationSourceCatalog::EndingCredits,
                    record_id,
                    &line.text,
                    LocalizationTextDisposition::Target,
                    LocalizationTextReason::ConsumerReachable,
                );
            }
        }
    }

    for template in [
        &inputs.ending_curry_result.month,
        &inputs.ending_curry_result.day,
    ] {
        add_japanese_item(
            &mut items,
            format!("ending-curry-result:{}", template.id),
            "ENDING.COM",
            LocalizationSourceCatalog::EndingCurryResult,
            template.id.clone(),
            &template.suffix_text,
            LocalizationTextDisposition::Target,
            LocalizationTextReason::ConsumerReachable,
        );
    }
    for message in &inputs.ending_curry_result.messages {
        add_japanese_item(
            &mut items,
            format!("ending-curry-result:{}", message.id),
            "ENDING.COM",
            LocalizationSourceCatalog::EndingCurryResult,
            message.id.clone(),
            &message.lines.join("\n"),
            LocalizationTextDisposition::Target,
            LocalizationTextReason::ConsumerReachable,
        );
    }

    for record in &inputs.ending_meal_captions.records {
        add_japanese_item(
            &mut items,
            format!("ending-meal-caption:{}", record.id),
            "ENDING.COM",
            LocalizationSourceCatalog::EndingMealCaptions,
            record.id.clone(),
            &record
                .spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            LocalizationTextDisposition::Target,
            LocalizationTextReason::ConsumerReachable,
        );
    }

    add_japanese_item(
        &mut items,
        "ending-meal-reaction:bite-sound-effect".to_owned(),
        "ENDING.COM",
        LocalizationSourceCatalog::EndingMealReaction,
        inputs.ending_meal_reaction.id.clone(),
        &inputs.ending_meal_reaction.text,
        LocalizationTextDisposition::Target,
        LocalizationTextReason::ConsumerReachable,
    );

    add_japanese_item(
        &mut items,
        "ending-interlude-caption".to_owned(),
        "ENDING.COM",
        LocalizationSourceCatalog::EndingInterludeCaption,
        "interlude-caption".to_owned(),
        &inputs.ending_interlude_caption.text,
        LocalizationTextDisposition::Target,
        LocalizationTextReason::ConsumerReachable,
    );

    for record in &inputs.ending_ingredient_labels.records {
        add_japanese_item(
            &mut items,
            format!("ending-ingredient-label:{}", record.id),
            "ENDING.COM",
            LocalizationSourceCatalog::EndingIngredientLabels,
            record.id.clone(),
            &record.text,
            LocalizationTextDisposition::Target,
            LocalizationTextReason::ConsumerReachable,
        );
    }

    for record in &inputs.ending_passwords.records {
        add_japanese_item(
            &mut items,
            format!("ending-password:{}", record.id),
            "ENDING.COM",
            LocalizationSourceCatalog::EndingPasswords,
            record.id.clone(),
            &record.text,
            LocalizationTextDisposition::Target,
            LocalizationTextReason::ConsumerReachable,
        );
    }

    for record in &inputs.mst_text.records {
        add_japanese_item(
            &mut items,
            format!("mst-text:{}", record.id),
            "MST.COM",
            LocalizationSourceCatalog::MstText,
            record.id.clone(),
            &record.text_segments.join(""),
            LocalizationTextDisposition::Excluded,
            LocalizationTextReason::OutsideGameplayLocalizationScope,
        );
    }

    for program in &inputs.dos_program_text.programs {
        for record in &program.records {
            let (disposition, reason) = if !record.bound_to_consumer {
                (
                    LocalizationTextDisposition::Excluded,
                    LocalizationTextReason::DormantNoConsumerReference,
                )
            } else if program.filename != "MAD.COM" {
                (
                    LocalizationTextDisposition::Excluded,
                    LocalizationTextReason::OutsideGameplayLocalizationScope,
                )
            } else {
                (
                    LocalizationTextDisposition::Target,
                    LocalizationTextReason::ConsumerReachable,
                )
            };
            add_japanese_item(
                &mut items,
                format!("dos-program-text:{}:{}", program.filename, record.id),
                &program.filename,
                LocalizationSourceCatalog::DosProgramText,
                record.id.clone(),
                &record.text_segments.join(""),
                disposition,
                reason,
            );
        }
    }

    for string in &inputs.mad_indexed_text.strings {
        let Some(text) = string.text.as_deref() else {
            continue;
        };
        let (disposition, reason) = if string.bound_to_consumer {
            (
                LocalizationTextDisposition::Target,
                LocalizationTextReason::ConsumerReachable,
            )
        } else {
            (
                LocalizationTextDisposition::Excluded,
                LocalizationTextReason::DormantNoConsumerReference,
            )
        };
        add_japanese_item(
            &mut items,
            format!("mad-indexed-text:{}", string.id),
            "MAD.COM",
            LocalizationSourceCatalog::MadIndexedText,
            string.id.clone(),
            text,
            disposition,
            reason,
        );
    }
    for binding in &inputs.mad_indexed_text.bindings {
        if binding.content_kind != MadIndexedBindingContentKind::CompositeGlyphBanner {
            continue;
        }
        if let Some(text) = binding.display_text.as_deref() {
            add_japanese_item(
                &mut items,
                format!("mad-indexed-binding:{}", binding.id),
                "MAD.COM",
                LocalizationSourceCatalog::MadIndexedText,
                binding.id.clone(),
                text,
                LocalizationTextDisposition::Target,
                LocalizationTextReason::GraphicConsumerReachable,
            );
        }
    }

    let ids: BTreeSet<_> = items.iter().map(|item| item.id.as_str()).collect();
    ensure!(
        ids.len() == items.len(),
        "localization text audit produced duplicate stable IDs"
    );
    let cataloged_dos_japanese = items
        .iter()
        .filter(|item| item.source_catalog == LocalizationSourceCatalog::DosProgramText)
        .count();
    ensure!(
        cataloged_dos_japanese == inputs.dos_program_text.japanese_record_count,
        "DOS program Japanese population does not match its source catalog"
    );
    let cataloged_unbound_dos = items
        .iter()
        .filter(|item| {
            item.source_catalog == LocalizationSourceCatalog::DosProgramText
                && item.reason == LocalizationTextReason::DormantNoConsumerReference
        })
        .count();
    ensure!(
        cataloged_unbound_dos == inputs.dos_program_text.unbound_japanese_record_ids.len(),
        "DOS program dormant-Japanese population does not match its consumer audit"
    );
    let cataloged_mad_strings = items
        .iter()
        .filter(|item| item.id.starts_with("mad-indexed-text:"))
        .count();
    let source_mad_japanese = inputs
        .mad_indexed_text
        .strings
        .iter()
        .filter(|string| string.contains_japanese)
        .count();
    ensure!(
        cataloged_mad_strings == source_mad_japanese,
        "MAD indexed Japanese population does not match its source catalog"
    );

    Ok(items)
}

#[allow(clippy::too_many_arguments)]
fn add_japanese_item(
    items: &mut Vec<LocalizationTextItem>,
    id: String,
    source_filename: &str,
    source_catalog: LocalizationSourceCatalog,
    source_record_id: String,
    text: &str,
    disposition: LocalizationTextDisposition,
    reason: LocalizationTextReason,
) {
    if contains_japanese(text) {
        add_source_item(
            items,
            id,
            source_filename,
            source_catalog,
            source_record_id,
            text,
            disposition,
            reason,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn add_source_item(
    items: &mut Vec<LocalizationTextItem>,
    id: String,
    source_filename: &str,
    source_catalog: LocalizationSourceCatalog,
    source_record_id: String,
    text: &str,
    disposition: LocalizationTextDisposition,
    reason: LocalizationTextReason,
) {
    items.push(LocalizationTextItem {
        id,
        source_filename: source_filename.to_owned(),
        source_catalog,
        source_record_id,
        text: text.to_owned(),
        disposition,
        reason,
    });
}

fn contains_japanese(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character,
            '\u{3040}'..='\u{30ff}'
                | '\u{31f0}'..='\u{31ff}'
                | '\u{3400}'..='\u{4dbf}'
                | '\u{4e00}'..='\u{9fff}'
                | '\u{f900}'..='\u{faff}'
                | '\u{ff66}'..='\u{ff9f}'
                | '\u{3005}'
                | '\u{3006}'
        )
    })
}

#[cfg(test)]
#[path = "localization_catalog_tests.rs"]
mod localization_catalog_tests;
