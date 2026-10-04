use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    WriteIntent, WritePlan,
};
use v30::{Assembler, CodeLocation, Instruction, Operand, Register16};

use crate::source::{MadIndexedString, MadIndexedTextCatalog};

use super::bank_plan::{MadFontPlan, battle_unit_number};
use super::{MadTextPoolReport, MadTranslation};
use crate::patch::apply_v30_patch_plan;

const COM_ORIGIN: usize = 0x100;
const TERMINATOR: u8 = 0xff;

pub(super) struct PatchedMadText {
    pub bytes: Vec<u8>,
    pub pool_reports: Vec<MadTextPoolReport>,
    pub unit_ranges: BTreeMap<u8, Range<u16>>,
    pub pointer_reference_count: usize,
    pub changed_pointer_reference_count: usize,
}

struct PointerReplacement {
    file_offset: usize,
    expected_runtime_address: u16,
    replacement_runtime_address: u16,
    machine_code: bool,
}

pub(super) fn replace_mad_indexed_text(
    bytes: &[u8],
    source: &MadIndexedTextCatalog,
    translations: &[MadTranslation],
    font_plan: &MadFontPlan,
) -> Result<PatchedMadText> {
    ensure!(
        source.file_size == bytes.len(),
        "MAD indexed-text catalog belongs to a {}-byte file, not {} bytes",
        source.file_size,
        bytes.len()
    );
    let translation_by_record = translations
        .iter()
        .map(|translation| (translation.record_id.as_str(), translation))
        .collect::<BTreeMap<_, _>>();
    let mut consumed_translations = BTreeSet::new();
    let mut relocated_addresses = BTreeMap::<usize, u16>::new();
    let mut unit_ranges = BTreeMap::<u8, Range<u16>>::new();
    let mut pool_replacements = Vec::new();
    let mut pool_reports = Vec::new();

    for pool in &source.pools {
        let pool_strings = source
            .strings
            .iter()
            .filter(|string| {
                string.file_offset >= pool.file_start && string.file_offset < pool.file_end
            })
            .collect::<Vec<_>>();
        let capacity = pool.file_end - pool.file_start;
        let mut replacement = vec![TERMINATOR; capacity];
        let mut cursor = 0_usize;
        for string in pool_strings {
            let new_runtime_address = pool
                .runtime_start
                .checked_add(cursor)
                .context("MAD translated text runtime address overflow")?;
            relocated_addresses.insert(
                string.runtime_address,
                u16::try_from(new_runtime_address)
                    .context("MAD translated text address exceeds 16 bits")?,
            );
            let (encoded, unit_number) = encode_string(
                bytes,
                string,
                &translation_by_record,
                font_plan,
                &mut consumed_translations,
            )?;
            let end = cursor
                .checked_add(encoded.len())
                .context("MAD translated text-pool cursor overflow")?;
            ensure!(
                end <= capacity,
                "MAD text pool {} requires {end} bytes but has {capacity}",
                pool.id
            );
            replacement[cursor..end].copy_from_slice(&encoded);
            if let Some(unit_number) = unit_number {
                let start = u16::try_from(new_runtime_address)
                    .context("MAD unit text start exceeds 16 bits")?;
                let end_address = u16::try_from(pool.runtime_start + end)
                    .context("MAD unit text end exceeds 16 bits")?;
                unit_ranges
                    .entry(unit_number)
                    .and_modify(|range| {
                        range.start = range.start.min(start);
                        range.end = range.end.max(end_address);
                    })
                    .or_insert(start..end_address);
            }
            cursor = end;
        }
        pool_reports.push(MadTextPoolReport {
            id: pool.id.clone(),
            file_offset: pool.file_start,
            capacity,
            used: cursor,
        });
        pool_replacements.push((pool, replacement));
    }
    ensure!(
        consumed_translations.len() == translations.len(),
        "MAD text compiler consumed {} of {} translations",
        consumed_translations.len(),
        translations.len()
    );
    ensure!(
        unit_ranges.keys().copied().collect::<BTreeSet<_>>()
            == font_plan
                .unit_font_entry_ids
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
        "MAD translated unit ranges and font-bank assignments differ"
    );

    let pointer_replacements = collect_pointer_replacements(bytes, source, &relocated_addresses)?;
    let pointer_reference_count = source
        .bindings
        .iter()
        .flat_map(|binding| &binding.reference_file_offsets)
        .collect::<BTreeSet<_>>()
        .len();
    ensure!(
        pointer_replacements.len() == pointer_reference_count,
        "MAD pointer replacement denominator changed"
    );
    let changed_pointer_reference_count = pointer_replacements
        .iter()
        .filter(|pointer| pointer.expected_runtime_address != pointer.replacement_runtime_address)
        .count();

    let mut plan = WritePlan::new();
    for (pool, replacement) in &pool_replacements {
        plan = plan
            .region(ImageRegion {
                id: format!("mad-indexed-pool-{}", pool.id),
                range: pool.file_start..pool.file_end,
                kind: RegionKind::Data,
                reason: format!(
                    "translated MAD indexed strings and neutralized tail for {}",
                    pool.id
                ),
            })
            .write(ExpectedWrite {
                id: format!("write-mad-indexed-pool-{}", pool.id),
                owner: "mad-indexed-text-compiler".into(),
                purpose: format!("repack the complete {} string pool", pool.id),
                offset: pool.file_start,
                expected_original: bytes[pool.file_start..pool.file_end].to_vec(),
                replacement: replacement.clone(),
                intent: WriteIntent::Data,
            });
    }

    let mut machine_sources = BTreeMap::new();
    for pointer in &pointer_replacements {
        if pointer.expected_runtime_address == pointer.replacement_runtime_address {
            continue;
        }
        let source_id = format!("mad-indexed-pointer-{:04x}", pointer.file_offset);
        if pointer.machine_code {
            let expected = assemble_mov_si(pointer.file_offset, pointer.expected_runtime_address)?;
            let replacement =
                assemble_mov_si(pointer.file_offset, pointer.replacement_runtime_address)?;
            ensure!(
                bytes.get(pointer.file_offset..pointer.file_offset + expected.bytes().len())
                    == Some(expected.bytes()),
                "MAD MOV SI source differs at {:#x}",
                pointer.file_offset
            );
            let write_len = replacement.bytes().len();
            plan = plan
                .region(ImageRegion {
                    id: format!("{source_id}-instruction"),
                    range: pointer.file_offset..pointer.file_offset + write_len,
                    kind: RegionKind::MachineCode,
                    reason: "typed MAD translated-string pointer immediate".into(),
                })
                .write(ExpectedWrite {
                    id: format!("write-{source_id}"),
                    owner: "mad-indexed-pointer-compiler".into(),
                    purpose: "retarget MOV SI to the repacked MAD string".into(),
                    offset: pointer.file_offset,
                    expected_original: expected.into_bytes(),
                    replacement: replacement.bytes().to_vec(),
                    intent: WriteIntent::MachineCode(MachineCodeProvenance {
                        assembly_source_id: source_id.clone(),
                        isa_profile_id: v30::PROFILE_ID.into(),
                    }),
                });
            machine_sources.insert(source_id, replacement);
        } else {
            plan = plan
                .region(ImageRegion {
                    id: format!("{source_id}-metadata"),
                    range: pointer.file_offset..pointer.file_offset + 2,
                    kind: RegionKind::Metadata,
                    reason: "MAD table pointer to a repacked indexed string".into(),
                })
                .write(ExpectedWrite {
                    id: format!("write-{source_id}"),
                    owner: "mad-indexed-pointer-compiler".into(),
                    purpose: "retarget the MAD indexed-string table entry".into(),
                    offset: pointer.file_offset,
                    expected_original: pointer.expected_runtime_address.to_le_bytes().to_vec(),
                    replacement: pointer.replacement_runtime_address.to_le_bytes().to_vec(),
                    intent: WriteIntent::Metadata,
                });
        }
    }

    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| {
        machine_sources.get(source_id).cloned().ok_or_else(|| {
            MachineCodeVerifierError::new(format!("unknown MAD pointer source {source_id}"))
        })
    })?;
    verify_repacked_text(
        &output,
        &pool_replacements,
        &pointer_replacements,
        changed_pointer_reference_count,
    )?;

    Ok(PatchedMadText {
        bytes: output,
        pool_reports,
        unit_ranges,
        pointer_reference_count,
        changed_pointer_reference_count,
    })
}

