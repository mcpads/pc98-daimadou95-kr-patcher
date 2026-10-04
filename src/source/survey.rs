use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use encoding_rs::SHIFT_JIS;
use serde::Serialize;

use crate::game_data::{
    compile_lz, dos_program_text, dos_text, ending_credits, ending_curry_result,
    ending_ingredient_labels, ending_meal_captions, ending_meal_reaction, flink, gaiji, gcs,
    indexed_text, mad_text, masked_sprite, opening_script,
};
use crate::localization::{LocalizationStrategy, plan_localization};
use crate::source::disk;
use crate::source::graphic_text_catalog::{GraphicTextCatalog, catalog_graphic_text};
use crate::source::localization_catalog::{
    LocalizationCatalog, LocalizationInputs, catalog_localization,
};
use crate::source::mad_indexed_catalog::{MadIndexedTextCatalog, catalog_mad_indexed_text};
use crate::source::state_graphic_catalog::{StateGraphicCatalog, catalog_state_graphics};
use crate::source::verified::load_verified_source;

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SourceStructureReport {
    pub source_sha256: String,
    pub localization: LocalizationCatalog,
    pub localization_strategy: LocalizationStrategy,
    pub maddat: MaddatCatalog,
    pub indexed_font: IndexedFontCatalog,
    pub select_dialogue: SelectDialogueCatalog,
    pub opening_text: OpeningTextCatalog,
    pub graphic_text: GraphicTextCatalog,
    pub state_graphics: StateGraphicCatalog,
    pub ending_credits: EndingCreditsCatalog,
    pub ending_curry_result: EndingCurryResultCatalog,
    pub ending_meal_captions: EndingMealCaptionCatalog,
    pub ending_meal_reaction: EndingMealReactionCatalog,
    pub ending_interlude_caption: EndingInterludeCaption,
    pub ending_ingredient_labels: EndingIngredientLabelCatalog,
    pub ending_passwords: EndingPasswordCatalog,
    pub mst_text: MstTextCatalog,
    pub dos_program_text: DosProgramTextCatalog,
    pub mad_indexed_text: MadIndexedTextCatalog,
    pub gaiji: GaijiCatalog,
    pub mad_text: MadTextCatalog,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MaddatCatalog {
    pub sha256: String,
    pub file_size: usize,
    pub flink_signature: String,
    pub uninterpreted_header_byte: u8,
    pub table_offset: usize,
    pub table_end_offset: usize,
    pub first_entry_offset: Option<usize>,
    pub last_entry_end_offset: Option<usize>,
    pub entries_are_contiguous: bool,
    pub entries: Vec<MaddatCatalogEntry>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MaddatCatalogEntry {
    pub id: usize,
    pub offset: usize,
    pub size: usize,
    pub sha256: String,
    pub storage: MaddatStorage,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MaddatStorage {
    CompileLz {
        decoded_size: usize,
        command_count: usize,
    },
    MaskedSpriteBank {
        decoded_size: usize,
        command_count: usize,
        tile_count: usize,
        tile_width: usize,
        tile_height: usize,
        mask_plane_count: usize,
        color_plane_count: usize,
        bytes_per_tile: usize,
    },
    GcsScreen {
        width: usize,
        height: usize,
        plane_count: usize,
        decoded_size: usize,
        command_counts: [usize; gcs::PLANE_COUNT],
    },
    OpeningTextScript {
        command_count: usize,
        glyph_count: usize,
        line_break_count: usize,
    },
    EndingCreditTextGrid {
        screen_count: usize,
        lines_per_screen: usize,
        cells_per_line: usize,
    },
    EndingCreditMotionTrack {
        point_count: usize,
        terminator: u16,
    },
    Unresolved {
        leading_bytes_hex: String,
    },
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct IndexedFontCatalog {
    pub maddat_entry_id: usize,
    pub decoded_size: usize,
    pub decoded_sha256: String,
    pub glyph_count: usize,
    pub glyph_size: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SelectDialogueCatalog {
    pub select_com_sha256: String,
    pub file_size: usize,
    pub interpreter: SelectDialogueInterpreterEvidence,
    pub pointer_table_file_offset: usize,
    pub pointer_table_runtime_address: usize,
    pub stages: Vec<SelectDialogueStage>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SelectDialogueInterpreterEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub call_site_file_offsets: Vec<usize>,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SelectDialogueStage {
    pub stage_number: usize,
    pub scripts: Vec<SelectDialogueScript>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct SelectDialogueScript {
    pub id: String,
    pub phase_number: usize,
    pub pointer_entry_file_offset: usize,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub initial_portrait: [u8; 2],
    pub text: String,
    pub tokens: Vec<IndexedTextToken>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IndexedTextToken {
    Glyph { index: u8, text: String },
    Voice { parameter: u8 },
    Portrait { parameters: [u8; 2] },
    LineBreak { parameters: [u8; 2] },
    Terminator,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct OpeningTextCatalog {
    pub maddat_entry_id: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub text: String,
    pub consumer: OpeningTextConsumerEvidence,
    pub tokens: Vec<OpeningTextToken>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct OpeningTextConsumerEvidence {
    pub opening_com_sha256: String,
    pub file_size: usize,
    pub load_reference_file_offset: usize,
    pub call_site_file_offset: usize,
    pub interpreter_file_offset: usize,
    pub interpreter_runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OpeningTextToken {
    Mode { value: u8 },
    Sprite { glyph_index: u8, position: u16 },
    Glyph { index: u8, text: String },
    LineBreak,
    Scroll,
    Yield,
    End,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditsCatalog {
    pub ending_com_sha256: String,
    pub file_size: usize,
    pub contains_japanese: bool,
    pub requires_localization: bool,
    pub font: EndingCreditFontCatalog,
    pub text: EndingCreditTextCatalog,
    pub motion_tracks: Vec<EndingCreditMotionTrackCatalog>,
    pub consumer: EndingCreditConsumerEvidence,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditFontCatalog {
    pub maddat_entry_id: usize,
    pub stored_size: usize,
    pub stored_sha256: String,
    pub decoded_size: usize,
    pub decoded_sha256: String,
    pub glyph_count: usize,
    pub glyph_size: usize,
    pub reviewed_glyph_inventory: Vec<String>,
    pub contains_japanese: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditTextCatalog {
    pub maddat_entry_id: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub screens: Vec<EndingCreditScreen>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditScreen {
    pub screen_number: usize,
    pub lines: Vec<EndingCreditLine>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditLine {
    pub line_number: usize,
    pub file_offset: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub text: String,
    pub reviewed_role_text: Option<String>,
    pub requires_localization: bool,
    pub glyph_indices: Vec<u8>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditMotionTrackCatalog {
    pub line_number: usize,
    pub maddat_entry_id: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub terminator: u16,
    pub points: Vec<EndingCreditPoint>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditPoint {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditConsumerEvidence {
    pub font_load_reference_file_offset: usize,
    pub entry_load_reference_file_offsets: Vec<EndingCreditEntryLoadReference>,
    pub sequence_call_site_file_offset: usize,
    pub sequence_file_offset: usize,
    pub sequence_runtime_address: usize,
    pub sequence_decoded_instruction_count: usize,
    pub glyph_renderer_call_site_file_offset: usize,
    pub glyph_renderer_file_offset: usize,
    pub glyph_renderer_runtime_address: usize,
    pub glyph_renderer_decoded_instruction_count: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCreditEntryLoadReference {
    pub maddat_entry_id: usize,
    pub file_offset: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCurryResultCatalog {
    pub ending_com_sha256: String,
    pub file_size: usize,
    pub contains_japanese: bool,
    pub consumer: EndingCurryResultConsumerEvidence,
    pub month: EndingDynamicDateTemplate,
    pub day: EndingDynamicDateTemplate,
    pub messages: Vec<EndingCurryResultMessage>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCurryResultConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub selector_runtime_address: usize,
    pub message_runtime_addresses: Vec<usize>,
    pub month_template_reference_file_offset: usize,
    pub day_template_reference_file_offset: usize,
    pub glyph_renderer_call_file_offsets: Vec<usize>,
    pub glyph_renderer_runtime_address: usize,
    pub isa_profile_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EndingDynamicDateUnit {
    Month,
    Day,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingDynamicDateTemplate {
    pub id: String,
    pub unit: EndingDynamicDateUnit,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub initial_digit_indices: Vec<u8>,
    pub suffix_text: String,
    pub suffix_glyphs: Vec<EndingPasswordGlyph>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingCurryResultMessage {
    pub id: String,
    pub result_class: usize,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub lines: Vec<String>,
    pub line_break_counts: Vec<usize>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingMealReactionCatalog {
    pub ending_com_sha256: String,
    pub file_size: usize,
    pub contains_japanese: bool,
    pub id: String,
    pub text: String,
    pub glyph_loads: Vec<EndingMealReactionGlyphLoad>,
    pub consumer: EndingMealReactionConsumerEvidence,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingMealReactionGlyphLoad {
    pub slot_number: usize,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub glyph: EndingPasswordGlyph,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingMealReactionConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub glyph_load_file_offsets: Vec<usize>,
    pub glyph_renderer_call_file_offsets: Vec<usize>,
    pub glyph_renderer_runtime_address: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingMealCaptionCatalog {
    pub ending_com_sha256: String,
    pub file_size: usize,
    pub contains_japanese: bool,
    pub records: Vec<EndingMealCaptionRecord>,
    pub consumer: EndingMealCaptionConsumerEvidence,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingMealCaptionRecord {
    pub id: String,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub visual_state_indices: Vec<u8>,
    pub spans: Vec<EndingMealCaptionSpan>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingMealCaptionSpan {
    pub span_number: usize,
    pub text: String,
    pub slot_file_offsets: Vec<usize>,
    pub glyphs: Vec<EndingPasswordGlyph>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingMealCaptionConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub glyph_renderer_call_file_offset: usize,
    pub glyph_renderer_runtime_address: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingPasswordCatalog {
    pub ending_com_sha256: String,
    pub file_size: usize,
    pub contains_japanese: bool,
    pub consumer: EndingPasswordConsumerEvidence,
    pub pointer_table_file_offset: usize,
    pub pointer_table_runtime_address: usize,
    pub selector_file_offset: usize,
    pub selector_runtime_address: usize,
    pub records: Vec<EndingPasswordRecord>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingIngredientLabelCatalog {
    pub ending_com_sha256: String,
    pub file_size: usize,
    pub contains_japanese: bool,
    pub consumer: EndingIngredientLabelConsumerEvidence,
    pub table_file_offset: usize,
    pub table_runtime_address: usize,
    pub table_byte_size: usize,
    pub records: Vec<EndingIngredientLabelRecord>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingIngredientLabelConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub pointer_list_runtime_address: usize,
    pub input_state_offsets: Vec<u8>,
    pub group_runtime_addresses: Vec<usize>,
    pub fixed_cell_count: usize,
    pub fixed_cell_loop_file_offset: usize,
    pub glyph_renderer_call_file_offset: usize,
    pub glyph_renderer_runtime_address: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingIngredientLabelRecord {
    pub id: String,
    pub group_number: usize,
    pub variant_number: usize,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub text: String,
    pub glyphs: Vec<EndingPasswordGlyph>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingInterludeCaption {
    pub ending_com_sha256: String,
    pub file_size: usize,
    pub contains_japanese: bool,
    pub consumer: EndingInterludeConsumerEvidence,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub text: String,
    pub glyphs: Vec<EndingPasswordGlyph>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingInterludeConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub font_load_file_offset: usize,
    pub background_entry_id: usize,
    pub background_load_file_offset: usize,
    pub glyph_renderer_call_file_offset: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingPasswordConsumerEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub call_site_file_offset: usize,
    pub font_load_file_offset: usize,
    pub glyph_renderer_call_file_offset: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingPasswordRecord {
    pub id: String,
    pub selector_value: usize,
    pub pointer_entry_file_offset: usize,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub text: String,
    pub glyphs: Vec<EndingPasswordGlyph>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct EndingPasswordGlyph {
    pub index: u8,
    pub text: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MstTextCatalog {
    pub mst_com_sha256: String,
    pub file_size: usize,
    pub records: Vec<MstTextRecord>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MstTextRecord {
    pub id: String,
    pub consumer: String,
    pub reference_file_offsets: Vec<usize>,
    pub numeric_template: Option<MstNumericTemplate>,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub contains_japanese: bool,
    pub text_segments: Vec<String>,
    pub tokens: Vec<DosDisplayToken>,
}

/// A run inside one MST record whose decimal digits the program rewrites at
/// run time before printing. The declared code sites load the run's base
/// address, so relocating the record has to retarget them as well.
#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MstNumericTemplate {
    pub interior_offset: usize,
    pub byte_len: usize,
    pub digit_mask: Vec<bool>,
    pub base_reference_file_offsets: Vec<usize>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DosDisplayToken {
    AnsiControl { raw_hex: String },
    CarriageReturn,
    LineFeed,
    SingleByte { byte: u8, text: String },
    ShiftJis { shift_jis_code: u16, text: String },
    Terminator,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct DosProgramTextCatalog {
    pub programs: Vec<DosProgramCatalog>,
    pub record_count: usize,
    pub japanese_record_count: usize,
    pub bound_japanese_record_count: usize,
    pub unbound_japanese_record_ids: Vec<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct DosProgramCatalog {
    pub filename: String,
    pub sha256: String,
    pub file_size: usize,
    pub display_sites: Vec<DosProgramDisplayEvidence>,
    pub records: Vec<DosProgramTextRecord>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct DosProgramDisplayEvidence {
    pub kind: DosProgramDisplayKind,
    pub file_offset: usize,
    pub runtime_address: usize,
    pub byte_len: usize,
    pub decoded_instruction_count: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DosProgramDisplayKind {
    Ah09Subroutine,
    DirectAh09,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct DosProgramTextRecord {
    pub id: String,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub bound_to_consumer: bool,
    pub contains_japanese: bool,
    pub text_segments: Vec<String>,
    pub tokens: Vec<DosDisplayToken>,
    pub references: Vec<DosProgramTextReference>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct DosProgramTextReference {
    pub kind: DosProgramTextReferenceKind,
    pub file_offset: usize,
    pub output_file_offset: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DosProgramTextReferenceKind {
    Immediate,
    PointerTable,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct GaijiCatalog {
    pub sha256: String,
    pub file_size: usize,
    pub pointer_table_offset: usize,
    pub first_character_code: Option<u16>,
    pub last_character_code: Option<u16>,
    pub first_shift_jis_code: Option<u16>,
    pub last_shift_jis_code: Option<u16>,
    pub uniform_glyph_size: Option<usize>,
    pub reviewed_content_kind: GaijiContentKind,
    pub contains_japanese_text: bool,
    pub glyphs: Vec<GaijiCatalogEntry>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GaijiContentKind {
    PixelArtFragments,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct GaijiCatalogEntry {
    pub index: usize,
    pub character_code: u16,
    pub shift_jis_code: u16,
    pub offset: usize,
    pub size: usize,
    pub sha256: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadTextCatalog {
    pub mad_com_sha256: String,
    pub file_size: usize,
    pub renderer: MadTextRendererEvidence,
    pub fixed_slots: Vec<MadFixedTextSlot>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadTextRendererEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub fixed_text_call_site_file_offset: usize,
    pub decoded_instruction_count: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadFixedTextSlot {
    pub id: String,
    pub pointer_entry_offsets: [usize; 2],
    pub file_offset: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub content_kind: MadFixedSlotContentKind,
    pub contains_japanese_text: bool,
    pub row_count: usize,
    pub cells_per_row: usize,
    pub tokens: Vec<MadTextToken>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MadFixedSlotContentKind {
    PixelArtComposite,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MadTextToken {
    DisplayControl {
        code: String,
        palette_index: u8,
    },
    LineBreak,
    SingleByte {
        byte: u8,
        text: String,
    },
    ShiftJis {
        shift_jis_code: u16,
        text: String,
    },
    Space,
    Gaiji {
        glyph_index: usize,
        character_code: u16,
        shift_jis_code: u16,
    },
    Terminator,
}

pub fn survey_source_path(source_path: &Path) -> Result<SourceStructureReport> {
    let verified_source = load_verified_source(source_path)?;
    let maddat = verified_source
        .installer_payload
        .get("MADDAT")
        .context("verified installer payload is missing MADDAT")?;
    let gaiji = verified_source
        .installer_payload
        .get("GAIJI.COM")
        .context("verified installer payload is missing GAIJI.COM")?;
    let mad_com = verified_source
        .installer_payload
        .get("MAD.COM")
        .context("verified installer payload is missing MAD.COM")?;
    let select_com = verified_source
        .installer_payload
        .get("SELECT.COM")
        .context("verified installer payload is missing SELECT.COM")?;
    let mst_com = verified_source
        .installer_payload
        .get("MST.COM")
        .context("verified installer payload is missing MST.COM")?;
    let opening_com = verified_source
        .installer_payload
        .get("OPENING.COM")
        .context("verified installer payload is missing OPENING.COM")?;
    let ending_com = verified_source
        .installer_payload
        .get("ENDING.COM")
        .context("verified installer payload is missing ENDING.COM")?;
    let gaiji_catalog = catalog_gaiji(gaiji)?;
    let (indexed_font, select_dialogue) = catalog_indexed_dialogue(maddat, select_com)?;
    let dos_program_text = catalog_dos_program_text(|filename| {
        verified_source
            .installer_payload
            .get(filename)
            .or_else(|| verified_source.source_files.tool_files.get(filename))
            .map(Vec::as_slice)
            .with_context(|| format!("verified source program set is missing {filename}"))
    })?;

    let maddat_catalog = catalog_maddat(maddat)?;
    let opening_text = catalog_opening_text(maddat, opening_com)?;
    let state_graphics = catalog_state_graphics(maddat, mad_com)?;
    let graphic_text =
        catalog_graphic_text(maddat, opening_com, select_com, mad_com, &state_graphics)?;
    let ending_credits = catalog_ending_credits(maddat, ending_com)?;
    let ending_curry_result = catalog_ending_curry_result(ending_com)?;
    let ending_meal_captions = catalog_ending_meal_captions(ending_com)?;
    let ending_meal_reaction = catalog_ending_meal_reaction(ending_com)?;
    let ending_interlude_caption = catalog_ending_interlude_caption(ending_com)?;
    let ending_ingredient_labels = catalog_ending_ingredient_labels(ending_com)?;
    let ending_passwords = catalog_ending_passwords(ending_com)?;
    let mst_text = catalog_mst_text(mst_com)?;
    let mad_indexed_text = catalog_mad_indexed_text(mad_com)?;
    let mad_text = catalog_mad_text(mad_com, &gaiji_catalog)?;
    let runtime_files = verified_source
        .source_files
        .system_files
        .iter()
        .chain(verified_source.source_files.tool_files.iter())
        .chain(verified_source.installer_payload.iter())
        .map(|(filename, bytes)| (filename.clone(), bytes.as_slice()))
        .collect();
    let localization = catalog_localization(LocalizationInputs {
        runtime_files,
        maddat: &maddat_catalog,
        select_dialogue: &select_dialogue,
        opening_text: &opening_text,
        graphic_text: &graphic_text,
        ending_credits: &ending_credits,
        ending_curry_result: &ending_curry_result,
        ending_meal_captions: &ending_meal_captions,
        ending_meal_reaction: &ending_meal_reaction,
        ending_interlude_caption: &ending_interlude_caption,
        ending_ingredient_labels: &ending_ingredient_labels,
        ending_passwords: &ending_passwords,
        mst_text: &mst_text,
        dos_program_text: &dos_program_text,
        mad_indexed_text: &mad_indexed_text,
    })?;
    let localization_strategy = plan_localization(&localization)?;

    Ok(SourceStructureReport {
        source_sha256: verified_source.verification.sha256,
        localization,
        localization_strategy,
        maddat: maddat_catalog,
        indexed_font,
        select_dialogue,
        opening_text,
        graphic_text,
        state_graphics,
        ending_credits,
        ending_curry_result,
        ending_meal_captions,
        ending_meal_reaction,
        ending_interlude_caption,
        ending_ingredient_labels,
        ending_passwords,
        mst_text,
        dos_program_text,
        mad_indexed_text,
        mad_text,
        gaiji: gaiji_catalog,
    })
}

fn catalog_maddat(bytes: &[u8]) -> Result<MaddatCatalog> {
    let archive = flink::parse_flink_archive(bytes)?;
    let entries: Vec<_> = archive
        .entries
        .iter()
        .map(|entry| {
            Ok(MaddatCatalogEntry {
                id: entry.id,
                offset: entry.offset,
                size: entry.bytes.len(),
                sha256: disk::sha256_hex(entry.bytes),
                storage: catalog_maddat_storage(entry.id, entry.bytes)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let table_end_offset = flink::HEADER_SIZE + entries.len() * flink::ENTRY_RECORD_SIZE;
    let entries_are_contiguous = entries
        .windows(2)
        .all(|pair| pair[0].offset + pair[0].size == pair[1].offset);

    Ok(MaddatCatalog {
        sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        flink_signature: String::from_utf8_lossy(flink::SIGNATURE).into_owned(),
        uninterpreted_header_byte: archive.uninterpreted_header_byte,
        table_offset: flink::HEADER_SIZE,
        table_end_offset,
        first_entry_offset: entries.first().map(|entry| entry.offset),
        last_entry_end_offset: entries.last().map(|entry| entry.offset + entry.size),
        entries_are_contiguous,
        entries,
    })
}

fn catalog_maddat_storage(entry_id: usize, bytes: &[u8]) -> Result<MaddatStorage> {
    if bytes.starts_with(gcs::SIGNATURE) {
        let screen = gcs::decode_gcs(bytes)?;
        return Ok(MaddatStorage::GcsScreen {
            width: gcs::SCREEN_WIDTH,
            height: gcs::SCREEN_HEIGHT,
            plane_count: gcs::PLANE_COUNT,
            decoded_size: gcs::PLANE_SIZE * gcs::PLANE_COUNT,
            command_counts: screen.command_counts,
        });
    }

    if matches!(entry_id, 7..=9 | 111..=119) {
        let bank = masked_sprite::decode_masked_sprite_bank(bytes)?;
        return Ok(MaddatStorage::MaskedSpriteBank {
            decoded_size: bank.decoded.len(),
            command_count: bank.command_count,
            tile_count: bank.tiles.len(),
            tile_width: masked_sprite::TILE_WIDTH,
            tile_height: masked_sprite::TILE_HEIGHT,
            mask_plane_count: masked_sprite::MASK_PLANE_COUNT,
            color_plane_count: masked_sprite::COLOR_PLANE_COUNT,
            bytes_per_tile: masked_sprite::BYTES_PER_TILE,
        });
    }

    if let Ok(stream) = compile_lz::decode_complete_compile_lz(bytes) {
        return Ok(MaddatStorage::CompileLz {
            decoded_size: stream.output.len(),
            command_count: stream.command_count,
        });
    }

    if entry_id == opening_script::MADDAT_ENTRY_ID {
        let script = opening_script::parse_opening_text_script(bytes)?;
        return Ok(MaddatStorage::OpeningTextScript {
            command_count: script.tokens.len(),
            glyph_count: script
                .tokens
                .iter()
                .filter(|token| matches!(token, opening_script::OpeningScriptToken::Glyph { .. }))
                .count(),
            line_break_count: script
                .tokens
                .iter()
                .filter(|token| matches!(token, opening_script::OpeningScriptToken::LineBreak))
                .count(),
        });
    }

    if entry_id == ending_credits::TEXT_ENTRY_ID {
        let grid = ending_credits::parse_ending_credit_grid(bytes)?;
        return Ok(MaddatStorage::EndingCreditTextGrid {
            screen_count: grid.screens.len(),
            lines_per_screen: ending_credits::LINES_PER_SCREEN,
            cells_per_line: ending_credits::CELLS_PER_LINE,
        });
    }

    if ending_credits::MOTION_TRACK_ENTRY_IDS.contains(&entry_id) {
        let track = ending_credits::parse_ending_credit_motion_track(bytes)?;
        return Ok(MaddatStorage::EndingCreditMotionTrack {
            point_count: track.points.len(),
            terminator: track.terminator,
        });
    }

    Ok(MaddatStorage::Unresolved {
        leading_bytes_hex: encode_hex(&bytes[..bytes.len().min(16)]),
    })
}

fn catalog_ending_credits(maddat: &[u8], ending_com: &[u8]) -> Result<EndingCreditsCatalog> {
    let archive = flink::parse_flink_archive(maddat)?;
    let entry = |entry_id: usize| -> Result<_> {
        let entry = archive
            .entries
            .get(entry_id - 1)
            .with_context(|| format!("MADDAT is missing ending-credit entry {entry_id}"))?;
        ensure!(
            entry.id == entry_id,
            "MADDAT ending-credit entry {entry_id} does not match its table position"
        );
        Ok(entry)
    };

    let font_entry = entry(ending_credits::FONT_ENTRY_ID)?;
    let decoded_font = compile_lz::decode_complete_compile_lz(font_entry.bytes)?.output;
    ending_credits::verify_ending_credit_font(&decoded_font)?;

    let text_entry = entry(ending_credits::TEXT_ENTRY_ID)?;
    let grid = ending_credits::parse_ending_credit_grid(text_entry.bytes)?;
    let screens = grid
        .screens
        .iter()
        .enumerate()
        .map(|(screen_index, screen)| {
            Ok(EndingCreditScreen {
                screen_number: screen_index + 1,
                lines: screen
                    .lines
                    .iter()
                    .enumerate()
                    .map(|(line_index, line)| {
                        let screen_number = screen_index + 1;
                        let line_number = line_index + 1;
                        let spec = ending_credits::role_line_spec(screen_number, line_number);
                        if let Some(spec) = spec {
                            ensure!(
                                line.text.trim() == spec.source_text,
                                "ending-credit role {} contains {:?} instead of {:?}",
                                spec.record_id(),
                                line.text,
                                spec.source_text
                            );
                        }
                        let line_offset =
                            ending_credits::ending_credit_line_offset(screen_number, line_number)?;
                        let line_bytes = &text_entry.bytes
                            [line_offset..line_offset + ending_credits::CELLS_PER_LINE];
                        Ok(EndingCreditLine {
                            line_number,
                            file_offset: text_entry.offset + line_offset,
                            byte_size: line_bytes.len(),
                            sha256: disk::sha256_hex(line_bytes),
                            raw_hex: encode_hex(line_bytes),
                            text: line.text.clone(),
                            reviewed_role_text: spec.map(|spec| spec.source_text.to_owned()),
                            requires_localization: spec.is_some(),
                            glyph_indices: line.glyph_indices.to_vec(),
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let contains_japanese = screens
        .iter()
        .flat_map(|screen| &screen.lines)
        .flat_map(|line| line.text.chars())
        .any(is_japanese_character);

    let motion_tracks = ending_credits::MOTION_TRACK_ENTRY_IDS
        .iter()
        .enumerate()
        .map(|(line_index, entry_id)| {
            let entry = entry(*entry_id)?;
            let track = ending_credits::parse_ending_credit_motion_track(entry.bytes)?;
            ensure!(
                ending_credits::reconstruct_ending_credit_motion_track(&track)? == entry.bytes,
                "ending-credit motion track {entry_id} does not reconstruct in the source catalog"
            );
            Ok(EndingCreditMotionTrackCatalog {
                line_number: line_index + 1,
                maddat_entry_id: *entry_id,
                byte_size: entry.bytes.len(),
                sha256: disk::sha256_hex(entry.bytes),
                terminator: track.terminator,
                points: track
                    .points
                    .into_iter()
                    .map(|point| EndingCreditPoint {
                        x: point.x,
                        y: point.y,
                    })
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let consumer = ending_credits::verify_ending_credit_consumer(ending_com)?;
    Ok(EndingCreditsCatalog {
        ending_com_sha256: disk::sha256_hex(ending_com),
        file_size: ending_com.len(),
        contains_japanese,
        requires_localization: screens
            .iter()
            .flat_map(|screen| &screen.lines)
            .any(|line| line.requires_localization),
        font: EndingCreditFontCatalog {
            maddat_entry_id: font_entry.id,
            stored_size: font_entry.bytes.len(),
            stored_sha256: disk::sha256_hex(font_entry.bytes),
            decoded_size: decoded_font.len(),
            decoded_sha256: disk::sha256_hex(&decoded_font),
            glyph_count: ending_credits::FONT_GLYPH_COUNT,
            glyph_size: ending_credits::FONT_GLYPH_SIZE,
            reviewed_glyph_inventory: vec![
                "Latin uppercase A-Z".to_owned(),
                "Latin lowercase a-z".to_owned(),
                "digits and symbols".to_owned(),
                "Puyo mascot glyph".to_owned(),
            ],
            contains_japanese: false,
        },
        text: EndingCreditTextCatalog {
            maddat_entry_id: text_entry.id,
            byte_size: text_entry.bytes.len(),
            sha256: disk::sha256_hex(text_entry.bytes),
            screens,
        },
        motion_tracks,
        consumer: EndingCreditConsumerEvidence {
            font_load_reference_file_offset: consumer.font_load_reference_file_offset,
            entry_load_reference_file_offsets: consumer
                .entry_load_reference_file_offsets
                .into_iter()
                .map(
                    |(maddat_entry_id, file_offset)| EndingCreditEntryLoadReference {
                        maddat_entry_id,
                        file_offset,
                    },
                )
                .collect(),
            sequence_call_site_file_offset: consumer.sequence_call_site_file_offset,
            sequence_file_offset: consumer.sequence_file_offset,
            sequence_runtime_address: consumer.sequence_runtime_address,
            sequence_decoded_instruction_count: consumer.sequence_decoded_instruction_count,
            glyph_renderer_call_site_file_offset: consumer.glyph_renderer_call_site_file_offset,
            glyph_renderer_file_offset: consumer.glyph_renderer_file_offset,
            glyph_renderer_runtime_address: consumer.glyph_renderer_runtime_address,
            glyph_renderer_decoded_instruction_count: consumer
                .glyph_renderer_decoded_instruction_count,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
    })
}

fn catalog_ending_curry_result(bytes: &[u8]) -> Result<EndingCurryResultCatalog> {
    let catalog = ending_curry_result::parse_ending_curry_result(bytes)?;
    let date_template =
        |id: &str, unit, template: ending_curry_result::EndingDynamicDateTemplate<'_>| {
            EndingDynamicDateTemplate {
                id: id.to_owned(),
                unit,
                file_offset: template.file_offset,
                runtime_address: template.runtime_address,
                byte_size: template.bytes.len(),
                sha256: disk::sha256_hex(template.bytes),
                raw_hex: encode_hex(template.bytes),
                initial_digit_indices: template.initial_digit_indices.into(),
                suffix_text: template.suffix_text,
                suffix_glyphs: template
                    .suffix_glyphs
                    .into_iter()
                    .map(|glyph| EndingPasswordGlyph {
                        index: glyph.index,
                        text: glyph
                            .text
                            .expect("ENDING date parser requires resolved glyphs")
                            .to_owned(),
                    })
                    .collect(),
            }
        };
    let month = date_template("month-suffix", EndingDynamicDateUnit::Month, catalog.month);
    let day = date_template("day-suffix", EndingDynamicDateUnit::Day, catalog.day);
    let messages = catalog
        .messages
        .into_iter()
        .map(|message| EndingCurryResultMessage {
            id: format!("result-class-{:02}", message.result_class + 1),
            result_class: message.result_class,
            file_offset: message.file_offset,
            runtime_address: message.runtime_address,
            byte_size: message.bytes.len(),
            sha256: disk::sha256_hex(message.bytes),
            raw_hex: encode_hex(message.bytes),
            lines: message.lines.into_iter().map(|line| line.text).collect(),
            line_break_counts: message.line_break_counts,
        })
        .collect::<Vec<_>>();
    let contains_japanese = month
        .suffix_text
        .chars()
        .chain(day.suffix_text.chars())
        .chain(
            messages
                .iter()
                .flat_map(|message| &message.lines)
                .flat_map(|line| line.chars()),
        )
        .any(is_japanese_character);
    ensure!(
        contains_japanese,
        "ENDING curry-result catalog no longer contains Japanese text"
    );
    Ok(EndingCurryResultCatalog {
        ending_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        contains_japanese,
        consumer: EndingCurryResultConsumerEvidence {
            file_offset: catalog.consumer.file_offset,
            runtime_address: catalog.consumer.runtime_address,
            decoded_instruction_count: catalog.consumer.decoded_instruction_count,
            selector_runtime_address: catalog.consumer.selector_runtime_address,
            message_runtime_addresses: catalog.consumer.message_runtime_addresses.into(),
            month_template_reference_file_offset: catalog
                .consumer
                .month_template_reference_file_offset,
            day_template_reference_file_offset: catalog.consumer.day_template_reference_file_offset,
            glyph_renderer_call_file_offsets: catalog
                .consumer
                .glyph_renderer_call_file_offsets
                .into(),
            glyph_renderer_runtime_address: catalog.consumer.glyph_renderer_runtime_address,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
        month,
        day,
        messages,
    })
}

fn catalog_ending_meal_reaction(bytes: &[u8]) -> Result<EndingMealReactionCatalog> {
    let catalog = ending_meal_reaction::parse_ending_meal_reaction(bytes)?;
    let contains_japanese = catalog.text.chars().any(is_japanese_character);
    ensure!(
        contains_japanese,
        "ENDING meal-reaction catalog no longer contains Japanese text"
    );
    Ok(EndingMealReactionCatalog {
        ending_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        contains_japanese,
        id: "bite-sound-effect".to_owned(),
        text: catalog.text,
        glyph_loads: catalog
            .glyph_loads
            .into_iter()
            .map(|load| EndingMealReactionGlyphLoad {
                slot_number: load.slot_index + 1,
                file_offset: load.file_offset,
                runtime_address: load.runtime_address,
                byte_size: load.bytes.len(),
                sha256: disk::sha256_hex(load.bytes),
                raw_hex: encode_hex(load.bytes),
                glyph: EndingPasswordGlyph {
                    index: load.glyph.index,
                    text: load
                        .glyph
                        .text
                        .expect("ENDING meal-reaction parser requires resolved glyphs")
                        .to_owned(),
                },
            })
            .collect(),
        consumer: EndingMealReactionConsumerEvidence {
            file_offset: catalog.consumer.file_offset,
            runtime_address: catalog.consumer.runtime_address,
            decoded_instruction_count: catalog.consumer.decoded_instruction_count,
            glyph_load_file_offsets: catalog.consumer.glyph_load_file_offsets.into(),
            glyph_renderer_call_file_offsets: catalog
                .consumer
                .glyph_renderer_call_file_offsets
                .into(),
            glyph_renderer_runtime_address: catalog.consumer.glyph_renderer_runtime_address,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
    })
}

fn catalog_ending_meal_captions(bytes: &[u8]) -> Result<EndingMealCaptionCatalog> {
    let catalog = ending_meal_captions::parse_ending_meal_captions(bytes)?;
    let records = catalog
        .records
        .into_iter()
        .map(|record| EndingMealCaptionRecord {
            id: record.id.to_owned(),
            file_offset: record.file_offset,
            runtime_address: record.runtime_address,
            byte_size: record.bytes.len(),
            sha256: disk::sha256_hex(record.bytes),
            raw_hex: encode_hex(record.bytes),
            visual_state_indices: record.visual_state_indices,
            spans: record
                .spans
                .into_iter()
                .enumerate()
                .map(|(span_index, span)| EndingMealCaptionSpan {
                    span_number: span_index + 1,
                    text: span.text,
                    slot_file_offsets: span.slot_file_offsets,
                    glyphs: span
                        .glyphs
                        .into_iter()
                        .map(|glyph| EndingPasswordGlyph {
                            index: glyph.index,
                            text: glyph
                                .text
                                .expect("ENDING meal-caption parser requires resolved glyphs")
                                .to_owned(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let contains_japanese = records
        .iter()
        .flat_map(|record| &record.spans)
        .flat_map(|span| span.text.chars())
        .any(is_japanese_character);
    ensure!(
        contains_japanese,
        "ENDING meal-caption catalog no longer contains Japanese text"
    );
    Ok(EndingMealCaptionCatalog {
        ending_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        contains_japanese,
        records,
        consumer: EndingMealCaptionConsumerEvidence {
            file_offset: catalog.consumer.file_offset,
            runtime_address: catalog.consumer.runtime_address,
            decoded_instruction_count: catalog.consumer.decoded_instruction_count,
            glyph_renderer_call_file_offset: catalog.consumer.glyph_renderer_call_file_offset,
            glyph_renderer_runtime_address: catalog.consumer.glyph_renderer_runtime_address,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
    })
}

fn catalog_ending_passwords(bytes: &[u8]) -> Result<EndingPasswordCatalog> {
    let catalog = indexed_text::parse_ending_passwords(bytes)?;
    let records = catalog
        .records
        .into_iter()
        .map(|record| {
            let glyphs = record
                .glyphs
                .into_iter()
                .map(|glyph| EndingPasswordGlyph {
                    index: glyph.index,
                    text: glyph
                        .text
                        .expect("ENDING password parser requires resolved glyphs")
                        .to_owned(),
                })
                .collect::<Vec<_>>();
            let text = glyphs.iter().map(|glyph| glyph.text.as_str()).collect();
            EndingPasswordRecord {
                id: format!("ending-password-{:02}", record.selection_index + 1),
                selector_value: record.selection_index + 5,
                pointer_entry_file_offset: record.pointer_entry_file_offset,
                runtime_address: record.runtime_address,
                file_offset: record.file_offset,
                byte_size: record.bytes.len(),
                sha256: disk::sha256_hex(record.bytes),
                raw_hex: encode_hex(record.bytes),
                text,
                glyphs,
            }
        })
        .collect::<Vec<_>>();
    let contains_japanese = records
        .iter()
        .flat_map(|record| record.text.chars())
        .any(is_japanese_character);
    ensure!(
        contains_japanese,
        "ENDING password catalog no longer contains Japanese text"
    );

    Ok(EndingPasswordCatalog {
        ending_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        contains_japanese,
        consumer: EndingPasswordConsumerEvidence {
            file_offset: catalog.consumer.file_offset,
            runtime_address: catalog.consumer.runtime_address,
            decoded_instruction_count: catalog.consumer.decoded_instruction_count,
            call_site_file_offset: catalog.consumer.call_site_file_offset,
            font_load_file_offset: catalog.consumer.font_load_file_offset,
            glyph_renderer_call_file_offset: catalog.consumer.glyph_renderer_call_file_offset,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
        pointer_table_file_offset: catalog.pointer_table_file_offset,
        pointer_table_runtime_address: catalog.pointer_table_runtime_address,
        selector_file_offset: catalog.selector_file_offset,
        selector_runtime_address: catalog.selector_runtime_address,
        records,
    })
}

fn catalog_ending_ingredient_labels(bytes: &[u8]) -> Result<EndingIngredientLabelCatalog> {
    let catalog = ending_ingredient_labels::parse_ending_ingredient_labels(bytes)?;
    let records = catalog
        .records
        .into_iter()
        .map(|record| EndingIngredientLabelRecord {
            id: format!(
                "slot-{:02}-variant-{:02}",
                record.group_index + 1,
                record.variant_index + 1
            ),
            group_number: record.group_index + 1,
            variant_number: record.variant_index + 1,
            file_offset: record.file_offset,
            runtime_address: record.runtime_address,
            byte_size: record.bytes.len(),
            sha256: disk::sha256_hex(record.bytes),
            raw_hex: encode_hex(record.bytes),
            text: record.text,
            glyphs: record
                .glyphs
                .into_iter()
                .map(|glyph| EndingPasswordGlyph {
                    index: glyph.index,
                    text: glyph
                        .text
                        .expect("ENDING ingredient parser requires resolved glyphs")
                        .to_owned(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let contains_japanese = records
        .iter()
        .flat_map(|record| record.text.chars())
        .any(is_japanese_character);
    ensure!(
        contains_japanese,
        "ENDING ingredient label catalog no longer contains Japanese text"
    );
    Ok(EndingIngredientLabelCatalog {
        ending_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        contains_japanese,
        consumer: EndingIngredientLabelConsumerEvidence {
            file_offset: catalog.consumer.file_offset,
            runtime_address: catalog.consumer.runtime_address,
            decoded_instruction_count: catalog.consumer.decoded_instruction_count,
            pointer_list_runtime_address: catalog.consumer.pointer_list_runtime_address,
            input_state_offsets: catalog.consumer.input_state_offsets.into(),
            group_runtime_addresses: catalog.consumer.group_runtime_addresses.into(),
            fixed_cell_count: catalog.consumer.fixed_cell_count,
            fixed_cell_loop_file_offset: catalog.consumer.fixed_cell_loop_file_offset,
            glyph_renderer_call_file_offset: catalog.consumer.glyph_renderer_call_file_offset,
            glyph_renderer_runtime_address: catalog.consumer.glyph_renderer_runtime_address,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
        table_file_offset: catalog.table_file_offset,
        table_runtime_address: catalog.table_runtime_address,
        table_byte_size: catalog.table_byte_size,
        records,
    })
}

fn catalog_ending_interlude_caption(bytes: &[u8]) -> Result<EndingInterludeCaption> {
    let caption = indexed_text::parse_ending_interlude_caption(bytes)?;
    let glyphs = caption
        .glyphs
        .into_iter()
        .map(|glyph| EndingPasswordGlyph {
            index: glyph.index,
            text: glyph
                .text
                .expect("ENDING interlude parser requires resolved glyphs")
                .to_owned(),
        })
        .collect::<Vec<_>>();
    let text: String = glyphs.iter().map(|glyph| glyph.text.as_str()).collect();
    let contains_japanese = text.chars().any(is_japanese_character);
    ensure!(
        contains_japanese,
        "ENDING interlude caption no longer contains Japanese text"
    );

    Ok(EndingInterludeCaption {
        ending_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        contains_japanese,
        consumer: EndingInterludeConsumerEvidence {
            file_offset: caption.consumer.file_offset,
            runtime_address: caption.consumer.runtime_address,
            decoded_instruction_count: caption.consumer.decoded_instruction_count,
            font_load_file_offset: caption.consumer.font_load_file_offset,
            background_entry_id: caption.consumer.background_entry_id,
            background_load_file_offset: caption.consumer.background_load_file_offset,
            glyph_renderer_call_file_offset: caption.consumer.glyph_renderer_call_file_offset,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
        file_offset: caption.file_offset,
        runtime_address: caption.runtime_address,
        byte_size: caption.bytes.len(),
        sha256: disk::sha256_hex(caption.bytes),
        raw_hex: encode_hex(caption.bytes),
        text,
        glyphs,
    })
}

fn catalog_opening_text(maddat: &[u8], opening_com: &[u8]) -> Result<OpeningTextCatalog> {
    let archive = flink::parse_flink_archive(maddat)?;
    let entry = archive
        .entries
        .get(opening_script::MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing the OPENING text-script entry")?;
    ensure!(
        entry.id == opening_script::MADDAT_ENTRY_ID,
        "MADDAT OPENING text-script entry ID does not match its table position"
    );
    let script = opening_script::parse_opening_text_script(entry.bytes)?;
    ensure!(
        opening_script::reconstruct_opening_text_script(&script.tokens)? == entry.bytes,
        "OPENING text script does not reconstruct exactly in the source catalog"
    );
    let consumer = opening_script::verify_opening_text_consumer(opening_com)?;
    let mut text = String::new();
    let tokens = script
        .tokens
        .iter()
        .map(|token| match token {
            opening_script::OpeningScriptToken::Mode(value) => {
                OpeningTextToken::Mode { value: *value }
            }
            opening_script::OpeningScriptToken::Sprite {
                glyph_index,
                position,
            } => OpeningTextToken::Sprite {
                glyph_index: *glyph_index,
                position: *position,
            },
            opening_script::OpeningScriptToken::Glyph { index, text: glyph } => {
                text.push_str(glyph);
                OpeningTextToken::Glyph {
                    index: *index,
                    text: (*glyph).to_owned(),
                }
            }
            opening_script::OpeningScriptToken::LineBreak => {
                text.push('\n');
                OpeningTextToken::LineBreak
            }
            opening_script::OpeningScriptToken::Scroll => OpeningTextToken::Scroll,
            opening_script::OpeningScriptToken::Yield => OpeningTextToken::Yield,
            opening_script::OpeningScriptToken::End => OpeningTextToken::End,
        })
        .collect();

    Ok(OpeningTextCatalog {
        maddat_entry_id: entry.id,
        byte_size: entry.bytes.len(),
        sha256: disk::sha256_hex(entry.bytes),
        raw_hex: encode_hex(entry.bytes),
        text,
        consumer: OpeningTextConsumerEvidence {
            opening_com_sha256: disk::sha256_hex(opening_com),
            file_size: opening_com.len(),
            load_reference_file_offset: consumer.load_reference_file_offset,
            call_site_file_offset: consumer.call_site_file_offset,
            interpreter_file_offset: consumer.interpreter_file_offset,
            interpreter_runtime_address: consumer.interpreter_runtime_address,
            decoded_instruction_count: consumer.decoded_instruction_count,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
        tokens,
    })
}

fn catalog_indexed_dialogue(
    maddat: &[u8],
    select_com: &[u8],
) -> Result<(IndexedFontCatalog, SelectDialogueCatalog)> {
    let archive = flink::parse_flink_archive(maddat)?;
    let font_entry = archive
        .entries
        .get(indexed_text::FONT_ENTRY_ID - 1)
        .context("MADDAT is missing the indexed font entry")?;
    ensure!(
        font_entry.id == indexed_text::FONT_ENTRY_ID,
        "MADDAT indexed font entry ID does not match its table position"
    );
    let decoded_font = compile_lz::decode_complete_compile_lz(font_entry.bytes)?.output;
    let font = indexed_text::verify_indexed_font(&decoded_font)?;
    let dialogue = indexed_text::parse_select_dialogue(select_com)?;

    let stages = dialogue
        .stages
        .into_iter()
        .map(|stage| {
            let stage_number = stage.stage_index + 1;
            let scripts = stage
                .scripts
                .into_iter()
                .map(|script| {
                    let phase_number = script.phase_index + 1;
                    let mut text = String::new();
                    let tokens = script
                        .tokens
                        .iter()
                        .map(|token| match token {
                            indexed_text::IndexedTextToken::Glyph { index, text: glyph } => {
                                text.push_str(glyph);
                                IndexedTextToken::Glyph {
                                    index: *index,
                                    text: (*glyph).to_owned(),
                                }
                            }
                            indexed_text::IndexedTextToken::Voice(parameter) => {
                                IndexedTextToken::Voice {
                                    parameter: *parameter,
                                }
                            }
                            indexed_text::IndexedTextToken::Portrait(parameters) => {
                                IndexedTextToken::Portrait {
                                    parameters: *parameters,
                                }
                            }
                            indexed_text::IndexedTextToken::LineBreak(parameters) => {
                                text.push('\n');
                                IndexedTextToken::LineBreak {
                                    parameters: *parameters,
                                }
                            }
                            indexed_text::IndexedTextToken::Terminator => {
                                IndexedTextToken::Terminator
                            }
                        })
                        .collect();
                    SelectDialogueScript {
                        id: format!("stage-{stage_number:02}-phase-{phase_number:02}"),
                        phase_number,
                        pointer_entry_file_offset: script.pointer_entry_file_offset,
                        runtime_address: script.runtime_address,
                        file_offset: script.file_offset,
                        byte_size: script.bytes.len(),
                        sha256: disk::sha256_hex(script.bytes),
                        raw_hex: encode_hex(script.bytes),
                        initial_portrait: script.initial_portrait,
                        text,
                        tokens,
                    }
                })
                .collect();
            SelectDialogueStage {
                stage_number,
                scripts,
            }
        })
        .collect();

    Ok((
        IndexedFontCatalog {
            maddat_entry_id: font.entry_id,
            decoded_size: font.decoded_size,
            decoded_sha256: disk::sha256_hex(&decoded_font),
            glyph_count: font.glyph_count,
            glyph_size: font.glyph_size,
        },
        SelectDialogueCatalog {
            select_com_sha256: disk::sha256_hex(select_com),
            file_size: select_com.len(),
            interpreter: SelectDialogueInterpreterEvidence {
                file_offset: dialogue.interpreter.file_offset,
                runtime_address: dialogue.interpreter.runtime_address,
                decoded_instruction_count: dialogue.interpreter.decoded_instruction_count,
                call_site_file_offsets: dialogue.interpreter.call_site_file_offsets.to_vec(),
                isa_profile_id: v30::PROFILE_ID.to_owned(),
            },
            pointer_table_file_offset: dialogue.pointer_table_file_offset,
            pointer_table_runtime_address: dialogue.pointer_table_runtime_address,
            stages,
        },
    ))
}

fn catalog_mst_text(bytes: &[u8]) -> Result<MstTextCatalog> {
    enum Reference {
        Immediate(usize),
        TableEntry(usize),
    }
    struct RecordSpec {
        id: &'static str,
        consumer: &'static str,
        runtime_address: usize,
        references: &'static [Reference],
        numeric_template: Option<NumericTemplateSpec>,
    }
    struct NumericTemplateSpec {
        interior_offset: usize,
        byte_len: usize,
        base_reference_file_offsets: &'static [usize],
    }

    const RECORDS: &[RecordSpec] = &[
        RecordSpec {
            id: "file-not-found",
            consumer: "returned execution error message",
            runtime_address: 0x06da,
            references: &[Reference::Immediate(0x022a)],
            numeric_template: None,
        },
        RecordSpec {
            id: "disk-write-protected",
            consumer: "DOS error selector table then AH=09h",
            runtime_address: 0x06f6,
            references: &[Reference::TableEntry(0x0270)],
            numeric_template: None,
        },
        RecordSpec {
            id: "drive-not-ready",
            consumer: "DOS error selector table then AH=09h",
            runtime_address: 0x071f,
            references: &[Reference::TableEntry(0x0274)],
            numeric_template: None,
        },
        RecordSpec {
            id: "disk-error",
            consumer: "DOS error selector table then AH=09h",
            runtime_address: 0x0743,
            references: &[Reference::TableEntry(0x0272), Reference::TableEntry(0x0276)],
            numeric_template: None,
        },
        RecordSpec {
            id: "song-data-not-found",
            consumer: "DOS AH=09h",
            runtime_address: 0x0753,
            references: &[Reference::Immediate(0x0039)],
            numeric_template: None,
        },
        RecordSpec {
            id: "line-break",
            consumer: "DOS AH=09h",
            runtime_address: 0x0773,
            references: &[Reference::Immediate(0x0040), Reference::Immediate(0x0244)],
            numeric_template: None,
        },
        RecordSpec {
            id: "memory-allocation-failure",
            consumer: "DOS AH=09h after numeric fields are updated",
            runtime_address: 0x0776,
            references: &[Reference::Immediate(0x02ef)],
            // `000Kb(000000` at COM 0x07af. Three `MOV BX, 0x07af` sites load
            // the run's base; the two that follow it add 6 and 9 to reach the
            // six-digit half, so the digit layout has to survive translation.
            numeric_template: Some(NumericTemplateSpec {
                interior_offset: 0x39,
                byte_len: 12,
                base_reference_file_offsets: &[0x049b, 0x04b0, 0x04bc],
            }),
        },
        RecordSpec {
            id: "program-end",
            consumer: "DOS AH=09h",
            runtime_address: 0x085a,
            references: &[Reference::Immediate(0x01f5)],
            numeric_template: None,
        },
    ];

    const COM_ORIGIN: usize = 0x100;
    let mut records = Vec::with_capacity(RECORDS.len());
    for spec in RECORDS {
        let pointer = u16::try_from(spec.runtime_address)
            .context("MST text runtime address does not fit in a 16-bit COM pointer")?
            .to_le_bytes();
        let mut reference_file_offsets = Vec::with_capacity(spec.references.len());
        for reference in spec.references {
            let file_offset = match reference {
                Reference::Immediate(file_offset) => {
                    ensure!(
                        bytes.get(*file_offset..*file_offset + 3)
                            == Some(&[0xba, pointer[0], pointer[1]]),
                        "MST immediate text reference does not match at {file_offset:#x}"
                    );
                    *file_offset
                }
                Reference::TableEntry(file_offset) => {
                    ensure!(
                        bytes.get(*file_offset..*file_offset + 2) == Some(&pointer),
                        "MST text pointer table does not match at {file_offset:#x}"
                    );
                    *file_offset
                }
            };
            reference_file_offsets.push(file_offset);
        }

        let file_offset = spec
            .runtime_address
            .checked_sub(COM_ORIGIN)
            .context("MST text pointer lies below the COM origin")?;
        let parsed =
            dos_text::parse_dos_text(bytes.get(file_offset..).with_context(|| {
                format!("MST text starts outside the file at {file_offset:#x}")
            })?)
            .with_context(|| format!("parse MST text record {}", spec.id))?;
        let record_bytes = &bytes[file_offset..file_offset + parsed.byte_len];
        ensure!(
            dos_text::reconstruct_dos_text(&parsed.tokens)? == record_bytes,
            "MST text record {} does not reconstruct exactly",
            spec.id
        );

        let (tokens, text_segments) = catalog_dos_display_tokens(&parsed.tokens)?;
        let contains_japanese = text_segments
            .iter()
            .flat_map(|segment| segment.chars())
            .any(is_japanese_character);

        let numeric_template = match &spec.numeric_template {
            None => None,
            Some(template) => {
                let end = template
                    .interior_offset
                    .checked_add(template.byte_len)
                    .context("MST numeric template overflows its record")?;
                ensure!(
                    end <= record_bytes.len(),
                    "MST record {} numeric template runs past the record",
                    spec.id
                );
                let run = &record_bytes[template.interior_offset..end];
                let digit_mask = run
                    .iter()
                    .map(|byte| byte.is_ascii_digit())
                    .collect::<Vec<_>>();
                ensure!(
                    digit_mask.iter().any(|is_digit| *is_digit)
                        && digit_mask.iter().any(|is_digit| !*is_digit),
                    "MST record {} numeric template has no digit and non-digit split",
                    spec.id
                );
                ensure!(
                    run.iter()
                        .zip(&digit_mask)
                        .all(|(byte, is_digit)| !*is_digit || *byte == b'0'),
                    "MST record {} numeric template digits are not zero placeholders",
                    spec.id
                );
                let base_address = u16::try_from(
                    spec.runtime_address
                        .checked_add(template.interior_offset)
                        .context("MST numeric template address overflow")?,
                )
                .context("MST numeric template address exceeds a COM pointer")?
                .to_le_bytes();
                for base_offset in template.base_reference_file_offsets {
                    ensure!(
                        bytes.get(*base_offset..*base_offset + 3)
                            == Some(&[0xbb, base_address[0], base_address[1]]),
                        "MST numeric template base load does not match at {base_offset:#x}"
                    );
                }
                Some(MstNumericTemplate {
                    interior_offset: template.interior_offset,
                    byte_len: template.byte_len,
                    digit_mask,
                    base_reference_file_offsets: template.base_reference_file_offsets.to_vec(),
                })
            }
        };

        records.push(MstTextRecord {
            id: spec.id.to_owned(),
            consumer: spec.consumer.to_owned(),
            reference_file_offsets,
            numeric_template,
            runtime_address: spec.runtime_address,
            file_offset,
            byte_size: record_bytes.len(),
            sha256: disk::sha256_hex(record_bytes),
            raw_hex: encode_hex(record_bytes),
            contains_japanese,
            text_segments,
            tokens,
        });
    }

    Ok(MstTextCatalog {
        mst_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        records,
    })
}

fn catalog_dos_program_text<'a, F>(load: F) -> Result<DosProgramTextCatalog>
where
    F: FnMut(&str) -> Result<&'a [u8]>,
{
    let catalog = dos_program_text::catalog_program_text(load)?;
    let mut record_count = 0;
    let mut japanese_record_count = 0;
    let mut bound_japanese_record_count = 0;
    let mut unbound_japanese_record_ids = Vec::new();
    let programs = catalog
        .programs
        .into_iter()
        .map(|program| {
            let filename = program.filename.to_owned();
            let program_bytes = program.bytes;
            let records = program
                .records
                .into_iter()
                .map(|record| {
                    let (tokens, text_segments) = catalog_dos_display_tokens(&record.tokens)?;
                    let contains_japanese = text_segments
                        .iter()
                        .flat_map(|segment| segment.chars())
                        .any(is_japanese_character);
                    let bound_to_consumer = !record.references.is_empty();
                    record_count += 1;
                    if contains_japanese {
                        japanese_record_count += 1;
                        if bound_to_consumer {
                            bound_japanese_record_count += 1;
                        } else {
                            unbound_japanese_record_ids
                                .push(format!("{}:{}", program.filename, record.id));
                        }
                    }
                    Ok(DosProgramTextRecord {
                        id: record.id.to_owned(),
                        runtime_address: record.runtime_address,
                        file_offset: record.file_offset,
                        byte_size: record.bytes.len(),
                        sha256: disk::sha256_hex(record.bytes),
                        raw_hex: encode_hex(record.bytes),
                        bound_to_consumer,
                        contains_japanese,
                        text_segments,
                        tokens,
                        references: record
                            .references
                            .into_iter()
                            .map(|reference| DosProgramTextReference {
                                kind: match reference.kind {
                                    dos_program_text::TextReferenceKind::Immediate => {
                                        DosProgramTextReferenceKind::Immediate
                                    }
                                    dos_program_text::TextReferenceKind::PointerTable => {
                                        DosProgramTextReferenceKind::PointerTable
                                    }
                                },
                                file_offset: reference.file_offset,
                                output_file_offset: reference.output_file_offset,
                            })
                            .collect(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(DosProgramCatalog {
                filename,
                sha256: disk::sha256_hex(program_bytes),
                file_size: program_bytes.len(),
                display_sites: program
                    .display_sites
                    .into_iter()
                    .map(|site| DosProgramDisplayEvidence {
                        kind: match site.kind {
                            dos_program_text::DisplaySiteKind::Ah09Subroutine => {
                                DosProgramDisplayKind::Ah09Subroutine
                            }
                            dos_program_text::DisplaySiteKind::DirectAh09 => {
                                DosProgramDisplayKind::DirectAh09
                            }
                        },
                        file_offset: site.file_offset,
                        runtime_address: site.runtime_address,
                        byte_len: site.byte_len,
                        decoded_instruction_count: site.decoded_instruction_count,
                        isa_profile_id: v30::PROFILE_ID.to_owned(),
                    })
                    .collect(),
                records,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(DosProgramTextCatalog {
        programs,
        record_count,
        japanese_record_count,
        bound_japanese_record_count,
        unbound_japanese_record_ids,
    })
}

fn catalog_dos_display_tokens(
    parsed: &[dos_text::DosTextToken],
) -> Result<(Vec<DosDisplayToken>, Vec<String>)> {
    let mut text_segments = vec![String::new()];
    let tokens = parsed
        .iter()
        .map(|token| match token {
            dos_text::DosTextToken::AnsiControl(control) => {
                if text_segments
                    .last()
                    .is_some_and(|segment| !segment.is_empty())
                {
                    text_segments.push(String::new());
                }
                Ok(DosDisplayToken::AnsiControl {
                    raw_hex: encode_hex(control),
                })
            }
            dos_text::DosTextToken::CarriageReturn => {
                text_segments
                    .last_mut()
                    .expect("one segment exists")
                    .push('\r');
                Ok(DosDisplayToken::CarriageReturn)
            }
            dos_text::DosTextToken::LineFeed => {
                text_segments
                    .last_mut()
                    .expect("one segment exists")
                    .push('\n');
                Ok(DosDisplayToken::LineFeed)
            }
            dos_text::DosTextToken::SingleByte(byte) => {
                let text = decode_shift_jis(&[*byte])?;
                text_segments
                    .last_mut()
                    .expect("one segment exists")
                    .push_str(&text);
                Ok(DosDisplayToken::SingleByte { byte: *byte, text })
            }
            dos_text::DosTextToken::DoubleByte(pair) => {
                let text = decode_shift_jis(pair)?;
                text_segments
                    .last_mut()
                    .expect("one segment exists")
                    .push_str(&text);
                Ok(DosDisplayToken::ShiftJis {
                    shift_jis_code: u16::from_be_bytes(*pair),
                    text,
                })
            }
            dos_text::DosTextToken::Terminator => Ok(DosDisplayToken::Terminator),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((tokens, text_segments))
}

fn is_japanese_character(character: char) -> bool {
    matches!(character,
        '\u{3040}'..='\u{30ff}'
            | '\u{31f0}'..='\u{31ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{ff66}'..='\u{ff9f}'
    )
}

fn catalog_gaiji(bytes: &[u8]) -> Result<GaijiCatalog> {
    let program = gaiji::parse_gaiji_program(bytes)?;
    let glyphs: Vec<_> = program
        .glyphs
        .iter()
        .map(|glyph| GaijiCatalogEntry {
            index: glyph.index,
            character_code: glyph.character_code,
            shift_jis_code: glyph.shift_jis_code,
            offset: glyph.offset,
            size: glyph.bytes.len(),
            sha256: disk::sha256_hex(glyph.bytes),
        })
        .collect();
    let uniform_glyph_size = glyphs.first().and_then(|first| {
        glyphs
            .iter()
            .all(|glyph| glyph.size == first.size)
            .then_some(first.size)
    });

    Ok(GaijiCatalog {
        sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        pointer_table_offset: program.pointer_table_offset,
        first_character_code: glyphs.first().map(|glyph| glyph.character_code),
        last_character_code: glyphs.last().map(|glyph| glyph.character_code),
        first_shift_jis_code: glyphs.first().map(|glyph| glyph.shift_jis_code),
        last_shift_jis_code: glyphs.last().map(|glyph| glyph.shift_jis_code),
        uniform_glyph_size,
        reviewed_content_kind: GaijiContentKind::PixelArtFragments,
        contains_japanese_text: false,
        glyphs,
    })
}

fn catalog_mad_text(bytes: &[u8], gaiji: &GaijiCatalog) -> Result<MadTextCatalog> {
    let catalog = mad_text::parse_mad_text_catalog(bytes)?;
    let gaiji_by_shift_jis: BTreeMap<_, _> = gaiji
        .glyphs
        .iter()
        .map(|glyph| (glyph.shift_jis_code, glyph))
        .collect();
    let fixed_slots = catalog
        .fixed_slots
        .into_iter()
        .map(|slot| {
            let tokens = slot
                .stream
                .tokens
                .iter()
                .map(|token| catalog_text_token(token, &gaiji_by_shift_jis))
                .collect::<Result<Vec<_>>>()?;
            Ok(MadFixedTextSlot {
                id: format!("fixed-text-{:02}", slot.index + 1),
                pointer_entry_offsets: slot.pointer_entry_offsets,
                file_offset: slot.file_offset,
                byte_size: slot.bytes.len(),
                sha256: disk::sha256_hex(slot.bytes),
                raw_hex: encode_hex(slot.bytes),
                content_kind: MadFixedSlotContentKind::PixelArtComposite,
                contains_japanese_text: false,
                row_count: 6,
                cells_per_row: 6,
                tokens,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(MadTextCatalog {
        mad_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        renderer: MadTextRendererEvidence {
            file_offset: catalog.renderer.file_offset,
            runtime_address: catalog.renderer.runtime_address,
            fixed_text_call_site_file_offset: catalog.renderer.call_site_file_offset,
            decoded_instruction_count: catalog.renderer.decoded_instruction_count,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
        fixed_slots,
    })
}

fn catalog_text_token(
    token: &mad_text::RendererTextToken,
    gaiji_by_shift_jis: &BTreeMap<u16, &GaijiCatalogEntry>,
) -> Result<MadTextToken> {
    Ok(match token {
        mad_text::RendererTextToken::DisplayControl(code) => MadTextToken::DisplayControl {
            code: char::from(*code).to_string(),
            palette_index: code - b'0',
        },
        mad_text::RendererTextToken::LineBreak => MadTextToken::LineBreak,
        mad_text::RendererTextToken::SingleByte(byte) => MadTextToken::SingleByte {
            byte: *byte,
            text: decode_shift_jis(&[*byte])?,
        },
        mad_text::RendererTextToken::DoubleByte([0x81, 0x40]) => MadTextToken::Space,
        mad_text::RendererTextToken::DoubleByte(pair) => {
            let shift_jis_code = u16::from_be_bytes(*pair);
            if let Some(glyph) = gaiji_by_shift_jis.get(&shift_jis_code) {
                MadTextToken::Gaiji {
                    glyph_index: glyph.index,
                    character_code: glyph.character_code,
                    shift_jis_code,
                }
            } else {
                MadTextToken::ShiftJis {
                    shift_jis_code,
                    text: decode_shift_jis(pair)?,
                }
            }
        }
        mad_text::RendererTextToken::Terminator => MadTextToken::Terminator,
    })
}

fn decode_shift_jis(bytes: &[u8]) -> Result<String> {
    let (decoded, _, had_errors) = SHIFT_JIS.decode(bytes);
    ensure!(
        !had_errors,
        "renderer text contains invalid Shift-JIS bytes"
    );
    Ok(decoded.into_owned())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[cfg(test)]
#[path = "survey_tests.rs"]
mod survey_tests;
