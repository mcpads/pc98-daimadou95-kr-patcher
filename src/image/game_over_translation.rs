use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::image::pipeline;
use crate::patch::{GameOverTranslationReport, compile_game_over_translation};
use crate::source::verified::load_verified_source;
use crate::translation::load_reviewed_translation_corpus;

use super::BuildReport;

#[derive(Debug, Eq, PartialEq)]
pub struct GameOverTranslationBuildReport {
    pub image: BuildReport,
    pub game_over: GameOverTranslationReport,
}

pub fn build_game_over_translation_image(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
) -> Result<GameOverTranslationBuildReport> {
    pipeline::refuse_existing_output(output_path)?;
    let corpus = load_reviewed_translation_corpus(source_path, drafts_directory)?;
    let verified_source = load_verified_source(source_path)?;
    let maddat = verified_source
        .installer_payload
        .get("MADDAT")
        .context("verified installer payload is missing MADDAT")?;
    let translated = compile_game_over_translation(maddat, &corpus)?;

    let mut installer_payload = verified_source.installer_payload.clone();
    installer_payload.insert("MADDAT".to_owned(), translated.maddat);
    let image =
        pipeline::build_standalone_from_payload(&verified_source, &installer_payload, output_path)?;

    Ok(GameOverTranslationBuildReport {
        image,
        game_over: translated.report,
    })
}

pub fn default_game_over_translation_output_path() -> PathBuf {
    PathBuf::from("out/Disc Station Vol. 08 (Daimadou Korean Game Over Development).hdm")
}
