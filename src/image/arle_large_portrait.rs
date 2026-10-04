use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::build_inputs::ArleAssetSet;
use crate::patch::{ArleLargePortraitReport, compile_arle_large_portrait};
use crate::source::verified::load_verified_source;

use super::{BuildReport, pipeline};

#[derive(Debug, Eq, PartialEq)]
pub struct ArleLargePortraitBuildReport {
    pub image: BuildReport,
    pub portrait: ArleLargePortraitReport,
}

pub fn build_arle_large_portrait_image(
    source_path: &Path,
    output_path: &Path,
) -> Result<ArleLargePortraitBuildReport> {
    build_arle_large_portrait_image_with_arle_assets(
        source_path,
        output_path,
        &ArleAssetSet::tracked(),
    )
}

pub fn build_arle_large_portrait_image_with_arle_assets(
    source_path: &Path,
    output_path: &Path,
    arle_asset_set: &ArleAssetSet,
) -> Result<ArleLargePortraitBuildReport> {
    pipeline::refuse_existing_output(output_path)?;
    let arle_assets = arle_asset_set.resolve()?;
    let verified_source = load_verified_source(source_path)?;
    let maddat = verified_source
        .installer_payload
        .get("MADDAT")
        .context("verified installer payload is missing MADDAT")?;
    let select_com = verified_source
        .installer_payload
        .get("SELECT.COM")
        .context("verified installer payload is missing SELECT.COM")?;
    let mad_com = verified_source
        .installer_payload
        .get("MAD.COM")
        .context("verified installer payload is missing MAD.COM")?;
    let compiled =
        compile_arle_large_portrait(maddat, select_com, mad_com, &arle_assets.large_portrait)?;

    let mut installer_payload = verified_source.installer_payload.clone();
    installer_payload.insert("MADDAT".to_owned(), compiled.maddat);
    installer_payload.insert("SELECT.COM".to_owned(), compiled.select_com);
    installer_payload.insert("MAD.COM".to_owned(), compiled.mad_com);
    let image =
        pipeline::build_standalone_from_payload(&verified_source, &installer_payload, output_path)?;

    Ok(ArleLargePortraitBuildReport {
        image,
        portrait: compiled.report,
    })
}

pub fn default_arle_large_portrait_output_path() -> PathBuf {
    PathBuf::from("out/Disc Station Vol. 08 (Daimadou Arle Large Portrait Development).hdm")
}
