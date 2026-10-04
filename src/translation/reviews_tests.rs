use super::*;
use crate::translation::TranslationDraftEntry;

fn segment(status: TranslationStatus) -> TranslationDraftSegment {
    TranslationDraftSegment {
        id: "scene".to_owned(),
        entries: vec![TranslationDraftEntry {
            id: "line".to_owned(),
            korean_text: vec![Some("검토할 문장".to_owned())],
            status,
            notes: None,
        }],
        review: None,
    }
}

#[test]
fn binding_a_review_promotes_entries_and_hashes_the_promoted_draft() {
    let mut draft = segment(TranslationStatus::NeedsReview);

    let recorded = bind_translation_segment_review(
        &mut draft,
        TranslationReviewOutcome::ReadyForHumanReview,
        vec!["원문과 문맥을 독립적으로 대조했다.".to_owned()],
    )
    .unwrap();

    assert_eq!(draft.entries[0].status, TranslationStatus::NeedsHumanReview);
    assert_eq!(recorded.entry_count, 1);
    assert_eq!(
        recorded.reviewed_draft_sha256,
        reviewable_draft_sha256(&draft).unwrap()
    );
    assert_eq!(
        draft.review.as_ref().unwrap().reviewed_draft_sha256,
        recorded.reviewed_draft_sha256
    );
}

#[test]
fn binding_a_review_rejects_an_unfinished_or_already_reviewed_segment() {
    let mut unfinished = segment(TranslationStatus::InProgress);
    assert!(
        bind_translation_segment_review(
            &mut unfinished,
            TranslationReviewOutcome::ReadyForHumanReview,
            vec!["검토 메모".to_owned()],
        )
        .unwrap_err()
        .to_string()
        .contains("requires every entry to need review")
    );

    let mut reviewed = segment(TranslationStatus::NeedsReview);
    bind_translation_segment_review(
        &mut reviewed,
        TranslationReviewOutcome::HumanDecisionRequired,
        vec!["사람이 고유명사를 결정해야 한다.".to_owned()],
    )
    .unwrap();
    assert!(
        bind_translation_segment_review(
            &mut reviewed,
            TranslationReviewOutcome::HumanDecisionRequired,
            vec!["다시 검토".to_owned()],
        )
        .unwrap_err()
        .to_string()
        .contains("already has an independent review")
    );
}

#[test]
fn binding_a_review_requires_a_nonempty_evidence_note() {
    let mut draft = segment(TranslationStatus::NeedsReview);
    assert!(
        bind_translation_segment_review(
            &mut draft,
            TranslationReviewOutcome::ReadyForHumanReview,
            Vec::new(),
        )
        .unwrap_err()
        .to_string()
        .contains("at least one note")
    );
}
