use std::collections::BTreeMap;

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

use crate::game_data::large_portrait::{DECODED_TILE_BANK_SIZE, DECODED_TILE_MAP_SIZE};
use crate::patch::apply_v30_patch_plan;

const COM_ORIGIN: usize = 0x100;
const ALLOCATION_CALL_FILE_OFFSET: usize = 0x1bad;
const ALLOCATION_CALL_RUNTIME_ADDRESS: u16 = 0x1cad;
const ORIGINAL_INITIALIZER_RUNTIME_ADDRESS: u16 = 0x34b3;
const CLEANUP_HOOK_SITE_FILE_OFFSET: usize = 0x1e4e;
const CLEANUP_HOOK_SITE_RUNTIME_ADDRESS: u16 = 0x1f4e;
const CLEANUP_CONTINUATION_RUNTIME_ADDRESS: u16 = 0x1f53;
const CLEANUP_START_RUNTIME_ADDRESS: u16 = 0x1f41;
const EXISTING_TEMPORARY_SEGMENT_ADDRESS: u16 = 0x4fe9;

const LOAD_HOOK_SITE_FILE_OFFSET: usize = 0x2f75;
const LOAD_HOOK_SITE_RUNTIME_ADDRESS: u16 = 0x3075;
const DRAW_CONTINUATION_RUNTIME_ADDRESS: u16 = 0x3158;
const LOAD_ERROR_RETURN_RUNTIME_ADDRESS: u16 = 0x319c;
const TILE_RENDERER_FILE_OFFSET: usize = 0x115b;
const TILE_RENDERER_RUNTIME_ADDRESS: u16 = 0x125b;
const INITIAL_BANK_SEGMENT_LOAD_FILE_OFFSET: usize = 0x305e;
const INITIAL_BANK_SEGMENT_LOAD_RUNTIME_ADDRESS: u16 = 0x315e;
const INITIAL_MAP_POINTER_FILE_OFFSET: usize = 0x3063;
const INITIAL_MAP_POINTER_RUNTIME_ADDRESS: u16 = 0x3163;
const STATE_BANK_SEGMENT_LOAD_FILE_OFFSET: usize = 0x30a5;
const STATE_BANK_SEGMENT_LOAD_RUNTIME_ADDRESS: u16 = 0x31a5;
const STATE_MAP_POINTER_FILE_OFFSET: usize = 0x30aa;
const STATE_MAP_POINTER_RUNTIME_ADDRESS: u16 = 0x31aa;
const INITIAL_TILE_ID_LOAD_FILE_OFFSET: usize = 0x3075;
const INITIAL_TILE_ID_LOAD_RUNTIME_ADDRESS: u16 = 0x3175;
const STATE_TILE_ID_LOAD_FILE_OFFSET: usize = 0x30c7;
const STATE_TILE_ID_LOAD_RUNTIME_ADDRESS: u16 = 0x31c7;

const MADDAT_TABLE_SEGMENT_ADDRESS: u16 = 0x088f;
const FILE_READ_SIZE_ADDRESS: u16 = 0x0873;
const FILE_READ_OFFSET_LOW_ADDRESS: u16 = 0x0871;
const FILE_READ_OFFSET_HIGH_ADDRESS: u16 = 0x086f;
const FILE_NAME_POINTER_ADDRESS: u16 = 0x086d;
const FILE_NAME_RUNTIME_ADDRESS: u16 = 0x42bf;
const FILE_READ_DESTINATION_SEGMENT_ADDRESS: u16 = 0x0869;
const FILE_READ_DESTINATION_OFFSET_ADDRESS: u16 = 0x086b;
const ORIGINAL_RESOURCE_SEGMENT_ADDRESS: u16 = 0x4283;
const FILE_READ_RUNTIME_ADDRESS: u16 = 0x03ae;
const DECOMPRESS_RUNTIME_ADDRESS: u16 = 0x158c;
const PORTRAIT_BANK_ENTRY_ID: u16 = 101;
const PORTRAIT_MAP_ENTRY_ID: u16 = 129;
const PORTRAIT_BANK_OFFSET: u16 = 0x0000;
const PORTRAIT_MAP_OFFSET: u16 = DECODED_TILE_BANK_SIZE as u16;
const PACKED_ENTRY_OFFSET: u16 =
    (DECODED_TILE_BANK_SIZE + DECODED_TILE_MAP_SIZE).div_ceil(16) as u16 * 16;

