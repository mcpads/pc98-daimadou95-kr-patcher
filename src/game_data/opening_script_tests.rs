use super::{
    CALL_SITE_FILE_OFFSET, INTERPRETER_BODY_LEN, INTERPRETER_FILE_OFFSET, INTERPRETER_SIGNATURE,
    LOAD_REFERENCE_FILE_OFFSET, OpeningDisplayRun, OpeningScriptToken, opening_display_runs,
    parse_opening_text_script, reconstruct_opening_text_script, replace_opening_glyph_indices,
    verify_opening_text_consumer,
};

#[test]
fn opening_script_preserves_text_timing_sprite_and_scroll_commands() {
    let source = [
        1, 0x0a, 0x52, 0x1e, 0x0a, 0xb2, 0x0b, 0x04, 0x3b, 0x56, 0x00, 0x14, 0x28,
    ];

    let script = parse_opening_text_script(&source).unwrap();

    assert_eq!(
        script.tokens,
        vec![
            OpeningScriptToken::Mode(1),
            OpeningScriptToken::Glyph {
                index: 0x52,
                text: "カ",
            },
            OpeningScriptToken::Yield,
            OpeningScriptToken::Glyph {
                index: 0xb2,
                text: "ー",
            },
            OpeningScriptToken::LineBreak,
            OpeningScriptToken::Sprite {
                glyph_index: 0x3b,
                position: 0x56,
            },
            OpeningScriptToken::Scroll,
            OpeningScriptToken::End,
        ]
    );
    assert_eq!(
        reconstruct_opening_text_script(&script.tokens).unwrap(),
        source
    );
}

#[test]
fn opening_script_rejects_unknown_commands_and_trailing_bytes() {
    assert!(
        parse_opening_text_script(&[0x29])
            .unwrap_err()
            .to_string()
            .contains("unsupported")
    );
    assert!(
        parse_opening_text_script(&[0x28, 0x00])
            .unwrap_err()
            .to_string()
            .contains("after its end")
    );
}

#[test]
fn opening_consumer_requires_load_interpreter_and_typed_call() {
    let mut program = vec![0; INTERPRETER_FILE_OFFSET + INTERPRETER_BODY_LEN];
    program[LOAD_REFERENCE_FILE_OFFSET..LOAD_REFERENCE_FILE_OFFSET + 3]
        .copy_from_slice(&[0xb8, 0x8e, 0x00]);
    program[CALL_SITE_FILE_OFFSET..CALL_SITE_FILE_OFFSET + 3].copy_from_slice(&[0xe8, 0x2f, 0x02]);
    program[INTERPRETER_FILE_OFFSET..INTERPRETER_FILE_OFFSET + INTERPRETER_SIGNATURE.len()]
        .copy_from_slice(INTERPRETER_SIGNATURE);
    // The fixture is intentionally not a valid copy of the whole interpreter.
    assert!(verify_opening_text_consumer(&program).is_err());

    program[LOAD_REFERENCE_FILE_OFFSET] = 0x90;
    assert!(
        verify_opening_text_consumer(&program)
            .unwrap_err()
            .to_string()
            .contains("load reference")
    );
}

#[test]
fn translated_glyph_indices_preserve_every_control_and_sprite_byte() {
    let source = [
        1, 0x1e, 0x0a, 0x52, 0x04, 0x3b, 0x56, 0x00, 0x0b, 0x0a, 0x8e, 0x14, 0x28,
    ];

    let translated = replace_opening_glyph_indices(&source, &[7, 9]).unwrap();

    assert_eq!(
        translated,
        [
            1, 0x1e, 0x0a, 7, 0x04, 0x3b, 0x56, 0x00, 0x0b, 0x0a, 9, 0x14, 0x28
        ]
    );
}

#[test]
fn translated_glyph_count_must_match_the_source_timing_slots() {
    let source = [0x0a, 0x52, 0x28];

    let error = replace_opening_glyph_indices(&source, &[]).unwrap_err();

    assert!(error.to_string().contains("0 glyph indices for 1"));
}

#[test]
fn display_runs_follow_mode_changes_and_multi_scroll_clears() {
    let glyph = |index| OpeningScriptToken::Glyph { index, text: " " };
    let tokens = vec![
        OpeningScriptToken::Mode(1),
        glyph(0x4d),
        OpeningScriptToken::LineBreak,
        glyph(0x4d),
        OpeningScriptToken::Mode(2),
        glyph(0x4d),
        OpeningScriptToken::Mode(2),
        glyph(0x4d),
        OpeningScriptToken::Mode(3),
        glyph(0x4d),
        OpeningScriptToken::Scroll,
        glyph(0x4d),
        OpeningScriptToken::Scroll,
        OpeningScriptToken::Yield,
        OpeningScriptToken::Scroll,
        OpeningScriptToken::LineBreak,
        glyph(0x4d),
        OpeningScriptToken::End,
    ];

    assert_eq!(
        opening_display_runs(&tokens).unwrap(),
        vec![
            OpeningDisplayRun {
                glyph_range: 0..2,
                line_break_offsets: vec![1],
            },
            OpeningDisplayRun {
                glyph_range: 2..4,
                line_break_offsets: vec![],
            },
            OpeningDisplayRun {
                glyph_range: 4..6,
                line_break_offsets: vec![],
            },
            OpeningDisplayRun {
                glyph_range: 6..7,
                line_break_offsets: vec![],
            },
        ]
    );
}
