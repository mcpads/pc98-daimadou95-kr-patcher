use super::*;
use crate::translation::{
    TranslationDraftEntry, TranslationDraftIndex, TranslationDraftSegment,
    TranslationReviewOutcome, TranslationSegmentReview,
};

fn segment(text: &str, status: TranslationStatus) -> TranslationDraftSegment {
    TranslationDraftSegment {
        id: "scene".to_owned(),
        entries: vec![TranslationDraftEntry {
            id: "line".to_owned(),
            korean_text: vec![Some(text.to_owned())],
            status,
            notes: None,
        }],
        review: None,
    }
}

#[test]
fn review_hash_changes_when_the_reviewed_draft_changes() {
    let baseline = segment("초안", TranslationStatus::NeedsReview);
    let baseline_hash = reviewable_draft_sha256(&baseline).unwrap();
    let changed = segment("다른 초안", TranslationStatus::NeedsReview);

    assert_ne!(reviewable_draft_sha256(&changed).unwrap(), baseline_hash);
}

#[test]
fn review_metadata_does_not_change_the_reviewed_draft_hash() {
    let mut value = segment("초안", TranslationStatus::NeedsHumanReview);
    let expected = reviewable_draft_sha256(&value).unwrap();
    value.review = Some(TranslationSegmentReview {
        reviewed_draft_sha256: expected.clone(),
        outcome: TranslationReviewOutcome::ReadyForHumanReview,
        notes: vec!["독립 재검토 완료".to_owned()],
    });

    assert_eq!(reviewable_draft_sha256(&value).unwrap(), expected);
}

#[test]
fn japanese_script_is_not_accepted_as_a_korean_draft() {
    assert!(contains_japanese_script("アルル"));
    assert!(contains_japanese_script("魔導"));
    assert!(!contains_japanese_script("아르르 '95"));
}

#[test]
fn an_explicit_spacing_cell_is_distinct_from_an_empty_translation() {
    assert!(validate_korean_text("opening", " ").is_ok());
    assert!(validate_korean_text("opening", "").is_err());
}

#[test]
#[ignore = "requires translation drafts in assets/translations/"]
fn tracked_review_bindings_match_every_current_draft_segment() {
    let root = std::path::Path::new("assets/translations");
    let index: TranslationDraftIndex = read_json(&root.join(INDEX_FILENAME)).unwrap();

    for indexed in index.segments {
        let segment: TranslationDraftSegment = read_json(&root.join(indexed.path)).unwrap();
        let review = segment
            .review
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no review binding", segment.id));
        assert_eq!(
            review.reviewed_draft_sha256,
            reviewable_draft_sha256(&segment).unwrap(),
            "{} review binding drifted",
            segment.id
        );
    }
}
