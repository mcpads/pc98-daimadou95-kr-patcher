use super::*;

#[test]
fn masked_title_menu_glyph_bank_round_trips_color_and_transparency() {
    let mut glyphs = vec![
        MaskedTitleMenuGlyph {
            pixels: vec![None; GLYPH_WIDTH * GLYPH_HEIGHT],
        };
        GLYPH_COUNT
    ];
    glyphs[0].pixels[0] = Some(0);
    glyphs[0].pixels[1] = Some(4);
    glyphs[10].pixels[GLYPH_WIDTH * GLYPH_HEIGHT - 1] = Some(7);

    let packed = encode_title_menu_glyph_bank(&glyphs).unwrap();
    let decoded = decode_title_menu_glyph_bank(&packed).unwrap();

    assert_eq!(decoded.glyphs, glyphs);
    assert_eq!(decoded.decoded.len(), GLYPH_COUNT * RECORD_SIZE);
}

#[test]
fn title_menu_glyph_bank_rejects_a_drifted_record_header() {
    let glyphs = vec![
        MaskedTitleMenuGlyph {
            pixels: vec![None; GLYPH_WIDTH * GLYPH_HEIGHT],
        };
        GLYPH_COUNT
    ];
    let packed = encode_title_menu_glyph_bank(&glyphs).unwrap();
    let mut decoded = decode_complete_compile_lz(&packed).unwrap().output;
    decoded[0] = 4;
    let malformed = encode_compile_lz(&decoded).unwrap();

    assert!(decode_title_menu_glyph_bank(&malformed).is_err());
}
