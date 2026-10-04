use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tempfile::NamedTempFile;

use crate::image::{reconstruction, standalone};
use crate::source::verified::VerifiedSource;
use crate::source::{disk, verified::load_verified_source};

#[derive(Debug, Eq, PartialEq)]
pub struct BuildReport {
    pub source_sha256: String,
    pub output_sha256: String,
    pub output_size: usize,
    pub file_count: usize,
}

pub fn build_standalone_image(source_path: &Path, output_path: &Path) -> Result<BuildReport> {
    refuse_existing_output(output_path)?;
    let verified_source = load_verified_source(source_path)?;
    build_standalone_from_payload(
        &verified_source,
        &verified_source.installer_payload,
        output_path,
    )
}

pub(crate) fn build_standalone_from_payload(
    verified_source: &VerifiedSource,
    installer_payload: &BTreeMap<String, Vec<u8>>,
    output_path: &Path,
) -> Result<BuildReport> {
    build_standalone_from_files(
        verified_source,
        installer_payload,
        &verified_source.source_files.tool_files,
        output_path,
    )
}

pub(crate) fn build_standalone_from_files(
    verified_source: &VerifiedSource,
    installer_payload: &BTreeMap<String, Vec<u8>>,
    tool_files: &BTreeMap<String, Vec<u8>>,
    output_path: &Path,
) -> Result<BuildReport> {
    let candidate = standalone::assemble_standalone_image(
        &verified_source.image,
        installer_payload,
        tool_files,
    )?;
    let verified_file_count = standalone::verify_standalone_image(
        &verified_source.image,
        &candidate,
        &verified_source.source_files,
        installer_payload,
        tool_files,
    )?;
    let differences = reconstruction::derive_reconstruction_diff(
        &verified_source.image,
        &candidate,
        0..disk::BOOT_SECTOR_SIZE,
    )?;
    let output = reconstruction::apply_reconstruction_diff(&verified_source.image, &differences)?;
    if output != candidate {
        bail!("applied reconstruction diff differs from the verified candidate image");
    }
    let output_sha256 = disk::sha256_hex(&output);

    write_new_output(output_path, &output)?;

    Ok(BuildReport {
        source_sha256: verified_source.verification.sha256.clone(),
        output_sha256,
        output_size: output.len(),
        file_count: verified_file_count,
    })
}

pub(crate) fn refuse_existing_output(output_path: &Path) -> Result<()> {
    if output_path.exists() {
        bail!(
            "refusing to overwrite existing output: {}",
            output_path.display()
        );
    }
    Ok(())
}

fn write_new_output(output_path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create output directory: {}", parent.display()))?;

    let mut temporary = NamedTempFile::new_in(parent)
        .with_context(|| format!("failed to create temporary output in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .context("failed to write temporary output image")?;
    temporary
        .as_file()
        .sync_all()
        .context("failed to flush temporary output image")?;
    temporary.persist_noclobber(output_path).map_err(|error| {
        anyhow::anyhow!(
            "failed to publish output without overwriting {}: {}",
            output_path.display(),
            error.error
        )
    })?;
    Ok(())
}

pub fn default_output_path() -> PathBuf {
    PathBuf::from("out/Disc Station Vol. 08 (Daimadou Senryaku Monogatari 95).hdm")
}
