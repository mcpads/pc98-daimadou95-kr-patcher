use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    WriteIntent, WritePlan,
};
use v30::{
    Assembler, CallTarget, CodeLocation, EffectiveAddress, EffectiveAddressBase,
    EffectiveAddressDisplacement, Instruction, Operand, OperandSize, PortAddress, Register8,
    Register16, SegmentRegister, encode_bytes,
};

use super::apply_v30_patch_plan;

pub(super) const PACKED_STAGING_CAPACITY: usize = 0x8000;
pub(super) const PACKED_DESTINATION_FILE_OFFSET: usize = 0x2556;
pub(super) const UNPACK_BLOCK_FILE_OFFSET: usize = 0x256b;

const PACKED_DESTINATION_RUNTIME_ADDRESS: u16 = 0x2656;
const UNPACK_BLOCK_RUNTIME_ADDRESS: u16 = 0x266b;
const DECOMPRESS_RUNTIME_ADDRESS: u16 = 0x215f;
const VRAM_COPY_RUNTIME_ADDRESS: u16 = 0x31da;
const OUTPUT_SEGMENT_WORD_ADDRESS: u16 = 0x3d98;
const STAGING_SEGMENT_WORD_ADDRESS: u16 = 0x3d9a;
const PACKED_DESTINATION_SOURCE_ID: &str = "ending-meal-packed-staging-segment";
const UNPACK_BLOCK_SOURCE_ID: &str = "ending-meal-memory-unpack-block";

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PatchedEndingMealLoader {
    pub bytes: Vec<u8>,
    pub packed_staging_capacity: usize,
    pub unpack_block_byte_size: usize,
}

pub(super) fn install_ending_meal_bank_loader(
    bytes: &[u8],
    packed_entry_size: usize,
) -> Result<PatchedEndingMealLoader> {
    ensure!(
        packed_entry_size <= PACKED_STAGING_CAPACITY,
        "ending meal packed tile banks require {packed_entry_size} bytes but the reusable staging segment has {PACKED_STAGING_CAPACITY}"
    );

    let source_destination = assemble_packed_destination(OUTPUT_SEGMENT_WORD_ADDRESS)?;
    let staged_destination = assemble_packed_destination(STAGING_SEGMENT_WORD_ADDRESS)?;
    verify_source_bytes(
        bytes,
        PACKED_DESTINATION_FILE_OFFSET,
        &source_destination,
        "ENDING meal packed-data destination",
    )?;

    let source_unpack = assemble_source_unpack_block()?;
    let memory_unpack = assemble_memory_unpack_block(source_unpack.bytes().len())?;
    verify_source_bytes(
        bytes,
        UNPACK_BLOCK_FILE_OFFSET,
        &source_unpack,
        "ENDING meal unpack and VRAM-copy block",
    )?;
    ensure!(
        memory_unpack.bytes().len() == source_unpack.bytes().len(),
        "ENDING meal memory unpack must preserve the original block width"
    );

    let plan = WritePlan::new()
        .region(ImageRegion {
            id: "ending-meal-packed-staging-segment".into(),
            range: PACKED_DESTINATION_FILE_OFFSET
                ..PACKED_DESTINATION_FILE_OFFSET + source_destination.bytes().len(),
            kind: RegionKind::MachineCode,
            reason: "load the packed replacement into the existing 32 KiB temporary segment"
                .into(),
        })
        .region(ImageRegion {
            id: "ending-meal-memory-unpack-block".into(),
            range: UNPACK_BLOCK_FILE_OFFSET
                ..UNPACK_BLOCK_FILE_OFFSET + source_unpack.bytes().len(),
            kind: RegionKind::MachineCode,
            reason: "unpack entry 145 from its temporary segment directly into the 64 KiB retained tile segment"
                .into(),
        })
        .write(ExpectedWrite {
            id: "select-ending-meal-packed-staging-segment".into(),
            owner: "arle-ending-meal-loader-patch".into(),
            purpose: "keep packed bytes separate from the retained decoded tile banks".into(),
            offset: PACKED_DESTINATION_FILE_OFFSET,
            expected_original: source_destination.into_bytes(),
            replacement: staged_destination.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: PACKED_DESTINATION_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "unpack-ending-meal-directly-to-retained-segment".into(),
            owner: "arle-ending-meal-loader-patch".into(),
            purpose: "avoid the original 32 KiB VRAM copy truncating the second tile bank".into(),
            offset: UNPACK_BLOCK_FILE_OFFSET,
            expected_original: source_unpack.into_bytes(),
            replacement: memory_unpack.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: UNPACK_BLOCK_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        });

    let staged_destination_source = staged_destination.clone();
    let memory_unpack_source = memory_unpack.clone();
    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| match source_id {
        PACKED_DESTINATION_SOURCE_ID => Ok(staged_destination_source.clone()),
        UNPACK_BLOCK_SOURCE_ID => Ok(memory_unpack_source.clone()),
        _ => Err(MachineCodeVerifierError::new(format!(
            "unknown ENDING meal loader V30 source {source_id}"
        ))),
    })?;

    Ok(PatchedEndingMealLoader {
        bytes: output,
        packed_staging_capacity: PACKED_STAGING_CAPACITY,
        unpack_block_byte_size: memory_unpack.bytes().len(),
    })
}

fn assemble_packed_destination(segment_word_address: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::AX),
        src: direct_operand(None, segment_word_address, OperandSize::Word)?,
    });
    assemble_at(&assembler, PACKED_DESTINATION_RUNTIME_ADDRESS)
}

