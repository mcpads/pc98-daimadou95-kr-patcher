use super::*;

#[test]
fn runtime_file_coverage_requires_the_exact_declared_population() {
    let declared: BTreeMap<_, &[u8]> = FILE_COVERAGE
        .iter()
        .map(|spec| (spec.filename.to_owned(), &[][..]))
        .collect();
    let coverage = catalog_runtime_files(&declared).unwrap();
    assert_eq!(coverage.len(), DECLARED_RUNTIME_FILE_COUNT);
    assert!(
        coverage
            .iter()
            .all(|file| file.status != LocalizationFileStatus::Unresolved)
    );

    let mut incomplete = declared;
    incomplete.remove("SELECT.COM");
    assert!(catalog_runtime_files(&incomplete).is_err());
}

#[test]
fn auxiliary_programs_remain_audited_but_are_not_product_targets() {
    let declared: BTreeMap<_, &[u8]> = FILE_COVERAGE
        .iter()
        .map(|spec| (spec.filename.to_owned(), &[][..]))
        .collect();
    let coverage = catalog_runtime_files(&declared).unwrap();

    for filename in [
        "MST.COM",
        "FPLAY.COM",
        "BPLAY.COM",
        "BSAMP.COM",
        "NMOUSE.COM",
    ] {
        let file = coverage
            .iter()
            .find(|file| file.filename == filename)
            .unwrap();
        assert_eq!(file.status, LocalizationFileStatus::Excluded);
        assert_eq!(file.reason, LocalizationFileReason::AuxiliaryProgramOutput);
    }
}

#[test]
fn japanese_detection_ignores_ascii_and_keeps_kana_and_kanji() {
    assert!(!contains_japanese("MOO NIITANI"));
    assert!(contains_japanese("ゲームオーバー"));
    assert!(contains_japanese("大魔導戦略物語'95"));
    assert!(contains_japanese("ﾃﾞｨｽｸｴﾗｰ"));
}

#[test]
fn text_items_are_added_only_for_japanese_content() {
    let mut items = Vec::new();
    add_japanese_item(
        &mut items,
        "english".to_owned(),
        "ENDING.COM",
        LocalizationSourceCatalog::EndingCredits,
        "staff".to_owned(),
        "STAFF",
        LocalizationTextDisposition::Target,
        LocalizationTextReason::ConsumerReachable,
    );
    add_japanese_item(
        &mut items,
        "japanese".to_owned(),
        "ENDING.COM",
        LocalizationSourceCatalog::EndingPasswords,
        "password".to_owned(),
        "ぱすわ～ど",
        LocalizationTextDisposition::Target,
        LocalizationTextReason::ConsumerReachable,
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "japanese");
}

#[test]
fn reviewed_graphic_source_can_explicitly_include_ascii_text() {
    let mut items = Vec::new();
    add_source_item(
        &mut items,
        "graphic-text:stage-select:01".to_owned(),
        "MADDAT",
        LocalizationSourceCatalog::GraphicText,
        "stage-select:01".to_owned(),
        "STAGE SELECT",
        LocalizationTextDisposition::Target,
        LocalizationTextReason::GraphicConsumerReachable,
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text, "STAGE SELECT");
}

#[test]
fn original_graphic_labels_remain_identified_without_becoming_translation_targets() {
    let mut items = Vec::new();
    add_source_item(
        &mut items,
        "graphic-text:title-menu:01".to_owned(),
        "MADDAT",
        LocalizationSourceCatalog::GraphicText,
        "title-menu:01".to_owned(),
        "START",
        LocalizationTextDisposition::Excluded,
        LocalizationTextReason::OriginalPresentationPreserved,
    );

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text, "START");
    assert_eq!(items[0].disposition, LocalizationTextDisposition::Excluded);
    assert_eq!(
        items[0].reason,
        LocalizationTextReason::OriginalPresentationPreserved
    );
}
