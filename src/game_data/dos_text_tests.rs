use super::{DosTextToken, parse_dos_text, reconstruct_dos_text};

#[test]
fn dos_display_tokens_preserve_positioning_cp932_and_line_endings() {
    let source =
        b"\x1b[24;6HSONG.DAT\x82\xaa\x8c\xa9\x82\xc2\x82\xa9\x82\xe8\x82\xdc\x82\xb9\x82\xf1\r\n$";

    let parsed = parse_dos_text(source).unwrap();

    assert!(matches!(parsed.tokens[0], DosTextToken::AnsiControl(_)));
    assert!(parsed.tokens.contains(&DosTextToken::CarriageReturn));
    assert!(parsed.tokens.contains(&DosTextToken::LineFeed));
    assert_eq!(reconstruct_dos_text(&parsed.tokens).unwrap(), source);
}

#[test]
fn dos_display_parser_rejects_unknown_controls_and_missing_terminators() {
    let control_error = parse_dos_text(b"\x1bX$").unwrap_err();
    let terminator_error = parse_dos_text(b"message").unwrap_err();

    assert!(control_error.to_string().contains("ANSI display control"));
    assert!(terminator_error.to_string().contains("no $ terminator"));
}
