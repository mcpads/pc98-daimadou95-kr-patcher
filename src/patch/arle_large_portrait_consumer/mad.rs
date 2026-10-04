use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    WriteIntent, WritePlan,
};
use v30::{
    Assembler, CodeLocation, EffectiveAddress, EffectiveAddressBase, EffectiveAddressDisplacement,
    Instruction, Operand, OperandSize, Register8, Register16, SegmentRegister, ShiftCount,
};

use crate::patch::apply_v30_patch_plan;

pub(super) const MAD_BANK_ALLOCATION_FILE_OFFSET: usize = 0x012f;
const MAD_BANK_ALLOCATION_RUNTIME_ADDRESS: u16 = 0x022f;
const MAD_TILE_ID_ADDRESS_FILE_OFFSET: usize = 0xb924;
const MAD_TILE_ID_ADDRESS_RUNTIME_ADDRESS: u16 = 0xba24;
const MAD_TILE_MAP_CURSOR_ADDRESS: u16 = 0xc30b;
const SOURCE_BANK_PARAGRAPHS: u16 = 0x0400;
const EXPANDED_BANK_PARAGRAPHS: u16 = 0x0800;
const ALLOCATION_SOURCE_ID: &str = "mad-large-portrait-bank-allocation";

pub(super) fn expand_portrait_bank_allocation(bytes: &[u8]) -> Result<Vec<u8>> {
    let original = assemble_bank_allocation(SOURCE_BANK_PARAGRAPHS)?;
    ensure!(
        bytes.get(
            MAD_BANK_ALLOCATION_FILE_OFFSET
                ..MAD_BANK_ALLOCATION_FILE_OFFSET + original.bytes().len()
        ) == Some(original.bytes()),
        "MAD large-portrait allocation differs from the verified typed V30 source"
    );
    validate_tile_id_address(bytes)?;
    let replacement = assemble_bank_allocation(EXPANDED_BANK_PARAGRAPHS)?;
    ensure!(
        replacement.bytes().len() == original.bytes().len(),
        "MAD large-portrait allocation changed instruction width"
    );

    let plan = WritePlan::new()
        .region(ImageRegion {
            id: "mad-large-portrait-bank-allocation".into(),
            range: MAD_BANK_ALLOCATION_FILE_OFFSET
                ..MAD_BANK_ALLOCATION_FILE_OFFSET + original.bytes().len(),
            kind: RegionKind::MachineCode,
            reason: "reserve 32 KiB for all 256 addressable large-portrait tiles".into(),
        })
        .write(ExpectedWrite {
            id: "expand-mad-large-portrait-bank-allocation".into(),
            owner: "arle-large-portrait-consumer-patch".into(),
            purpose: "keep the following opponent portrait allocation outside Arle's expanded bank"
                .into(),
            offset: MAD_BANK_ALLOCATION_FILE_OFFSET,
            expected_original: original.into_bytes(),
            replacement: replacement.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: ALLOCATION_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        });
    let replacement_source = replacement.clone();
    Ok(apply_v30_patch_plan(
        bytes,
        &plan,
        move |source_id| match source_id {
            ALLOCATION_SOURCE_ID => Ok(replacement_source.clone()),
            _ => Err(MachineCodeVerifierError::new(format!(
                "unknown MAD large-portrait V30 source {source_id}"
            ))),
        },
    )?)
}

fn validate_tile_id_address(bytes: &[u8]) -> Result<()> {
    let expected = assemble_tile_id_address()?;
    let source = bytes
        .get(MAD_TILE_ID_ADDRESS_FILE_OFFSET..)
        .context("MAD.COM is shorter than the large-portrait tile-ID address calculation")?;
    let mut consumed = 0;

    for expected_span in expected.instruction_spans() {
        let decoded = v30::decode_bytes(
            source
                .get(consumed..)
                .context("MAD large-portrait tile-ID address calculation is truncated")?,
        )
        .context("decode MAD large-portrait tile-ID address calculation as typed V30")?;
        ensure!(
            decoded.instruction == expected_span.instruction,
            "MAD large-portrait tile-ID address calculation differs from the verified typed V30 source"
        );
        consumed += decoded.byte_len;
    }

    Ok(())
}

fn assemble_tile_id_address() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Xor {
            dest: Operand::Reg16(Register16::AX),
            src: Operand::Reg16(Register16::AX),
        })
        .emit(Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Mem(EffectiveAddress::new(
                Some(SegmentRegister::CS),
                EffectiveAddressBase::Bx,
                EffectiveAddressDisplacement::Signed(0),
                OperandSize::Byte,
            )?),
        })
        .emit(Instruction::Inc {
            dest: Operand::Reg16(Register16::BX),
        })
        .emit(Instruction::Mov {
            dest: Operand::Mem(EffectiveAddress::new(
                Some(SegmentRegister::CS),
                EffectiveAddressBase::Direct,
                EffectiveAddressDisplacement::Absolute(MAD_TILE_MAP_CURSOR_ADDRESS),
                OperandSize::Word,
            )?),
            src: Operand::Reg16(Register16::BX),
        })
        .emit(Instruction::Shr {
            dest: Operand::Reg16(Register16::AX),
            count: ShiftCount::One,
        })
        .emit(Instruction::Mov {
            dest: Operand::Reg16(Register16::SI),
            src: Operand::Reg16(Register16::AX),
        });
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: MAD_TILE_ID_ADDRESS_RUNTIME_ADDRESS,
        })
        .context("assemble typed V30 MAD 8-bit portrait tile address")
}

fn assemble_bank_allocation(paragraphs: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::BX),
        src: Operand::Imm16(paragraphs),
    });
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: MAD_BANK_ALLOCATION_RUNTIME_ADDRESS,
        })
        .context("assemble typed V30 MAD large-portrait allocation")
}

#[cfg(test)]
#[path = "mad_tests.rs"]
mod mad_tests;
