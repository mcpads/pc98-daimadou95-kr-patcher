use super::*;

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn embedded_profile_binds_the_font_and_license() {
    let report = verify_hangul_repertoire(['가']).unwrap();

    assert_eq!(report.provenance.profile_id, "neodunggeunmo-pc98-16x16");
    assert_eq!(
        report.provenance.font_sha256,
        "d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6"
    );
    assert_eq!(report.provenance.font_version, "1.600");
    assert_eq!(
        report.provenance.upstream_revision,
        "9aa2daa8fb965528987da05c01bf627cf5c27cdb"
    );
    assert_eq!(report.verified_character_count, 1);
    assert_eq!(report.verified_hangul_syllable_count, 1);
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn rasterized_ga_matches_the_reviewed_pc98_bitmap() {
    let reviewed_bitmap = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0xfc, 0x30, 0x0c, 0x30, 0x0c, 0x30, 0x0c,
        0x30, 0x0c, 0x30, 0x0c, 0x3e, 0x0c, 0x30, 0x0c, 0x30, 0x0c, 0x30, 0x18, 0x30, 0x70, 0x30,
        0x00, 0x30,
    ];

    assert_eq!(rasterize_hangul_syllable('가').unwrap(), reviewed_bitmap);
    assert_eq!(rasterize_hangul_syllable('가').unwrap(), reviewed_bitmap);
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn reviewed_dense_and_punctuation_bitmaps_do_not_lose_strokes() {
    let mut bitmaps = Vec::new();
    for character in "가힐꽃뷔뿌쥬!.,-…「」".chars() {
        bitmaps.extend_from_slice(&rasterize_pc98_character(character).unwrap());
    }
    let digest = Sha256::digest(bitmaps)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    assert_eq!(
        digest,
        "71db33bec809f379ee0dad58cc2a30d98af7bb892c6f93e211ed41d6af509168"
    );
}

#[test]
fn non_hangul_input_is_rejected_instead_of_rendering_notdef() {
    let error = rasterize_hangul_syllable('A').unwrap_err();

    assert!(error.to_string().contains("not a modern Hangul syllable"));
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn dense_current_style_samples_fit_the_pc98_cell() {
    let report = verify_hangul_repertoire(['힐', '꽃', '뷔', '뿌', '쥬']).unwrap();

    assert_eq!(report.verified_hangul_syllable_count, 5);
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn opening_punctuation_and_space_have_pc98_cell_bitmaps() {
    assert_eq!(rasterize_pc98_character(' ').unwrap(), [0; GLYPH_BYTES]);
    assert!(
        rasterize_pc98_character('!')
            .unwrap()
            .iter()
            .any(|byte| *byte != 0)
    );
    assert!(
        rasterize_pc98_character('.')
            .unwrap()
            .iter()
            .any(|byte| *byte != 0)
    );
    assert!(
        rasterize_pc98_character(',')
            .unwrap()
            .iter()
            .any(|byte| *byte != 0)
    );
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn complete_repertoire_verification_includes_spacing_and_punctuation() {
    let report = verify_pc98_repertoire(['가', ' ', ',', '!']).unwrap();

    assert_eq!(report.verified_character_count, 4);
    assert_eq!(report.verified_hangul_syllable_count, 1);
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn ending_credit_profile_rasterizes_directly_into_the_24_by_32_cell() {
    let report = verify_ending_credit_repertoire(['제', '작', '진']).unwrap();
    let glyph = rasterize_ending_credit_character('제').unwrap();

    assert_eq!(
        report.provenance.profile_id,
        "neodunggeunmo-ending-credit-24x32"
    );
    assert_eq!(report.verified_hangul_syllable_count, 3);
    assert_eq!(glyph.len(), ENDING_CREDIT_GLYPH_BYTES);
    assert!(glyph.iter().any(|byte| *byte != 0));
    assert!(
        glyph
            .chunks_exact(ENDING_CREDIT_GLYPH_WIDTH / 8)
            .filter(|row| row.iter().any(|byte| *byte != 0))
            .count()
            >= 16
    );
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn reviewed_ending_credit_bitmaps_bind_size_and_thickening() {
    let characters = "제작진기획그래픽디자인프로그램사운드특별감독듀서"
        .chars()
        .collect::<BTreeSet<_>>();
    let mut bitmaps = Vec::new();
    for character in characters {
        bitmaps.extend_from_slice(&rasterize_ending_credit_character(character).unwrap());
    }
    let digest = Sha256::digest(bitmaps)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    assert_eq!(
        digest,
        "5f442c0d99aec1b1dda975b12f002a0297db21173f72d59d91ccc5d994850c24"
    );
}