const ALLOCATION_CALL_SOURCE_ID: &str = "select-large-portrait-allocation-call";
const LOAD_JUMP_SOURCE_ID: &str = "select-large-portrait-load-jump";
const CLEANUP_JUMP_SOURCE_ID: &str = "select-large-portrait-cleanup-jump";
const INITIAL_BANK_SEGMENT_SOURCE_ID: &str = "select-initial-large-portrait-bank-segment";
const INITIAL_MAP_POINTER_SOURCE_ID: &str = "select-initial-large-portrait-map-pointer";
const STATE_BANK_SEGMENT_SOURCE_ID: &str = "select-state-large-portrait-bank-segment";
const STATE_MAP_POINTER_SOURCE_ID: &str = "select-state-large-portrait-map-pointer";
const HOOK_BODY_SOURCE_ID: &str = "select-large-portrait-resource-loader";

#[derive(Debug)]
pub(super) struct PatchedSelectPortraitConsumer {
    pub bytes: Vec<u8>,
    pub hook_file_offset: usize,
    pub hook_runtime_address: usize,
    pub hook_byte_size: usize,
    pub portrait_segment_byte_size: usize,
}

pub(super) fn install_expanded_portrait_bank(
    bytes: &[u8],
    packed_tile_bank_size: usize,
    packed_tile_map_size: usize,
) -> Result<PatchedSelectPortraitConsumer> {
    let portrait_segment_paragraphs =
        portrait_segment_paragraphs(packed_tile_bank_size.max(packed_tile_map_size))?;
    let portrait_segment_byte_size = usize::from(portrait_segment_paragraphs) * 16;
    ensure!(
        portrait_segment_byte_size
            >= usize::from(PACKED_ENTRY_OFFSET) + packed_tile_bank_size.max(packed_tile_map_size),
        "SELECT portrait resource segment does not cover the decoded resources and packed staging area"
    );
    let hook_file_offset = bytes.len();
    let hook_runtime_address = hook_file_offset
        .checked_add(COM_ORIGIN)
        .context("SELECT large-portrait hook runtime address overflow")?;
    let hook_runtime_address_u16 = u16::try_from(hook_runtime_address)
        .context("SELECT large-portrait hook exceeds the COM code segment")?;

    let provisional_hook =
        assemble_hook_body(hook_runtime_address_u16, 0, portrait_segment_paragraphs)?;
    let segment_word_runtime_address = hook_runtime_address_u16
        .checked_add(
            u16::try_from(provisional_hook.bytes().len())
                .context("SELECT large-portrait hook length exceeds 16 bits")?,
        )
        .context("SELECT large-portrait segment word address overflow")?;
    let hook_body = assemble_hook_body(
        hook_runtime_address_u16,
        segment_word_runtime_address,
        portrait_segment_paragraphs,
    )?;
    ensure!(
        hook_body.bytes().len() == provisional_hook.bytes().len(),
        "SELECT large-portrait hook length changed after binding its segment word"
    );

    let allocation_runtime_address = hook_label(&hook_body, "allocate-portrait-segment")?;
    let load_runtime_address = hook_label(&hook_body, "load-portrait-resources")?;
    let cleanup_runtime_address = hook_label(&hook_body, "release-portrait-segment")?;

    let original_allocation_call = assemble_call_site(
        ALLOCATION_CALL_RUNTIME_ADDRESS,
        ORIGINAL_INITIALIZER_RUNTIME_ADDRESS,
    )?;
    let allocation_call =
        assemble_call_site(ALLOCATION_CALL_RUNTIME_ADDRESS, allocation_runtime_address)?;
    let original_load_site = assemble_original_load_site()?;
    let load_jump = assemble_jump_site(LOAD_HOOK_SITE_RUNTIME_ADDRESS, load_runtime_address)?;
    let original_cleanup_site = assemble_original_cleanup_site()?;
    let cleanup_jump =
        assemble_jump_site(CLEANUP_HOOK_SITE_RUNTIME_ADDRESS, cleanup_runtime_address)?;
    let original_initial_bank_segment = assemble_bank_segment_load(
        INITIAL_BANK_SEGMENT_LOAD_RUNTIME_ADDRESS,
        ORIGINAL_RESOURCE_SEGMENT_ADDRESS,
    )?;
    let initial_bank_segment = assemble_bank_segment_load(
        INITIAL_BANK_SEGMENT_LOAD_RUNTIME_ADDRESS,
        segment_word_runtime_address,
    )?;
    let original_initial_map_pointer =
        assemble_map_pointer(INITIAL_MAP_POINTER_RUNTIME_ADDRESS, 0x4a38)?;
    let initial_map_pointer =
        assemble_map_pointer(INITIAL_MAP_POINTER_RUNTIME_ADDRESS, PORTRAIT_MAP_OFFSET)?;
    let original_state_bank_segment = assemble_bank_segment_load(
        STATE_BANK_SEGMENT_LOAD_RUNTIME_ADDRESS,
        ORIGINAL_RESOURCE_SEGMENT_ADDRESS,
    )?;
    let state_bank_segment = assemble_bank_segment_load(
        STATE_BANK_SEGMENT_LOAD_RUNTIME_ADDRESS,
        segment_word_runtime_address,
    )?;
    let original_state_map_pointer =
        assemble_map_pointer(STATE_MAP_POINTER_RUNTIME_ADDRESS, 0x4a38)?;
    let state_map_pointer =
        assemble_map_pointer(STATE_MAP_POINTER_RUNTIME_ADDRESS, PORTRAIT_MAP_OFFSET)?;
    let tile_renderer = assemble_tile_renderer_entry()?;
    let initial_tile_id_load = assemble_tile_id_load(INITIAL_TILE_ID_LOAD_RUNTIME_ADDRESS)?;
    let state_tile_id_load = assemble_tile_id_load(STATE_TILE_ID_LOAD_RUNTIME_ADDRESS)?;

    verify_source_bytes(
        bytes,
        ALLOCATION_CALL_FILE_OFFSET,
        &original_allocation_call,
        "SELECT large-portrait allocation call",
    )?;
    verify_source_bytes(
        bytes,
        LOAD_HOOK_SITE_FILE_OFFSET,
        &original_load_site,
        "SELECT large-portrait loader entry",
    )?;
    verify_source_bytes(
        bytes,
        CLEANUP_HOOK_SITE_FILE_OFFSET,
        &original_cleanup_site,
        "SELECT large-portrait cleanup entry",
    )?;
    verify_source_bytes(
        bytes,
        INITIAL_BANK_SEGMENT_LOAD_FILE_OFFSET,
        &original_initial_bank_segment,
        "SELECT initial large-portrait bank segment load",
    )?;
    verify_source_bytes(
        bytes,
        INITIAL_MAP_POINTER_FILE_OFFSET,
        &original_initial_map_pointer,
        "SELECT initial large-portrait map pointer",
    )?;
    verify_source_bytes(
        bytes,
        STATE_BANK_SEGMENT_LOAD_FILE_OFFSET,
        &original_state_bank_segment,
        "SELECT state large-portrait bank segment load",
    )?;
    verify_source_bytes(
        bytes,
        STATE_MAP_POINTER_FILE_OFFSET,
        &original_state_map_pointer,
        "SELECT state large-portrait map pointer",
    )?;
    verify_source_bytes(
        bytes,
        TILE_RENDERER_FILE_OFFSET,
        &tile_renderer,
        "SELECT large-portrait 8-bit tile renderer",
    )?;
    verify_source_bytes(
        bytes,
        INITIAL_TILE_ID_LOAD_FILE_OFFSET,
        &initial_tile_id_load,
        "SELECT initial large-portrait tile-ID load",
    )?;
    verify_source_bytes(
        bytes,
        STATE_TILE_ID_LOAD_FILE_OFFSET,
        &state_tile_id_load,
        "SELECT state large-portrait tile-ID load",
    )?;

    let segment_word_file_offset = hook_file_offset + hook_body.bytes().len();
    let output_len = segment_word_file_offset
        .checked_add(2)
        .context("SELECT large-portrait patch size overflow")?;
    let plan = WritePlan::new()
        .resize(ResizePlan {
            owner: "arle-large-portrait-consumer-patch".into(),
            purpose: "append the SELECT 256-tile portrait loader and its segment word".into(),
            expected_input_len: bytes.len(),
            output_len,
        })
        .region(code_region(
            "select-large-portrait-allocation-call",
            ALLOCATION_CALL_FILE_OFFSET,
            original_allocation_call.bytes().len(),
            "allocate one segment for decoded portrait resources and packed staging",
        ))
        .region(code_region(
            "select-large-portrait-load-jump",
            LOAD_HOOK_SITE_FILE_OFFSET,
            original_load_site.bytes().len(),
            "load entries 101 and 129 into the separate portrait segment",
        ))
        .region(code_region(
            "select-large-portrait-cleanup-jump",
            CLEANUP_HOOK_SITE_FILE_OFFSET,
            original_cleanup_site.bytes().len(),
            "release the separate portrait segment before SELECT exits",
        ))
        .region(code_region(
            "select-initial-large-portrait-bank-segment",
            INITIAL_BANK_SEGMENT_LOAD_FILE_OFFSET,
            original_initial_bank_segment.bytes().len(),
            "draw the initial portrait from the separate bank segment",
        ))
        .region(code_region(
            "select-initial-large-portrait-map-pointer",
            INITIAL_MAP_POINTER_FILE_OFFSET,
            original_initial_map_pointer.bytes().len(),
            "read the initial portrait map after the expanded bank",
        ))
        .region(code_region(
            "select-state-large-portrait-bank-segment",
            STATE_BANK_SEGMENT_LOAD_FILE_OFFSET,
            original_state_bank_segment.bytes().len(),
            "draw later health states from the separate bank segment",
        ))
        .region(code_region(
            "select-state-large-portrait-map-pointer",
            STATE_MAP_POINTER_FILE_OFFSET,
            original_state_map_pointer.bytes().len(),
            "read later health-state maps after the expanded bank",
        ))
        .region(code_region(
            "select-large-portrait-resource-loader",
            hook_file_offset,
            hook_body.bytes().len(),
            "typed allocation, MADDAT read, Compile LZ decode, and cleanup flow",
        ))
        .region(ImageRegion {
            id: "select-large-portrait-segment-word".into(),
            range: segment_word_file_offset..output_len,
            kind: RegionKind::Data,
            reason: "runtime segment returned by DOS allocation for the portrait bank".into(),
        })
        .write(machine_code_write(
            "install-select-large-portrait-allocation-call",
            ALLOCATION_CALL_SOURCE_ID,
            ALLOCATION_CALL_FILE_OFFSET,
            original_allocation_call.into_bytes(),
            allocation_call.bytes().to_vec(),
            "route SELECT initialization through the portrait segment allocator",
        ))
        .write(machine_code_write(
            "install-select-large-portrait-load-jump",
            LOAD_JUMP_SOURCE_ID,
            LOAD_HOOK_SITE_FILE_OFFSET,
            original_load_site.into_bytes(),
            load_jump.bytes().to_vec(),
            "replace the overlapping common-buffer load with the separate-bank loader",
        ))
        .write(machine_code_write(
            "install-select-large-portrait-cleanup-jump",
            CLEANUP_JUMP_SOURCE_ID,
            CLEANUP_HOOK_SITE_FILE_OFFSET,
            original_cleanup_site.into_bytes(),
            cleanup_jump.bytes().to_vec(),
            "release the separate portrait segment through the existing cleanup path",
        ))
        .write(machine_code_write(
            "bind-select-initial-large-portrait-bank",
            INITIAL_BANK_SEGMENT_SOURCE_ID,
            INITIAL_BANK_SEGMENT_LOAD_FILE_OFFSET,
            original_initial_bank_segment.into_bytes(),
            initial_bank_segment.bytes().to_vec(),
            "select the expanded bank before drawing the initial portrait",
        ))
        .write(machine_code_write(
            "bind-select-initial-large-portrait-map",
            INITIAL_MAP_POINTER_SOURCE_ID,
            INITIAL_MAP_POINTER_FILE_OFFSET,
            original_initial_map_pointer.into_bytes(),
            initial_map_pointer.bytes().to_vec(),
            "select the relocated map before drawing the initial portrait",
        ))
        .write(machine_code_write(
            "bind-select-state-large-portrait-bank",
            STATE_BANK_SEGMENT_SOURCE_ID,
            STATE_BANK_SEGMENT_LOAD_FILE_OFFSET,
            original_state_bank_segment.into_bytes(),
            state_bank_segment.bytes().to_vec(),
            "select the expanded bank before drawing a health state",
        ))
        .write(machine_code_write(
            "bind-select-state-large-portrait-map",
            STATE_MAP_POINTER_SOURCE_ID,
            STATE_MAP_POINTER_FILE_OFFSET,
            original_state_map_pointer.into_bytes(),
            state_map_pointer.bytes().to_vec(),
            "select the relocated map before drawing a health state",
        ))
        .write(machine_code_write(
            "append-select-large-portrait-resource-loader",
            HOOK_BODY_SOURCE_ID,
            hook_file_offset,
            Vec::new(),
            hook_body.bytes().to_vec(),
            "allocate, load, decode, and release the independent portrait resources",
        ))
        .write(ExpectedWrite {
            id: "append-select-large-portrait-segment-word".into(),
            owner: "arle-large-portrait-consumer-patch".into(),
            purpose: "reserve the SELECT runtime segment variable".into(),
            offset: segment_word_file_offset,
            expected_original: Vec::new(),
            replacement: vec![0, 0],
            intent: WriteIntent::Data,
        });

    let sources = BTreeMap::from([
        (ALLOCATION_CALL_SOURCE_ID, allocation_call.clone()),
        (LOAD_JUMP_SOURCE_ID, load_jump.clone()),
        (CLEANUP_JUMP_SOURCE_ID, cleanup_jump.clone()),
        (INITIAL_BANK_SEGMENT_SOURCE_ID, initial_bank_segment.clone()),
        (INITIAL_MAP_POINTER_SOURCE_ID, initial_map_pointer.clone()),
        (STATE_BANK_SEGMENT_SOURCE_ID, state_bank_segment.clone()),
        (STATE_MAP_POINTER_SOURCE_ID, state_map_pointer.clone()),
        (HOOK_BODY_SOURCE_ID, hook_body.clone()),
    ]);
    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| {
        sources.get(source_id).cloned().ok_or_else(|| {
            MachineCodeVerifierError::new(format!(
                "unknown SELECT large-portrait V30 source {source_id}"
            ))
        })
    })?;
    ensure!(
        output[hook_file_offset..segment_word_file_offset] == *hook_body.bytes(),
        "SELECT large-portrait hook body did not survive its Expected Write"
    );
    ensure!(
        output[segment_word_file_offset..output_len] == [0, 0],
        "SELECT large-portrait segment word was not zero-initialized"
    );

    Ok(PatchedSelectPortraitConsumer {
        bytes: output,
        hook_file_offset,
        hook_runtime_address,
        hook_byte_size: hook_body.bytes().len(),
        portrait_segment_byte_size,
    })
}

