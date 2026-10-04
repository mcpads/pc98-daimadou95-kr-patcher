use super::{
    FONT_DECODED_SIZE, IndexedTextToken, SelectDialogueScript, indexed_glyph_text,
    parse_indexed_lines, reconstruct_indexed_lines, reconstruct_select_script, verify_indexed_font,
};

#[test]
fn indexed_font_codebook_decodes_names_and_preserves_verified_spaces() {
    let decoded: String = [0x52, 0xb2, 0x5a, 0x7f, 0x72, 0x7e]
        .into_iter()
        .map(|index| indexed_glyph_text(index).unwrap())
        .collect();

    assert_eq!(decoded, "カーバンクル");
    assert_eq!(indexed_glyph_text(0x3d), Some(" "));
    assert_eq!(indexed_glyph_text(0x4d), Some(" "));
    assert_eq!(indexed_glyph_text(0xe1), Some("「"));
    assert_eq!(indexed_glyph_text(0xa2), Some("」"));
    assert_eq!(indexed_glyph_text(0xf0), None);
}

#[test]
fn indexed_font_requires_the_complete_256_glyph_resource() {
    assert!(verify_indexed_font(&vec![0; FONT_DECODED_SIZE]).is_ok());

    let error = verify_indexed_font(&vec![0; FONT_DECODED_SIZE - 1]).unwrap_err();

    assert!(error.to_string().contains("decoded to"));
}

#[test]
fn select_controls_reconstruct_with_their_exact_parameter_widths() {
    let source = [
        1, 3, 0x52, 0xfa, 2, 0xfe, 1, 3, 0x4d, 0xfd, 0, 1, 0x7e, 0xff,
    ];
    let script = SelectDialogueScript {
        phase_index: 0,
        pointer_entry_file_offset: 0,
        runtime_address: 0x100,
        file_offset: 0,
        bytes: &source,
        initial_portrait: [1, 3],
        tokens: vec![
            IndexedTextToken::Glyph {
                index: 0x52,
                text: "カ",
            },
            IndexedTextToken::Voice(2),
            IndexedTextToken::LineBreak([1, 3]),
            IndexedTextToken::Glyph {
                index: 0x4d,
                text: " ",
            },
            IndexedTextToken::Portrait([0, 1]),
            IndexedTextToken::Glyph {
                index: 0x7e,
                text: "ル",
            },
            IndexedTextToken::Terminator,
        ],
    };

    assert_eq!(reconstruct_select_script(&script).unwrap(), source);
}

#[test]
fn select_reconstruction_rejects_missing_or_early_terminators() {
    let missing_source = [0, 0, 0x00];
    let missing = SelectDialogueScript {
        phase_index: 0,
        pointer_entry_file_offset: 0,
        runtime_address: 0x100,
        file_offset: 0,
        bytes: &missing_source,
        initial_portrait: [0, 0],
        tokens: vec![IndexedTextToken::Glyph {
            index: 0,
            text: "あ",
        }],
    };
    assert!(
        reconstruct_select_script(&missing)
            .unwrap_err()
            .to_string()
            .contains("must end")
    );

    let early_source = [0, 0, 0xff, 0xff];
    let early = SelectDialogueScript {
        phase_index: 0,
        pointer_entry_file_offset: 0,
        runtime_address: 0x100,
        file_offset: 0,
        bytes: &early_source,
        initial_portrait: [0, 0],
        tokens: vec![IndexedTextToken::Terminator, IndexedTextToken::Terminator],
    };
    assert!(
        reconstruct_select_script(&early)
            .unwrap_err()
            .to_string()
            .contains("early terminator")
    );
}

#[test]
fn ff_terminated_lines_preserve_unknown_graphic_glyphs_without_guessing_text() {
    let source = [0x52, 0xb2, 0x7e, 0xff, 0xb4, 0xb5, 0xff];

    let stream = parse_indexed_lines(&source, 2).unwrap();

    assert_eq!(stream.byte_len, source.len());
    assert_eq!(stream.lines[0].glyphs[0].text, Some("カ"));
    assert_eq!(stream.lines[1].glyphs[0].text, None);
    assert_eq!(reconstruct_indexed_lines(&stream.lines).unwrap(), source);
}