fn assemble_source_unpack_block() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    let mut byte_len = 0_usize;
    for instruction in [
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::ES),
            src: direct_operand(
                Some(SegmentRegister::CS),
                OUTPUT_SEGMENT_WORD_ADDRESS,
                OperandSize::Word,
            )?,
        },
        Instruction::Mov {
            dest: Operand::Reg16(Register16::DI),
            src: Operand::Imm16(0),
        },
        Instruction::Mov {
            dest: Operand::Reg16(Register16::BX),
            src: Operand::Imm16(0),
        },
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AL),
            src: Operand::Imm8(1),
        },
        Instruction::Mov {
            dest: Operand::Reg8(Register8::CL),
            src: Operand::Imm8(1),
        },
    ] {
        emit(&mut assembler, &mut byte_len, instruction)?;
    }
    emit_external_call(
        &mut assembler,
        &mut byte_len,
        UNPACK_BLOCK_RUNTIME_ADDRESS,
        DECOMPRESS_RUNTIME_ADDRESS,
    )?;
    for instruction in [
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::ES),
            src: direct_operand(
                Some(SegmentRegister::CS),
                OUTPUT_SEGMENT_WORD_ADDRESS,
                OperandSize::Word,
            )?,
        },
        Instruction::Mov {
            dest: Operand::Reg16(Register16::DI),
            src: Operand::Imm16(0),
        },
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AL),
            src: Operand::Imm8(1),
        },
        Instruction::OutAl {
            port: PortAddress::Imm8(0xa6),
        },
        Instruction::Mov {
            dest: Operand::Reg16(Register16::CX),
            src: Operand::Imm16(0x4000),
        },
    ] {
        emit(&mut assembler, &mut byte_len, instruction)?;
    }
    emit_external_call(
        &mut assembler,
        &mut byte_len,
        UNPACK_BLOCK_RUNTIME_ADDRESS,
        VRAM_COPY_RUNTIME_ADDRESS,
    )?;
    assemble_at(&assembler, UNPACK_BLOCK_RUNTIME_ADDRESS)
}

fn assemble_memory_unpack_block(block_byte_size: usize) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    let mut byte_len = 0_usize;
    for instruction in [
        Instruction::Push {
            src: Operand::Sreg(SegmentRegister::DS),
        },
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::ES),
            src: direct_operand(
                Some(SegmentRegister::CS),
                STAGING_SEGMENT_WORD_ADDRESS,
                OperandSize::Word,
            )?,
        },
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::DS),
            src: direct_operand(
                Some(SegmentRegister::CS),
                OUTPUT_SEGMENT_WORD_ADDRESS,
                OperandSize::Word,
            )?,
        },
        Instruction::Mov {
            dest: Operand::Reg16(Register16::DI),
            src: Operand::Imm16(0),
        },
        Instruction::Mov {
            dest: Operand::Reg16(Register16::BX),
            src: Operand::Imm16(0),
        },
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AL),
            src: Operand::Imm8(1),
        },
        Instruction::Mov {
            dest: Operand::Reg8(Register8::CL),
            src: Operand::Imm8(0x10),
        },
    ] {
        emit(&mut assembler, &mut byte_len, instruction)?;
    }
    emit_external_call(
        &mut assembler,
        &mut byte_len,
        UNPACK_BLOCK_RUNTIME_ADDRESS,
        DECOMPRESS_RUNTIME_ADDRESS,
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Pop {
            dest: Operand::Sreg(SegmentRegister::DS),
        },
    )?;
    ensure!(
        byte_len <= block_byte_size,
        "ENDING meal direct-memory unpack exceeds the original block width"
    );
    for _ in byte_len..block_byte_size {
        assembler.emit(Instruction::Nop);
    }
    assemble_at(&assembler, UNPACK_BLOCK_RUNTIME_ADDRESS)
}

fn emit_external_call(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
    target: u16,
) -> Result<()> {
    let site = usize::from(origin)
        .checked_add(*byte_len)
        .context("ENDING meal loader call-site overflow")?;
    let site = u16::try_from(site).context("ENDING meal loader call site exceeds 16 bits")?;
    emit(
        assembler,
        byte_len,
        Instruction::Call {
            target: CallTarget::Rel16(relative_near_displacement(site, target)?),
        },
    )
}

fn relative_near_displacement(current: u16, target: u16) -> Result<i16> {
    let next = i32::from(current) + 3;
    i16::try_from(i32::from(target) - next).context("ENDING meal loader target is out of range")
}

fn emit(assembler: &mut Assembler, byte_len: &mut usize, instruction: Instruction) -> Result<()> {
    *byte_len = byte_len
        .checked_add(encode_bytes(&instruction)?.len())
        .context("ENDING meal loader instruction length overflow")?;
    assembler.emit(instruction);
    Ok(())
}

fn direct_operand(
    segment: Option<SegmentRegister>,
    address: u16,
    size: OperandSize,
) -> Result<Operand> {
    Ok(Operand::Mem(EffectiveAddress::new(
        segment,
        EffectiveAddressBase::Direct,
        EffectiveAddressDisplacement::Absolute(address),
        size,
    )?))
}

fn verify_source_bytes(
    bytes: &[u8],
    file_offset: usize,
    source: &v30::AssembledProgram,
    description: &str,
) -> Result<()> {
    ensure!(
        bytes.get(file_offset..file_offset + source.bytes().len()) == Some(source.bytes()),
        "{description} differs from the verified typed V30 source"
    );
    Ok(())
}

fn assemble_at(assembler: &Assembler, runtime_address: u16) -> Result<v30::AssembledProgram> {
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: runtime_address,
        })
        .context("assemble typed V30 ENDING meal loader patch")
}

#[cfg(test)]
#[path = "arle_ending_meal_loader_tests.rs"]
mod arle_ending_meal_loader_tests;
