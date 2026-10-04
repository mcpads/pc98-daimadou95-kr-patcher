use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    ResizePlan, WriteIntent, WritePlan,
};
use v30::{
    Assembler, CallTarget, CodeLocation, Condition, EffectiveAddress, EffectiveAddressBase,
    EffectiveAddressDisplacement, Instruction, JmpTarget, Operand, OperandSize, Register8,
    Register16, SegmentRegister, encode_bytes,
};

use crate::font::rasterize_pc98_character;
use crate::patch::apply_v30_patch_plan;

use super::bank_plan::{GaijiCodebook, GaijiSlot};

const COM_ORIGIN: usize = 0x100;
const ENTRY_WIDTH: usize = 3;
pub(super) const GLYPH_RECORD_SIZE: usize = 34;
const ORIGINAL_INITIALIZATION_TARGET: u16 = 0x09f5;
const ENTRY_CONTINUATION: u16 = 0x0103;

pub(super) struct PatchedMadFontInstaller {
    pub bytes: Vec<u8>,
    pub hook_file_offset: usize,
    pub hook_byte_size: usize,
}

pub(super) fn install_mad_error_font_at_entry(
    bytes: &[u8],
    codebook: &GaijiCodebook,
) -> Result<PatchedMadFontInstaller> {
    ensure!(
        !codebook.is_empty(),
        "MAD.COM media errors have no Hangul glyph demand"
    );
    let slots = ordered_slots(codebook)?;

    let hook_file_offset = bytes.len();
    let hook_runtime_address = file_to_runtime_address(hook_file_offset, "MAD font hook")?;
    let placeholder = assemble_installer_hook(hook_runtime_address, 0, 0, slots.len())?;
    let slot_codes_file_offset = hook_file_offset
        .checked_add(placeholder.bytes().len())
        .context("MAD slot-code offset overflow")?;
    let slot_codes_runtime_address =
        file_to_runtime_address(slot_codes_file_offset, "MAD slot codes")?;
    let records_file_offset = hook_file_offset
        .checked_add(placeholder.bytes().len())
        .and_then(|offset| offset.checked_add(slots.len() * 2))
        .context("MAD glyph-record offset overflow")?;
    let records_runtime_address =
        file_to_runtime_address(records_file_offset, "MAD glyph records")?;
    let hook = assemble_installer_hook(
        hook_runtime_address,
        records_runtime_address,
        slot_codes_runtime_address,
        slots.len(),
    )?;
    ensure!(
        hook.bytes().len() == placeholder.bytes().len(),
        "MAD glyph-record placement changed the typed hook width"
    );
    let slot_codes = compile_slot_codes(&slots);
    let records = compile_glyph_records(&slots)?;
    let output_size = records_file_offset
        .checked_add(records.len())
        .context("MAD font-installer output size overflow")?;
    ensure!(
        output_size + COM_ORIGIN <= usize::from(u16::MAX) + 1,
        "MAD font installer exceeds the COM address space"
    );

    let original_entry = assemble_original_entry()?;
    ensure!(
        bytes.get(..ENTRY_WIDTH) == Some(original_entry.bytes()),
        "MAD.COM entry differs from the verified typed V30 source"
    );
    let hook_entry = assemble_hook_entry(hook_runtime_address)?;

    let entry_source_id = "mad-media-error-font-entry";
    let hook_source_id = "mad-media-error-font-installer";
    let plan = WritePlan::new()
        .resize(ResizePlan {
            owner: "mad-media-error-font-installer".into(),
            purpose: "append the Korean glyph bank used by MAD.COM media errors".into(),
            expected_input_len: bytes.len(),
            output_len: output_size,
        })
        .region(ImageRegion {
            id: "mad-font-entry-hook".into(),
            range: 0..ENTRY_WIDTH,
            kind: RegionKind::MachineCode,
            reason: "install MAD.COM DOS glyphs before its original initialization".into(),
        })
        .region(ImageRegion {
            id: "mad-font-installer-body".into(),
            range: hook_file_offset..slot_codes_file_offset,
            kind: RegionKind::MachineCode,
            reason: "compact typed loop over the MAD.COM external-character bank".into(),
        })
        .region(ImageRegion {
            id: "mad-font-slot-codes".into(),
            range: slot_codes_file_offset..records_file_offset,
            kind: RegionKind::Data,
            reason: "uninstalled PC-98 external-character codes not referenced by persistent consumers"
                .into(),
        })
        .region(ImageRegion {
            id: "mad-font-records".into(),
            range: records_file_offset..output_size,
            kind: RegionKind::Data,
            reason: "MAD.COM-specific 34-byte PC-98 external-character records".into(),
        })
        .write(ExpectedWrite {
            id: "write-mad-font-entry-hook".into(),
            owner: "mad-media-error-font-installer".into(),
            purpose: "route the verified MAD.COM entry through its font installer".into(),
            offset: 0,
            expected_original: original_entry.into_bytes(),
            replacement: hook_entry.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: entry_source_id.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "append-mad-font-installer".into(),
            owner: "mad-media-error-font-installer".into(),
            purpose: "install every glyph required by translated MAD.COM media errors".into(),
            offset: hook_file_offset,
            expected_original: Vec::new(),
            replacement: hook.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: hook_source_id.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "append-mad-font-slot-codes".into(),
            owner: "mad-media-error-font-installer".into(),
            purpose: "install Korean glyphs without replacing any GAIJI.COM art or a code referenced after MAD returns"
                .into(),
            offset: slot_codes_file_offset,
            expected_original: Vec::new(),
            replacement: slot_codes.clone(),
            intent: WriteIntent::Data,
        })
        .write(ExpectedWrite {
            id: "append-mad-font-records".into(),
            owner: "mad-media-error-font-installer".into(),
            purpose: "append reproducible NeoDunggeunmo PC-98 glyph records".into(),
            offset: records_file_offset,
            expected_original: Vec::new(),
            replacement: records.clone(),
            intent: WriteIntent::Data,
        });

    let entry_source = hook_entry.clone();
    let hook_source = hook.clone();
    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| {
        if source_id == entry_source_id {
            Ok(entry_source.clone())
        } else if source_id == hook_source_id {
            Ok(hook_source.clone())
        } else {
            Err(MachineCodeVerifierError::new(format!(
                "unknown MAD.COM media-error font V30 source {source_id}"
            )))
        }
    })?;
    ensure!(
        output[slot_codes_file_offset..records_file_offset] == slot_codes,
        "MAD slot codes differ after Expected Writes"
    );
    ensure!(
        output[records_file_offset..] == records,
        "MAD glyph records differ after Expected Writes"
    );
    Ok(PatchedMadFontInstaller {
        bytes: output,
        hook_file_offset,
        hook_byte_size: hook.bytes().len(),
    })
}

