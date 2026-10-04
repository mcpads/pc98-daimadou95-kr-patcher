use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::build_inputs::{ArleAssetSelection, ArleAssetSet, ResolvedArleAssetSelection};
use crate::image::pipeline;
use crate::patch::{
    EndingTranslationReport, FieldUnitCalloutTranslationReport, GameOverTranslationReport,
    MadTranslationReport, OpeningTranslationReport, SelectTranslationReport,
    StageCompletionTranslationReport, StageSelectTranslationReport, TitleTranslationReport,
    compile_ending_translation, compile_field_unit_callout_translation,
    compile_game_over_translation, compile_mad_translation, compile_opening_translation,
    compile_select_translation, compile_stage_completion_translation,
    compile_stage_select_translation, compile_title_translation,
};
use crate::source::verified::{VerifiedSource, load_verified_source};
use crate::translation::{ReviewedTranslationCorpus, load_reviewed_translation_corpus};

use super::BuildReport;
use super::arle_assets::{ArleAssetBuildReport, apply_arle_asset_selection};

#[derive(Debug, Eq, PartialEq)]
pub struct IngameTranslationBuildReport {
    pub image: BuildReport,
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

pub(crate) struct IngameTranslationFiles {
    pub installer_payload: BTreeMap<String, Vec<u8>>,
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

pub fn build_ingame_translation_image(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
) -> Result<IngameTranslationBuildReport> {
    build_ingame_translation_image_with_arle_assets(
        source_path,
        drafts_directory,
        output_path,
        &ArleAssetSet::tracked(),
    )
}

pub fn build_ingame_translation_image_with_arle_assets(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
    arle_asset_set: &ArleAssetSet,
) -> Result<IngameTranslationBuildReport> {
    build_ingame_translation_image_with_arle_asset_selection(
        source_path,
        drafts_directory,
        output_path,
        &ArleAssetSelection::replace(arle_asset_set.clone()),
    )
}

pub fn build_ingame_translation_image_with_arle_asset_selection(
    source_path: &Path,
    drafts_directory: &Path,
    output_path: &Path,
    arle_asset_selection: &ArleAssetSelection,
) -> Result<IngameTranslationBuildReport> {
    pipeline::refuse_existing_output(output_path)?;
    let arle_assets = arle_asset_selection.resolve()?;
    let corpus = load_reviewed_translation_corpus(source_path, drafts_directory)?;
    let verified_source = load_verified_source(source_path)?;
    let compiled = compile_ingame_translation(&verified_source, &corpus, &arle_assets)?;
    let image = pipeline::build_standalone_from_payload(
        &verified_source,
        &compiled.installer_payload,
        output_path,
    )?;

    Ok(IngameTranslationBuildReport {
        image,
        opening: compiled.opening,
        ending: compiled.ending,
        select: compiled.select,
        mad: compiled.mad,
        field_unit_callout: compiled.field_unit_callout,
        title: compiled.title,
        game_over: compiled.game_over,
        stage_completion: compiled.stage_completion,
        stage_select: compiled.stage_select,
        arle_assets: compiled.arle_assets,
    })
}

pub(crate) fn compile_ingame_translation(
    verified_source: &VerifiedSource,
    corpus: &ReviewedTranslationCorpus,
    arle_assets: &ResolvedArleAssetSelection,
) -> Result<IngameTranslationFiles> {
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

    let field_unit_callout =
        compile_field_unit_callout_translation(source_maddat, source_mad_com, corpus)?;
    let opening =
        compile_opening_translation(&field_unit_callout.maddat, source_opening_com, corpus)?;
    let ending = compile_ending_translation(&opening.maddat, source_ending_com, corpus)?;
    let select = compile_select_translation(&ending.maddat, source_select_com, corpus)?;
    let mad = compile_mad_translation(&select.maddat, source_mad_com, corpus)?;
    let title = compile_title_translation(&mad.maddat, &opening.opening_com, corpus)?;
    let game_over = compile_game_over_translation(&title.maddat, corpus)?;
    let stage_completion = compile_stage_completion_translation(&game_over.maddat, corpus)?;
    let stage_select = compile_stage_select_translation(&stage_completion.maddat, corpus)?;
    let arle_assets = apply_arle_asset_selection(
        &stage_select.maddat,
        &select.select_com,
        &mad.mad_com,
        &ending.ending_com,
        arle_assets,
    )?;

    let mut installer_payload = verified_source.installer_payload.clone();
    installer_payload.insert("MADDAT".to_owned(), arle_assets.maddat);
    installer_payload.insert("MAD.COM".to_owned(), arle_assets.mad_com);
    installer_payload.insert("OPENING.COM".to_owned(), title.opening_com);
    installer_payload.insert("ENDING.COM".to_owned(), arle_assets.ending_com);
    installer_payload.insert("SELECT.COM".to_owned(), arle_assets.select_com);
    Ok(IngameTranslationFiles {
        installer_payload,
        opening: opening.report,
        ending: ending.report,
        select: select.report,
        mad: mad.report,
        field_unit_callout: field_unit_callout.report,
        title: title.report,
        game_over: game_over.report,
        stage_completion: stage_completion.report,
        stage_select: stage_select.report,
        arle_assets: arle_assets.report,
    })
}

pub fn default_ingame_translation_output_path() -> PathBuf {
    PathBuf::from("out/Disc Station Vol. 08 (Daimadou Korean In-Game Development).hdm")
}
