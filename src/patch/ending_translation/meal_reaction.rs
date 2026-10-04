use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    WriteIntent, WritePlan,
};
use v30::{Assembler, CodeLocation, Instruction, Operand, Register16};

use crate::game_data::ending_ingredient_labels::BLANK_GLYPH_INDEX;
use crate::game_data::ending_meal_reaction::{
    COM_ORIGIN, GLYPH_LOAD_FILE_OFFSETS, GLYPH_SLOT_COUNT, parse_ending_meal_reaction,
};
use crate::patch::apply_v30_patch_plan;

#[derive(Debug, Eq, PartialEq)]
pub struct EndingMealReactionPatchReport {
    pub glyph_load_file_offsets: [usize; GLYPH_SLOT_COUNT],
    pub glyph_slot_count: usize,
    pub translated_glyph_count: usize,
}

#[derive(Debug)]
pub(super) struct PatchedEndingMealReaction {
    pub bytes: Vec<u8>,
    pub report: EndingMealReactionPatchReport,
}

pub(super) fn replace_ending_meal_reaction(
    bytes: &[u8],
    translation: &str,
    codebook: &BTreeMap<char, u8>,
) -> Result<PatchedEndingMealReaction> {
    let catalog = parse_ending_meal_reaction(bytes)?;
    let translated_glyph_count = translation.chars().count();
    ensure!(
        translated_glyph_count > 0 && translated_glyph_count <= GLYPH_SLOT_COUNT,
        "ENDING meal reaction uses {translated_glyph_count} glyphs but its consumer requires 1..={GLYPH_SLOT_COUNT}"
    );
    let mut replacement_indices = [BLANK_GLYPH_INDEX; GLYPH_SLOT_COUNT];
    for (slot, character) in translation.chars().enumerate() {
        let index = codebook
            .get(&character)
            .copied()
            .with_context(|| format!("ENDING meal-reaction codebook is missing {character:?}"))?;
        ensure!(
            index != BLANK_GLYPH_INDEX && index != 0xfe && index != 0xff,
            "ENDING meal reaction maps {character:?} to a reserved glyph"
        );
        replacement_indices[slot] = index;
    }

    let mut plan = WritePlan::new();
    let mut replacements = Vec::with_capacity(GLYPH_SLOT_COUNT);
    for (load, replacement_index) in catalog.glyph_loads.iter().zip(replacement_indices) {
        let source_id = format!("ending-meal-reaction-glyph-slot-{:02}", load.slot_index + 1);
        let replacement_program = assemble_glyph_load(load.file_offset, replacement_index)?;
        let replacement = replacement_program.bytes().to_vec();
        ensure!(
            replacement.len() == load.bytes.len(),
            "ENDING meal-reaction glyph slot {} changed instruction width",
            load.slot_index + 1
        );
        plan = plan
            .region(ImageRegion {
                id: format!("{source_id}-instruction"),
                range: load.file_offset..load.file_offset + load.bytes.len(),
                kind: RegionKind::MachineCode,
                reason: "typed SI glyph source for the progressive meal reaction".into(),
            })
            .write(ExpectedWrite {
                id: format!("write-{source_id}"),
                owner: "ending-meal-reaction-compiler".into(),
                purpose: "replace one consumer-reachable meal-reaction glyph source with tracked Korean text".into(),
                offset: load.file_offset,
                expected_original: load.bytes.to_vec(),
                replacement,
                intent: WriteIntent::MachineCode(MachineCodeProvenance {
                    assembly_source_id: source_id.clone(),
                    isa_profile_id: v30::PROFILE_ID.into(),
                }),
            });
        replacements.push((source_id, replacement_program));
    }

    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| {
        replacements
            .iter()
            .find(|(candidate, _)| candidate == source_id)
            .map(|(_, program)| program.clone())
            .ok_or_else(|| {
                MachineCodeVerifierError::new(format!(
                    "unknown ENDING meal-reaction V30 source {source_id}"
                ))
            })
    })?;
    for (file_offset, replacement_index) in
        GLYPH_LOAD_FILE_OFFSETS.into_iter().zip(replacement_indices)
    {
        ensure!(
            output.get(file_offset..file_offset + 3) == Some(&[0xbe, replacement_index, 0x00]),
            "ENDING meal-reaction glyph source did not survive its Expected Write at {file_offset:#x}"
        );
    }

    Ok(PatchedEndingMealReaction {
        bytes: output,
        report: EndingMealReactionPatchReport {
            glyph_load_file_offsets: GLYPH_LOAD_FILE_OFFSETS,
            glyph_slot_count: GLYPH_SLOT_COUNT,
            translated_glyph_count,
        },
    })
}

fn assemble_glyph_load(file_offset: usize, glyph_index: u8) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::SI),
        src: Operand::Imm16(u16::from(glyph_index)),
    });
    let runtime_offset = file_offset
        .checked_add(COM_ORIGIN)
        .context("ENDING meal-reaction runtime address overflow")?;
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: u16::try_from(runtime_offset)
                .context("ENDING meal-reaction runtime address exceeds 16 bits")?,
        })
        .context("assemble typed V30 ENDING meal-reaction glyph load")
}

#[cfg(test)]
#[path = "meal_reaction_tests.rs"]
mod meal_reaction_tests;
