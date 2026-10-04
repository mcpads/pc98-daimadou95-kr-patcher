pub(crate) mod disk;
mod graphic_text_catalog;
pub(crate) mod installer;
mod localization_catalog;
mod mad_indexed_catalog;
mod state_graphic_catalog;
mod survey;
pub(crate) mod verified;

pub use disk::{SOURCE_DISK_SHA256, SOURCE_DISK_SIZE};
pub use graphic_text_catalog::{
    GraphicTextCatalog, GraphicTextConsumerEvidence, GraphicTextSurface,
};
pub(crate) use graphic_text_catalog::{TITLE_MENU_PACKED_SHA256, verify_title_menu_consumer};
pub use localization_catalog::{
    LocalizationCatalog, LocalizationFileCoverage, LocalizationFileReason, LocalizationFileStatus,
    LocalizationMaddatEntryCoverage, LocalizationMaddatReason, LocalizationMaddatStatus,
    LocalizationSourceCatalog, LocalizationSummary, LocalizationTextDisposition,
    LocalizationTextItem, LocalizationTextReason,
};
pub use mad_indexed_catalog::{
    MadIndexedBinding, MadIndexedBindingContentKind, MadIndexedGlyph, MadIndexedRendererEvidence,
    MadIndexedString, MadIndexedTextCatalog, MadIndexedTextPool,
};
pub(crate) use state_graphic_catalog::catalog_state_graphics;
pub use state_graphic_catalog::{
    FieldUnitCallout, FieldUnitCalloutBank, FieldUnitCalloutCatalog,
    FieldUnitCalloutConsumerEvidence, FieldUnitCalloutPaletteUsage, StateGraphicCatalog,
    StateGraphicConsumerEvidence, StateGraphicEvidenceNeed, StateGraphicFrame,
    StateGraphicLocalizationStatus, StateGraphicObservation, StateGraphicSurface,
    StateGraphicTrigger,
};
pub use survey::{
    DosDisplayToken, DosProgramCatalog, DosProgramDisplayEvidence, DosProgramDisplayKind,
    DosProgramTextCatalog, DosProgramTextRecord, DosProgramTextReference,
    DosProgramTextReferenceKind, EndingCreditConsumerEvidence, EndingCreditEntryLoadReference,
    EndingCreditFontCatalog, EndingCreditLine, EndingCreditMotionTrackCatalog, EndingCreditPoint,
    EndingCreditScreen, EndingCreditTextCatalog, EndingCreditsCatalog, EndingCurryResultCatalog,
    EndingCurryResultConsumerEvidence, EndingCurryResultMessage, EndingDynamicDateTemplate,
    EndingDynamicDateUnit, EndingIngredientLabelCatalog, EndingIngredientLabelConsumerEvidence,
    EndingIngredientLabelRecord, EndingInterludeCaption, EndingInterludeConsumerEvidence,
    EndingMealCaptionCatalog, EndingMealCaptionConsumerEvidence, EndingMealCaptionRecord,
    EndingMealCaptionSpan, EndingPasswordCatalog, EndingPasswordConsumerEvidence,
    EndingPasswordGlyph, EndingPasswordRecord, GaijiCatalog, GaijiCatalogEntry, GaijiContentKind,
    IndexedFontCatalog, IndexedTextToken, MadFixedSlotContentKind, MadFixedTextSlot,
    MadTextCatalog, MadTextRendererEvidence, MadTextToken, MaddatCatalog, MaddatCatalogEntry,
    MaddatStorage, MstNumericTemplate, MstTextCatalog, MstTextRecord, OpeningTextCatalog,
    OpeningTextConsumerEvidence, OpeningTextToken, SelectDialogueCatalog,
    SelectDialogueInterpreterEvidence, SelectDialogueScript, SelectDialogueStage,
    SourceStructureReport, survey_source_path,
};
pub use verified::{SourceVerification, verify_source_path};