fn assemble_hook_body(
    origin: u16,
    segment_word_address: u16,
    portrait_segment_paragraphs: u16,
) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    let mut byte_len = 0_usize;

    assembler.label("allocate-portrait-segment");
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::BX),
            src: Operand::Imm16(portrait_segment_paragraphs),
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Imm8(0x48),
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Int { vector: 0x21 },
    )?;
    emit_short_branch(
        &mut assembler,
        &mut byte_len,
        Condition::Nb,
        "portrait-segment-allocated",
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Pop {
            dest: Operand::Reg16(Register16::AX),
        },
    )?;
    emit_external_jump(
        &mut assembler,
        &mut byte_len,
        origin,
        CLEANUP_START_RUNTIME_ADDRESS,
    )?;
    assembler.label("portrait-segment-allocated");
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: direct_operand(
                Some(SegmentRegister::CS),
                segment_word_address,
                OperandSize::Word,
            )?,
            src: Operand::Reg16(Register16::AX),
        },
    )?;
    emit_external_call(
        &mut assembler,
        &mut byte_len,
        origin,
        ORIGINAL_INITIALIZER_RUNTIME_ADDRESS,
    )?;
    emit(&mut assembler, &mut byte_len, Instruction::Ret { pop: 0 })?;

    assembler.label("load-portrait-resources");
    emit_load_entry(
        &mut assembler,
        &mut byte_len,
        origin,
        segment_word_address,
        PORTRAIT_BANK_ENTRY_ID,
        PORTRAIT_BANK_OFFSET,
        "portrait-bank-read-succeeded",
    )?;
    emit_load_entry(
        &mut assembler,
        &mut byte_len,
        origin,
        segment_word_address,
        PORTRAIT_MAP_ENTRY_ID,
        PORTRAIT_MAP_OFFSET,
        "portrait-map-read-succeeded",
    )?;
    emit_external_jump(
        &mut assembler,
        &mut byte_len,
        origin,
        DRAW_CONTINUATION_RUNTIME_ADDRESS,
    )?;

    assembler.label("release-portrait-segment");
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Push {
            src: Operand::Sreg(SegmentRegister::ES),
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: direct_operand(
                Some(SegmentRegister::CS),
                segment_word_address,
                OperandSize::Word,
            )?,
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Cmp {
            a: Operand::Reg16(Register16::AX),
            b: Operand::Imm16(0),
        },
    )?;
    emit_short_branch(
        &mut assembler,
        &mut byte_len,
        Condition::E,
        "portrait-segment-released",
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Sreg(SegmentRegister::ES),
            src: Operand::Reg16(Register16::AX),
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Reg8(Register8::AH),
            src: Operand::Imm8(0x49),
        },
    )?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Int { vector: 0x21 },
    )?;
    assembler.label("portrait-segment-released");
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: direct_operand(
                Some(SegmentRegister::CS),
                EXISTING_TEMPORARY_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        },
    )?;
    emit_external_jump(
        &mut assembler,
        &mut byte_len,
        origin,
        CLEANUP_CONTINUATION_RUNTIME_ADDRESS,
    )?;

    let program = assemble_at(&assembler, origin)?;
    ensure!(
        program.bytes().len() == byte_len,
        "SELECT large-portrait hook length tracking drifted"
    );
    Ok(program)
}

