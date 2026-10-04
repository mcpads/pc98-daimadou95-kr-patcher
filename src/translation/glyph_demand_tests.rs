use super::*;

fn demand(texts: &[&str]) -> DemandAccumulator {
    let mut demand = DemandAccumulator::default();
    for text in texts {
        demand.add_span(text);
    }
    demand
}

#[test]
fn shared_indexed_font_counts_every_distinct_rendered_character_as_a_slot() {
    let report =
        demand(&["가A★", "가나"]).report(LocalizationRenderPath::SharedIndexedFont, Some(3));

    assert_eq!(report.unique_rendered_character_count, 4);
    assert_eq!(report.unique_hangul_syllable_count, 2);
    assert_eq!(report.slot_glyph_count, Some(4));
    assert_eq!(report.physical_capacity_headroom, Some(-1));
    assert_eq!(report.physical_capacity_fit, Some(false));
}

#[test]
fn ending_credit_font_uses_blank_cells_for_spaces_instead_of_glyph_slots() {
    let report =
        demand(&["특별 감사", "제작"]).report(LocalizationRenderPath::EndingCreditFont, Some(6));

    assert_eq!(report.unique_rendered_character_count, 7);
    assert_eq!(report.slot_glyph_count, Some(6));
    assert_eq!(
        report.slot_glyph_characters,
        ["감", "별", "사", "작", "제", "특"]
    );
    assert_eq!(report.physical_capacity_headroom, Some(0));
    assert_eq!(report.physical_capacity_fit, Some(true));
}

#[test]
fn dos_system_text_uses_gaiji_slots_only_for_shift_jis_unencodable_characters() {
    let demand = demand(&["가A★", "가나"]);
    let report = demand.report(LocalizationRenderPath::DosSystemText, Some(2));

    assert_eq!(report.unique_rendered_character_count, 4);
    assert_eq!(report.slot_glyph_count, Some(2));
    assert_eq!(report.slot_glyph_characters, ["가", "나"]);
    assert_eq!(report.physical_capacity_headroom, Some(0));
    assert_eq!(report.physical_capacity_fit, Some(true));
    assert_eq!(
        demand.embedded_font_characters(LocalizationRenderPath::DosSystemText),
        BTreeSet::from(['가', '나'])
    );
}

#[test]
fn planar_graphics_reports_text_repertoire_without_claiming_a_slot_capacity() {
    let report = demand(&["한글"]).report(LocalizationRenderPath::PlanarGraphics, None);

    assert_eq!(report.unique_rendered_character_count, 2);
    assert_eq!(report.slot_glyph_count, None);
    assert_eq!(report.verified_physical_capacity, None);
    assert_eq!(report.physical_capacity_headroom, None);
    assert_eq!(report.physical_capacity_fit, None);
    assert!(report.slot_glyph_characters.is_empty());
}
