use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

pub(crate) const SIGNATURE: &[u8; 13] = b"FLINK Ver2.01";
pub(crate) const HEADER_SIZE: usize = 16;
pub(crate) const ENTRY_RECORD_SIZE: usize = 6;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FlinkEntry<'a> {
    pub id: usize,
    pub offset: usize,
    pub bytes: &'a [u8],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FlinkArchive<'a> {
    pub uninterpreted_header_byte: u8,
    pub entries: Vec<FlinkEntry<'a>>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FlinkRepack {
    pub bytes: Vec<u8>,
    pub replaced_entry_ids: BTreeSet<usize>,
    pub appended_entry_ids: BTreeSet<usize>,
    pub input_size: usize,
    pub output_size: usize,
}

pub(crate) fn parse_flink_archive(bytes: &[u8]) -> Result<FlinkArchive<'_>> {
    ensure!(
        bytes.len() >= HEADER_SIZE,
        "FLINK archive is shorter than its {HEADER_SIZE}-byte header"
    );
    ensure!(
        &bytes[..SIGNATURE.len()] == SIGNATURE,
        "FLINK archive signature does not match"
    );

    let entry_count = u16::from_le_bytes([bytes[13], bytes[14]]) as usize;
    let table_size = entry_count
        .checked_mul(ENTRY_RECORD_SIZE)
        .context("FLINK entry table size overflow")?;
    let table_end = HEADER_SIZE
        .checked_add(table_size)
        .context("FLINK entry table boundary overflow")?;
    ensure!(
        table_end <= bytes.len(),
        "FLINK entry table ends outside the archive: {table_end:#x} > {:#x}",
        bytes.len()
    );

    let mut entries = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let record_offset = HEADER_SIZE + index * ENTRY_RECORD_SIZE;
        let size = u16::from_le_bytes([bytes[record_offset], bytes[record_offset + 1]]) as usize;
        let data_offset = u32::from_le_bytes(
            bytes[record_offset + 2..record_offset + ENTRY_RECORD_SIZE]
                .try_into()
                .expect("a six-byte FLINK record always has a four-byte offset"),
        ) as usize;
        let data_end = data_offset
            .checked_add(size)
            .context("FLINK entry boundary overflow")?;
        let entry_bytes = bytes.get(data_offset..data_end).with_context(|| {
            format!(
                "FLINK entry {} lies outside the archive: {data_offset:#x}..{data_end:#x}",
                index + 1
            )
        })?;
        entries.push(FlinkEntry {
            id: index + 1,
            offset: data_offset,
            bytes: entry_bytes,
        });
    }

    Ok(FlinkArchive {
        uninterpreted_header_byte: bytes[15],
        entries,
    })
}