fn portrait_segment_paragraphs(max_packed_size: usize) -> Result<u16> {
    ensure!(
        max_packed_size > 0,
        "SELECT portrait packed staging area cannot be empty"
    );
    let required_byte_size = usize::from(PACKED_ENTRY_OFFSET)
        .checked_add(max_packed_size)
        .context("SELECT portrait segment size overflow")?;
    ensure!(
        required_byte_size <= 0x1_0000,
        "SELECT portrait decoded resources and packed staging area exceed one segment"
    );
    u16::try_from(required_byte_size.div_ceil(16))
        .context("SELECT portrait allocation exceeds the DOS paragraph field")
}

#[allow(clippy::too_many_arguments)]
fn emit_load_entry(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    origin: u16,
    segment_word_address: u16,
    entry_id: u16,
    decoded_offset: u16,
    success_label: &str,
) -> Result<()> {
    emit(
        assembler,
        byte_len,
        Instruction::Push {
            src: Operand::Sreg(SegmentRegister::DS),
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
    emit(
        assembler,
        byte_len,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: Operand::Imm16(entry_id),
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
    copy_entry_field(assembler, byte_len, 0, FILE_READ_SIZE_ADDRESS)?;
    copy_entry_field(assembler, byte_len, 2, FILE_READ_OFFSET_LOW_ADDRESS)?;
    copy_entry_field(assembler, byte_len, 4, FILE_READ_OFFSET_HIGH_ADDRESS)?;
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
                segment_word_address,
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
    emit_mov_ax_imm_store(
        assembler,
        byte_len,
        PACKED_ENTRY_OFFSET,
        FILE_READ_DESTINATION_OFFSET_ADDRESS,
    )?;
    emit_external_call(assembler, byte_len, origin, FILE_READ_RUNTIME_ADDRESS)?;
    emit(
        assembler,
        byte_len,
        Instruction::Pop {
            dest: Operand::Sreg(SegmentRegister::DS),
        },
    )?;
    emit_short_branch(assembler, byte_len, Condition::Nb, success_label)?;
    emit_external_jump(
        assembler,
        byte_len,
        origin,
        LOAD_ERROR_RETURN_RUNTIME_ADDRESS,
    )?;
    assembler.label(success_label);
    for segment in [SegmentRegister::DS, SegmentRegister::ES] {
        emit(
            assembler,
            byte_len,
            Instruction::Mov {
                dest: Operand::Sreg(segment),
                src: direct_operand(
                    Some(SegmentRegister::CS),
                    segment_word_address,
                    OperandSize::Word,
                )?,
            },
        )?;
    }
    for (register, value) in [
        (Register16::DI, decoded_offset),
        (Register16::BX, PACKED_ENTRY_OFFSET),
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
    )
}

fn assemble_original_load_site() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Push {
            src: Operand::Sreg(SegmentRegister::DS),
        })
        .emit(Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: direct_operand(
                Some(SegmentRegister::CS),
                MADDAT_TABLE_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        });
    assemble_at(&assembler, LOAD_HOOK_SITE_RUNTIME_ADDRESS)
}

fn assemble_original_cleanup_site() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Push {
            src: Operand::Sreg(SegmentRegister::ES),
        })
        .emit(Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: direct_operand(
                Some(SegmentRegister::CS),
                EXISTING_TEMPORARY_SEGMENT_ADDRESS,
                OperandSize::Word,
            )?,
        });
    assemble_at(&assembler, CLEANUP_HOOK_SITE_RUNTIME_ADDRESS)
}

