use std::collections::BTreeMap;
use std::ops::Range;

use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    ResizePlan, WriteIntent, WritePlan,
};
use v30::{
    Assembler, CallTarget, CodeLocation, Condition, EffectiveAddress, EffectiveAddressBase,
    EffectiveAddressDisplacement, Instruction, JmpTarget, Operand, OperandSize, Register16,
    SegmentRegister, encode_bytes,
};

use super::bank_plan::MAD_FONT_FIRST_ENTRY_ID;
use crate::patch::apply_v30_patch_plan;

const COM_ORIGIN: usize = 0x100;
pub(super) const FONT_INITIALIZER_FILE_OFFSET: usize = 0x1312;
const FONT_INITIALIZER_RUNTIME_ADDRESS: u16 = 0x1412;
pub(super) const RENDERER_HOOK_SITE_FILE_OFFSET: usize = 0x4db4;
const RENDERER_HOOK_SITE_RUNTIME_ADDRESS: u16 = 0x4eb4;
const RENDERER_CONTINUATION_RUNTIME_ADDRESS: u16 = 0x4eb9;
const SOURCE_FONT_ENTRY_ID: u16 = 6;
const FONT_BUFFER_SEGMENT_ADDRESS: u16 = 0xc024;
const LOADER_MODE_ADDRESS: u16 = 0xbf24;
const MADDAT_LOADER_RUNTIME_ADDRESS: u16 = 0xb400;
const INITIALIZER_SOURCE_ID: &str = "mad-base-font-bank-selector";
const HOOK_SITE_SOURCE_ID: &str = "mad-font-bank-hook-site";
const HOOK_BODY_SOURCE_ID: &str = "mad-font-bank-loader";

pub(super) struct PatchedMadConsumer {
    pub bytes: Vec<u8>,
    pub hook_site_file_offset: usize,
    pub hook_file_offset: usize,
    pub hook_runtime_address: usize,
    pub hook_byte_size: usize,
}

