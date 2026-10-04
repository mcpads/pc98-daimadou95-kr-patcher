use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read, Write};

use anyhow::{Context, Result, ensure};
use fatfs::{FatType, FileSystem, FsOptions};

use crate::source::disk::{
    BOOT_SECTOR_SIZE, PRESERVED_SYSTEM_FILES, REQUIRED_TOOL_FILES, SourceFiles, fatfs_mount_copy,
};
use crate::source::installer::payload_write_order;

pub(crate) fn assemble_standalone_image(
    source: &[u8],
    installer_payload: &BTreeMap<String, Vec<u8>>,
    tool_files: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut image = fatfs_mount_copy(source)?;
    {
        let cursor = Cursor::new(image.as_mut_slice());
        let filesystem = FileSystem::new(cursor, FsOptions::new())
            .context("failed to mount working FAT12 image")?;
        ensure!(
            matches!(filesystem.fat_type(), FatType::Fat12),
            "working image is not FAT12"
        );
        let root = filesystem.root_dir();

        remove_disc_station_files(&root)?;
        for name in REQUIRED_TOOL_FILES {
            write_root_file(
                &root,
                name,
                tool_files
                    .get(name)
                    .with_context(|| format!("missing build tool file: {name}"))?,
            )?;
        }
        for &name in payload_write_order() {
            write_root_file(
                &root,
                name,
                installer_payload
                    .get(name)
                    .with_context(|| format!("missing verified installer file: {name}"))?,
            )?;
        }
        drop(root);
        filesystem
            .unmount()
            .context("failed to unmount assembled FAT12 image")?;
    }
    image[..BOOT_SECTOR_SIZE].copy_from_slice(&source[..BOOT_SECTOR_SIZE]);
    Ok(image)
}

pub(crate) fn verify_standalone_image(
    source: &[u8],
    image: &[u8],
    source_files: &SourceFiles,
    installer_payload: &BTreeMap<String, Vec<u8>>,
    tool_files: &BTreeMap<String, Vec<u8>>,
) -> Result<usize> {
    ensure!(
        image.len() == source.len(),
        "standalone image size changed: expected {}, got {}",
        source.len(),
        image.len()
    );
    ensure!(
        image[..BOOT_SECTOR_SIZE] == source[..BOOT_SECTOR_SIZE],
        "standalone image changed the PC-98 boot sector"
    );
    verify_fat_mirrors(image)?;

    // fatfs 0.3 requires a writable stream and an IBM-PC signature even though
    // this pass only verifies a PC-98 image. Both accommodations stay in this copy.
    let cursor = Cursor::new(fatfs_mount_copy(image)?);
    let filesystem = FileSystem::new(cursor, FsOptions::new())
        .context("failed to remount assembled FAT12 image")?;
    ensure!(
        matches!(filesystem.fat_type(), FatType::Fat12),
        "assembled image did not remount as FAT12"
    );
    let root = filesystem.root_dir();

    let expected_names = expected_root_names();
    let actual_names: BTreeSet<String> = root
        .iter()
        .map(|entry| entry.map(|entry| entry.file_name().to_ascii_uppercase()))
        .collect::<std::io::Result<_>>()
        .context("failed to enumerate assembled FAT12 root")?;
    ensure!(
        actual_names == expected_names,
        "assembled root file set differs: expected {expected_names:?}, got {actual_names:?}"
    );

    for (name, expected) in &source_files.system_files {
        verify_root_file(&root, name, expected)?;
    }
    ensure!(
        tool_files.keys().eq(source_files.tool_files.keys()),
        "assembled tool-file population differs from the verified source"
    );
    for (name, expected) in tool_files {
        verify_root_file(&root, name, expected)?;
    }
    for (name, expected) in installer_payload {
        verify_root_file(&root, name, expected)?;
    }

    Ok(expected_names.len())
}

fn remove_disc_station_files<T: fatfs::ReadWriteSeek>(root: &fatfs::Dir<'_, T>) -> Result<()> {
    if let Ok(stext) = root.open_dir("STEXT") {
        let child_names: Vec<String> = stext
            .iter()
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<std::io::Result<Vec<String>>>()
            .context("failed to enumerate STEXT")?
            .into_iter()
            .filter(|name| name != "." && name != "..")
            .collect();
        for name in child_names {
            stext
                .remove(&name)
                .with_context(|| format!("failed to remove STEXT/{name}"))?;
        }
        drop(stext);
        root.remove("STEXT")
            .context("failed to remove empty STEXT directory")?;
    }

    let retained: BTreeSet<_> = PRESERVED_SYSTEM_FILES.into_iter().collect();
    let root_names: Vec<String> = root
        .iter()
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<std::io::Result<Vec<String>>>()
        .context("failed to enumerate source root")?;
    for name in root_names {
        let upper_name = name.to_ascii_uppercase();
        if !retained.contains(upper_name.as_str()) {
            root.remove(&name)
                .with_context(|| format!("failed to remove Disc Station file: {name}"))?;
        }
    }
    Ok(())
}

fn write_root_file<T: fatfs::ReadWriteSeek>(
    root: &fatfs::Dir<'_, T>,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    let mut file = root
        .create_file(name)
        .with_context(|| format!("failed to create FAT12 file: {name}"))?;
    file.truncate()
        .with_context(|| format!("failed to truncate FAT12 file: {name}"))?;
    file.write_all(bytes)
        .with_context(|| format!("failed to write FAT12 file: {name}"))?;
    Ok(())
}

fn verify_root_file<T: fatfs::ReadWriteSeek>(
    root: &fatfs::Dir<'_, T>,
    name: &str,
    expected: &[u8],
) -> Result<()> {
    let mut file = root
        .open_file(name)
        .with_context(|| format!("assembled image is missing file: {name}"))?;
    let mut actual = Vec::new();
    file.read_to_end(&mut actual)
        .with_context(|| format!("failed to read assembled file: {name}"))?;
    ensure!(
        actual == expected,
        "assembled file differs from its verified producer output: {name}"
    );
    Ok(())
}

fn expected_root_names() -> BTreeSet<String> {
    PRESERVED_SYSTEM_FILES
        .into_iter()
        .chain(REQUIRED_TOOL_FILES)
        .chain(payload_write_order().iter().copied())
        .map(str::to_owned)
        .collect()
}

fn verify_fat_mirrors(image: &[u8]) -> Result<()> {
    ensure!(image.len() >= BOOT_SECTOR_SIZE, "truncated assembled image");
    let bytes_per_sector = u16::from_le_bytes([image[11], image[12]]) as usize;
    let reserved_sectors = u16::from_le_bytes([image[14], image[15]]) as usize;
    let fat_count = image[16] as usize;
    let sectors_per_fat = u16::from_le_bytes([image[22], image[23]]) as usize;
    ensure!(
        fat_count == 2,
        "assembled image does not have two FAT copies"
    );

    let fat_size = bytes_per_sector
        .checked_mul(sectors_per_fat)
        .context("FAT byte size overflow")?;
    let first_offset = bytes_per_sector
        .checked_mul(reserved_sectors)
        .context("FAT offset overflow")?;
    let second_offset = first_offset
        .checked_add(fat_size)
        .context("second FAT offset overflow")?;
    let first = image
        .get(first_offset..first_offset + fat_size)
        .context("first FAT lies outside assembled image")?;
    let second = image
        .get(second_offset..second_offset + fat_size)
        .context("second FAT lies outside assembled image")?;
    ensure!(first == second, "assembled FAT mirrors differ");
    Ok(())
}