fn assemble_call_site(runtime_address: u16, target: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Call {
        target: CallTarget::Rel16(relative_near_displacement(runtime_address, target)?),
    });
    assemble_at(&assembler, runtime_address)
}

fn assemble_jump_site(runtime_address: u16, target: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Jmp {
            target: JmpTarget::Rel16(relative_near_displacement(runtime_address, target)?),
        })
        .emit(Instruction::Nop)
        .emit(Instruction::Nop);
    assemble_at(&assembler, runtime_address)
}

fn assemble_bank_segment_load(
    runtime_address: u16,
    segment_word_address: u16,
) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Sreg(SegmentRegister::DS),
        src: direct_operand(
            Some(SegmentRegister::CS),
            segment_word_address,
            OperandSize::Word,
        )?,
    });
    assemble_at(&assembler, runtime_address)
}

fn assemble_map_pointer(runtime_address: u16, map_offset: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(Register16::SI),
        src: Operand::Imm16(map_offset),
    });
    assemble_at(&assembler, runtime_address)
}

fn assemble_tile_renderer_entry() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Push {
            src: Operand::Sreg(SegmentRegister::DS),
        })
        .emit(Instruction::Push {
            src: Operand::Sreg(SegmentRegister::ES),
        })
        .emit(Instruction::Shl {
            dest: Operand::Reg16(Register16::AX),
            count: ShiftCount::Imm8(7),
        });
    assemble_at(&assembler, TILE_RENDERER_RUNTIME_ADDRESS)
}

