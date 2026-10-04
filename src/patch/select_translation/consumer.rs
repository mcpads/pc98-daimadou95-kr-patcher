use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    ResizePlan, WriteIntent, WritePlan,
};
use v30::{
    Assembler, CallTarget, CodeLocation, Condition, EffectiveAddress, EffectiveAddressBase,
    EffectiveAddressDisplacement, Instruction, JmpTarget, Operand, OperandSize, Register8,
    Register16, SegmentRegister, ShiftCount, encode_bytes,
};

use crate::patch::apply_v30_patch_plan;

const COM_ORIGIN: usize = 0x100;
pub(super) const STAGE_FONT_SETUP_FILE_OFFSET: usize = 0x3fc7;
const STAGE_FONT_SETUP_RUNTIME_ADDRESS: u16 = 0x40c7;
const STAGE_FONT_SETUP_CONTINUATION: u16 = 0x40cc;
pub(super) const STAGE_FONT_RESUME_SETUP_FILE_OFFSET: usize = 0x4017;
const STAGE_FONT_RESUME_SETUP_RUNTIME_ADDRESS: u16 = 0x4117;
const STAGE_FONT_RESUME_SETUP_CONTINUATION: u16 = 0x411b;
const SELECT_FONT_ENTRY_BASE: u16 = 157;
const MADDAT_TABLE_SEGMENT_ADDRESS: u16 = 0x088f;
const FILE_READ_SIZE_ADDRESS: u16 = 0x0873;
const FILE_READ_OFFSET_LOW_ADDRESS: u16 = 0x0871;
const FILE_READ_OFFSET_HIGH_ADDRESS: u16 = 0x086f;
const FILE_NAME_POINTER_ADDRESS: u16 = 0x086d;
const FILE_NAME_RUNTIME_ADDRESS: u16 = 0x42bf;
const FILE_READ_DESTINATION_SEGMENT_ADDRESS: u16 = 0x0869;
const FILE_READ_DESTINATION_OFFSET_ADDRESS: u16 = 0x086b;
const FONT_BUFFER_SEGMENT_ADDRESS: u16 = 0x4287;
const DECOMPRESSION_BUFFER_SEGMENT_ADDRESS: u16 = 0x4283;
const FILE_READ_RUNTIME_ADDRESS: u16 = 0x03ae;
const DECOMPRESS_RUNTIME_ADDRESS: u16 = 0x158c;
const SELECT_FONT_COPY_WORD_COUNT: u16 = 0x0f00;
const INITIAL_HOOK_SITE_SOURCE_ID: &str = "select-initial-stage-font-hook-site";
const RESUME_HOOK_SITE_SOURCE_ID: &str = "select-resumed-stage-font-hook-site";
const HOOK_BODY_SOURCE_ID: &str = "select-stage-font-loader-program";

pub(super) struct PatchedSelectConsumer {
    pub bytes: Vec<u8>,
    pub hook_site_file_offset: usize,
    pub resume_hook_site_file_offset: usize,
    pub hook_file_offset: usize,
    pub hook_runtime_address: usize,
    pub resume_hook_runtime_address: usize,
    pub shared_loader_runtime_address: usize,
    pub hook_byte_size: usize,
    pub copied_glyph_count: usize,
}

