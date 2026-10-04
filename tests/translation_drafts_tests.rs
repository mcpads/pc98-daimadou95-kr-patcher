//! User-owned supported-source translation draft overlay gate.

use std::env;
use std::fs;

use ds8_daimadou_builder::{
    TranslationDraftIndex, TranslationDraftSegment, TranslationReviewOutcome, TranslationStatus,
    prepare_translation_drafts, record_translation_segment_review, validate_translation_drafts,
};

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image"]
fn draft_overlay_covers_the_source_population_without_claiming_review_readiness() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let directory = tempfile::tempdir().unwrap();
    let drafts = directory.path().join("drafts");

    let prepared = prepare_translation_drafts(source.as_ref(), &drafts).unwrap();
    assert_eq!(prepared.target_item_count, 384);
    assert!(prepared.segment_count > 1);
    assert!(
        prepare_translation_drafts(source.as_ref(), &drafts)
            .unwrap_err()
            .to_string()
            .contains("refusing to overwrite")
    );

    let validated = validate_translation_drafts(source.as_ref(), &drafts, false).unwrap();
    assert_eq!(validated.target_item_count, 384);
    assert_eq!(validated.untranslated_item_count, 384);
    assert_eq!(validated.reviewed_segment_count, 0);
    assert!(!validated.human_review_ready);

    let readiness_error = validate_translation_drafts(source.as_ref(), &drafts, true)
        .unwrap_err()
        .to_string();
    assert!(readiness_error.contains("not ready for human review"));

    fs::write(drafts.join("segments/unindexed.json"), b"{}\n").unwrap();
    let extra_file_error = validate_translation_drafts(source.as_ref(), &drafts, false)
        .unwrap_err()
        .to_string();
    assert!(extra_file_error.contains("files differ from the index"));
}

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image"]
fn a_recorded_review_is_bound_to_the_tracked_segment_content() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let directory = tempfile::tempdir().unwrap();
    let drafts = directory.path().join("drafts");
    prepare_translation_drafts(source.as_ref(), &drafts).unwrap();

    let index: TranslationDraftIndex =
        serde_json::from_slice(&fs::read(drafts.join("index.json")).unwrap()).unwrap();
    let indexed = &index.segments[0];
    let segment_path = drafts.join(&indexed.path);
    let mut segment: TranslationDraftSegment =
        serde_json::from_slice(&fs::read(&segment_path).unwrap()).unwrap();
    for entry in &mut segment.entries {
        for text in &mut entry.korean_text {
            *text = Some("검토할 초안".to_owned());
        }
        entry.status = TranslationStatus::NeedsReview;
    }
    let mut bytes = serde_json::to_vec_pretty(&segment).unwrap();
    bytes.push(b'\n');
    fs::write(&segment_path, bytes).unwrap();

    let recorded = record_translation_segment_review(
        source.as_ref(),
        &drafts,
        &indexed.id,
        TranslationReviewOutcome::ReadyForHumanReview,
        vec!["보호 원문과 소비 문맥을 대조했다.".to_owned()],
    )
    .unwrap();
    assert_eq!(recorded.entry_count, segment.entries.len());

    let validated = validate_translation_drafts(source.as_ref(), &drafts, false).unwrap();
    assert_eq!(validated.reviewed_segment_count, 1);
    assert_eq!(
        validated.needs_human_review_item_count,
        segment.entries.len()
    );
}