fn assemble_tile_id_load(runtime_address: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Lodsb).emit(Instruction::Mov {
        dest: Operand::Reg8(Register8::AH),
        src: Operand::Imm8(0),
    });
    assemble_at(&assembler, runtime_address)
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

fn code_region(id: &str, file_offset: usize, byte_len: usize, reason: &str) -> ImageRegion {
    ImageRegion {
        id: id.into(),
        range: file_offset..file_offset + byte_len,
        kind: RegionKind::MachineCode,
        reason: reason.into(),
    }
}

fn machine_code_write(
    id: &str,
    source_id: &str,
    offset: usize,
    expected_original: Vec<u8>,
    replacement: Vec<u8>,
    purpose: &str,
) -> ExpectedWrite {
    ExpectedWrite {
        id: id.into(),
        owner: "arle-large-portrait-consumer-patch".into(),
        purpose: purpose.into(),
        offset,
        expected_original,
        replacement,
        intent: WriteIntent::MachineCode(MachineCodeProvenance {
            assembly_source_id: source_id.into(),
            isa_profile_id: v30::PROFILE_ID.into(),
        }),
    }
}

fn hook_label(program: &v30::AssembledProgram, label: &str) -> Result<u16> {
    program
        .label_location(label)
        .map(|location| location.off)
        .with_context(|| format!("SELECT large-portrait hook is missing label {label}"))
}

fn emit_short_branch(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    condition: Condition,
    label: &str,
) -> Result<()> {
    *byte_len = byte_len
        .checked_add(2)
        .context("SELECT large-portrait hook branch length overflow")?;
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
        .checked_add(
            u16::try_from(*byte_len)
                .context("SELECT large-portrait hook offset exceeds 16 bits")?,
        )
        .context("SELECT large-portrait hook call address overflow")?;
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
        .checked_add(
            u16::try_from(*byte_len)
                .context("SELECT large-portrait hook offset exceeds 16 bits")?,
        )
        .context("SELECT large-portrait hook jump address overflow")?;
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
    i16::try_from(i32::from(target) - next)
        .context("SELECT large-portrait near target is out of range")
}

fn emit(assembler: &mut Assembler, byte_len: &mut usize, instruction: Instruction) -> Result<()> {
    *byte_len = byte_len
        .checked_add(encode_bytes(&instruction)?.len())
        .context("SELECT large-portrait hook length overflow")?;
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
        .context("assemble typed V30 SELECT large-portrait program")
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod select_tests;