pub(super) fn install_select_stage_font_loader(bytes: &[u8]) -> Result<PatchedSelectConsumer> {
    let hook_file_offset = bytes.len();
    let hook_runtime_address = hook_file_offset
        .checked_add(COM_ORIGIN)
        .context("SELECT stage-font hook runtime address overflow")?;
    let hook_runtime_address_u16 = u16::try_from(hook_runtime_address)
        .context("SELECT stage-font hook exceeds the COM code segment")?;
    let original_setup = assemble_original_stage_setup()?;
    let original_resume_setup = assemble_original_stage_resume_setup()?;
    ensure!(
        bytes.get(
            STAGE_FONT_SETUP_FILE_OFFSET
                ..STAGE_FONT_SETUP_FILE_OFFSET + original_setup.bytes().len()
        ) == Some(original_setup.bytes()),
        "SELECT stage-font setup differs from the verified typed V30 source"
    );
    ensure!(
        bytes.get(
            STAGE_FONT_RESUME_SETUP_FILE_OFFSET
                ..STAGE_FONT_RESUME_SETUP_FILE_OFFSET + original_resume_setup.bytes().len()
        ) == Some(original_resume_setup.bytes()),
        "SELECT resumed-stage font setup differs from the verified typed V30 source"
    );
    let hook_body = assemble_hook_body(hook_runtime_address_u16)?;
    let initial_hook_runtime_address = hook_body
        .label_location("initial-stage-entry")
        .context("SELECT hook program has no initial-stage entry")?
        .off;
    let resume_hook_runtime_address = hook_body
        .label_location("resumed-stage-entry")
        .context("SELECT hook program has no resumed-stage entry")?
        .off;
    let shared_loader_runtime_address = hook_body
        .label_location("load-stage-font")
        .context("SELECT hook program has no shared font loader")?
        .off;
    let hook_site = assemble_hook_site(
        STAGE_FONT_SETUP_RUNTIME_ADDRESS,
        initial_hook_runtime_address,
        original_setup.bytes().len(),
    )?;
    ensure!(
        hook_site.bytes().len() == original_setup.bytes().len(),
        "SELECT stage-font hook site changed instruction width"
    );
    let resume_hook_site = assemble_hook_site(
        STAGE_FONT_RESUME_SETUP_RUNTIME_ADDRESS,
        resume_hook_runtime_address,
        original_resume_setup.bytes().len(),
    )?;
    ensure!(
        resume_hook_site.bytes().len() == original_resume_setup.bytes().len(),
        "SELECT resumed-stage font hook site changed instruction width"
    );
    let output_len = bytes
        .len()
        .checked_add(hook_body.bytes().len())
        .context("SELECT stage-font hook size overflow")?;

    let plan = WritePlan::new()
        .resize(ResizePlan {
            owner: "select-font-consumer-patch".into(),
            purpose: "append the stage-aware SELECT font loader".into(),
            expected_input_len: bytes.len(),
            output_len,
        })
        .region(ImageRegion {
            id: "select-stage-font-hook-site".into(),
            range: STAGE_FONT_SETUP_FILE_OFFSET
                ..STAGE_FONT_SETUP_FILE_OFFSET + original_setup.bytes().len(),
            kind: RegionKind::MachineCode,
            reason: "jump to the stage-aware font loader when SELECT consumes the chosen stage"
                .into(),
        })
        .region(ImageRegion {
            id: "select-resumed-stage-font-hook-site".into(),
            range: STAGE_FONT_RESUME_SETUP_FILE_OFFSET
                ..STAGE_FONT_RESUME_SETUP_FILE_OFFSET + original_resume_setup.bytes().len(),
            kind: RegionKind::MachineCode,
            reason: "reload the selected stage font when SELECT resumes after MAD".into(),
        })
        .region(ImageRegion {
            id: "select-stage-font-hook-body".into(),
            range: hook_file_offset..output_len,
            kind: RegionKind::MachineCode,
            reason: "typed MADDAT load, Compile LZ decode, and 240-glyph SELECT copy".into(),
        })
        .write(ExpectedWrite {
            id: "install-select-stage-font-hook-jump".into(),
            owner: "select-font-consumer-patch".into(),
            purpose: "replace the stage setup with a width-preserving typed hook jump".into(),
            offset: STAGE_FONT_SETUP_FILE_OFFSET,
            expected_original: original_setup.into_bytes(),
            replacement: hook_site.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: INITIAL_HOOK_SITE_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "install-select-resumed-stage-font-hook-jump".into(),
            owner: "select-font-consumer-patch".into(),
            purpose: "replace the post-MAD stage setup with a width-preserving typed hook jump"
                .into(),
            offset: STAGE_FONT_RESUME_SETUP_FILE_OFFSET,
            expected_original: original_resume_setup.into_bytes(),
            replacement: resume_hook_site.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: RESUME_HOOK_SITE_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "append-select-stage-font-loader".into(),
            owner: "select-font-consumer-patch".into(),
            purpose: "load and decode the font bank selected by the current stage".into(),
            offset: hook_file_offset,
            expected_original: Vec::new(),
            replacement: hook_body.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: HOOK_BODY_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        });
    let hook_site_source = hook_site.clone();
    let resume_hook_site_source = resume_hook_site.clone();
    let hook_body_source = hook_body.clone();
    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| match source_id {
        INITIAL_HOOK_SITE_SOURCE_ID => Ok(hook_site_source.clone()),
        RESUME_HOOK_SITE_SOURCE_ID => Ok(resume_hook_site_source.clone()),
        HOOK_BODY_SOURCE_ID => Ok(hook_body_source.clone()),
        _ => Err(MachineCodeVerifierError::new(format!(
            "unknown SELECT V30 source {source_id}"
        ))),
    })?;
    ensure!(
        output[hook_file_offset..] == *hook_body.bytes(),
        "SELECT stage-font hook body did not survive its Expected Write"
    );

    Ok(PatchedSelectConsumer {
        bytes: output,
        hook_site_file_offset: STAGE_FONT_SETUP_FILE_OFFSET,
        resume_hook_site_file_offset: STAGE_FONT_RESUME_SETUP_FILE_OFFSET,
        hook_file_offset,
        hook_runtime_address,
        resume_hook_runtime_address: usize::from(resume_hook_runtime_address),
        shared_loader_runtime_address: usize::from(shared_loader_runtime_address),
        hook_byte_size: hook_body.bytes().len(),
        copied_glyph_count: usize::from(SELECT_FONT_COPY_WORD_COUNT) * 2 / 32,
    })
}

