use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::source::{disk, installer};

#[derive(Debug, Eq, PartialEq)]
pub struct SourceVerification {
    pub sha256: String,
    pub size: usize,
}

pub(crate) struct VerifiedSource {
    pub image: Vec<u8>,
    pub verification: SourceVerification,
    pub source_files: disk::SourceFiles,
    pub installer_payload: BTreeMap<String, Vec<u8>>,
}

pub fn verify_source_path(source_path: &Path) -> Result<SourceVerification> {
    let source = fs::read(source_path)
        .with_context(|| format!("failed to read source disk: {}", source_path.display()))?;
    disk::verify_source_bytes(&source)
}

pub(crate) fn load_verified_source(source_path: &Path) -> Result<VerifiedSource> {
    let image = fs::read(source_path)
        .with_context(|| format!("failed to read source disk: {}", source_path.display()))?;
    let verification = disk::verify_source_bytes(&image)?;
    let source_files = disk::read_source_files(&image)?;
    let installer_payload = installer::extract_installer_payload(&source_files.installer)?;
    Ok(VerifiedSource {
        image,
        verification,
        source_files,
        installer_payload,
    })
}
