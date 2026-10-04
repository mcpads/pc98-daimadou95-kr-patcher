mod bank_plan;
mod font_installer;
mod text;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};

use crate::source::{
    DosProgramTextReferenceKind, LocalizationSourceCatalog, SourceStructureReport,
};
use crate::translation::{ReviewedTranslationCorpus, TranslationContentPart, TranslationStatus};

use self::bank_plan::compile_mad_media_error_font_plan;
use self::text::{DosSourceRecord, DosSourceReference, DosSourceReferenceKind};

const MAD_MEDIA_ERROR_COUNT: usize = 3;
const PERSISTENT_GAIJI_CONSUMERS: [&str; 4] =
    ["MAD.COM", "SELECT.COM", "ENDING.COM", "OPENING.COM"];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MadMediaErrorFiles {
    pub installer_payload: BTreeMap<String, Vec<u8>>,
    pub report: MadMediaErrorReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MadMediaErrorReport {
    pub translated_entry_count: usize,
    pub glyph_count: usize,
    pub referenced_source_gaiji_count: usize,
    pub available_external_slot_count: usize,
    pub blocked_external_slot_count: usize,
    pub preserved_original_gaiji_glyph_count: usize,
    pub pointer_reference_count: usize,
    pub translated_text_byte_count: usize,
    pub input_size: usize,
    pub text_output_size: usize,
    pub output_size: usize,
    pub installer_hook_file_offset: usize,
    pub installer_hook_byte_size: usize,
}

pub(super) struct DosTranslation {
    pub entry_id: String,
    pub filename: String,
    pub record_id: String,
    pub parts: Vec<DosOutputPart>,
}

pub(super) enum DosOutputPart {
    Text(String),
    Bytes(Vec<u8>),
    Terminator,
}

pub(crate) fn compile_mad_media_errors(
    installer_payload: &BTreeMap<String, Vec<u8>>,
    corpus: &ReviewedTranslationCorpus,
) -> Result<MadMediaErrorFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "MAD media-error build requires a human-review-ready translation corpus"
    );
    let translations = load_mad_media_error_translations(corpus)?;
    ensure!(
        translations.len() == MAD_MEDIA_ERROR_COUNT,
        "MAD media-error corpus has {} entries instead of {MAD_MEDIA_ERROR_COUNT}",
        translations.len()
    );

    let mad_com = installer_payload
        .get("MAD.COM")
        .context("MAD media-error input is missing MAD.COM")?;
    let persistent_consumer_programs = PERSISTENT_GAIJI_CONSUMERS
        .iter()
        .map(|filename| {
            installer_payload
                .get(*filename)
                .map(Vec::as_slice)
                .with_context(|| format!("MAD media-error input is missing {filename}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let font_plan = compile_mad_media_error_font_plan(
        &corpus.source.gaiji,
        &corpus.source.mad_text,
        mad_com,
        &persistent_consumer_programs,
        &translations,
    )?;
    let records = mad_source_records(&corpus.source)?;
    let selected = translations.iter().collect::<Vec<_>>();
    let patched_text = text::reinsert_dos_text(
        mad_com,
        mad_source_file_size(&corpus.source)?,
        &selected,
        &records,
        &font_plan.codebook,
    )?;
    let text_output_size = patched_text.output_size;
    let patched_font =
        font_installer::install_mad_error_font_at_entry(&patched_text.bytes, &font_plan.codebook)?;

    let source_gaiji = installer_payload
        .get("GAIJI.COM")
        .context("MAD media-error input is missing GAIJI.COM")?;
    let mut output_payload = installer_payload.clone();
    output_payload.insert("MAD.COM".to_owned(), patched_font.bytes);
    ensure!(
        output_payload.get("GAIJI.COM") == Some(source_gaiji),
        "MAD media-error build must preserve GAIJI.COM for indexed pixel art"
    );
    let output_size = output_payload
        .get("MAD.COM")
        .expect("MAD.COM was inserted above")
        .len();

    Ok(MadMediaErrorFiles {
        installer_payload: output_payload,
        report: MadMediaErrorReport {
            translated_entry_count: translations.len(),
            glyph_count: font_plan.codebook.len(),
            referenced_source_gaiji_count: font_plan.referenced_source_gaiji_count,
            available_external_slot_count: font_plan.available_external_slot_count,
            blocked_external_slot_count: font_plan.blocked_external_slot_count,
            preserved_original_gaiji_glyph_count: corpus.source.gaiji.glyphs.len(),
            pointer_reference_count: patched_text.pointer_reference_count,
            translated_text_byte_count: patched_text.translated_text_byte_count,
            input_size: mad_com.len(),
            text_output_size,
            output_size,
            installer_hook_file_offset: patched_font.hook_file_offset,
            installer_hook_byte_size: patched_font.hook_byte_size,
        },
    })
}

fn load_mad_media_error_translations(
    corpus: &ReviewedTranslationCorpus,
) -> Result<Vec<DosTranslation>> {
    let mut translations = Vec::new();
    let mut entry_ids = BTreeSet::new();
    for segment in corpus.segments.iter().filter(|segment| {
        segment.protected.source_catalog == LocalizationSourceCatalog::DosProgramText
    }) {
        for (protected, draft) in segment.protected.entries.iter().zip(&segment.draft.entries) {
            ensure!(
                protected.id == draft.id,
                "MAD media-error draft order changed"
            );
            ensure!(
                protected.source_filename == "MAD.COM",
                "{} is outside the MAD media-error product scope",
                protected.id
            );
            ensure!(
                matches!(
                    draft.status,
                    TranslationStatus::NeedsHumanReview | TranslationStatus::Complete
                ),
                "MAD media-error translation {} is not ready for development reinsertion",
                draft.id
            );
            ensure!(
                entry_ids.insert(protected.id.clone()),
                "MAD media-error corpus duplicates {}",
                protected.id
            );
            let mut text_index = 0_usize;
            let mut parts = Vec::new();
            for part in &protected.content {
                match part {
                    TranslationContentPart::Text { .. } => {
                        let text = draft
                            .korean_text
                            .get(text_index)
                            .with_context(|| format!("{} has too few Korean spans", draft.id))?
                            .clone()
                            .with_context(|| {
                                format!("{} Korean span {text_index} is empty", draft.id)
                            })?;
                        parts.push(DosOutputPart::Text(text));
                        text_index += 1;
                    }
                    TranslationContentPart::AnsiControl { raw_hex } => {
                        parts.push(DosOutputPart::Bytes(decode_hex(raw_hex)?));
                    }
                    TranslationContentPart::CarriageReturn => {
                        parts.push(DosOutputPart::Bytes(vec![b'\r']));
                    }
                    TranslationContentPart::LineFeed => {
                        parts.push(DosOutputPart::Bytes(vec![b'\n']));
                    }
                    TranslationContentPart::Terminator => parts.push(DosOutputPart::Terminator),
                    _ => bail!("{} contains a non-DOS protected token", protected.id),
                }
            }
            ensure!(
                text_index == draft.korean_text.len(),
                "{} has {} unused Korean spans",
                draft.id,
                draft.korean_text.len() - text_index
            );
            ensure!(
                matches!(parts.last(), Some(DosOutputPart::Terminator)),
                "{} does not end in the protected DOS terminator",
                protected.id
            );
            translations.push(DosTranslation {
                entry_id: protected.id.clone(),
                filename: protected.source_filename.clone(),
                record_id: protected.source_record_id.clone(),
                parts,
            });
        }
    }
    Ok(translations)
}

fn mad_source_records(source: &SourceStructureReport) -> Result<Vec<DosSourceRecord>> {
    let program = source
        .dos_program_text
        .programs
        .iter()
        .find(|program| program.filename == "MAD.COM")
        .context("source catalog has no MAD.COM DOS text")?;
    program
        .records
        .iter()
        .map(|record| {
            Ok(DosSourceRecord {
                id: record.id.clone(),
                runtime_address: record.runtime_address,
                file_offset: record.file_offset,
                byte_size: record.byte_size,
                references: record
                    .references
                    .iter()
                    .map(|reference| DosSourceReference {
                        file_offset: reference.file_offset,
                        kind: match reference.kind {
                            DosProgramTextReferenceKind::Immediate => {
                                DosSourceReferenceKind::Immediate
                            }
                            DosProgramTextReferenceKind::PointerTable => {
                                DosSourceReferenceKind::PointerTable
                            }
                        },
                    })
                    .collect(),
            })
        })
        .collect()
}

fn mad_source_file_size(source: &SourceStructureReport) -> Result<usize> {
    source
        .dos_program_text
        .programs
        .iter()
        .find(|program| program.filename == "MAD.COM")
        .map(|program| program.file_size)
        .context("source catalog has no file size for MAD.COM")
}

fn decode_hex(raw: &str) -> Result<Vec<u8>> {
    ensure!(
        raw.len().is_multiple_of(2),
        "MAD media-error control hex has odd length"
    );
    (0..raw.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&raw[index..index + 2], 16).with_context(|| {
                format!("invalid MAD media-error control hex at byte {}", index / 2)
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "mad_media_errors_tests.rs"]
mod mad_media_errors_tests;
