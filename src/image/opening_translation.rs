use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::image::pipeline;
use crate::patch::{OpeningTranslationReport, compile_opening_translation};
use crate::source::verified::load_verified_source;
use crate::translation::load_reviewed_translation_corpus;

use super::BuildReport;

#[derive(Debug, Eq, PartialEq)]
pub struct OpeningTranslationBuildReport {
    pub image: BuildReport,
    pub opening: OpeningTranslationReport,
}

pub fn build_opening_translation_image(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
) -> Result<OpeningTranslationBuildReport> {
    pipeline::refuse_existing_output(output_path)?;
    let corpus = load_reviewed_translation_corpus(source_path, drafts_directory)?;
    let verified_source = load_verified_source(source_path)?;
    let maddat = verified_source
        .installer_payload
        .get("MADDAT")
        .context("verified installer payload is missing MADDAT")?;
    let opening_com = verified_source
        .installer_payload
        .get("OPENING.COM")
        .context("verified installer payload is missing OPENING.COM")?;
    let translated = compile_opening_translation(maddat, opening_com, &corpus)?;

    let mut installer_payload = verified_source.installer_payload.clone();
    installer_payload.insert("MADDAT".to_owned(), translated.maddat);
    installer_payload.insert("OPENING.COM".to_owned(), translated.opening_com);
    let image =
        pipeline::build_standalone_from_payload(&verified_source, &installer_payload, output_path)?;

    Ok(OpeningTranslationBuildReport {
        image,
        opening: translated.report,
    })
}

pub fn default_opening_translation_output_path() -> PathBuf {
    PathBuf::from("out/Disc Station Vol. 08 (Daimadou Korean Opening Development).hdm")
}