fn assemble_original_stage_setup() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Dec {
            dest: Operand::Reg8(Register8::AL),
        })
        .emit(Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Imm8(0),
        })
        .emit(Instruction::Push {
            src: Operand::Reg16(Register16::AX),
        });
    assemble_at(&assembler, STAGE_FONT_SETUP_RUNTIME_ADDRESS)
}

fn assemble_original_stage_resume_setup() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Imm8(0),
        })
        .emit(Instruction::Dec {
            dest: Operand::Reg16(Register16::AX),
        })
        .emit(Instruction::Push {
            src: Operand::Reg16(Register16::AX),
        });
    assemble_at(&assembler, STAGE_FONT_RESUME_SETUP_RUNTIME_ADDRESS)
}

fn assemble_hook_site(
    site_runtime_address: u16,
    hook_runtime_address: u16,
    replaced_byte_size: usize,
) -> Result<v30::AssembledProgram> {
    ensure!(
        replaced_byte_size >= 3,
        "SELECT stage-font hook site is shorter than a near jump"
    );
    let displacement = relative_near_displacement(site_runtime_address, hook_runtime_address)?;
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Jmp {
        target: JmpTarget::Rel16(displacement),
    });
    for _ in 3..replaced_byte_size {
        assembler.emit(Instruction::Nop);
    }
    assemble_at(&assembler, site_runtime_address)
}

fn assemble_hook_body(origin: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    let mut byte_len = 0_usize;

    assembler.label("initial-stage-entry");
    emit_original_stage_setup(&mut assembler, &mut byte_len)?;
    emit_shared_loader_call(&mut assembler, &mut byte_len)?;
    emit_external_jump(
        &mut assembler,
        &mut byte_len,
        origin,
        STAGE_FONT_SETUP_CONTINUATION,
    )?;

    assembler.label("resumed-stage-entry");
    emit_original_stage_resume_setup(&mut assembler, &mut byte_len)?;
    emit_shared_loader_call(&mut assembler, &mut byte_len)?;
    emit_external_jump(
        &mut assembler,
        &mut byte_len,
        origin,
        STAGE_FONT_RESUME_SETUP_CONTINUATION,
    )?;

    assembler.label("load-stage-font");
    emit_preserve_context(&mut assembler, &mut byte_len)?;
    emit_validate_stage(&mut assembler, &mut byte_len)?;
    emit_select_font_record(&mut assembler, &mut byte_len)?;
    emit_file_read_request(&mut assembler, &mut byte_len, origin)?;
    emit_decode_and_copy_font(&mut assembler, &mut byte_len, origin)?;
    assembler.label("restore");
    emit_restore_context(&mut assembler, &mut byte_len)?;
    emit(&mut assembler, &mut byte_len, Instruction::Ret { pop: 0 })?;

    let program = assemble_at(&assembler, origin)?;
    ensure!(
        program.bytes().len() == byte_len,
        "SELECT stage-font hook length tracking drifted"
    );
    Ok(program)
}

