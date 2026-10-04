use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    ResizePlan, WriteIntent, WritePlan,
};
use v30::{
    Assembler, CallTarget, CodeLocation, Condition, EffectiveAddress, EffectiveAddressBase,
    EffectiveAddressDisplacement, Instruction, Operand, OperandSize, Register16, SegmentRegister,
    ShiftCount, encode_bytes,
};

use crate::game_data::ending_meal_scene::{FRAME_COUNT, MAP_RECORD_SIZE, OUTPUT_BANK_DECODED_SIZE};

use super::apply_v30_patch_plan;

const COM_ORIGIN: usize = 0x100;
pub(super) const MAP_FILE_OFFSET: usize = 0x3d8e;
const MAP_BYTE_SIZE: usize = FRAME_COUNT * MAP_RECORD_SIZE;
pub(super) const TILE_ADDRESS_FILE_OFFSET: usize = 0x314a;
const TILE_ADDRESS_RUNTIME_ADDRESS: u16 = 0x324a;
const STATE_SELECTOR_RUNTIME_ADDRESS: u16 = 0x3d8f;
const SOURCE_TILE_ADDRESS_BYTES: [u8; 10] =
    [0x8b, 0xf0, 0xc1, 0xe0, 0x07, 0xc1, 0xe6, 0x02, 0x03, 0xf0];
const HOOK_SITE_SOURCE_ID: &str = "ending-meal-split-bank-hook-site";
const HOOK_BODY_SOURCE_ID: &str = "ending-meal-split-bank-address-selector";

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PatchedEndingMealConsumer {
    pub bytes: Vec<u8>,
    pub hook_file_offset: usize,
    pub hook_runtime_address: usize,
    pub hook_byte_size: usize,
}

