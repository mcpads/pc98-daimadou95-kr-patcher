use super::*;

fn mad_com_with_result_banner_layout(
    inner_tile_columns: u16,
    window_origin: u16,
    text_origin: u16,
) -> Vec<u8> {
    let width = assemble_mov_immediate(
        WIDTH_INSTRUCTION_FILE_OFFSET,
        Register16::AX,
        inner_tile_columns,
        "test width",
    )
    .unwrap();
    let window = assemble_mov_immediate(
        WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
        Register16::DI,
        window_origin,
        "test window origin",
    )
    .unwrap();
    let text = assemble_mov_immediate(
        TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
        Register16::DI,
        text_origin,
        "test text origin",
    )
    .unwrap();
    let mut bytes = vec![0x90; TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET + text.bytes().len() + 8];
    for (file_offset, instruction) in [
        (WIDTH_INSTRUCTION_FILE_OFFSET, width),
        (WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET, window),
        (TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET, text),
    ] {
        bytes[file_offset..file_offset + instruction.bytes().len()]
            .copy_from_slice(instruction.bytes());
    }
    bytes
}

#[test]
fn translated_banner_expands_result_window_to_its_compiled_width() {
    let source = mad_com_with_result_banner_layout(14, 0x370e, 0x3990);

    let patched = fit_result_banner_window(&source, 16).unwrap();

    assert_eq!(
        patched.report,
        MadResultBannerWindowReport {
            width_instruction_file_offset: 0x6165,
            window_origin_instruction_file_offset: 0x616b,
            text_origin_instruction_file_offset: 0x617d,
            source_inner_tile_columns: 14,
            required_inner_tile_columns: 16,
            output_inner_tile_columns: 16,
            horizontal_shift_tile_columns: 1,
            horizontal_shift_pixels: 16,
        }
    );
    let decoded_width = v30::decode_bytes(&patched.bytes[WIDTH_INSTRUCTION_FILE_OFFSET..]).unwrap();
    assert_eq!(
        decoded_width.instruction,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::AX),
            src: Operand::Imm16(16),
        }
    );
    let decoded_window =
        v30::decode_bytes(&patched.bytes[WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET..]).unwrap();
    assert_eq!(
        decoded_window.instruction,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::DI),
            src: Operand::Imm16(0x370c),
        }
    );
    let decoded_text =
        v30::decode_bytes(&patched.bytes[TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET..]).unwrap();
    assert_eq!(
        decoded_text.instruction,
        Instruction::Mov {
            dest: Operand::Reg16(Register16::DI),
            src: Operand::Imm16(0x398e),
        }
    );

    let mut expected = source.clone();
    for (file_offset, instruction) in [
        (
            WIDTH_INSTRUCTION_FILE_OFFSET,
            assemble_mov_immediate(
                WIDTH_INSTRUCTION_FILE_OFFSET,
                Register16::AX,
                16,
                "expected width",
            )
            .unwrap(),
        ),
        (
            WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
            assemble_mov_immediate(
                WINDOW_ORIGIN_INSTRUCTION_FILE_OFFSET,
                Register16::DI,
                0x370c,
                "expected window origin",
            )
            .unwrap(),
        ),
        (
            TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
            assemble_mov_immediate(
                TEXT_ORIGIN_INSTRUCTION_FILE_OFFSET,
                Register16::DI,
                0x398e,
                "expected text origin",
            )
            .unwrap(),
        ),
    ] {
        expected[file_offset..file_offset + instruction.bytes().len()]
            .copy_from_slice(instruction.bytes());
    }
    assert_eq!(patched.bytes, expected);
}

#[test]
fn source_width_is_preserved_when_the_banner_already_fits() {
    let source = mad_com_with_result_banner_layout(14, 0x370e, 0x3990);

    let patched = fit_result_banner_window(&source, 12).unwrap();

    assert_eq!(patched.bytes, source);
    assert_eq!(patched.report.output_inner_tile_columns, 14);
    assert_eq!(patched.report.required_inner_tile_columns, 12);
    assert_eq!(patched.report.horizontal_shift_pixels, 0);
}

#[test]
fn unverified_source_width_is_rejected() {
    let source = mad_com_with_result_banner_layout(13, 0x370e, 0x3990);

    let error = fit_result_banner_window(&source, 16).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("differs from the verified typed V30 source")
    );
}

#[test]
fn unverified_source_origin_is_rejected() {
    let source = mad_com_with_result_banner_layout(14, 0x370d, 0x3990);

    let error = fit_result_banner_window(&source, 16).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("window origin instruction differs from the verified typed V30 source")
    );
}

#[test]
fn partial_composite_cells_are_rejected() {
    let source = mad_com_with_result_banner_layout(14, 0x370e, 0x3990);

    let error = fit_result_banner_window(&source, 15).unwrap_err();

    assert!(error.to_string().contains("positive, even tile width"));
}