fn encode_string(
    bytes: &[u8],
    string: &MadIndexedString,
    translations: &BTreeMap<&str, &MadTranslation>,
    font_plan: &MadFontPlan,
    consumed: &mut BTreeSet<String>,
) -> Result<(Vec<u8>, Option<u8>)> {
    if string.id == "mad-indexed-81c2" || string.id == "mad-indexed-81d1" {
        let banner = translations
            .get("result-banner")
            .context("MAD result banner translation is missing")?;
        consumed.insert(banner.record_id.clone());
        let encoded = if string.id == "mad-indexed-81c2" {
            font_plan.result_banner_top.clone()
        } else {
            font_plan.result_banner_bottom.clone()
        };
        return Ok((encoded, None));
    }
    if let Some(translation) = translations.get(string.id.as_str()) {
        consumed.insert(translation.record_id.clone());
        let unit_number = battle_unit_number(&translation.segment_id)?;
        let codebook = unit_number
            .and_then(|unit| font_plan.unit_codebooks.get(&unit))
            .unwrap_or(&font_plan.base_codebook);
        return Ok((
            encode_text(&translation.entry_id, &translation.text, codebook)?,
            unit_number,
        ));
    }
    if string.bound_to_consumer && !string.contains_japanese {
        let text = string
            .text
            .as_deref()
            .with_context(|| format!("MAD support string {} has undecoded glyphs", string.id))?;
        return Ok((
            encode_text(&string.id, text, &font_plan.base_codebook)?,
            None,
        ));
    }
    ensure!(
        !string.bound_to_consumer,
        "MAD bound Japanese string {} has no translation",
        string.id
    );
    Ok((
        bytes[string.file_offset..string.file_offset + string.byte_size].to_vec(),
        None,
    ))
}