pub(super) fn install_mad_font_bank_loader(
    bytes: &[u8],
    unit_ranges: &BTreeMap<u8, Range<u16>>,
    unit_font_entry_ids: &BTreeMap<u8, usize>,
) -> Result<PatchedMadConsumer> {
    ensure!(
        unit_ranges.keys().eq(unit_font_entry_ids.keys()),
        "MAD unit ranges and font entry IDs differ"
    );
    let base_entry_id =
        u16::try_from(MAD_FONT_FIRST_ENTRY_ID).context("MAD base font entry ID exceeds 16 bits")?;
    let hook_file_offset = bytes.len();
    let hook_runtime_address = hook_file_offset
        .checked_add(COM_ORIGIN)
        .context("MAD font hook runtime address overflow")?;
    let hook_runtime_address_u16 = u16::try_from(hook_runtime_address)
        .context("MAD font hook exceeds the COM code segment")?;

    let original_initializer = assemble_font_initializer(SOURCE_FONT_ENTRY_ID)?;
    ensure!(
        bytes.get(
            FONT_INITIALIZER_FILE_OFFSET
                ..FONT_INITIALIZER_FILE_OFFSET + original_initializer.bytes().len()
        ) == Some(original_initializer.bytes()),
        "MAD font initializer differs from the verified typed V30 source"
    );
    let initializer = assemble_font_initializer(base_entry_id)?;
    let original_hook_site = assemble_original_hook_site()?;
    ensure!(
        bytes.get(
            RENDERER_HOOK_SITE_FILE_OFFSET
                ..RENDERER_HOOK_SITE_FILE_OFFSET + original_hook_site.bytes().len()
        ) == Some(original_hook_site.bytes()),
        "MAD indexed renderer entry differs from the verified typed V30 source"
    );
    let hook_site = assemble_hook_site(hook_runtime_address_u16)?;
    ensure!(
        hook_site.bytes().len() == original_hook_site.bytes().len(),
        "MAD font hook site changed instruction width"
    );
    let (hook_body, cache_runtime_address) = assemble_hook_body(
        hook_runtime_address_u16,
        base_entry_id,
        unit_ranges,
        unit_font_entry_ids,
    )?;
    let cache_file_offset = hook_file_offset + hook_body.bytes().len();
    ensure!(
        cache_runtime_address as usize == cache_file_offset + COM_ORIGIN,
        "MAD font cache placement drifted"
    );
    let output_len = cache_file_offset
        .checked_add(2)
        .context("MAD font hook size overflow")?;
    ensure!(
        output_len + COM_ORIGIN <= usize::from(u16::MAX) + 1,
        "MAD font hook exceeds the 16-bit COM address space"
    );

    let plan = WritePlan::new()
        .resize(ResizePlan {
            owner: "mad-font-consumer-patch".into(),
            purpose: "append the MAD state-specific font loader and cache".into(),
            expected_input_len: bytes.len(),
            output_len,
        })
        .region(ImageRegion {
            id: "mad-base-font-initializer".into(),
            range: FONT_INITIALIZER_FILE_OFFSET
                ..FONT_INITIALIZER_FILE_OFFSET + initializer.bytes().len(),
            kind: RegionKind::MachineCode,
            reason: "load the common Korean MAD font bank during initialization".into(),
        })
        .region(ImageRegion {
            id: "mad-font-bank-hook-site".into(),
            range: RENDERER_HOOK_SITE_FILE_OFFSET
                ..RENDERER_HOOK_SITE_FILE_OFFSET + hook_site.bytes().len(),
            kind: RegionKind::MachineCode,
            reason: "select the active MAD font bank before indexed glyph consumption".into(),
        })
        .region(ImageRegion {
            id: "mad-font-bank-hook-body".into(),
            range: hook_file_offset..cache_file_offset,
            kind: RegionKind::MachineCode,
            reason: "typed unit-range selection and existing MADDAT loader invocation".into(),
        })
        .region(ImageRegion {
            id: "mad-font-bank-cache".into(),
            range: cache_file_offset..output_len,
            kind: RegionKind::Data,
            reason: "currently resident MAD font entry ID".into(),
        })
        .write(ExpectedWrite {
            id: "select-mad-base-font-at-initialization".into(),
            owner: "mad-font-consumer-patch".into(),
            purpose: "load appended MADDAT entry 166 instead of shared entry 6".into(),
            offset: FONT_INITIALIZER_FILE_OFFSET,
            expected_original: original_initializer.into_bytes(),
            replacement: initializer.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: INITIALIZER_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "install-mad-font-hook-jump".into(),
            owner: "mad-font-consumer-patch".into(),
            purpose: "replace the renderer font-segment load with a typed hook jump".into(),
            offset: RENDERER_HOOK_SITE_FILE_OFFSET,
            expected_original: original_hook_site.into_bytes(),
            replacement: hook_site.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: HOOK_SITE_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "append-mad-font-bank-loader".into(),
            owner: "mad-font-consumer-patch".into(),
            purpose: "load the font bank required by the current translated string range".into(),
            offset: hook_file_offset,
            expected_original: Vec::new(),
            replacement: hook_body.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: HOOK_BODY_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "initialize-mad-font-bank-cache".into(),
            owner: "mad-font-consumer-patch".into(),
            purpose: "match the cache to the base font loaded during initialization".into(),
            offset: cache_file_offset,
            expected_original: Vec::new(),
            replacement: base_entry_id.to_le_bytes().to_vec(),
            intent: WriteIntent::Data,
        });

    let initializer_source = initializer.clone();
    let hook_site_source = hook_site.clone();
    let hook_body_source = hook_body.clone();
    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| match source_id {
        INITIALIZER_SOURCE_ID => Ok(initializer_source.clone()),
        HOOK_SITE_SOURCE_ID => Ok(hook_site_source.clone()),
        HOOK_BODY_SOURCE_ID => Ok(hook_body_source.clone()),
        _ => Err(MachineCodeVerifierError::new(format!(
            "unknown MAD V30 source {source_id}"
        ))),
    })?;
    ensure!(
        output[cache_file_offset..output_len] == base_entry_id.to_le_bytes(),
        "MAD font cache did not survive its Expected Write"
    );

    Ok(PatchedMadConsumer {
        bytes: output,
        hook_site_file_offset: RENDERER_HOOK_SITE_FILE_OFFSET,
        hook_file_offset,
        hook_runtime_address,
        hook_byte_size: hook_body.bytes().len(),
    })
}

