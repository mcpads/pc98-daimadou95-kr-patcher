use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    WriteIntent, WritePlan,
};
use v30::{Assembler, CodeLocation, Instruction, Operand, Register16};

use crate::patch::apply_v30_patch_plan;

const COM_ORIGIN: usize = 0x100;
pub(super) const FONT_LOAD_FILE_OFFSET: usize = 0x28d7;
const SOURCE_FONT_ENTRY_ID: u16 = 6;
const OPENING_FONT_ENTRY_ID: u16 = 155;
const ASSEMBLY_SOURCE_ID: &str = "opening-font-bank-selector";

pub(super) struct PatchedOpeningConsumer {
    pub bytes: Vec<u8>,
    pub file_offset: usize,
}

pub(super) fn select_opening_font_bank(bytes: &[u8]) -> Result<PatchedOpeningConsumer> {
    let expected_original = assemble_font_load(SOURCE_FONT_ENTRY_ID)?.into_bytes();
    let replacement_program = assemble_font_load(OPENING_FONT_ENTRY_ID)?;
    ensure!(
        bytes.get(FONT_LOAD_FILE_OFFSET..FONT_LOAD_FILE_OFFSET + expected_original.len())
            == Some(expected_original.as_slice()),
        "OPENING font-load instruction differs from the verified typed V30 source"
    );
    let replacement = replacement_program.bytes().to_vec();
    let write_len = replacement.len();
    ensure!(
        write_len == expected_original.len(),
        "OPENING font-bank selector changed instruction width"
    );
    let plan = WritePlan::new()
        .region(ImageRegion {
            id: "opening-font-load-instruction".into(),
            range: FONT_LOAD_FILE_OFFSET..FONT_LOAD_FILE_OFFSET + write_len,
            kind: RegionKind::MachineCode,
            reason: "typed MOV selects the isolated opening font bank".into(),
        })
        .write(ExpectedWrite {
            id: "select-opening-font-bank".into(),
            owner: "opening-font-consumer-patch".into(),
            purpose: "load appended MADDAT entry 155 instead of shared entry 6".into(),
            offset: FONT_LOAD_FILE_OFFSET,
            expected_original,
            replacement,
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: ASSEMBLY_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        });
    let bytes = apply_v30_patch_plan(bytes, &plan, move |source_id| {
        if source_id == ASSEMBLY_SOURCE_ID {
            Ok(replacement_program.clone())
        } else {
            Err(MachineCodeVerifierError::new(format!(
                "unknown opening V30 source {source_id}"
            )))
        }
    })?;
    Ok(PatchedOpeningConsumer {
        bytes,
        file_offset: FONT_LOAD_FILE_OFFSET,
    })
}

fn assemble_font_load(entry_id: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::AX),
        src: Operand::Imm16(entry_id),
    });
    let runtime_offset = FONT_LOAD_FILE_OFFSET
        .checked_add(COM_ORIGIN)
        .context("OPENING font-load runtime address overflow")?;
    let runtime_offset = u16::try_from(runtime_offset)
        .context("OPENING font-load runtime address exceeds 16 bits")?;
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: runtime_offset,
        })
        .context("assemble typed V30 OPENING font-load instruction")
}