fn encode_text(id: &str, text: &str, codebook: &BTreeMap<char, u8>) -> Result<Vec<u8>> {
    let mut encoded = Vec::with_capacity(text.chars().count() + 1);
    for character in text.chars() {
        let index = codebook
            .get(&character)
            .copied()
            .with_context(|| format!("MAD string {id} codebook is missing {character:?}"))?;
        ensure!(
            index != TERMINATOR,
            "MAD string {id} maps {character:?} to the terminator"
        );
        encoded.push(index);
    }
    encoded.push(TERMINATOR);
    Ok(encoded)
}

fn collect_pointer_replacements(
    bytes: &[u8],
    source: &MadIndexedTextCatalog,
    relocated: &BTreeMap<usize, u16>,
) -> Result<Vec<PointerReplacement>> {
    let mut pointers = BTreeMap::<usize, PointerReplacement>::new();
    for binding in &source.bindings {
        for &file_offset in &binding.reference_file_offsets {
            let metadata_address = usize::from(read_u16(bytes, file_offset)?);
            let instruction_address = (bytes.get(file_offset) == Some(&0xbe))
                .then(|| read_u16(bytes, file_offset + 1))
                .transpose()?
                .map(usize::from);
            let metadata_matches = relocated.contains_key(&metadata_address);
            let instruction_matches =
                instruction_address.is_some_and(|address| relocated.contains_key(&address));
            ensure!(
                metadata_matches ^ instruction_matches,
                "MAD binding {} reference at {file_offset:#x} has ambiguous or unknown pointer representation",
                binding.id
            );
            let machine_code = instruction_matches;
            let value_offset = file_offset + usize::from(machine_code);
            let expected_runtime_address = read_u16(bytes, value_offset)?;
            let replacement_runtime_address = *relocated
                .get(&usize::from(expected_runtime_address))
                .with_context(|| {
                    format!(
                        "MAD binding {} pointer {expected_runtime_address:#x} has no relocated string",
                        binding.id
                    )
                })?;
            let replacement = PointerReplacement {
                file_offset,
                expected_runtime_address,
                replacement_runtime_address,
                machine_code,
            };
            if let Some(existing) = pointers.get(&file_offset) {
                ensure!(
                    existing.expected_runtime_address == replacement.expected_runtime_address
                        && existing.replacement_runtime_address
                            == replacement.replacement_runtime_address
                        && existing.machine_code == replacement.machine_code,
                    "MAD bindings disagree about pointer at {file_offset:#x}"
                );
            } else {
                pointers.insert(file_offset, replacement);
            }
        }
    }
    Ok(pointers.into_values().collect())
}

fn assemble_mov_si(file_offset: usize, runtime_address: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::SI),
        src: Operand::Imm16(runtime_address),
    });
    let origin = u16::try_from(file_offset + COM_ORIGIN)
        .context("MAD MOV SI runtime location exceeds 16 bits")?;
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: origin,
        })
        .context("assemble typed MAD string pointer")
}

fn verify_repacked_text(
    output: &[u8],
    pools: &[(&crate::source::MadIndexedTextPool, Vec<u8>)],
    pointers: &[PointerReplacement],
    changed_pointer_count: usize,
) -> Result<()> {
    for (pool, replacement) in pools {
        ensure!(
            output[pool.file_start..pool.file_end] == **replacement,
            "MAD text pool {} did not survive its Expected Write",
            pool.id
        );
    }
    let mut observed_changes = 0;
    for pointer in pointers {
        let value_offset = pointer.file_offset + usize::from(pointer.machine_code);
        ensure!(
            read_u16(output, value_offset)? == pointer.replacement_runtime_address,
            "MAD pointer at {:#x} did not retain its relocated address",
            pointer.file_offset
        );
        observed_changes +=
            usize::from(pointer.expected_runtime_address != pointer.replacement_runtime_address);
    }
    ensure!(
        observed_changes == changed_pointer_count,
        "MAD changed-pointer verification denominator drifted"
    );
    Ok(())
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let raw: [u8; 2] = bytes
        .get(offset..offset + 2)
        .with_context(|| format!("MAD pointer is truncated at {offset:#x}"))?
        .try_into()
        .expect("a two-byte range converts to two bytes");
    Ok(u16::from_le_bytes(raw))
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod text_tests;