pub(super) fn replace_ending_meal_maps_and_install_bank_selector(
    bytes: &[u8],
    replacement_maps: &[Vec<u8>; FRAME_COUNT],
) -> Result<PatchedEndingMealConsumer> {
    ensure!(
        replacement_maps
            .iter()
            .all(|map| map.len() == MAP_RECORD_SIZE),
        "ending meal replacement maps must contain four complete 16x11 records"
    );
    let replacement_maps = replacement_maps.concat();
    let expected_maps = bytes
        .get(MAP_FILE_OFFSET..MAP_FILE_OFFSET + MAP_BYTE_SIZE)
        .context("ENDING.COM is shorter than its four ending meal maps")?
        .to_vec();

    verify_source_tile_address(bytes)?;

    let hook_file_offset = bytes.len();
    let hook_runtime_address = hook_file_offset
        .checked_add(COM_ORIGIN)
        .context("ENDING meal bank hook runtime address overflow")?;
    let hook_runtime_address_u16 = u16::try_from(hook_runtime_address)
        .context("ENDING meal bank hook exceeds the COM code segment")?;
    let hook_body = assemble_hook_body(hook_runtime_address_u16)?;
    let hook_site = assemble_hook_site(
        TILE_ADDRESS_RUNTIME_ADDRESS,
        hook_runtime_address_u16,
        SOURCE_TILE_ADDRESS_BYTES.len(),
    )?;
    ensure!(
        hook_site.bytes().len() == SOURCE_TILE_ADDRESS_BYTES.len(),
        "ENDING meal bank hook changed the tile-address site width"
    );
    let output_len = bytes
        .len()
        .checked_add(hook_body.bytes().len())
        .context("ENDING meal bank hook output size overflow")?;

    let plan = WritePlan::new()
        .resize(ResizePlan {
            owner: "arle-ending-meal-consumer-patch".into(),
            purpose: "append the state-aware second-bank tile-address selector".into(),
            expected_input_len: bytes.len(),
            output_len,
        })
        .region(ImageRegion {
            id: "ending-meal-four-state-maps".into(),
            range: MAP_FILE_OFFSET..MAP_FILE_OFFSET + MAP_BYTE_SIZE,
            kind: RegionKind::Data,
            reason: "four fixed 16x11 maps selected by the meal-caption state commands".into(),
        })
        .region(ImageRegion {
            id: "ending-meal-split-bank-hook-site".into(),
            range: TILE_ADDRESS_FILE_OFFSET
                ..TILE_ADDRESS_FILE_OFFSET + SOURCE_TILE_ADDRESS_BYTES.len(),
            kind: RegionKind::MachineCode,
            reason: "delegate tile record address calculation to the split-bank selector".into(),
        })
        .region(ImageRegion {
            id: "ending-meal-split-bank-hook-body".into(),
            range: hook_file_offset..output_len,
            kind: RegionKind::MachineCode,
            reason:
                "preserve the original ID x 132 calculation and add bank two for states 3 and 4"
                    .into(),
        })
        .write(ExpectedWrite {
            id: "replace-ending-meal-four-state-maps".into(),
            owner: "arle-ending-meal-consumer-patch".into(),
            purpose: "bind the four approved semantic scenes to their original state numbers"
                .into(),
            offset: MAP_FILE_OFFSET,
            expected_original: expected_maps,
            replacement: replacement_maps,
            intent: WriteIntent::Data,
        })
        .write(ExpectedWrite {
            id: "install-ending-meal-split-bank-call".into(),
            owner: "arle-ending-meal-consumer-patch".into(),
            purpose:
                "replace the verified tile-address calculation with a width-preserving typed call"
                    .into(),
            offset: TILE_ADDRESS_FILE_OFFSET,
            expected_original: SOURCE_TILE_ADDRESS_BYTES.to_vec(),
            replacement: hook_site.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: HOOK_SITE_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "append-ending-meal-split-bank-selector".into(),
            owner: "arle-ending-meal-consumer-patch".into(),
            purpose: "add the second decoded-bank base only for semantic states 3 and 4".into(),
            offset: hook_file_offset,
            expected_original: Vec::new(),
            replacement: hook_body.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: HOOK_BODY_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        });

    let hook_site_source = hook_site.clone();
    let hook_body_source = hook_body.clone();
    let output = apply_v30_patch_plan(bytes, &plan, move |source_id| match source_id {
        HOOK_SITE_SOURCE_ID => Ok(hook_site_source.clone()),
        HOOK_BODY_SOURCE_ID => Ok(hook_body_source.clone()),
        _ => Err(MachineCodeVerifierError::new(format!(
            "unknown ENDING meal V30 source {source_id}"
        ))),
    })?;
    ensure!(
        output[hook_file_offset..] == *hook_body.bytes(),
        "ENDING meal bank hook did not survive its Expected Write"
    );

    Ok(PatchedEndingMealConsumer {
        bytes: output,
        hook_file_offset,
        hook_runtime_address,
        hook_byte_size: hook_body.bytes().len(),
    })
}

fn verify_source_tile_address(bytes: &[u8]) -> Result<()> {
    let source = bytes
        .get(TILE_ADDRESS_FILE_OFFSET..TILE_ADDRESS_FILE_OFFSET + SOURCE_TILE_ADDRESS_BYTES.len())
        .context("ENDING.COM is shorter than the meal tile-address calculation")?;
    ensure!(
        source == SOURCE_TILE_ADDRESS_BYTES,
        "ENDING meal tile-address calculation differs from the verified source bytes"
    );

    let expected = assemble_original_tile_address()?;
    let mut consumed = 0;
    for expected_span in expected.instruction_spans() {
        let decoded = v30::decode_bytes(
            source
                .get(consumed..)
                .context("ENDING meal tile-address calculation is truncated")?,
        )
        .context("decode ENDING meal tile-address calculation as typed V30")?;
        ensure!(
            decoded.instruction == expected_span.instruction,
            "ENDING meal tile-address calculation differs from the verified typed V30 source"
        );
        consumed += decoded.byte_len;
    }
    ensure!(
        consumed == SOURCE_TILE_ADDRESS_BYTES.len(),
        "ENDING meal tile-address calculation has an unexpected instruction width"
    );
    Ok(())
}