fn assemble_font_initializer(entry_id: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::CX),
        src: Operand::Imm16(entry_id),
    });
    assemble_at(&assembler, FONT_INITIALIZER_RUNTIME_ADDRESS)
}

fn assemble_original_hook_site() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Sreg(SegmentRegister::DS),
        src: direct_operand(
            Some(SegmentRegister::CS),
            FONT_BUFFER_SEGMENT_ADDRESS,
            OperandSize::Word,
        )?,
    });
    assemble_at(&assembler, RENDERER_HOOK_SITE_RUNTIME_ADDRESS)
}

fn assemble_hook_site(hook_runtime_address: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Jmp {
            target: JmpTarget::Rel16(relative_near_displacement(
                RENDERER_HOOK_SITE_RUNTIME_ADDRESS,
                hook_runtime_address,
            )?),
        })
        .emit(Instruction::Nop)
        .emit(Instruction::Nop);
    assemble_at(&assembler, RENDERER_HOOK_SITE_RUNTIME_ADDRESS)
}

fn assemble_hook_body(
    origin: u16,
    base_entry_id: u16,
    unit_ranges: &BTreeMap<u8, Range<u16>>,
    unit_font_entry_ids: &BTreeMap<u8, usize>,
) -> Result<(v30::AssembledProgram, u16)> {
    let placeholder =
        assemble_hook_body_with_cache(origin, 0, base_entry_id, unit_ranges, unit_font_entry_ids)?;
    let cache_runtime_address = origin
        .checked_add(
            u16::try_from(placeholder.bytes().len())
                .context("MAD font hook body exceeds 16 bits")?,
        )
        .context("MAD font cache runtime address overflow")?;
    let body = assemble_hook_body_with_cache(
        origin,
        cache_runtime_address,
        base_entry_id,
        unit_ranges,
        unit_font_entry_ids,
    )?;
    ensure!(
        body.bytes().len() == placeholder.bytes().len(),
        "MAD font cache address changed the typed hook width"
    );
    Ok((body, cache_runtime_address))
}

