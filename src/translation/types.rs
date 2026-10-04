use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::font::FontRepertoireReport;
use crate::localization::{
    LocalizationContext, LocalizationGlyphGate, LocalizationGlyphSupply,
    LocalizationReinsertionGate, LocalizationRenderPath, LocalizationSourceLayout,
};
use crate::source::LocalizationSourceCatalog;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationWorkspaceIndex {
    pub source_sha256: String,
    pub target_item_count: usize,
    pub segment_count: usize,
    pub segments: Vec<TranslationSegmentIndex>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationSegmentIndex {
    pub id: String,
    pub path: String,
    pub entry_count: usize,
    pub protected_sha256: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationSegment {
    pub id: String,
    pub source_sha256: String,
    pub source_catalog: LocalizationSourceCatalog,
    pub context: LocalizationContext,
    pub render_path: LocalizationRenderPath,
    pub glyph_supply: LocalizationGlyphSupply,
    pub source_layout: LocalizationSourceLayout,
    pub glyph_gate: LocalizationGlyphGate,
    pub reinsertion_gate: LocalizationReinsertionGate,
    pub entries: Vec<TranslationEntry>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationEntry {
    pub id: String,
    pub source_filename: String,
    pub source_record_id: String,
    pub evidence: TranslationSourceEvidence,
    pub consumer_contexts: Vec<TranslationConsumerContext>,
    pub content: Vec<TranslationContentPart>,
    pub status: TranslationStatus,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationSourceEvidence {
    pub byte_records: Vec<TranslationByteRecord>,
    pub graphic_asset: Option<TranslationGraphicAsset>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationByteRecord {
    pub id: String,
    pub file_offset: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationGraphicAsset {
    pub maddat_entry_id: usize,
    pub entry_file_offset: usize,
    pub packed_size: usize,
    pub packed_sha256: String,
    pub decoded_size: usize,
    pub decoded_sha256: String,
    pub width: usize,
    pub height: usize,
    pub plane_count: usize,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationConsumerContext {
    pub id: String,
    pub role: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TranslationContentPart {
    Text {
        source_text: String,
        korean_text: Option<String>,
    },
    Voice {
        parameter: u8,
    },
    Portrait {
        parameters: [u8; 2],
    },
    IndexedLineBreak {
        parameters: [u8; 2],
    },
    LineBreak,
    DynamicDecimal {
        cell_count: usize,
    },
    Terminator,
    OpeningMode {
        value: u8,
    },
    OpeningSprite {
        glyph_index: u8,
        position: u16,
    },
    OpeningScroll,
    OpeningYield,
    OpeningEnd,
    AnsiControl {
        raw_hex: String,
    },
    CarriageReturn,
    LineFeed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationStatus {
    Untranslated,
    InProgress,
    NeedsReview,
    NeedsHumanReview,
    Complete,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TranslationPreparationReport {
    pub source_sha256: String,
    pub output_directory: PathBuf,
    pub target_item_count: usize,
    pub segment_count: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TranslationValidationReport {
    pub source_sha256: String,
    pub target_item_count: usize,
    pub segment_count: usize,
    pub untranslated_item_count: usize,
    pub in_progress_item_count: usize,
    pub needs_review_item_count: usize,
    pub needs_human_review_item_count: usize,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationDraftIndex {
    pub source_sha256: String,
    pub target_item_count: usize,
    pub segment_count: usize,
    pub segments: Vec<TranslationDraftSegmentIndex>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationDraftSegmentIndex {
    pub id: String,
    pub path: String,
    pub entry_count: usize,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationDraftSegment {
    pub id: String,
    pub entries: Vec<TranslationDraftEntry>,
    pub review: Option<TranslationSegmentReview>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationDraftEntry {
    pub id: String,
    pub korean_text: Vec<Option<String>>,
    pub status: TranslationStatus,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationSegmentReview {
    pub reviewed_draft_sha256: String,
    pub outcome: TranslationReviewOutcome,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationReviewOutcome {
    ReadyForHumanReview,
    HumanDecisionRequired,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RecordedTranslationReview {
    pub segment_id: String,
    pub entry_count: usize,
    pub reviewed_draft_sha256: String,
    pub outcome: TranslationReviewOutcome,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TranslationDraftPreparationReport {
    pub source_sha256: String,
    pub output_directory: PathBuf,
    pub target_item_count: usize,
    pub segment_count: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TranslationDraftValidationReport {
    pub source_sha256: String,
    pub target_item_count: usize,
    pub segment_count: usize,
    pub untranslated_item_count: usize,
    pub in_progress_item_count: usize,
    pub needs_review_item_count: usize,
    pub needs_human_review_item_count: usize,
    pub reviewed_segment_count: usize,
    pub human_review_ready: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationGlyphAuditReport {
    pub source_sha256: String,
    pub target_item_count: usize,
    pub segment_count: usize,
    pub human_review_ready: bool,
    pub font: FontRepertoireReport,
    pub ending_credit_font: FontRepertoireReport,
    pub render_paths: Vec<TranslationGlyphRenderPathReport>,
    pub families: Vec<TranslationGlyphFamilyReport>,
    pub segments: Vec<TranslationGlyphSegmentReport>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationGlyphRenderPathReport {
    pub render_path: LocalizationRenderPath,
    pub entry_count: usize,
    pub text_span_count: usize,
    pub demand: TranslationGlyphDemand,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationGlyphFamilyReport {
    pub source_catalog: LocalizationSourceCatalog,
    pub render_path: LocalizationRenderPath,
    pub entry_count: usize,
    pub text_span_count: usize,
    pub demand: TranslationGlyphDemand,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationGlyphSegmentReport {
    pub id: String,
    pub source_catalog: LocalizationSourceCatalog,
    pub render_path: LocalizationRenderPath,
    pub entry_count: usize,
    pub text_span_count: usize,
    pub demand: TranslationGlyphDemand,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationGlyphDemand {
    pub unique_rendered_character_count: usize,
    pub unique_hangul_syllable_count: usize,
    pub slot_glyph_count: Option<usize>,
    pub verified_physical_capacity: Option<usize>,
    pub physical_capacity_headroom: Option<isize>,
    pub physical_capacity_fit: Option<bool>,
    pub rendered_characters: Vec<String>,
    pub slot_glyph_characters: Vec<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationLayoutAuditReport {
    pub source_sha256: String,
    pub target_item_count: usize,
    pub segment_count: usize,
    pub human_review_ready: bool,
    pub verified_hard_limits_fit: bool,
    pub measured_unit_count: usize,
    pub hard_limit_unit_count: usize,
    pub glyph_token_count: usize,
    pub hard_violation_count: usize,
    pub source_growth_advisory_count: usize,
    pub families: Vec<TranslationLayoutFamilyReport>,
    pub open_gates: Vec<TranslationLayoutOpenGate>,
    pub findings: Vec<TranslationLayoutFinding>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationLayoutFamilyReport {
    pub source_catalog: LocalizationSourceCatalog,
    pub source_layout: LocalizationSourceLayout,
    pub entry_count: usize,
    pub measured_unit_count: usize,
    pub hard_limit_cells: Option<usize>,
    pub max_source_cells: usize,
    pub max_korean_cells: usize,
    pub hard_violation_count: usize,
    pub source_growth_advisory_count: usize,
    pub consumer_specific_limit_resolved: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationLayoutOpenGate {
    pub source_catalog: LocalizationSourceCatalog,
    pub affected_entry_count: usize,
    pub reason: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationLayoutFindingKind {
    HardOverflow,
    GlyphTokenWidth,
    EmbeddedLineBreak,
    SourceGrowth,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationLayoutFinding {
    pub kind: TranslationLayoutFindingKind,
    pub source_catalog: LocalizationSourceCatalog,
    pub segment_id: String,
    pub entry_id: String,
    pub unit_index: usize,
    pub start_column: usize,
    pub source_cells: usize,
    pub korean_cells: usize,
    pub hard_limit_cells: Option<usize>,
    pub source_text: String,
    pub korean_text: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationReviewQueueReport {
    pub source_sha256: String,
    pub target_item_count: usize,
    pub segment_count: usize,
    pub human_review_ready: bool,
    pub human_decision_segment_count: usize,
    pub human_decision_entry_count: usize,
    pub ready_segment_count: usize,
    pub ready_entry_count: usize,
    pub segments: Vec<TranslationReviewQueueSegment>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationReviewQueueSegment {
    pub id: String,
    pub outcome: TranslationReviewOutcome,
    pub notes: Vec<String>,
    pub entries: Vec<TranslationReviewQueueEntry>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationReviewQueueEntry {
    pub id: String,
    pub source_filename: String,
    pub source_record_id: String,
    pub consumer_roles: Vec<String>,
    pub text: Vec<TranslationReviewTextPair>,
    pub notes: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct TranslationReviewTextPair {
    pub source: String,
    pub korean: String,
}