fn assemble_original_tile_address() -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler
        .emit(Instruction::Mov {
            dest: Operand::Reg16(Register16::SI),
            src: Operand::Reg16(Register16::AX),
        })
        .emit(Instruction::Shl {
            dest: Operand::Reg16(Register16::AX),
            count: ShiftCount::Imm8(7),
        })
        .emit(Instruction::Shl {
            dest: Operand::Reg16(Register16::SI),
            count: ShiftCount::Imm8(2),
        })
        .emit(Instruction::Add {
            dest: Operand::Reg16(Register16::SI),
            src: Operand::Reg16(Register16::AX),
        });
    assemble_at(&assembler, TILE_ADDRESS_RUNTIME_ADDRESS)
}

fn assemble_hook_site(
    site_runtime_address: u16,
    hook_runtime_address: u16,
    replaced_byte_size: usize,
) -> Result<v30::AssembledProgram> {
    ensure!(
        replaced_byte_size >= 3,
        "ENDING meal tile-address site is shorter than a near call"
    );
    let displacement = relative_near_displacement(site_runtime_address, hook_runtime_address)?;
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Call {
        target: CallTarget::Rel16(displacement),
    });
    for _ in 3..replaced_byte_size {
        assembler.emit(Instruction::Nop);
    }
    assemble_at(&assembler, site_runtime_address)
}

fn assemble_hook_body(origin: u16) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    let mut byte_len = 0_usize;
    for instruction in [
        Instruction::Mov {
            dest: Operand::Reg16(Register16::SI),
            src: Operand::Reg16(Register16::AX),
        },
        Instruction::Shl {
            dest: Operand::Reg16(Register16::AX),
            count: ShiftCount::Imm8(7),
        },
        Instruction::Shl {
            dest: Operand::Reg16(Register16::SI),
            count: ShiftCount::Imm8(2),
        },
        Instruction::Add {
            dest: Operand::Reg16(Register16::SI),
            src: Operand::Reg16(Register16::AX),
        },
    ] {
        emit(&mut assembler, &mut byte_len, instruction)?;
    }
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Cmp {
            a: direct_operand(
                Some(SegmentRegister::CS),
                STATE_SELECTOR_RUNTIME_ADDRESS,
                OperandSize::Byte,
            )?,
            b: Operand::Imm8(3),
        },
    )?;
    emit_short_branch(&mut assembler, &mut byte_len, Condition::B, "return")?;
    emit(
        &mut assembler,
        &mut byte_len,
        Instruction::Add {
            dest: Operand::Reg16(Register16::SI),
            src: Operand::Imm16(
                u16::try_from(OUTPUT_BANK_DECODED_SIZE)
                    .expect("the second ending meal bank starts below 64 KiB"),
            ),
        },
    )?;
    assembler.label("return");
    emit(&mut assembler, &mut byte_len, Instruction::Ret { pop: 0 })?;
    assemble_at(&assembler, origin)
}

fn relative_near_displacement(current: u16, target: u16) -> Result<i16> {
    let next = i32::from(current) + 3;
    i16::try_from(i32::from(target) - next).context("ENDING meal near target is out of range")
}

fn emit_short_branch(
    assembler: &mut Assembler,
    byte_len: &mut usize,
    condition: Condition,
    label: &str,
) -> Result<()> {
    *byte_len = byte_len
        .checked_add(2)
        .context("ENDING meal hook branch length overflow")?;
    assembler.emit_branch(condition, label);
    Ok(())
}

fn emit(assembler: &mut Assembler, byte_len: &mut usize, instruction: Instruction) -> Result<()> {
    *byte_len = byte_len
        .checked_add(encode_bytes(&instruction)?.len())
        .context("ENDING meal hook length overflow")?;
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
        .context("assemble typed V30 ENDING meal consumer patch")
}

#[cfg(test)]
#[path = "arle_ending_meal_consumer_tests.rs"]
mod arle_ending_meal_consumer_tests;