fn ordered_slots(codebook: &GaijiCodebook) -> Result<Vec<(char, GaijiSlot)>> {
    let mut slots = codebook
        .iter()
        .map(|(&character, &slot)| (character, slot))
        .collect::<Vec<_>>();
    slots.sort_by_key(|(_, slot)| slot.character_code);
    ensure!(
        slots
            .windows(2)
            .all(|pair| pair[0].1.character_code < pair[1].1.character_code),
        "MAD media-error codebook assigns an external-character slot more than once"
    );
    Ok(slots)
}

fn compile_glyph_records(slots: &[(char, GaijiSlot)]) -> Result<Vec<u8>> {
    let mut records = Vec::with_capacity(slots.len() * GLYPH_RECORD_SIZE);
    for &(character, _) in slots {
        records.extend_from_slice(&[0, 0]);
        records.extend_from_slice(
            &rasterize_pc98_character(character)
                .with_context(|| format!("rasterize MAD media-error glyph {character:?}"))?,
        );
    }
    ensure!(
        records.len() == slots.len() * GLYPH_RECORD_SIZE,
        "MAD media-error glyph-record size drifted"
    );
    Ok(records)
}

fn compile_slot_codes(slots: &[(char, super::bank_plan::GaijiSlot)]) -> Vec<u8> {
    slots
        .iter()
        .flat_map(|(_, slot)| slot.character_code.to_le_bytes())
        .collect()
}

fn assemble_original_entry() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Call {
        target: CallTarget::Rel16(relative_near_displacement(
            COM_ORIGIN as u16,
            ORIGINAL_INITIALIZATION_TARGET,
        )),
    });
    assemble_at(&assembler, COM_ORIGIN as u16, "MAD.COM entry")
}

fn assemble_hook_entry(hook_runtime_address: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Jmp {
        target: JmpTarget::Rel16(relative_near_displacement(
            COM_ORIGIN as u16,
            hook_runtime_address,
        )),
    });
    assemble_at(&assembler, COM_ORIGIN as u16, "MAD.COM entry")
}

