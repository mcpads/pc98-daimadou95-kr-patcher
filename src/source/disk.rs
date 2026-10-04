use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use anyhow::{Context, Result, bail, ensure};
use fatfs::{FatType, FileSystem, FsOptions};
use sha2::{Digest, Sha256};

use crate::source::verified::SourceVerification;

pub const SOURCE_DISK_SIZE: usize = 1_261_568;
pub const SOURCE_DISK_SHA256: &str =
    "b2ade325198210914e1cd1a6add3724fb4de1a1c5e5f8f617acfe7999234c122";

pub(crate) const BOOT_SECTOR_SIZE: usize = 1_024;
const FATFS_HIDDEN_SECTORS_OFFSET: usize = 28;
const FATFS_TOTAL_SECTORS_32_OFFSET: usize = 32;
const FATFS_SIGNATURE_OFFSET: usize = 510;
pub(crate) const PRESERVED_SYSTEM_FILES: [&str; 2] = ["IO98.SYS", "MEGDOS.SYS"];
pub(crate) const REQUIRED_TOOL_FILES: [&str; 4] =
    ["FPLAY.COM", "BPLAY.COM", "BSAMP.COM", "NMOUSE.COM"];

const INSTALLER_NAME: &str = "MADO95.EXE";
const INSTALLER_SHA256: &str = "7491eaa1ce1f867b630ea2d0e755fd4f30567f43c45602c5dee97c48413ec64b";

#[derive(Debug)]
pub(crate) struct SourceFiles {
    pub installer: Vec<u8>,
    pub system_files: BTreeMap<String, Vec<u8>>,
    pub tool_files: BTreeMap<String, Vec<u8>>,
}

pub(crate) fn verify_source_bytes(source: &[u8]) -> Result<SourceVerification> {
    ensure!(
        source.len() == SOURCE_DISK_SIZE,
        "unsupported source size: expected {SOURCE_DISK_SIZE} bytes, got {}",
        source.len()
    );
    let actual_sha256 = sha256_hex(source);
    ensure!(
        actual_sha256 == SOURCE_DISK_SHA256,
        "unsupported Disc Station Vol. 08 Disk 1 revision: expected SHA-256 {SOURCE_DISK_SHA256}, got {actual_sha256}"
    );
    validate_pc98_fat12_geometry(source)?;
    Ok(SourceVerification {
        sha256: actual_sha256,
        size: source.len(),
    })
}

pub(crate) fn read_source_files(source: &[u8]) -> Result<SourceFiles> {
    // fatfs 0.3 uses one ReadWriteSeek bound for both read-only and writable mounts.
    // Mount an owned copy so source verification can never mutate the caller's bytes.
    let cursor = Cursor::new(fatfs_mount_copy(source)?);
    let filesystem = FileSystem::new(cursor, FsOptions::new())
        .context("failed to mount supported source as FAT12")?;
    ensure!(
        matches!(filesystem.fat_type(), FatType::Fat12),
        "supported source did not mount as FAT12"
    );
    let root = filesystem.root_dir();

    let installer = read_root_file(&root, INSTALLER_NAME)?;
    ensure!(
        sha256_hex(&installer) == INSTALLER_SHA256,
        "{INSTALLER_NAME} did not match the supported installer payload"
    );

    let mut system_files = BTreeMap::new();
    for name in PRESERVED_SYSTEM_FILES {
        system_files.insert(name.to_owned(), read_root_file(&root, name)?);
    }

    let mut tool_files = BTreeMap::new();
    for name in REQUIRED_TOOL_FILES {
        tool_files.insert(name.to_owned(), read_root_file(&root, name)?);
    }

    Ok(SourceFiles {
        installer,
        system_files,
        tool_files,
    })
}

pub(crate) fn fatfs_mount_copy(image: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        image.len() > FATFS_SIGNATURE_OFFSET + 1,
        "image is too short for the fatfs compatibility signature"
    );
    let mut mount_copy = image.to_vec();
    // This PC-98 DOS 3.x BPB ends before the later hidden-sectors and
    // total-sectors-32 fields. fatfs interprets the following boot code as
    // those fields, so clear only the compatibility copy's later extensions.
    mount_copy[FATFS_HIDDEN_SECTORS_OFFSET..FATFS_TOTAL_SECTORS_32_OFFSET].fill(0);
    mount_copy[FATFS_TOTAL_SECTORS_32_OFFSET..FATFS_TOTAL_SECTORS_32_OFFSET + 4].fill(0);
    mount_copy[FATFS_SIGNATURE_OFFSET..FATFS_SIGNATURE_OFFSET + 2].copy_from_slice(&[0x55, 0xaa]);
    Ok(mount_copy)
}

fn read_root_file<T: fatfs::ReadWriteSeek>(
    root: &fatfs::Dir<'_, T>,
    name: &str,
) -> Result<Vec<u8>> {
    let mut file = root
        .open_file(name)
        .with_context(|| format!("required source file is missing: {name}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("failed to read source file: {name}"))?;
    Ok(bytes)
}

fn validate_pc98_fat12_geometry(source: &[u8]) -> Result<()> {
    let bpb = source
        .get(..BOOT_SECTOR_SIZE)
        .context("source is missing its 1024-byte boot sector")?;
    let u16_at = |offset: usize| -> Result<u16> {
        let bytes: [u8; 2] = bpb
            .get(offset..offset + 2)
            .context("truncated BIOS parameter block")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid BIOS parameter block field"))?;
        Ok(u16::from_le_bytes(bytes))
    };

    let observed = (
        u16_at(11)?,
        bpb[13],
        u16_at(14)?,
        bpb[16],
        u16_at(17)?,
        u16_at(19)?,
        bpb[21],
        u16_at(22)?,
        u16_at(24)?,
        u16_at(26)?,
    );
    let expected = (1024, 1, 1, 2, 192, 1232, 0xfe, 2, 8, 2);
    if observed != expected {
        bail!("supported source has unexpected PC-98 FAT12 geometry: {observed:?}");
    }
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[cfg(test)]
#[path = "disk_tests.rs"]
mod disk_tests;