fn emit_shared_loader_call(assembler: &mut Assembler, byte_len: &mut usize) -> Result<()> {
    *byte_len = byte_len
        .checked_add(3)
        .context("SELECT shared stage-font call length overflow")?;
    assembler.emit_call_near("load-stage-font");
    Ok(())
}

fn emit_original_stage_setup(assembler: &mut Assembler, byte_len: &mut usize) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Dec {
            dest: Operand::Reg8(Register8::AL),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Imm8(0),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Push {
            src: Operand::Reg16(Register16::AX),
        },
    )
}

fn emit_original_stage_resume_setup(assembler: &mut Assembler, byte_len: &mut usize) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Imm8(0),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Dec {
            dest: Operand::Reg16(Register16::AX),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Push {
            src: Operand::Reg16(Register16::AX),
        },
    )
}

fn emit_preserve_context(assembler: &mut Assembler, byte_len: &mut usize) -> Result<()> {
    emit(assembler, byte_len, Instruction::Pushf)?;
    emit(assembler, byte_len, Instruction::Pusha)?;
    for register in [SegmentRegister::DS, SegmentRegister::ES] {
        emit(
            assembler,
            byte_len,
            Instruction::Push {
                src: Operand::Sreg(register),
            },
        )?;
    }
    Ok(())
}

fn emit_validate_stage(assembler: &mut Assembler, byte_len: &mut usize) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Cmp {
            a: Operand::Reg8(Register8::AL),
            b: Operand::Imm8(9),
        },
    )?;
    emit_short_branch(assembler, byte_len, Condition::Nb, "restore")
}

fn emit_select_font_record(assembler: &mut Assembler, byte_len: &mut usize) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Imm8(0),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Add {
            dest: Operand::Reg16(Register16::AX),
            src: Operand::Imm16(SELECT_FONT_ENTRY_BASE),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Dec {
            dest: Operand::Reg16(Register16::AX),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::BX),
            src: Operand::Reg16(Register16::AX),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Shl {
            dest: Operand::Reg16(Register16::AX),
            count: ShiftCount::Imm8(2),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Shl {
            dest: Operand::Reg16(Register16::BX),
            count: ShiftCount::One,
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Add {
            dest: Operand::Reg16(Register16::BX),
            src: Operand::Reg16(Register16::AX),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: direct_operand(
                Some(SegmentRegister::CS),
                MADDAT_TABLE_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::DS),
            src: Operand::Reg16(Register16::AX),
        },
    )?;
    copy_entry_field(assembler, byte_len, 0, FILE_READ_SIZE_ADDRESS)?;
    copy_entry_field(assembler, byte_len, 2, FILE_READ_OFFSET_LOW_ADDRESS)?;
    copy_entry_field(assembler, byte_len, 4, FILE_READ_OFFSET_HIGH_ADDRESS)
}

fn emit_file_read_request(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: Operand::Sreg(SegmentRegister::CS),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::DS),
            src: Operand::Reg16(Register16::AX),
        },
    )?;
    emit_mov_ax_imm_store(
        assembler,
        byte_len,
        FILE_NAME_RUNTIME_ADDRESS,
        FILE_NAME_POINTER_ADDRESS,
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: direct_operand(
                Some(SegmentRegister::CS),
                FONT_BUFFER_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: direct_operand(
                None,
                FILE_READ_DESTINATION_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
            src: Operand::Reg16(Register16::AX),
        },
    )?;
    emit_mov_ax_imm_store(assembler, byte_len, 0, FILE_READ_DESTINATION_OFFSET_ADDRESS)?;
    emit_external_call(assembler, byte_len, origin, FILE_READ_RUNTIME_ADDRESS)?;
    emit_short_branch(assembler, byte_len, Condition::B, "restore")
}

