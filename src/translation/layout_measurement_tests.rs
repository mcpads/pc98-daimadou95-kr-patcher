use super::*;
use crate::translation::{
    TranslationConsumerContext, TranslationSourceEvidence, TranslationStatus,
};

fn source_entry(id: &str, content: Vec<TranslationContentPart>) -> TranslationEntry {
    TranslationEntry {
        id: id.to_owned(),
        source_filename: "SOURCE.COM".to_owned(),
        source_record_id: id.to_owned(),
        evidence: TranslationSourceEvidence {
            byte_records: Vec::new(),
            graphic_asset: None,
        },
        consumer_contexts: vec![TranslationConsumerContext {
            id: id.to_owned(),
            role: "layout test".to_owned(),
        }],
        content,
        status: TranslationStatus::Untranslated,
        notes: None,
    }
}

fn draft_entry(id: &str, korean_text: &[&str]) -> TranslationDraftEntry {
    TranslationDraftEntry {
        id: id.to_owned(),
        korean_text: korean_text
            .iter()
            .map(|text| Some((*text).to_owned()))
            .collect(),
        status: TranslationStatus::NeedsHumanReview,
        notes: None,
    }
}

fn text(value: &str) -> TranslationContentPart {
    TranslationContentPart::Text {
        source_text: value.to_owned(),
        korean_text: None,
    }
}

#[test]
fn select_voice_keeps_the_line_while_line_and_portrait_controls_reset_it() {
    let source = source_entry(
        "select",
        vec![
            text("1234567890"),
            TranslationContentPart::Voice { parameter: 1 },
            text("123456789"),
            TranslationContentPart::IndexedLineBreak { parameters: [0, 0] },
            text("abc"),
            TranslationContentPart::Portrait { parameters: [1, 0] },
            text("de"),
            TranslationContentPart::Terminator,
        ],
    );
    let draft = draft_entry("select", &["1234567890", "123456789", "가나다", "라마"]);

    let report = measure_translation_entry(
        "select-stage",
        LocalizationSourceCatalog::SelectDialogue,
        &source,
        &draft,
    )
    .unwrap();

    assert_eq!(report.units.len(), 3);
    assert_eq!(report.units[0].korean_cells, 19);
    assert_eq!(report.units[0].hard_limit_cells, Some(SELECT_LINE_CELLS));
    assert_eq!(report.units[1].korean_text, "가나다");
    assert_eq!(report.units[2].korean_text, "라마");
}

#[test]
fn opening_glyph_command_rejects_a_multi_character_translation_span() {
    let source = source_entry(
        "opening",
        vec![text("カ"), TranslationContentPart::OpeningEnd],
    );
    let draft = draft_entry("opening", &["카레"]);

    let report = measure_translation_entry(
        "opening-sequence",
        LocalizationSourceCatalog::OpeningText,
        &source,
        &draft,
    )
    .unwrap();

    assert_eq!(report.glyph_token_count, 1);
    assert_eq!(report.structural_findings.len(), 1);
    assert_eq!(
        report.structural_findings[0].kind,
        TranslationLayoutFindingKind::GlyphTokenWidth
    );
}

#[test]
fn dos_columns_account_for_cursor_position_and_double_byte_glyphs() {
    let source = source_entry(
        "dos",
        vec![
            TranslationContentPart::AnsiControl {
                raw_hex: "1b5b32343b323048".to_owned(),
            },
            text("日本A"),
            TranslationContentPart::Terminator,
        ],
    );
    let draft = draft_entry("dos", &["한글A"]);

    let report = measure_translation_entry(
        "dos-program",
        LocalizationSourceCatalog::DosProgramText,
        &source,
        &draft,
    )
    .unwrap();

    assert_eq!(report.units.len(), 1);
    assert_eq!(report.units[0].start_column, 19);
    assert_eq!(report.units[0].source_cells, 5);
    assert_eq!(report.units[0].korean_cells, 5);
    assert_eq!(report.units[0].hard_limit_cells, Some(61));
}

#[test]
fn system_menu_entries_use_the_observed_six_cell_window() {
    let mut source = source_entry(
        "mad-indexed-text:mad-indexed-79e0",
        vec![text("でーたろーど"), TranslationContentPart::Terminator],
    );
    source.consumer_contexts[0].id = "menu-group-03".to_owned();
    let draft = draft_entry("mad-indexed-text:mad-indexed-79e0", &["데이터 불러오기"]);

    let report = measure_translation_entry(
        "mad-commands",
        LocalizationSourceCatalog::MadIndexedText,
        &source,
        &draft,
    )
    .unwrap();

    assert_eq!(report.units[0].korean_cells, 8);
    assert_eq!(report.units[0].hard_limit_cells, Some(6));
}

#[test]
fn ending_meal_caption_phases_keep_their_individual_slot_limits() {
    let source = source_entry(
        "legendary-reaction",
        vec![text("・・・これが・・・"), text("でんせつのカレ～！！！！")],
    );
    let draft = draft_entry("legendary-reaction", &["…이것이…", "전설의 카레~!!!!"]);

    let report = measure_translation_entry(
        "ending-meal-captions",
        LocalizationSourceCatalog::EndingMealCaptions,
        &source,
        &draft,
    )
    .unwrap();

    assert_eq!(report.units.len(), 2);
    assert_eq!(report.units[0].hard_limit_cells, Some(9));
    assert_eq!(report.units[1].hard_limit_cells, Some(12));
}