fn assemble_hook_body_with_cache(
    origin: u16,
    cache_runtime_address: u16,
    base_entry_id: u16,
    unit_ranges: &BTreeMap<u8, Range<u16>>,
    unit_font_entry_ids: &BTreeMap<u8, usize>,
) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    let mut byte_len = 0_usize;
    emit(&mut assembler, &mut byte_len, Instruction::Pushf)?;
    emit(&mut assembler, &mut byte_len, Instruction::Pusha)?;
    for register in [SegmentRegister::DS, SegmentRegister::ES] {
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Push {
                src: Operand::Sreg(register),
            },
        )?;
    }
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::CX),
            src: Operand::Imm16(base_entry_id),
        },
    )?;

    for (&unit_number, range) in unit_ranges {
        let next_label = format!("unit-{unit_number:02}-next");
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Cmp {
                a: Operand::Reg16(Register16::SI),
                b: Operand::Imm16(range.start),
            },
        )?;
        emit_short_branch(&mut assembler, &mut byte_len, Condition::B, &next_label)?;
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Cmp {
                a: Operand::Reg16(Register16::SI),
                b: Operand::Imm16(range.end),
            },
        )?;
        emit_short_branch(&mut assembler, &mut byte_len, Condition::Nb, &next_label)?;
        let entry_id = u16::try_from(unit_font_entry_ids[&unit_number])
            .context("MAD unit font entry ID exceeds 16 bits")?;
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Mov {
                dest: Operand::Reg16(Register16::CX),
                src: Operand::Imm16(entry_id),
            },
        )?;
        byte_len = byte_len
            .checked_add(3)
            .context("MAD font hook jump length overflow")?;
        assembler.emit_jump_near("selected");
        assembler.label(next_label);
    }

    assembler.label("selected");
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Cmp {
            a: Operand::Reg16(Register16::CX),
            b: direct_operand(
                Some(SegmentRegister::CS),
                cache_runtime_address,
                OperandSize::Word,
            )?,
        },
    )?;
    emit_short_branch(&mut assembler, &mut byte_len, Condition::E, "restore")?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Push {
            src: Operand::Reg16(Register16::CX),
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::BX),
            src: direct_operand(
                Some(SegmentRegister::CS),
                FONT_BUFFER_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: direct_operand(
                Some(SegmentRegister::CS),
                LOADER_MODE_ADDRESS,
                OperandSize::Word,
            )?,
            src: Operand::Imm16(0),
        },
    )?;
    emit_external_call(
        &mut assembler,
        &mut byte_len,
        origin,
        MADDAT_LOADER_RUNTIME_ADDRESS,
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Pop {
            dest: Operand::Reg16(Register16::CX),
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: direct_operand(
                Some(SegmentRegister::CS),
                cache_runtime_address,
                OperandSize::Word,
            )?,
            src: Operand::Reg16(Register16::CX),
        },
    )?;

    assembler.label("restore");
    for register in [SegmentRegister::ES, SegmentRegister::DS] {
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Pop {
                dest: Operand::Sreg(register),
            },
        )?;
    }
    emit(&mut assembler, &mut byte_len, Instruction::Popa)?;
    emit(&mut assembler, &mut byte_len, Instruction::Popf)?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::DS),
            src: direct_operand(
                Some(SegmentRegister::CS),
                FONT_BUFFER_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        },
    )?;
    emit_external_jump(
        &mut assembler,
        &mut byte_len,
        origin,
        RENDERER_CONTINUATION_RUNTIME_ADDRESS,
    )?;

    let program = assemble_at(&assembler, origin)?;
    ensure!(
        program.bytes().len() == byte_len,
        "MAD font hook length tracking drifted"
    );
    Ok(program)
}

fn emit_short_branch(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    condition: Condition,
    label: &str,
) -> Result<()> {
    *byte_len = byte_len
        .checked_add(2)
        .context("MAD font hook branch length overflow")?;
    assembler.emit_branch(condition, label);
    Ok(())
}

fn emit_external_call(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
    target: u16,
) -> Result<()> {
    let current = origin
        .checked_add(u16::try_from(*byte_len).context("MAD hook offset exceeds 16 bits")?)
        .context("MAD hook call address overflow")?;
    emit(
        assembler,
        byte_len,
        Instruction::Call {
            target: CallTarget::Rel16(relative_near_displacement(current, target)?),
        },
    )
}

fn emit_external_jump(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
    target: u16,
) -> Result<()> {
    let current = origin
        .checked_add(u16::try_from(*byte_len).context("MAD hook offset exceeds 16 bits")?)
        .context("MAD hook jump address overflow")?;
    emit(
        assembler,
        byte_len,
        Instruction::Jmp {
            target: JmpTarget::Rel16(relative_near_displacement(current, target)?),
        },
    )
}

fn relative_near_displacement(current: u16, target: u16) -> Result<i16> {
    Ok(target.wrapping_sub(current.wrapping_add(3)) as i16)
}

fn emit(assembler: &mut Assembler, byte_len: &mut usize, instruction: Instruction) -> Result<()> {
    *byte_len = byte_len
        .checked_add(encode_bytes(&instruction)?.len())
        .context("MAD font hook length overflow")?;
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

fn assemble_at(assembler: &Assembler, runtime_address: u16) -> Result<v30::AssembledProgram> {
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: runtime_address,
        })
        .context("assemble typed V30 MAD font-bank program")
}

#[cfg(test)]
#[path = "consumer_tests.rs"]
mod consumer_tests;
