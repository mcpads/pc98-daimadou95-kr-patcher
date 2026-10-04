mod corpus;
mod drafts;
mod glyph_demand;
mod layout;
mod layout_measurement;
mod projection;
mod review_queue;
mod reviews;
mod segments;
mod types;
mod workspace;

pub(crate) use corpus::{ReviewedTranslationCorpus, load_reviewed_translation_corpus};
pub use drafts::{prepare_translation_drafts, validate_translation_drafts};
pub use glyph_demand::audit_translation_glyphs;
pub use layout::audit_translation_layout;
pub use review_queue::report_translation_review;
pub use reviews::record_translation_segment_review;
pub use types::{
    RecordedTranslationReview, TranslationByteRecord, TranslationConsumerContext,
    TranslationContentPart, TranslationDraftEntry, TranslationDraftIndex,
    TranslationDraftPreparationReport, TranslationDraftSegment, TranslationDraftSegmentIndex,
    TranslationDraftValidationReport, TranslationEntry, TranslationGlyphAuditReport,
    TranslationGlyphDemand, TranslationGlyphFamilyReport, TranslationGlyphRenderPathReport,
    TranslationGlyphSegmentReport, TranslationGraphicAsset, TranslationLayoutAuditReport,
    TranslationLayoutFamilyReport, TranslationLayoutFinding, TranslationLayoutFindingKind,
    TranslationLayoutOpenGate, TranslationPreparationReport, TranslationReviewOutcome,
    TranslationReviewQueueEntry, TranslationReviewQueueReport, TranslationReviewQueueSegment,
    TranslationReviewTextPair, TranslationSegment, TranslationSegmentIndex,
    TranslationSegmentReview, TranslationSourceEvidence, TranslationStatus,
    TranslationValidationReport, TranslationWorkspaceIndex,
};
pub use workspace::{prepare_translation_workspace, validate_translation_workspace};

#[cfg(test)]
use workspace::{protected_segment_bytes, validate_editable_fields};

#[cfg(test)]
#[path = "translation_tests.rs"]
mod translation_tests;
