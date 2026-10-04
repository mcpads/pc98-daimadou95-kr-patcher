use super::*;
use crate::localization::{
    LocalizationContext, LocalizationGlyphGate, LocalizationGlyphSupply,
    LocalizationReinsertionGate, LocalizationRenderPath, LocalizationSourceLayout,
};
use crate::source::LocalizationSourceCatalog;
use crate::source::disk;

fn entry() -> TranslationEntry {
    TranslationEntry {
        id: "dialogue".to_owned(),
        source_filename: "SELECT.COM".to_owned(),
        source_record_id: "stage-01-phase-01".to_owned(),
        evidence: TranslationSourceEvidence {
            byte_records: vec![TranslationByteRecord {
                id: "stage-01-phase-01".to_owned(),
                file_offset: 0x100,
                byte_size: 2,
                sha256: "0".repeat(64),
                raw_hex: "01ff".to_owned(),
            }],
            graphic_asset: None,
        },
        consumer_contexts: vec![TranslationConsumerContext {
            id: "stage-01-phase-01".to_owned(),
            role: "dialogue".to_owned(),
        }],
        content: vec![
            TranslationContentPart::Text {
                source_text: "あ".to_owned(),
                korean_text: None,
            },
            TranslationContentPart::Terminator,
        ],
        status: TranslationStatus::Untranslated,
        notes: None,
    }
}

fn segment(entry: TranslationEntry) -> TranslationSegment {
    TranslationSegment {
        id: "select-stage-01".to_owned(),
        source_sha256: "1".repeat(64),
        source_catalog: LocalizationSourceCatalog::SelectDialogue,
        context: LocalizationContext::SceneSpeakerAndSequence,
        render_path: LocalizationRenderPath::SharedIndexedFont,
        glyph_supply: LocalizationGlyphSupply::SharedIndexedFont,
        source_layout: LocalizationSourceLayout::PointerSelectedScript,
        glyph_gate: LocalizationGlyphGate::MeasureIndexedRepertoireAndLayout,
        reinsertion_gate: LocalizationReinsertionGate::RelocatePointerSelectedScripts,
        entries: vec![entry],
    }
}

#[test]
fn protected_hash_ignores_translation_edits_and_detects_source_edits() {
    let baseline = segment(entry());
    let baseline_hash = disk::sha256_hex(&protected_segment_bytes(&baseline).unwrap());

    let mut translated = segment(entry());
    let TranslationContentPart::Text { korean_text, .. } = &mut translated.entries[0].content[0]
    else {
        panic!("first part must be text");
    };
    *korean_text = Some("아".to_owned());
    translated.entries[0].status = TranslationStatus::NeedsReview;
    translated.entries[0].notes = Some("short greeting".to_owned());
    assert_eq!(
        disk::sha256_hex(&protected_segment_bytes(&translated).unwrap()),
        baseline_hash
    );

    let TranslationContentPart::Text { source_text, .. } = &mut translated.entries[0].content[0]
    else {
        panic!("first part must be text");
    };
    *source_text = "い".to_owned();
    assert_ne!(
        disk::sha256_hex(&protected_segment_bytes(&translated).unwrap()),
        baseline_hash
    );
}

#[test]
fn review_status_requires_every_text_span_to_have_korean() {
    let mut value = entry();
    value.content.insert(
        1,
        TranslationContentPart::Text {
            source_text: "い".to_owned(),
            korean_text: None,
        },
    );
    let TranslationContentPart::Text { korean_text, .. } = &mut value.content[0] else {
        panic!("first part must be text");
    };
    *korean_text = Some("아".to_owned());
    value.status = TranslationStatus::NeedsReview;

    assert!(validate_editable_fields(&value).is_err());
    let TranslationContentPart::Text { korean_text, .. } = &mut value.content[1] else {
        panic!("second part must be text");
    };
    *korean_text = Some("이".to_owned());
    assert!(validate_editable_fields(&value).is_ok());
}

#[test]
fn complete_status_fails_until_human_approval_is_bound() {
    let mut value = entry();
    let TranslationContentPart::Text { korean_text, .. } = &mut value.content[0] else {
        panic!("first part must be text");
    };
    *korean_text = Some("아".to_owned());
    value.status = TranslationStatus::Complete;

    let error = validate_editable_fields(&value).unwrap_err().to_string();
    assert!(error.contains("human-approval input"), "{error}");
}