pub(crate) fn repack_contiguous_flink_archive(
    bytes: &[u8],
    replacements: &BTreeMap<usize, Vec<u8>>,
    appended_entries: &[Vec<u8>],
) -> Result<FlinkRepack> {
    ensure!(
        !replacements.is_empty() || !appended_entries.is_empty(),
        "FLINK repack requires at least one replacement or appended entry"
    );
    let archive = parse_flink_archive(bytes)?;
    let input_table_end = HEADER_SIZE
        .checked_add(archive.entries.len() * ENTRY_RECORD_SIZE)
        .context("FLINK table boundary overflow during repack")?;
    let mut expected_offset = input_table_end;
    for entry in &archive.entries {
        ensure!(
            entry.offset == expected_offset,
            "FLINK repack requires entries contiguous in table order; entry {} starts at {:#x} instead of {expected_offset:#x}",
            entry.id,
            entry.offset
        );
        expected_offset = expected_offset
            .checked_add(entry.bytes.len())
            .context("FLINK data boundary overflow during repack")?;
    }
    ensure!(
        expected_offset == bytes.len(),
        "FLINK repack requires the final entry to end at the archive boundary"
    );
    for &entry_id in replacements.keys() {
        ensure!(
            (1..=archive.entries.len()).contains(&entry_id),
            "FLINK replacement entry {entry_id} is outside 1..={}",
            archive.entries.len()
        );
    }

    let output_entry_count = archive
        .entries
        .len()
        .checked_add(appended_entries.len())
        .context("FLINK appended entry count overflow")?;
    let output_entry_count_u16 =
        u16::try_from(output_entry_count).context("FLINK appended entry count exceeds 16 bits")?;
    let output_table_end = HEADER_SIZE
        .checked_add(output_entry_count * ENTRY_RECORD_SIZE)
        .context("FLINK output table boundary overflow")?;

    let payload_size = archive
        .entries
        .iter()
        .map(|entry| {
            replacements
                .get(&entry.id)
                .map_or(entry.bytes.len(), Vec::len)
        })
        .chain(appended_entries.iter().map(Vec::len))
        .try_fold(0usize, |total, size| total.checked_add(size))
        .context("FLINK output payload size overflow")?;
    let output_size = output_table_end
        .checked_add(payload_size)
        .context("FLINK output size overflow")?;
    let mut output = Vec::with_capacity(output_size);
    output.resize(output_table_end, 0);
    output[..input_table_end].copy_from_slice(&bytes[..input_table_end]);
    output[13..15].copy_from_slice(&output_entry_count_u16.to_le_bytes());
    let mut output_offset = output_table_end;
    for (index, entry) in archive.entries.iter().enumerate() {
        let data = replacements
            .get(&entry.id)
            .map_or(entry.bytes, Vec::as_slice);
        let size = u16::try_from(data.len())
            .with_context(|| format!("FLINK entry {} exceeds its 16-bit size field", entry.id))?;
        let offset = u32::try_from(output_offset)
            .with_context(|| format!("FLINK entry {} offset exceeds 32 bits", entry.id))?;
        let record_offset = HEADER_SIZE + index * ENTRY_RECORD_SIZE;
        output[record_offset..record_offset + 2].copy_from_slice(&size.to_le_bytes());
        output[record_offset + 2..record_offset + ENTRY_RECORD_SIZE]
            .copy_from_slice(&offset.to_le_bytes());
        output.extend_from_slice(data);
        output_offset += data.len();
    }
    for (appended_index, data) in appended_entries.iter().enumerate() {
        let entry_id = archive.entries.len() + appended_index + 1;
        let size = u16::try_from(data.len())
            .with_context(|| format!("FLINK entry {entry_id} exceeds its 16-bit size field"))?;
        let offset = u32::try_from(output_offset)
            .with_context(|| format!("FLINK entry {entry_id} offset exceeds 32 bits"))?;
        let record_offset = HEADER_SIZE + (entry_id - 1) * ENTRY_RECORD_SIZE;
        output[record_offset..record_offset + 2].copy_from_slice(&size.to_le_bytes());
        output[record_offset + 2..record_offset + ENTRY_RECORD_SIZE]
            .copy_from_slice(&offset.to_le_bytes());
        output.extend_from_slice(data);
        output_offset += data.len();
    }
    ensure!(
        output.len() == output_size,
        "FLINK repack output length differs from its planned size"
    );

    let reparsed = parse_flink_archive(&output)?;
    ensure!(
        reparsed.uninterpreted_header_byte == archive.uninterpreted_header_byte
            && reparsed.entries.len() == output_entry_count,
        "FLINK repack changed protected header metadata"
    );
    for (original, candidate) in archive.entries.iter().zip(&reparsed.entries) {
        let expected = replacements
            .get(&original.id)
            .map_or(original.bytes, Vec::as_slice);
        ensure!(
            candidate.id == original.id && candidate.bytes == expected,
            "FLINK repack verification failed for entry {}",
            original.id
        );
    }
    for (appended_index, expected) in appended_entries.iter().enumerate() {
        let entry_id = archive.entries.len() + appended_index + 1;
        ensure!(
            reparsed.entries[entry_id - 1].bytes == expected,
            "FLINK repack verification failed for appended entry {entry_id}"
        );
    }

    Ok(FlinkRepack {
        bytes: output,
        replaced_entry_ids: replacements.keys().copied().collect(),
        appended_entry_ids: (archive.entries.len() + 1..=output_entry_count).collect(),
        input_size: bytes.len(),
        output_size,
    })
}

#[cfg(test)]
#[path = "flink_tests.rs"]
mod flink_tests;
