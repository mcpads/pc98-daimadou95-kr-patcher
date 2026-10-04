use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use encoding_rs::SHIFT_JIS;
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    ResizePlan, WriteIntent, WritePlan,
};
use v30::{Assembler, CodeLocation, Instruction, Operand, Register16};

use crate::game_data::dos_text;
use crate::patch::apply_v30_patch_plan;

use super::bank_plan::GaijiCodebook;
use super::{DosOutputPart, DosTranslation};

const COM_ORIGIN: usize = 0x100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DosSourceReferenceKind {
    Immediate,
    PointerTable,
}

pub(super) struct DosSourceReference {
    pub file_offset: usize,
    pub kind: DosSourceReferenceKind,
}

pub(super) struct DosSourceRecord {
    pub id: String,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub byte_size: usize,
    pub references: Vec<DosSourceReference>,
}

pub(super) struct PatchedDosText {
    pub bytes: Vec<u8>,
    pub pointer_reference_count: usize,
    pub translated_text_byte_count: usize,
    pub output_size: usize,
}

struct RelocatedRecord<'a> {
    translation: &'a DosTranslation,
    source: &'a DosSourceRecord,
    runtime_address: u16,
    bytes: Vec<u8>,
}

pub(super) fn reinsert_dos_text(
    bytes: &[u8],
    source_file_size: usize,
    translations: &[&DosTranslation],
    source_records: &[DosSourceRecord],
    codebook: &GaijiCodebook,
) -> Result<PatchedDosText> {
    ensure!(
        bytes.len() >= source_file_size,
        "DOS translation input is shorter than its verified source file"
    );
    let source_by_id = source_records
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    let mut seen_records = BTreeSet::new();
    let mut appended = Vec::new();
    let mut relocated = Vec::with_capacity(translations.len());
    for translation in translations {
        ensure!(
            seen_records.insert(translation.record_id.as_str()),
            "DOS translation duplicates source record {}",
            translation.record_id
        );
        let source = source_by_id
            .get(translation.record_id.as_str())
            .with_context(|| {
                format!(
                    "{} has no verified source record {}",
                    translation.filename, translation.record_id
                )
            })?;
        ensure!(
            !source.references.is_empty(),
            "target DOS source record {} has no consumer reference",
            source.id
        );
        let original = bytes
            .get(source.file_offset..source.file_offset + source.byte_size)
            .with_context(|| format!("DOS source record {} is truncated", source.id))?;
        ensure!(
            original.last() == Some(&b'$'),
            "DOS source record {} lost its verified terminator",
            source.id
        );
        let encoded = encode_output_parts(&translation.entry_id, &translation.parts, codebook)?;
        let parsed = dos_text::parse_dos_text(&encoded)
            .with_context(|| format!("parse reinserted DOS text {}", translation.entry_id))?;
        ensure!(
            parsed.byte_len == encoded.len()
                && dos_text::reconstruct_dos_text(&parsed.tokens)? == encoded,
            "reinserted DOS text {} does not round trip",
            translation.entry_id
        );
        let runtime_address = bytes
            .len()
            .checked_add(appended.len())
            .and_then(|offset| offset.checked_add(COM_ORIGIN))
            .context("DOS translated text address overflow")?;
        let runtime_address = u16::try_from(runtime_address)
            .context("DOS translated text exceeds the COM address space")?;
        appended.extend_from_slice(&encoded);
        relocated.push(RelocatedRecord {
            translation,
            source,
            runtime_address,
            bytes: encoded,
        });
    }

    let output_size = bytes
        .len()
        .checked_add(appended.len())
        .context("DOS translated file size overflow")?;
    ensure!(
        output_size + COM_ORIGIN <= usize::from(u16::MAX) + 1,
        "DOS translated file exceeds the COM address space"
    );
    let mut plan = WritePlan::new()
        .resize(ResizePlan {
            owner: "dos-text-reinserter".into(),
            purpose: "append translated DOS strings without fixed-pool truncation".into(),
            expected_input_len: bytes.len(),
            output_len: output_size,
        })
        .region(ImageRegion {
            id: "dos-translated-text-block".into(),
            range: bytes.len()..output_size,
            kind: RegionKind::Data,
            reason: "relocated dollar-terminated Korean DOS strings".into(),
        })
        .write(ExpectedWrite {
            id: "append-dos-translated-text".into(),
            owner: "dos-text-reinserter".into(),
            purpose: "append every translated DOS display record".into(),
            offset: bytes.len(),
            expected_original: Vec::new(),
            replacement: appended.clone(),
            intent: WriteIntent::Data,
        });

    let mut machine_sources = BTreeMap::new();
    let mut reference_offsets = BTreeSet::new();
    for record in &relocated {
        let neutralized = neutralized_source_record(record.source.byte_size)?;
        plan = plan
            .region(ImageRegion {
                id: format!("dos-source-record-{}", record.translation.entry_id),
                range: record.source.file_offset
                    ..record.source.file_offset + record.source.byte_size,
                kind: RegionKind::Data,
                reason: "translated source record is unreachable and contains no stale Japanese"
                    .into(),
            })
            .write(ExpectedWrite {
                id: format!("neutralize-dos-source-{}", record.translation.entry_id),
                owner: "dos-text-reinserter".into(),
                purpose: "remove the relocated target-language predecessor bytes".into(),
                offset: record.source.file_offset,
                expected_original: bytes[record.source.file_offset
                    ..record.source.file_offset + record.source.byte_size]
                    .to_vec(),
                replacement: neutralized,
                intent: WriteIntent::Data,
            });

        for reference in &record.source.references {
            ensure!(
                reference_offsets.insert(reference.file_offset),
                "DOS reference offset {:#x} is owned more than once",
                reference.file_offset
            );
            let source_id = format!(
                "dos-text-pointer-{}-{:04x}",
                record.translation.filename.to_ascii_lowercase(),
                reference.file_offset
            );
            let original_address = u16::try_from(record.source.runtime_address)
                .context("DOS source text address exceeds 16 bits")?;
            match reference.kind {
                DosSourceReferenceKind::Immediate => {
                    let original = assemble_mov_dx(reference.file_offset, original_address)?;
                    let replacement =
                        assemble_mov_dx(reference.file_offset, record.runtime_address)?;
                    ensure!(
                        bytes.get(
                            reference.file_offset..reference.file_offset + original.bytes().len()
                        ) == Some(original.bytes()),
                        "DOS MOV DX source differs at {:#x}",
                        reference.file_offset
                    );
                    let write_len = replacement.bytes().len();
                    plan = plan
                        .region(ImageRegion {
                            id: format!("{source_id}-instruction"),
                            range: reference.file_offset..reference.file_offset + write_len,
                            kind: RegionKind::MachineCode,
                            reason: "typed pointer to relocated DOS display text".into(),
                        })
                        .write(ExpectedWrite {
                            id: format!("write-{source_id}"),
                            owner: "dos-text-pointer-compiler".into(),
                            purpose: "retarget MOV DX to translated DOS text".into(),
                            offset: reference.file_offset,
                            expected_original: original.into_bytes(),
                            replacement: replacement.bytes().to_vec(),
                            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                                assembly_source_id: source_id.clone(),
                                isa_profile_id: v30::PROFILE_ID.into(),
                            }),
                        });
                    machine_sources.insert(source_id, replacement);
                }
                DosSourceReferenceKind::PointerTable => {
                    let original = original_address.to_le_bytes();
                    ensure!(
                        bytes.get(reference.file_offset..reference.file_offset + 2)
                            == Some(&original),
                        "DOS pointer-table source differs at {:#x}",
                        reference.file_offset
                    );
                    plan = plan
                        .region(ImageRegion {
                            id: format!("{source_id}-metadata"),
                            range: reference.file_offset..reference.file_offset + 2,
                            kind: RegionKind::Metadata,
                            reason: "table pointer to relocated DOS display text".into(),
                        })
                        .write(ExpectedWrite {
                            id: format!("write-{source_id}"),
                            owner: "dos-text-pointer-compiler".into(),
                            purpose: "retarget DOS text table entry".into(),
                            offset: reference.file_offset,
                            expected_original: original.to_vec(),
                            replacement: record.runtime_address.to_le_bytes().to_vec(),
                            intent: WriteIntent::Metadata,
                        });
                }
            }
        }
    }

    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| {
        machine_sources.get(source_id).cloned().ok_or_else(|| {
            MachineCodeVerifierError::new(format!("unknown DOS pointer source {source_id}"))
        })
    })?;
    ensure!(
        output[bytes.len()..] == appended,
        "DOS appended translation block differs after Expected Writes"
    );
    for record in &relocated {
        let offset = usize::from(record.runtime_address) - COM_ORIGIN;
        ensure!(
            output.get(offset..offset + record.bytes.len()) == Some(record.bytes.as_slice()),
            "DOS translated record {} moved after pointer planning",
            record.translation.entry_id
        );
        ensure!(
            output[record.source.file_offset..record.source.file_offset + record.source.byte_size]
                == neutralized_source_record(record.source.byte_size)?,
            "DOS source record {} was not neutralized",
            record.source.id
        );
    }

    Ok(PatchedDosText {
        bytes: output,
        pointer_reference_count: reference_offsets.len(),
        translated_text_byte_count: appended.len(),
        output_size,
    })
}

