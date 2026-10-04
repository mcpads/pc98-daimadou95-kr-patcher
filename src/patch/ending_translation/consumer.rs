use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    WriteIntent, WritePlan,
};
use v30::{Assembler, CodeLocation, Instruction, Operand, Register16};

use crate::patch::apply_v30_patch_plan;

const COM_ORIGIN: usize = 0x100;
pub(super) const INTERLUDE_FONT_LOAD_FILE_OFFSET: usize = 0x24bc;
pub(super) const PASSWORD_FONT_LOAD_FILE_OFFSET: usize = 0x3be1;
const SOURCE_FONT_ENTRY_ID: u16 = 6;
const ENDING_FONT_ENTRY_ID: u16 = 156;
const INTERLUDE_SOURCE_ID: &str = "ending-interlude-font-bank-selector";
const PASSWORD_SOURCE_ID: &str = "ending-password-font-bank-selector";

pub(super) struct PatchedEndingConsumer {
    pub bytes: Vec<u8>,
    pub font_load_file_offsets: [usize; 2],
}

pub(super) fn select_ending_font_bank(bytes: &[u8]) -> Result<PatchedEndingConsumer> {
    let selectors = [
        (INTERLUDE_FONT_LOAD_FILE_OFFSET, INTERLUDE_SOURCE_ID),
        (PASSWORD_FONT_LOAD_FILE_OFFSET, PASSWORD_SOURCE_ID),
    ];
    let mut replacements = Vec::with_capacity(selectors.len());
    let mut plan = WritePlan::new();
    for (file_offset, source_id) in selectors {
        let expected_original = assemble_font_load(file_offset, SOURCE_FONT_ENTRY_ID)?.into_bytes();
        let replacement_program = assemble_font_load(file_offset, ENDING_FONT_ENTRY_ID)?;
        ensure!(
            bytes.get(file_offset..file_offset + expected_original.len())
                == Some(expected_original.as_slice()),
            "ENDING font-load instruction differs from the verified typed V30 source at {file_offset:#x}"
        );
        let replacement = replacement_program.bytes().to_vec();
        ensure!(
            replacement.len() == expected_original.len(),
            "ENDING font-bank selector changed instruction width at {file_offset:#x}"
        );
        let write_len = replacement.len();
        plan = plan
            .region(ImageRegion {
                id: format!("{source_id}-instruction"),
                range: file_offset..file_offset + write_len,
                kind: RegionKind::MachineCode,
                reason: "typed MOV selects the isolated ending font bank".into(),
            })
            .write(ExpectedWrite {
                id: format!("write-{source_id}"),
                owner: "ending-font-consumer-patch".into(),
                purpose: "load appended MADDAT entry 156 instead of shared entry 6".into(),
                offset: file_offset,
                expected_original,
                replacement,
                intent: WriteIntent::MachineCode(MachineCodeProvenance {
                    assembly_source_id: source_id.into(),
                    isa_profile_id: v30::PROFILE_ID.into(),
                }),
            });
        replacements.push((source_id, replacement_program));
    }

    let patched = apply_v30_patch_plan(bytes, &plan, move |source_id| {
        replacements
            .iter()
            .find(|(candidate, _)| *candidate == source_id)
            .map(|(_, program)| program.clone())
            .ok_or_else(|| {
                MachineCodeVerifierError::new(format!("unknown ENDING V30 source {source_id}"))
            })
    })?;
    Ok(PatchedEndingConsumer {
        bytes: patched,
        font_load_file_offsets: [
            INTERLUDE_FONT_LOAD_FILE_OFFSET,
            PASSWORD_FONT_LOAD_FILE_OFFSET,
        ],
    })
}

fn assemble_font_load(file_offset: usize, entry_id: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::AX),
        src: Operand::Imm16(entry_id),
    });
    let runtime_offset = file_offset
        .checked_add(COM_ORIGIN)
        .context("ENDING font-load runtime address overflow")?;
    let runtime_offset = u16::try_from(runtime_offset)
        .context("ENDING font-load runtime address exceeds 16 bits")?;
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: runtime_offset,
        })
        .context("assemble typed V30 ENDING font-load instruction")
}
