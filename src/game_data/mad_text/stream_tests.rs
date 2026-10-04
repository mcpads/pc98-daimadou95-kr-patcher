use super::{RendererTextToken, parse_renderer_text_stream, reconstruct_renderer_text_stream};

#[test]
fn renderer_tokens_reconstruct_the_exact_source_bytes() {
    let bytes = b"$3A\x81\x40\xeb\xa0$0\xa6$8$$";

    let parsed = parse_renderer_text_stream(bytes).unwrap();

    assert_eq!(
        parsed.tokens,
        vec![
            RendererTextToken::DisplayControl(b'3'),
            RendererTextToken::SingleByte(b'A'),
            RendererTextToken::DoubleByte([0x81, 0x40]),
            RendererTextToken::DoubleByte([0xeb, 0xa0]),
            RendererTextToken::LineBreak,
            RendererTextToken::SingleByte(0xa6),
            RendererTextToken::DisplayControl(b'8'),
            RendererTextToken::Terminator,
        ]
    );
    assert_eq!(parsed.byte_len, bytes.len());
    assert_eq!(
        reconstruct_renderer_text_stream(&parsed.tokens).unwrap(),
        bytes
    );
}

#[test]
fn renderer_stream_rejects_unknown_controls_and_unterminated_text() {
    let unknown = parse_renderer_text_stream(b"$x$$").unwrap_err();
    let unterminated = parse_renderer_text_stream(b"$3\x81\x40").unwrap_err();

    assert!(unknown.to_string().contains("unsupported renderer control"));
    assert!(unterminated.to_string().contains("no $$ terminator"));
}

#[test]
fn reconstruction_requires_one_final_terminator() {
    let missing = reconstruct_renderer_text_stream(&[RendererTextToken::SingleByte(b'A')]);
    let early = reconstruct_renderer_text_stream(&[
        RendererTextToken::Terminator,
        RendererTextToken::Terminator,
    ]);

    assert!(missing.unwrap_err().to_string().contains("must end"));
    assert!(early.unwrap_err().to_string().contains("early terminator"));
}