pub(super) fn encode_output_parts(
    id: &str,
    parts: &[DosOutputPart],
    codebook: &GaijiCodebook,
) -> Result<Vec<u8>> {
    ensure!(
        matches!(parts.last(), Some(DosOutputPart::Terminator)),
        "DOS translation {id} has no final terminator"
    );
    ensure!(
        parts[..parts.len() - 1]
            .iter()
            .all(|part| !matches!(part, DosOutputPart::Terminator)),
        "DOS translation {id} has an early terminator"
    );
    let mut output = Vec::new();
    for part in parts {
        match part {
            DosOutputPart::Text(text) => {
                for character in text.chars() {
                    if let Some(slot) = codebook.get(&character) {
                        output.extend_from_slice(&slot.shift_jis_code.to_be_bytes());
                        continue;
                    }
                    let mut utf8 = [0_u8; 4];
                    let encoded_text = character.encode_utf8(&mut utf8);
                    let (encoded, _, had_errors) = SHIFT_JIS.encode(encoded_text);
                    ensure!(
                        !had_errors,
                        "DOS translation {id} has no glyph mapping for {character:?}"
                    );
                    ensure!(
                        !encoded.contains(&b'$'),
                        "DOS translation {id} contains the display terminator as text"
                    );
                    output.extend_from_slice(&encoded);
                }
            }
            DosOutputPart::Bytes(bytes) => {
                ensure!(
                    !bytes.contains(&b'$'),
                    "DOS translation {id} protected bytes contain an early terminator"
                );
                output.extend_from_slice(bytes);
            }
            DosOutputPart::Terminator => output.push(b'$'),
        }
    }
    Ok(output)
}

fn neutralized_source_record(byte_size: usize) -> Result<Vec<u8>> {
    ensure!(byte_size > 0, "DOS source record cannot be empty");
    let mut bytes = vec![0_u8; byte_size];
    bytes[0] = b'$';
    Ok(bytes)
}

fn assemble_mov_dx(file_offset: usize, address: u16) -> Result<v30::AssembledProgram> {
    let runtime_address = file_offset
        .checked_add(COM_ORIGIN)
        .context("DOS pointer instruction address overflow")?;
    let runtime_address = u16::try_from(runtime_address)
        .context("DOS pointer instruction exceeds the COM address space")?;
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::DX),
        src: Operand::Imm16(address),
    });
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: runtime_address,
        })
        .context("assemble typed DOS text pointer")
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod text_tests;