fn emit_decode_and_copy_font(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::ES),
            src: direct_operand(
                Some(SegmentRegister::CS),
                FONT_BUFFER_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::DS),
            src: direct_operand(
                Some(SegmentRegister::CS),
                DECOMPRESSION_BUFFER_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        },
    )?;
    for register in [Register16::DI, Register16::BX] {
        emit(
            assembler,
            byte_len,
            Instruction::Mov {
                dest: Operand::Reg16(register),
                src: Operand::Imm16(0),
            },
        )?;
    }
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg8(Register8::CL),
            src: Operand::Imm8(0x10),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Push {
            src: Operand::Sreg(SegmentRegister::DS),
        },
    )?;
    emit_external_call(assembler, byte_len, origin, DECOMPRESS_RUNTIME_ADDRESS)?;
    emit(
        assembler,
        byte_len,
        Instruction::Pop {
            dest: Operand::Sreg(SegmentRegister::DS),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::ES),
            src: direct_operand(
                Some(SegmentRegister::CS),
                FONT_BUFFER_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        },
    )?;
    for (register, value) in [
        (Register16::DI, 0),
        (Register16::CX, SELECT_FONT_COPY_WORD_COUNT),
        (Register16::SI, 0),
    ] {
        emit(
            assembler,
            byte_len,
            Instruction::Mov {
                dest: Operand::Reg16(register),
                src: Operand::Imm16(value),
            },
        )?;
    }
    emit(assembler, byte_len, Instruction::Cld)?;
    emit(
        assembler,
        byte_len,
        Instruction::Rep(Box::new(Instruction::Movsw)),
    )
}

fn emit_restore_context(assembler: &mut Assembler, byte_len: &mut usize) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Pop {
            dest: Operand::Sreg(SegmentRegister::ES),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Pop {
            dest: Operand::Sreg(SegmentRegister::DS),
        },
    )?;
    emit(assembler, byte_len, Instruction::Popa)?;
    emit(assembler, byte_len, Instruction::Popf)
}

fn emit_short_branch(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    condition: Condition,
    label: &str,
) -> Result<()> {
    *byte_len = byte_len
        .checked_add(2)
        .context("SELECT stage-font hook branch length overflow")?;
    assembler.emit_branch(condition, label);
    Ok(())
}

fn copy_entry_field(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    displacement: i16,
    destination: u16,
) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: based_operand(EffectiveAddressBase::Bx, displacement, OperandSize::Word)?,
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: direct_operand(Some(SegmentRegister::CS), destination, OperandSize::Word)?,
            src: Operand::Reg16(Register16::AX),
        },
    )
}

fn emit_mov_ax_imm_store(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    value: u16,
    destination: u16,
) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: Operand::Imm16(value),
        },
    )?;
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: direct_operand(None, destination, OperandSize::Word)?,
            src: Operand::Reg16(Register16::AX),
        },
    )
}

fn emit_external_call(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
    target: u16,
) -> Result<()> {
    let current = origin
        .checked_add(u16::try_from(*byte_len).context("SELECT hook offset exceeds 16 bits")?)
        .context("SELECT hook call address overflow")?;
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
        .checked_add(u16::try_from(*byte_len).context("SELECT hook offset exceeds 16 bits")?)
        .context("SELECT hook jump address overflow")?;
    emit(
        assembler,
        byte_len,
        Instruction::Jmp {
            target: JmpTarget::Rel16(relative_near_displacement(current, target)?),
        },
    )
}

fn relative_near_displacement(current: u16, target: u16) -> Result<i16> {
    let next = i32::from(current) + 3;
    i16::try_from(i32::from(target) - next).context("SELECT near target is out of range")
}

fn emit(assembler: &mut Assembler, byte_len: &mut usize, instruction: Instruction) -> Result<()> {
    *byte_len = byte_len
        .checked_add(encode_bytes(&instruction)?.len())
        .context("SELECT stage-font hook length overflow")?;
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

fn based_operand(
    base: EffectiveAddressBase,
    displacement: i16,
    size: OperandSize,
) -> Result<Operand> {
    Ok(Operand::Mem(EffectiveAddress::new(
        None,
        base,
        EffectiveAddressDisplacement::Signed(displacement),
        size,
    )?))
}

fn assemble_at(assembler: &Assembler, runtime_address: u16) -> Result<v30::AssembledProgram> {
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: runtime_address,
        })
        .context("assemble typed V30 SELECT stage-font program")
}