fn assemble_installer_hook(
    origin: u16,
    records_runtime_address: u16,
    slot_codes_runtime_address: u16,
    glyph_count: usize,
) -> Result<v30::AssembledProgram> {
    let glyph_count = u16::try_from(glyph_count).context("MAD glyph count exceeds 16 bits")?;
    let mut assembler = Assembler::new();
    let mut byte_len = 0_usize;
    for instruction in [Instruction::Pushf, Instruction::Pusha] {
        emit(&mut assembler, &mut byte_len, instruction)?;
    }
    for register in [SegmentRegister::DS, SegmentRegister::ES] {
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Push {
                src: Operand::Sreg(register),
            },
        )?;
    }
    for (register, value) in [
        (Register16::SI, records_runtime_address),
        (Register16::DI, slot_codes_runtime_address),
        (Register16::BP, glyph_count),
    ] {
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Mov {
                dest: Operand::Reg16(register),
                src: Operand::Imm16(value),
            },
        )?;
    }

    assembler.label("install-glyph");
    for register in [Register16::SI, Register16::DI, Register16::BP] {
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Push {
                src: Operand::Reg16(register),
            },
        )?;
    }
    for instruction in [
        Instruction::Mov {
            dest: Operand::Reg16(Register16::BX),
            src: Operand::Sreg(SegmentRegister::CS),
        },
        Instruction::Mov {
            dest: Operand::Reg16(Register16::CX),
            src: Operand::Reg16(Register16::SI),
        },
        Instruction::Mov {
            dest: Operand::Reg16(Register16::DX),
            src: Operand::Mem(EffectiveAddress::new(
                Some(SegmentRegister::CS),
                EffectiveAddressBase::Di,
                EffectiveAddressDisplacement::Signed(0),
                OperandSize::Word,
            )?),
        },
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Imm8(0x1a),
        },
        Instruction::Int { vector: 0x18 },
    ] {
        emit(&mut assembler, &mut byte_len, instruction)?;
    }
    for register in [Register16::BP, Register16::DI, Register16::SI] {
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Pop {
                dest: Operand::Reg16(register),
            },
        )?;
    }
    for instruction in [
        Instruction::Add {
            dest: Operand::Reg16(Register16::SI),
            src: Operand::Imm16(GLYPH_RECORD_SIZE as u16),
        },
        Instruction::Add {
            dest: Operand::Reg16(Register16::DI),
            src: Operand::Imm16(2),
        },
        Instruction::Dec {
            dest: Operand::Reg16(Register16::BP),
        },
    ] {
        emit(&mut assembler, &mut byte_len, instruction)?;
    }
    byte_len = byte_len
        .checked_add(2)
        .context("MAD font loop branch length overflow")?;
    assembler.emit_branch(Condition::Ne, "install-glyph");

    for register in [SegmentRegister::ES, SegmentRegister::DS] {
        emit(
            &mut assembler,
            &mut byte_len,
            Instruction::Pop {
                dest: Operand::Sreg(register),
            },
        )?;
    }
    for instruction in [Instruction::Popa, Instruction::Popf] {
        emit(&mut assembler, &mut byte_len, instruction)?;
    }
    emit_call_to(
        &mut assembler,
        &mut byte_len,
        origin,
        ORIGINAL_INITIALIZATION_TARGET,
    )?;
    emit_jump_to(&mut assembler, &mut byte_len, origin, ENTRY_CONTINUATION)?;

    let program = assemble_at(&assembler, origin, "MAD.COM font installer")?;
    ensure!(
        program.bytes().len() == byte_len,
        "MAD compact font-installer length tracking drifted"
    );
    Ok(program)
}

fn emit_call_to(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
    target: u16,
) -> Result<()> {
    let current = origin
        .wrapping_add(u16::try_from(*byte_len).context("MAD font hook position exceeds 16 bits")?);
    emit(
        assembler,
        byte_len,
        Instruction::Call {
            target: CallTarget::Rel16(relative_near_displacement(current, target)),
        },
    )
}

fn emit_jump_to(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
    target: u16,
) -> Result<()> {
    let current = origin
        .wrapping_add(u16::try_from(*byte_len).context("MAD font hook position exceeds 16 bits")?);
    emit(
        assembler,
        byte_len,
        Instruction::Jmp {
            target: JmpTarget::Rel16(relative_near_displacement(current, target)),
        },
    )
}

fn emit(assembler: &mut Assembler, byte_len: &mut usize, instruction: Instruction) -> Result<()> {
    *byte_len = byte_len
        .checked_add(encode_bytes(&instruction)?.len())
        .context("MAD font-installer length overflow")?;
    assembler.emit(instruction);
    Ok(())
}

fn relative_near_displacement(current: u16, target: u16) -> i16 {
    target.wrapping_sub(current.wrapping_add(3)) as i16
}

fn file_to_runtime_address(file_offset: usize, label: &str) -> Result<u16> {
    let runtime_address = file_offset
        .checked_add(COM_ORIGIN)
        .with_context(|| format!("{label} address overflow"))?;
    u16::try_from(runtime_address).with_context(|| format!("{label} exceeds the COM address space"))
}

fn assemble_at(
    assembler: &Assembler,
    runtime_address: u16,
    label: &str,
) -> Result<v30::AssembledProgram> {
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: runtime_address,
        })
        .with_context(|| format!("assemble typed V30 {label}"))
}

#[cfg(test)]
#[path = "font_installer_tests.rs"]
mod font_installer_tests;
