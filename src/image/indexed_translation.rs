use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::image::pipeline;
use crate::patch::{
    EndingTranslationReport, MadTranslationReport, OpeningTranslationReport,
    SelectTranslationReport, compile_ending_translation, compile_mad_translation,
    compile_opening_translation, compile_select_translation,
};
use crate::source::verified::load_verified_source;
use crate::translation::load_reviewed_translation_corpus;

use super::BuildReport;

#[derive(Debug, Eq, PartialEq)]
pub struct IndexedTranslationBuildReport {
    pub image: BuildReport,
    pub opening: OpeningTranslationReport,
    pub ending: EndingTranslationReport,
    pub select: SelectTranslationReport,
    pub mad: MadTranslationReport,
}

pub fn build_indexed_translation_image(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
) -> Result<IndexedTranslationBuildReport> {
    pipeline::refuse_existing_output(output_path)?;
    let corpus = load_reviewed_translation_corpus(source_path, drafts_directory)?;
    let verified_source = load_verified_source(source_path)?;
    let source_maddat = verified_source
        .installer_payload
        .get("MADDAT")
        .context("verified installer payload is missing MADDAT")?;
    let source_opening_com = verified_source
        .installer_payload
        .get("OPENING.COM")
        .context("verified installer payload is missing OPENING.COM")?;
    let source_ending_com = verified_source
        .installer_payload
        .get("ENDING.COM")
        .context("verified installer payload is missing ENDING.COM")?;
    let source_select_com = verified_source
        .installer_payload
        .get("SELECT.COM")
        .context("verified installer payload is missing SELECT.COM")?;
    let source_mad_com = verified_source
        .installer_payload
        .get("MAD.COM")
        .context("verified installer payload is missing MAD.COM")?;

    let opening = compile_opening_translation(source_maddat, source_opening_com, &corpus)?;
    let ending = compile_ending_translation(&opening.maddat, source_ending_com, &corpus)?;
    let select = compile_select_translation(&ending.maddat, source_select_com, &corpus)?;
    let mad = compile_mad_translation(&select.maddat, source_mad_com, &corpus)?;

    let mut installer_payload = verified_source.installer_payload.clone();
    installer_payload.insert("MADDAT".to_owned(), mad.maddat);
    installer_payload.insert("MAD.COM".to_owned(), mad.mad_com);
    installer_payload.insert("OPENING.COM".to_owned(), opening.opening_com);
    installer_payload.insert("ENDING.COM".to_owned(), ending.ending_com);
    installer_payload.insert("SELECT.COM".to_owned(), select.select_com);
    let image =
        pipeline::build_standalone_from_payload(&verified_source, &installer_payload, output_path)?;

    Ok(IndexedTranslationBuildReport {
        image,
        opening: opening.report,
        ending: ending.report,
        select: select.report,
        mad: mad.report,
    })
}

pub fn default_indexed_translation_output_path() -> PathBuf {
    PathBuf::from("out/Disc Station Vol. 08 (Daimadou Korean Indexed MAD Development).hdm")
}
