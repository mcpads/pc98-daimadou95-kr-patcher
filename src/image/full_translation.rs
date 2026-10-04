use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};

use crate::build_inputs::{ArleAssetSelection, ArleAssetSet};
use crate::patch::{
    EndingTranslationReport, FieldUnitCalloutTranslationReport, GameOverTranslationReport,
    MadMediaErrorReport, MadTranslationReport, OpeningTranslationReport, SelectTranslationReport,
    StageCompletionTranslationReport, StageSelectTranslationReport, TitleTranslationReport,
    compile_mad_media_errors,
};
use crate::source::verified::load_verified_source;
use crate::translation::load_reviewed_translation_corpus;

use super::ingame_translation::compile_ingame_translation;
use super::{ArleAssetBuildReport, BuildReport, pipeline};

#[derive(Debug, Eq, PartialEq)]
pub struct FullTranslationBuildReport {
    pub image: BuildReport,
    pub media_errors: MadMediaErrorReport,
    pub changed_runtime_files: Vec<String>,
    pub opening: OpeningTranslationReport,
    pub ending: EndingTranslationReport,
    pub select: SelectTranslationReport,
    pub mad: MadTranslationReport,
    pub field_unit_callout: FieldUnitCalloutTranslationReport,
    pub title: TitleTranslationReport,
    pub game_over: GameOverTranslationReport,
    pub stage_completion: StageCompletionTranslationReport,
    pub stage_select: StageSelectTranslationReport,
    pub arle_assets: ArleAssetBuildReport,
}

pub fn build_full_translation_image(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
) -> Result<FullTranslationBuildReport> {
    build_full_translation_image_with_arle_assets(
        source_path,
        drafts_directory,
        output_path,
        &ArleAssetSet::tracked(),
    )
}

pub fn build_full_translation_image_with_arle_assets(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
    arle_asset_set: &ArleAssetSet,
) -> Result<FullTranslationBuildReport> {
    build_full_translation_image_with_arle_asset_selection(
        source_path,
        drafts_directory,
        output_path,
        &ArleAssetSelection::replace(arle_asset_set.clone()),
    )
}

pub fn build_full_translation_image_with_arle_asset_selection(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
    arle_asset_selection: &ArleAssetSelection,
) -> Result<FullTranslationBuildReport> {
    pipeline::refuse_existing_output(output_path)?;
    let arle_assets = arle_asset_selection.resolve()?;
    let corpus = load_reviewed_translation_corpus(source_path, drafts_directory)?;
    let verified_source = load_verified_source(source_path)?;

    let ingame = compile_ingame_translation(&verified_source, &corpus, &arle_assets)?;
    let media_errors = compile_mad_media_errors(&ingame.installer_payload, &corpus)?;
    let changed_runtime_files = verify_gameplay_file_scope(
        &verified_source.installer_payload,
        &media_errors.installer_payload,
    )?;
    let image = pipeline::build_standalone_from_payload(
        &verified_source,
        &media_errors.installer_payload,
        output_path,
    )?;

    Ok(FullTranslationBuildReport {
        image,
        media_errors: media_errors.report,
        changed_runtime_files,
        opening: ingame.opening,
        ending: ingame.ending,
        select: ingame.select,
        mad: ingame.mad,
        field_unit_callout: ingame.field_unit_callout,
        title: ingame.title,
        game_over: ingame.game_over,
        stage_completion: ingame.stage_completion,
        stage_select: ingame.stage_select,
        arle_assets: ingame.arle_assets,
    })
}

fn verify_gameplay_file_scope(
    original: &BTreeMap<String, Vec<u8>>,
    patched: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<String>> {
    ensure!(
        original.keys().eq(patched.keys()),
        "full translation changed the installer payload population"
    );
    let changed = original
        .iter()
        .filter(|(filename, bytes)| patched.get(*filename) != Some(*bytes))
        .map(|(filename, _)| filename.clone())
        .collect::<Vec<_>>();
    let expected = [
        "ENDING.COM",
        "MAD.COM",
        "MADDAT",
        "OPENING.COM",
        "SELECT.COM",
    ];
    ensure!(
        changed.iter().map(String::as_str).eq(expected),
        "full translation must change exactly the five gameplay files; got {changed:?}"
    );
    Ok(changed)
}

pub fn default_full_translation_output_path() -> PathBuf {
    PathBuf::from("out/Disc Station Vol. 08 (Daimadou Korean Full Development).hdm")
}
