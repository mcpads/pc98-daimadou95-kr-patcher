use anyhow::{Context, Result, ensure};
use expected_write::{
    ExpectedWrite, ImageRegion, MachineCodeProvenance, MachineCodeVerifierError, RegionKind,
    WriteIntent, WritePlan,
};
use v30::{Assembler, CodeLocation, Instruction, Operand, Register16};

use crate::patch::apply_v30_patch_plan;

const COM_ORIGIN: usize = 0x100;
const WIDTH_INSTRUCTION_FILE_OFFSET: usize = 0x6165;
const WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET: usize = 0x616b;
const TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET: usize = 0x617d;
const SOURCE_INNER_TILE_COLUMNS: usize = 14;
const SOURCE_WINDOW_ORIGIN: u16 = 0x370e;
const SOURCE_TEXT_ORIGIN: u16 = 0x3990;
const GRAPHICS_ROW_BYTES: usize = 80;
const TILE_BYTE_WIDTH: usize = 2;
const TILE_PIXEL_WIDTH: usize = 16;
const BORDER_TILE_COLUMNS: usize = 2;
const WIDTH_SOURCE_ID: &str = "mad-result-banner-window-width";
const WINDOW_ORIGIN_SOURCE_ID: &str = "mad-result-banner-window-origin";
const TEXT_ORIGIN_SOURCE_ID: &str = "mad-result-banner-text-origin";

