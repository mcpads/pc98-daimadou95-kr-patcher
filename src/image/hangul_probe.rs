use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::image::pipeline;
use crate::patch::{HangulProbeReport, apply_hangul_probe};
use crate::source::verified::load_verified_source;

use super::BuildReport;

#[derive(Debug, Eq, PartialEq)]
pub struct HangulProbeBuildReport {
    pub image: BuildReport,
    pub probe: HangulProbeReport,
}

pub fn build_hangul_probe_image(
    source_path: &Path,
    output_path: &Path,
) -> Result<HangulProbeBuildReport> {
    pipeline::refuse_existing_output(output_path)?;
    let verified_source = load_verified_source(source_path)?;
    let mad_com = verified_source
        .installer_payload
        .get("MAD.COM")
        .context("verified installer payload is missing MAD.COM")?;
    let gaiji_com = verified_source
        .installer_payload
        .get("GAIJI.COM")
        .context("verified installer payload is missing GAIJI.COM")?;
    let patched = apply_hangul_probe(mad_com, gaiji_com)?;

    let mut installer_payload = verified_source.installer_payload.clone();
    installer_payload.insert("MAD.COM".to_owned(), patched.mad_com);
    installer_payload.insert("GAIJI.COM".to_owned(), patched.gaiji_com);
    let image =
        pipeline::build_standalone_from_payload(&verified_source, &installer_payload, output_path)?;

    Ok(HangulProbeBuildReport {
        image,
        probe: patched.report,
    })
}

pub fn default_hangul_probe_output_path() -> PathBuf {
    PathBuf::from("out/Disc Station Vol. 08 (Daimadou Hangul Probe).hdm")
}