#[derive(Debug)]
pub(super) struct PatchedResultBannerWindow {
    pub bytes: Vec<u8>,
    pub report: MadResultBannerWindowReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MadResultBannerWindowReport {
    pub width_instruction_file_offset: usize,
    pub window_origin_instruction_file_offset: usize,
    pub text_origin_instruction_file_offset: usize,
    pub source_inner_tile_columns: usize,
    pub required_inner_tile_columns: usize,
    pub output_inner_tile_columns: usize,
    pub horizontal_shift_tile_columns: usize,
    pub horizontal_shift_pixels: usize,
}

pub(super) fn fit_result_banner_window(
    bytes: &[u8],
    required_inner_tile_columns: usize,
) -> Result<PatchedResultBannerWindow> {
    ensure!(
        required_inner_tile_columns > 0 && required_inner_tile_columns.is_multiple_of(2),
        "MAD composite result banner requires a positive, even tile width"
    );
    let output_inner_tile_columns = SOURCE_INNER_TILE_COLUMNS.max(required_inner_tile_columns);
    let width_growth = output_inner_tile_columns - SOURCE_INNER_TILE_COLUMNS;
    ensure!(
        width_growth.is_multiple_of(2),
        "MAD result-banner window growth must be symmetric in whole tile columns"
    );
    let horizontal_shift_tile_columns = width_growth / 2;
    let horizontal_shift_bytes = horizontal_shift_tile_columns
        .checked_mul(TILE_BYTE_WIDTH)
        .context("MAD result-banner horizontal shift overflow")?;
    let output_window_origin = usize::from(SOURCE_WINDOW_ORIGIN)
        .checked_sub(horizontal_shift_bytes)
        .context("MAD result-banner window cannot stay centered on the graphics row")?;
    let output_text_origin = usize::from(SOURCE_TEXT_ORIGIN)
        .checked_sub(horizontal_shift_bytes)
        .context("MAD result-banner text cannot stay centered on the graphics row")?;
    ensure!(
        output_window_origin / GRAPHICS_ROW_BYTES
            == usize::from(SOURCE_WINDOW_ORIGIN) / GRAPHICS_ROW_BYTES
            && output_text_origin / GRAPHICS_ROW_BYTES
                == usize::from(SOURCE_TEXT_ORIGIN) / GRAPHICS_ROW_BYTES,
        "MAD result-banner horizontal shift crossed a graphics row"
    );
    let output_byte_width = output_inner_tile_columns
        .checked_add(BORDER_TILE_COLUMNS)
        .and_then(|columns| columns.checked_mul(TILE_BYTE_WIDTH))
        .context("MAD result-banner window width overflow")?;
    let output_window_left_byte = output_window_origin % GRAPHICS_ROW_BYTES;
    let output_text_left_byte = output_text_origin % GRAPHICS_ROW_BYTES;
    ensure!(
        output_window_left_byte + output_byte_width <= GRAPHICS_ROW_BYTES,
        "MAD result-banner window requires {output_inner_tile_columns} inner tile columns but would exceed the graphics row"
    );
    ensure!(
        output_text_left_byte == output_window_left_byte + TILE_BYTE_WIDTH
            && output_text_left_byte + output_inner_tile_columns * TILE_BYTE_WIDTH
                == output_window_left_byte + output_byte_width - TILE_BYTE_WIDTH,
        "MAD result-banner text and centered window geometry differ"
    );

    let source_width = u16::try_from(SOURCE_INNER_TILE_COLUMNS)
        .context("MAD source result-banner width exceeds 16 bits")?;
    let expected_width = assemble_mov_immediate(
        WIDTH_INSTRUCTION_FILE_OFFSET,
        Register16::AX,
        source_width,
        "window width",
    )?;
    let expected_window_origin = assemble_mov_immediate(
        WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
        Register16::DI,
        SOURCE_WINDOW_ORIGIN,
        "window origin",
    )?;
    let expected_text_origin = assemble_mov_immediate(
        TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
        Register16::DI,
        SOURCE_TEXT_ORIGIN,
        "text origin",
    )?;
    verify_source_instruction(
        bytes,
        WIDTH_INSTRUCTION_FILE_OFFSET,
        &expected_width,
        "width",
    )?;
    verify_source_instruction(
        bytes,
        WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
        &expected_window_origin,
        "window origin",
    )?;
    verify_source_instruction(
        bytes,
        TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
        &expected_text_origin,
        "text origin",
    )?;

    let report = MadResultBannerWindowReport {
        width_instruction_file_offset: WIDTH_INSTRUCTION_FILE_OFFSET,
        window_origin_instruction_file_offset: WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
        text_origin_instruction_file_offset: TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
        source_inner_tile_columns: SOURCE_INNER_TILE_COLUMNS,
        required_inner_tile_columns,
        output_inner_tile_columns,
        horizontal_shift_tile_columns,
        horizontal_shift_pixels: horizontal_shift_tile_columns * TILE_PIXEL_WIDTH,
    };
    if output_inner_tile_columns == SOURCE_INNER_TILE_COLUMNS {
        return Ok(PatchedResultBannerWindow {
            bytes: bytes.to_vec(),
            report,
        });
    }

    let output_width = u16::try_from(output_inner_tile_columns)
        .context("MAD result-banner output width exceeds 16 bits")?;
    let output_window_origin = u16::try_from(output_window_origin)
        .context("MAD result-banner output window origin exceeds 16 bits")?;
    let output_text_origin = u16::try_from(output_text_origin)
        .context("MAD result-banner output text origin exceeds 16 bits")?;
    let replacement_width = assemble_mov_immediate(
        WIDTH_INSTRUCTION_FILE_OFFSET,
        Register16::AX,
        output_width,
        "window width",
    )?;
    let replacement_window_origin = assemble_mov_immediate(
        WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
        Register16::DI,
        output_window_origin,
        "window origin",
    )?;
    let replacement_text_origin = assemble_mov_immediate(
        TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
        Register16::DI,
        output_text_origin,
        "text origin",
    )?;
    ensure!(
        replacement_width.bytes().len() == expected_width.bytes().len()
            && replacement_window_origin.bytes().len() == expected_window_origin.bytes().len()
            && replacement_text_origin.bytes().len() == expected_text_origin.bytes().len(),
        "MAD result-banner layout changed an instruction size"
    );
    let width_write_len = replacement_width.bytes().len();
    let window_origin_write_len = replacement_window_origin.bytes().len();
    let text_origin_write_len = replacement_text_origin.bytes().len();
    let plan = WritePlan::new()
        .region(ImageRegion {
            id: "mad-result-banner-window-width".into(),
            range: WIDTH_INSTRUCTION_FILE_OFFSET..WIDTH_INSTRUCTION_FILE_OFFSET + width_write_len,
            kind: RegionKind::MachineCode,
            reason: "fit the composite Korean result banner inside its framed window".into(),
        })
        .region(ImageRegion {
            id: "mad-result-banner-window-origin".into(),
            range: WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET
                ..WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET + window_origin_write_len,
            kind: RegionKind::MachineCode,
            reason: "keep the expanded result window centered on the play field".into(),
        })
        .region(ImageRegion {
            id: "mad-result-banner-text-origin".into(),
            range: TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET
                ..TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET + text_origin_write_len,
            kind: RegionKind::MachineCode,
            reason: "keep the composite result text centered inside its expanded window".into(),
        })
        .write(ExpectedWrite {
            id: "fit-mad-result-banner-window".into(),
            owner: "mad-result-banner-window-compiler".into(),
            purpose: "derive the framed window width from the compiled composite banner".into(),
            offset: WIDTH_INSTRUCTION_FILE_OFFSET,
            expected_original: expected_width.into_bytes(),
            replacement: replacement_width.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: WIDTH_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "center-mad-result-banner-window".into(),
            owner: "mad-result-banner-window-compiler".into(),
            purpose: "split the added window width equally across both sides".into(),
            offset: WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
            expected_original: expected_window_origin.into_bytes(),
            replacement: replacement_window_origin.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: WINDOW_ORIGIN_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        })
        .write(ExpectedWrite {
            id: "center-mad-result-banner-text".into(),
            owner: "mad-result-banner-window-compiler".into(),
            purpose: "move the composite text with its centered result window".into(),
            offset: TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
            expected_original: expected_text_origin.into_bytes(),
            replacement: replacement_text_origin.bytes().to_vec(),
            intent: WriteIntent::MachineCode(MachineCodeProvenance {
                assembly_source_id: TEXT_ORIGIN_SOURCE_ID.into(),
                isa_profile_id: v30::PROFILE_ID.into(),
            }),
        });
    let width_source = replacement_width.clone();
    let window_origin_source = replacement_window_origin.clone();
    let text_origin_source = replacement_text_origin.clone();
    let bytes = apply_v30_patch_plan(bytes, &plan, move |source_id| match source_id {
        WIDTH_SOURCE_ID => Ok(width_source.clone()),
        WINDOW_ORIGIN_SOURCE_ID => Ok(window_origin_source.clone()),
        TEXT_ORIGIN_SOURCE_ID => Ok(text_origin_source.clone()),
        _ => Err(MachineCodeVerifierError::new(format!(
            "unknown MAD result-banner V30 source {source_id}"
        ))),
    })?;
    verify_output_instruction(
        &bytes,
        WIDTH_INSTRUCTION_FILE_OFFSET,
        &replacement_width,
        "width",
    )?;
    verify_output_instruction(
        &bytes,
        WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
        &replacement_window_origin,
        "window origin",
    )?;
    verify_output_instruction(
        &bytes,
        TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
        &replacement_text_origin,
        "text origin",
    )?;

    Ok(PatchedResultBannerWindow { bytes, report })
}

fn assemble_mov_immediate(
    file_offset: usize,
    register: Register16,
    immediate: u16,
    role: &str,
) -> Result<v30::AssembledProgram> {
    let mut assembler = Assembler::new();
    assembler.emit(Instruction::Mov {
        dest: Operand::Reg16(register),
        src: Operand::Imm16(immediate),
    });
    let runtime_offset = file_offset
        .checked_add(COM_ORIGIN)
        .context("MAD result-banner layout runtime address overflow")?;
    let runtime_offset = u16::try_from(runtime_offset)
        .context("MAD result-banner layout runtime address exceeds 16 bits")?;
    assembler
        .assemble(CodeLocation {
            seg: 0,
            off: runtime_offset,
        })
        .with_context(|| format!("assemble typed V30 MAD result-banner {role}"))
}

fn verify_source_instruction(
    bytes: &[u8],
    file_offset: usize,
    expected: &v30::AssembledProgram,
    role: &str,
) -> Result<()> {
    ensure!(
        bytes.get(file_offset..file_offset + expected.bytes().len()) == Some(expected.bytes()),
        "MAD result-banner {role} instruction differs from the verified typed V30 source"
    );
    Ok(())
}

fn verify_output_instruction(
    bytes: &[u8],
    file_offset: usize,
    expected: &v30::AssembledProgram,
    role: &str,
) -> Result<()> {
    ensure!(
        bytes.get(file_offset..file_offset + expected.bytes().len()) == Some(expected.bytes()),
        "MAD result-banner {role} did not survive its Expected Write"
    );
    Ok(())
}

#[cfg(test)]
#[path = "result_banner_window_tests.rs"]
mod result_banner_window_tests;
