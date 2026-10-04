use anyhow::{Context, Result, ensure};
use serde::Serialize;
use v30::{Instruction, decode_bytes};

use crate::game_data::{
    flink, game_over_graphic, gcs, stage_completion_graphic, stage_select_graphic,
    title_menu_glyphs,
};
use crate::source::disk;

use super::state_graphic_catalog::StateGraphicCatalog;

const COM_ORIGIN: usize = 0x100;

const STAGE_SELECT_PACKED_SHA256: &str =
    "e4df4f46979599271e0374c3a079aec87ad462428b4b80f4653fe86a4edbb90b";
const STAGE_SELECT_DECODED_SHA256: &str =
    "9e397b0947c18da816c2324c6503fd85c34b50ef1c8875d8847ff5b31f620c52";
const STAGE_SELECT_LOAD_FILE_OFFSET: usize = 0x1c30;
const STAGE_SELECT_LOAD_BLOCK_LEN: usize = 0x3a;
const STAGE_SELECT_FILE_READ_CALL_FILE_OFFSET: usize = 0x1c67;
const STAGE_SELECT_FILE_READ_RUNTIME_ADDRESS: usize = 0x03ae;
const STAGE_SELECT_RENDER_CALL_FILE_OFFSETS: [usize; 2] = [0x1c80, 0x1c8b];
const STAGE_SELECT_RENDER_RUNTIME_ADDRESS: usize = 0x1689;

const TITLE_ENTRY_ID: usize = 140;
const TITLE_PACKED_SHA256: &str =
    "a41ff29626aed261abe6af07e36831282312561b894f229c23ba57bfd9fe742b";
const TITLE_LOAD_FILE_OFFSET: usize = 0x1ef0;
const TITLE_LOAD_BLOCK_LEN: usize = 0x0d;
const TITLE_RENDER_CALL_FILE_OFFSETS: [usize; 2] = [0x1f41, 0x1f51];
const TITLE_RENDER_RUNTIME_ADDRESS: usize = 0x1689;

pub(crate) const TITLE_MENU_PACKED_SHA256: &str =
    "12a0de87b545e9c9d762556acfb754e0ffdcea4a5fe3ffbf9e4833574c51e7e5";
const TITLE_MENU_DECODED_SHA256: &str =
    "8f8d40e1040ce38bc25da66309470b8ee4ae724f79df839da4bb71198bcb2312";
const TITLE_MENU_LOAD_FILE_OFFSET: usize = 0x1e80;
const TITLE_MENU_LOAD_BLOCK_LEN: usize = 0x53;
const TITLE_MENU_FILE_READ_CALL_FILE_OFFSET: usize = 0x1eb8;
const TITLE_MENU_FILE_READ_RUNTIME_ADDRESS: usize = 0x03ae;
const TITLE_MENU_DECOMPRESS_CALL_FILE_OFFSET: usize = 0x1ed0;
const TITLE_MENU_DECOMPRESS_RUNTIME_ADDRESS: usize = 0x158c;
const TITLE_MENU_DECODED_COPY_WORD_COUNT_FILE_OFFSET: usize = 0x1edf;
const TITLE_MENU_DECODED_COPY_WORD_COUNT: usize = 0x870;
const TITLE_MENU_DECODED_COPY_CALL_FILE_OFFSET: usize = 0x1ee2;
const TITLE_MENU_DECODED_COPY_RUNTIME_ADDRESS: usize = 0x34ad;
const TITLE_MENU_PRECOMPOSE_CALL_FILE_OFFSET: usize = 0x1fb7;
const TITLE_MENU_PRECOMPOSE_RUNTIME_ADDRESS: usize = 0x2683;
const TITLE_MENU_RENDER_CALLS: [(usize, usize); 4] = [
    (0x1fdc, 0x2377),
    (0x1fdf, 0x239c),
    (0x1fec, 0x2377),
    (0x1fef, 0x239c),
];
const TITLE_MENU_SEQUENCE_FILE_OFFSETS: [usize; 2] = [0x36be, 0x36c9];
const TITLE_MENU_START_SEQUENCE: [u8; 11] = [0, 0, 1, 0, 2, 0, 3, 0, 1, 0, 0xff];
const TITLE_MENU_CONTINUE_SEQUENCE: [u8; 17] =
    [4, 0, 5, 0, 6, 0, 1, 0, 7, 0, 6, 0, 8, 0, 9, 0, 0xff];

const GAME_OVER_PACKED_SHA256: &str =
    "619110eef3f23da87bd53a3996707a56f29ee5b1a56cc3c2276840caffedb974";
const GAME_OVER_LOAD_BLOCK_FILE_OFFSET: usize = 0x5f92;
const GAME_OVER_LOAD_BLOCK_LEN: usize = 0x12;
const GAME_OVER_LOADER_CALL_FILE_OFFSET: usize = 0x5fa1;
const GAME_OVER_LOADER_RUNTIME_ADDRESS: usize = 0xb400;
const GAME_OVER_RENDERER_CALL_FILE_OFFSET: usize = 0x62b8;
const GAME_OVER_RENDERER_FILE_OFFSET: usize = 0x62c9;
const GAME_OVER_RENDERER_BODY_LEN: usize = 0x2a;

const STAGE_COMPLETION_PACKED_SHA256: &str =
    "03bc9c55ff2522ef8aacdce07aa31ade634eed0f53973479a4d969b0b3fa4751";
const STAGE_COMPLETION_DECODED_SHA256: &str =
    "f4ec196d1b52526402b9bf092f88d8489113cc536baa7d6c8c0015c457b51c8e";
const STAGE_COMPLETION_LOAD_BLOCK_FILE_OFFSET: usize = 0x359b;
const STAGE_COMPLETION_LOAD_BLOCK_LEN: usize = 0x3b;
const STAGE_COMPLETION_FILE_READ_CALL_FILE_OFFSET: usize = 0x35d3;
const STAGE_COMPLETION_FILE_READ_RUNTIME_ADDRESS: usize = 0x03ae;
const STAGE_COMPLETION_DECOMPRESS_CALL_FILE_OFFSET: usize = 0x35f1;
const STAGE_COMPLETION_DECOMPRESS_RUNTIME_ADDRESS: usize = 0x158c;

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct GraphicTextCatalog {
    pub surfaces: Vec<GraphicTextSurface>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct GraphicTextSurface {
    pub id: String,
    pub maddat_entry_id: usize,
    pub packed_size: usize,
    pub packed_sha256: String,
    pub decoded_size: usize,
    pub decoded_sha256: String,
    pub width: usize,
    pub height: usize,
    pub plane_count: usize,
    pub contains_japanese: bool,
    pub requires_localization: bool,
    pub reviewed_text: Vec<String>,
    pub consumer: GraphicTextConsumerEvidence,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GraphicTextConsumerEvidence {
    GcsScreen {
        program_filename: String,
        program_sha256: String,
        load_reference_file_offset: usize,
        load_reference_runtime_address: usize,
        load_decoded_instruction_count: usize,
        renderer_call_file_offsets: Vec<usize>,
        renderer_runtime_address: usize,
        isa_profile_id: String,
    },
    PlanarBlock {
        program_filename: String,
        program_sha256: String,
        load_block_file_offset: usize,
        load_block_runtime_address: usize,
        load_block_decoded_instruction_count: usize,
        loader_call_file_offset: usize,
        loader_runtime_address: usize,
        renderer_call_file_offset: usize,
        renderer_file_offset: usize,
        renderer_runtime_address: usize,
        renderer_decoded_instruction_count: usize,
        bytes_per_row: usize,
        words_per_row: usize,
        isa_profile_id: String,
    },
    TiledPlanarRegion {
        program_filename: String,
        program_sha256: String,
        load_block_file_offset: usize,
        load_block_runtime_address: usize,
        load_block_decoded_instruction_count: usize,
        file_read_call_file_offset: usize,
        file_read_runtime_address: usize,
        decompressor_call_file_offset: usize,
        decompressor_runtime_address: usize,
        tile_ids: Vec<usize>,
        tile_columns: usize,
        tile_rows: usize,
        tile_width: usize,
        tile_height: usize,
        isa_profile_id: String,
    },
    MaskedSpriteLabels {
        program_filename: String,
        program_sha256: String,
        load_block_file_offset: usize,
        load_block_runtime_address: usize,
        load_block_decoded_instruction_count: usize,
        file_read_call_file_offset: usize,
        file_read_runtime_address: usize,
        decompressor_call_file_offset: usize,
        decompressor_runtime_address: usize,
        decoded_copy_word_count_file_offset: usize,
        decoded_copy_word_count: usize,
        decoded_copy_byte_count: usize,
        decoded_copy_call_file_offset: usize,
        decoded_copy_runtime_address: usize,
        precompose_call_file_offset: usize,
        precompose_runtime_address: usize,
        renderer_call_file_offsets: Vec<usize>,
        renderer_runtime_addresses: Vec<usize>,
        sequence_file_offsets: Vec<usize>,
        source_glyph_sequences: Vec<Vec<u8>>,
        label_slot_counts: Vec<usize>,
        stored_glyph_count: usize,
        selectable_glyph_count: usize,
        color_plane_count: usize,
        mask_plane_count: usize,
        isa_profile_id: String,
    },
    FieldUnitCallout {
        program_filename: String,
        program_sha256: String,
        runtime_unit_class_id: usize,
        map_file_offset: usize,
        map_runtime_address: usize,
        tile_ids: Vec<u8>,
        bank_load_block_file_offset: usize,
        bank_load_block_runtime_address: usize,
        bank_loader_call_file_offset: usize,
        bank_loader_runtime_address: usize,
        bank_selection_file_offsets: Vec<usize>,
        renderer_file_offset: usize,
        renderer_runtime_address: usize,
        pointer_table_file_offset: usize,
        pointer_table_runtime_address: usize,
        call_site_file_offsets: Vec<usize>,
        isa_profile_id: String,
    },
}

pub(crate) fn catalog_graphic_text(
    maddat: &[u8],
    opening_com: &[u8],
    select_com: &[u8],
    mad_com: &[u8],
    state_graphics: &StateGraphicCatalog,
) -> Result<GraphicTextCatalog> {
    let archive = flink::parse_flink_archive(maddat)?;
    let entry = |entry_id: usize| -> Result<_> {
        let entry = archive
            .entries
            .get(entry_id - 1)
            .with_context(|| format!("MADDAT is missing graphic-text entry {entry_id}"))?;
        ensure!(
            entry.id == entry_id,
            "MADDAT graphic-text entry {entry_id} does not match its table position"
        );
        Ok(entry)
    };

    let stage_select_entry = entry(stage_select_graphic::MADDAT_ENTRY_ID)?;
    verify_packed_asset(
        stage_select_entry.bytes,
        STAGE_SELECT_PACKED_SHA256,
        "stage-select graphic",
    )?;
    let stage_select = gcs::decode_gcs(stage_select_entry.bytes)?;
    let stage_select_decoded: Vec<_> = stage_select.planes.iter().flatten().copied().collect();
    ensure!(
        disk::sha256_hex(&stage_select_decoded) == STAGE_SELECT_DECODED_SHA256,
        "stage-select decoded GCS differs from the reviewed source"
    );
    let stage_select_consumer = verify_stage_select_consumer(select_com)?;

    let title_entry = entry(TITLE_ENTRY_ID)?;
    verify_packed_asset(title_entry.bytes, TITLE_PACKED_SHA256, "title graphic")?;
    let title = gcs::decode_gcs(title_entry.bytes)?;
    let title_decoded: Vec<_> = title.planes.iter().flatten().copied().collect();
    let title_consumer = verify_title_consumer(opening_com)?;

    let title_menu_entry = entry(title_menu_glyphs::MADDAT_ENTRY_ID)?;
    verify_packed_asset(
        title_menu_entry.bytes,
        TITLE_MENU_PACKED_SHA256,
        "title-menu glyph bank",
    )?;
    let title_menu = title_menu_glyphs::decode_title_menu_glyph_bank(title_menu_entry.bytes)?;
    ensure!(
        disk::sha256_hex(&title_menu.decoded) == TITLE_MENU_DECODED_SHA256,
        "title-menu decoded glyph bank differs from the reviewed source"
    );
    let title_menu_consumer = verify_title_menu_consumer(opening_com)?;

    let game_over_entry = entry(game_over_graphic::MADDAT_ENTRY_ID)?;
    verify_packed_asset(
        game_over_entry.bytes,
        GAME_OVER_PACKED_SHA256,
        "game-over graphic",
    )?;
    let game_over = game_over_graphic::decode_game_over_graphic(game_over_entry.bytes)?;
    let game_over_consumer = verify_game_over_consumer(mad_com)?;

    let stage_completion_entry = entry(stage_completion_graphic::MADDAT_ENTRY_ID)?;
    verify_packed_asset(
        stage_completion_entry.bytes,
        STAGE_COMPLETION_PACKED_SHA256,
        "stage-completion graphic",
    )?;
    let stage_completion =
        stage_completion_graphic::decode_stage_completion_graphic(stage_completion_entry.bytes)?;
    ensure!(
        disk::sha256_hex(&stage_completion.decoded_tile_bank) == STAGE_COMPLETION_DECODED_SHA256,
        "stage-completion decoded tile bank differs from the reviewed source"
    );
    let stage_completion_consumer = verify_stage_completion_consumer(select_com)?;

    let callout_consumer = &state_graphics.field_unit_callouts.consumer;
    let field_unit_callout_surfaces = state_graphics
        .field_unit_callouts
        .callouts
        .iter()
        .filter(|callout| callout.source_text.is_some())
        .map(|callout| {
            let expected_maddat_entry_id = if callout.runtime_unit_class_id <= 10 {
                8
            } else {
                callout.runtime_unit_class_id + 100
            };
            ensure!(
                callout.id == format!("field-unit-callout-{:02}", callout.runtime_unit_class_id)
                    && callout.maddat_entry_id == expected_maddat_entry_id
                    && callout.tile_ids.len() == 3
                    && callout.source_text.is_some(),
                "field-unit callout {} differs from the adopted source identity",
                callout.runtime_unit_class_id
            );
            let bank = state_graphics
                .field_unit_callouts
                .banks
                .iter()
                .find(|bank| bank.maddat_entry_id == callout.maddat_entry_id)
                .with_context(|| {
                    format!(
                        "state-graphic catalog is missing MADDAT {} callout bank",
                        callout.maddat_entry_id
                    )
                })?;
            Ok(GraphicTextSurface {
                id: callout.id.clone(),
                maddat_entry_id: callout.maddat_entry_id,
                packed_size: bank.packed_size,
                packed_sha256: bank.packed_sha256.clone(),
                decoded_size: bank.decoded_size,
                decoded_sha256: bank.decoded_sha256.clone(),
                width: 48,
                height: 16,
                plane_count: 5,
                contains_japanese: true,
                requires_localization: true,
                reviewed_text: vec![
                    callout
                        .source_text
                        .clone()
                        .expect("adopted callout has transcribed source text"),
                ],
                consumer: GraphicTextConsumerEvidence::FieldUnitCallout {
                    program_filename: callout_consumer.program_filename.clone(),
                    program_sha256: callout_consumer.program_sha256.clone(),
                    runtime_unit_class_id: callout.runtime_unit_class_id,
                    map_file_offset: callout.map_file_offset,
                    map_runtime_address: callout.map_runtime_address,
                    tile_ids: callout.tile_ids.clone(),
                    bank_load_block_file_offset: if callout.runtime_unit_class_id <= 10 {
                        callout_consumer.fixed_bank_load_block_file_offset
                    } else {
                        callout_consumer.dynamic_bank_load_block_file_offset
                    },
                    bank_load_block_runtime_address: if callout.runtime_unit_class_id <= 10 {
                        callout_consumer.fixed_bank_load_block_runtime_address
                    } else {
                        callout_consumer.dynamic_bank_load_block_runtime_address
                    },
                    bank_loader_call_file_offset: if callout.runtime_unit_class_id <= 10 {
                        callout_consumer.fixed_bank_loader_call_file_offset
                    } else {
                        callout_consumer.dynamic_bank_loader_call_file_offset
                    },
                    bank_loader_runtime_address: callout_consumer.bank_loader_runtime_address,
                    bank_selection_file_offsets: callout_consumer
                        .bank_selection_file_offsets
                        .clone(),
                    renderer_file_offset: callout_consumer.renderer_file_offset,
                    renderer_runtime_address: callout_consumer.renderer_runtime_address,
                    pointer_table_file_offset: callout_consumer.pointer_table_file_offset,
                    pointer_table_runtime_address: callout_consumer.pointer_table_runtime_address,
                    call_site_file_offsets: callout_consumer.call_site_file_offsets.clone(),
                    isa_profile_id: callout_consumer.isa_profile_id.clone(),
                },
            })
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        field_unit_callout_surfaces.len() == 19,
        "graphic-text catalog found {} transcribed field-unit callouts instead of 19",
        field_unit_callout_surfaces.len()
    );

    let mut surfaces = vec![
        GraphicTextSurface {
            id: "stage-select".to_owned(),
            maddat_entry_id: stage_select_entry.id,
            packed_size: stage_select_entry.bytes.len(),
            packed_sha256: disk::sha256_hex(stage_select_entry.bytes),
            decoded_size: stage_select_decoded.len(),
            decoded_sha256: disk::sha256_hex(&stage_select_decoded),
            width: gcs::SCREEN_WIDTH,
            height: gcs::SCREEN_HEIGHT,
            plane_count: gcs::PLANE_COUNT,
            contains_japanese: false,
            requires_localization: true,
            reviewed_text: vec!["STAGE SELECT".to_owned()],
            consumer: stage_select_consumer,
        },
        GraphicTextSurface {
            id: "title".to_owned(),
            maddat_entry_id: title_entry.id,
            packed_size: title_entry.bytes.len(),
            packed_sha256: disk::sha256_hex(title_entry.bytes),
            decoded_size: title_decoded.len(),
            decoded_sha256: disk::sha256_hex(&title_decoded),
            width: gcs::SCREEN_WIDTH,
            height: gcs::SCREEN_HEIGHT,
            plane_count: gcs::PLANE_COUNT,
            contains_japanese: true,
            requires_localization: true,
            reviewed_text: vec![
                "大魔導戦略物語'95".to_owned(),
                "だいまどうせんりゃくものがたり'95".to_owned(),
                "DAIMADOUSENRYAKU MONOGATARI'95".to_owned(),
            ],
            consumer: title_consumer,
        },
        GraphicTextSurface {
            id: "title-menu".to_owned(),
            maddat_entry_id: title_menu_entry.id,
            packed_size: title_menu_entry.bytes.len(),
            packed_sha256: disk::sha256_hex(title_menu_entry.bytes),
            decoded_size: title_menu.decoded.len(),
            decoded_sha256: disk::sha256_hex(&title_menu.decoded),
            width: title_menu_glyphs::GLYPH_WIDTH,
            height: title_menu_glyphs::GLYPH_HEIGHT,
            plane_count: title_menu_glyphs::COLOR_PLANE_COUNT + title_menu_glyphs::MASK_PLANE_COUNT,
            contains_japanese: false,
            requires_localization: false,
            reviewed_text: vec!["START".to_owned(), "CONTINUE".to_owned()],
            consumer: title_menu_consumer,
        },
        GraphicTextSurface {
            id: "game-over".to_owned(),
            maddat_entry_id: game_over_entry.id,
            packed_size: game_over_entry.bytes.len(),
            packed_sha256: disk::sha256_hex(game_over_entry.bytes),
            decoded_size: game_over.planar_bytes.len(),
            decoded_sha256: disk::sha256_hex(&game_over.planar_bytes),
            width: game_over_graphic::WIDTH,
            height: game_over_graphic::HEIGHT,
            plane_count: game_over_graphic::PLANE_COUNT,
            contains_japanese: true,
            requires_localization: true,
            reviewed_text: vec!["ゲームオーバー".to_owned()],
            consumer: game_over_consumer,
        },
        GraphicTextSurface {
            id: "stage-completion-legend".to_owned(),
            maddat_entry_id: stage_completion_entry.id,
            packed_size: stage_completion_entry.bytes.len(),
            packed_sha256: disk::sha256_hex(stage_completion_entry.bytes),
            decoded_size: stage_completion.decoded_tile_bank.len(),
            decoded_sha256: disk::sha256_hex(&stage_completion.decoded_tile_bank),
            width: stage_completion_graphic::WIDTH,
            height: stage_completion_graphic::HEIGHT,
            plane_count: stage_completion_graphic::PLANE_COUNT,
            contains_japanese: true,
            requires_localization: true,
            reviewed_text: vec!["伝説".to_owned()],
            consumer: stage_completion_consumer,
        },
    ];
    surfaces.extend(field_unit_callout_surfaces);

    Ok(GraphicTextCatalog { surfaces })
}

fn verify_stage_select_consumer(bytes: &[u8]) -> Result<GraphicTextConsumerEvidence> {
    let load = bytes
        .get(
            STAGE_SELECT_LOAD_FILE_OFFSET
                ..STAGE_SELECT_LOAD_FILE_OFFSET + STAGE_SELECT_LOAD_BLOCK_LEN,
        )
        .context("SELECT stage-select load block is truncated")?;
    ensure!(
        load.starts_with(&[0xb8, stage_select_graphic::MADDAT_ENTRY_ID as u8, 0x00]),
        "SELECT stage-select MADDAT entry reference does not match"
    );
    let load_decoded_instruction_count = decode_exact_block(load, "SELECT stage-select load")?;
    verify_near_call(
        bytes,
        STAGE_SELECT_FILE_READ_CALL_FILE_OFFSET,
        STAGE_SELECT_FILE_READ_RUNTIME_ADDRESS,
        "SELECT stage-select file reader",
    )?;
    for file_offset in STAGE_SELECT_RENDER_CALL_FILE_OFFSETS {
        verify_near_call(
            bytes,
            file_offset,
            STAGE_SELECT_RENDER_RUNTIME_ADDRESS,
            "SELECT stage-select GCS renderer",
        )?;
    }

    Ok(GraphicTextConsumerEvidence::GcsScreen {
        program_filename: "SELECT.COM".to_owned(),
        program_sha256: disk::sha256_hex(bytes),
        load_reference_file_offset: STAGE_SELECT_LOAD_FILE_OFFSET,
        load_reference_runtime_address: STAGE_SELECT_LOAD_FILE_OFFSET + COM_ORIGIN,
        load_decoded_instruction_count,
        renderer_call_file_offsets: STAGE_SELECT_RENDER_CALL_FILE_OFFSETS.to_vec(),
        renderer_runtime_address: STAGE_SELECT_RENDER_RUNTIME_ADDRESS,
        isa_profile_id: v30::PROFILE_ID.to_owned(),
    })
}

fn verify_stage_completion_consumer(bytes: &[u8]) -> Result<GraphicTextConsumerEvidence> {
    let load = bytes
        .get(
            STAGE_COMPLETION_LOAD_BLOCK_FILE_OFFSET
                ..STAGE_COMPLETION_LOAD_BLOCK_FILE_OFFSET + STAGE_COMPLETION_LOAD_BLOCK_LEN,
        )
        .context("SELECT stage-completion load block is truncated")?;
    ensure!(
        load.starts_with(&[0xb8, stage_completion_graphic::MADDAT_ENTRY_ID as u8, 0x00,]),
        "SELECT stage-completion MADDAT entry reference does not match"
    );
    let load_block_decoded_instruction_count =
        decode_exact_block(load, "SELECT stage-completion load")?;
    verify_near_call(
        bytes,
        STAGE_COMPLETION_FILE_READ_CALL_FILE_OFFSET,
        STAGE_COMPLETION_FILE_READ_RUNTIME_ADDRESS,
        "SELECT stage-completion file reader",
    )?;
    verify_near_call(
        bytes,
        STAGE_COMPLETION_DECOMPRESS_CALL_FILE_OFFSET,
        STAGE_COMPLETION_DECOMPRESS_RUNTIME_ADDRESS,
        "SELECT stage-completion decompressor",
    )?;

    Ok(GraphicTextConsumerEvidence::TiledPlanarRegion {
        program_filename: "SELECT.COM".to_owned(),
        program_sha256: disk::sha256_hex(bytes),
        load_block_file_offset: STAGE_COMPLETION_LOAD_BLOCK_FILE_OFFSET,
        load_block_runtime_address: STAGE_COMPLETION_LOAD_BLOCK_FILE_OFFSET + COM_ORIGIN,
        load_block_decoded_instruction_count,
        file_read_call_file_offset: STAGE_COMPLETION_FILE_READ_CALL_FILE_OFFSET,
        file_read_runtime_address: STAGE_COMPLETION_FILE_READ_RUNTIME_ADDRESS,
        decompressor_call_file_offset: STAGE_COMPLETION_DECOMPRESS_CALL_FILE_OFFSET,
        decompressor_runtime_address: STAGE_COMPLETION_DECOMPRESS_RUNTIME_ADDRESS,
        tile_ids: stage_completion_graphic::TILE_IDS.to_vec(),
        tile_columns: 3,
        tile_rows: 2,
        tile_width: stage_completion_graphic::TILE_WIDTH,
        tile_height: stage_completion_graphic::TILE_HEIGHT,
        isa_profile_id: v30::PROFILE_ID.to_owned(),
    })
}

fn verify_title_consumer(bytes: &[u8]) -> Result<GraphicTextConsumerEvidence> {
    let load = bytes
        .get(TITLE_LOAD_FILE_OFFSET..TITLE_LOAD_FILE_OFFSET + TITLE_LOAD_BLOCK_LEN)
        .context("OPENING title load block is truncated")?;
    ensure!(
        load.starts_with(&[0xb8, TITLE_ENTRY_ID as u8, 0x00]),
        "OPENING title MADDAT entry reference does not match"
    );
    let load_decoded_instruction_count = decode_exact_block(load, "OPENING title load")?;
    for file_offset in TITLE_RENDER_CALL_FILE_OFFSETS {
        verify_near_call(
            bytes,
            file_offset,
            TITLE_RENDER_RUNTIME_ADDRESS,
            "OPENING title GCS renderer",
        )?;
    }

    Ok(GraphicTextConsumerEvidence::GcsScreen {
        program_filename: "OPENING.COM".to_owned(),
        program_sha256: disk::sha256_hex(bytes),
        load_reference_file_offset: TITLE_LOAD_FILE_OFFSET,
        load_reference_runtime_address: TITLE_LOAD_FILE_OFFSET + COM_ORIGIN,
        load_decoded_instruction_count,
        renderer_call_file_offsets: TITLE_RENDER_CALL_FILE_OFFSETS.to_vec(),
        renderer_runtime_address: TITLE_RENDER_RUNTIME_ADDRESS,
        isa_profile_id: v30::PROFILE_ID.to_owned(),
    })
}

pub(crate) fn verify_title_menu_consumer(bytes: &[u8]) -> Result<GraphicTextConsumerEvidence> {
    let load = bytes
        .get(TITLE_MENU_LOAD_FILE_OFFSET..TITLE_MENU_LOAD_FILE_OFFSET + TITLE_MENU_LOAD_BLOCK_LEN)
        .context("OPENING title-menu glyph load block is truncated")?;
    ensure!(
        load.starts_with(&[0xb8, title_menu_glyphs::MADDAT_ENTRY_ID as u8, 0x00,]),
        "OPENING title-menu MADDAT entry reference does not match"
    );
    let load_block_decoded_instruction_count =
        decode_exact_block(load, "OPENING title-menu glyph load")?;
    verify_near_call(
        bytes,
        TITLE_MENU_FILE_READ_CALL_FILE_OFFSET,
        TITLE_MENU_FILE_READ_RUNTIME_ADDRESS,
        "OPENING title-menu file reader",
    )?;
    verify_near_call(
        bytes,
        TITLE_MENU_DECOMPRESS_CALL_FILE_OFFSET,
        TITLE_MENU_DECOMPRESS_RUNTIME_ADDRESS,
        "OPENING title-menu decompressor",
    )?;
    ensure!(
        bytes.get(
            TITLE_MENU_DECODED_COPY_WORD_COUNT_FILE_OFFSET
                ..TITLE_MENU_DECODED_COPY_WORD_COUNT_FILE_OFFSET + 3
        ) == Some(
            [
                0xb9,
                TITLE_MENU_DECODED_COPY_WORD_COUNT as u8,
                (TITLE_MENU_DECODED_COPY_WORD_COUNT >> 8) as u8,
            ]
            .as_slice()
        ),
        "OPENING title-menu decoded-copy word count differs from the reviewed source"
    );
    verify_near_call(
        bytes,
        TITLE_MENU_DECODED_COPY_CALL_FILE_OFFSET,
        TITLE_MENU_DECODED_COPY_RUNTIME_ADDRESS,
        "OPENING title-menu decoded VRAM copy",
    )?;
    verify_near_call(
        bytes,
        TITLE_MENU_PRECOMPOSE_CALL_FILE_OFFSET,
        TITLE_MENU_PRECOMPOSE_RUNTIME_ADDRESS,
        "OPENING title-menu precomposer",
    )?;
    for (file_offset, runtime_address) in TITLE_MENU_RENDER_CALLS {
        verify_near_call(
            bytes,
            file_offset,
            runtime_address,
            "OPENING title-menu animation renderer",
        )?;
    }
    ensure!(
        bytes.get(
            TITLE_MENU_SEQUENCE_FILE_OFFSETS[0]
                ..TITLE_MENU_SEQUENCE_FILE_OFFSETS[0] + TITLE_MENU_START_SEQUENCE.len()
        ) == Some(TITLE_MENU_START_SEQUENCE.as_slice()),
        "OPENING START glyph sequence differs from the reviewed source"
    );
    ensure!(
        bytes.get(
            TITLE_MENU_SEQUENCE_FILE_OFFSETS[1]
                ..TITLE_MENU_SEQUENCE_FILE_OFFSETS[1] + TITLE_MENU_CONTINUE_SEQUENCE.len()
        ) == Some(TITLE_MENU_CONTINUE_SEQUENCE.as_slice()),
        "OPENING CONTINUE glyph sequence differs from the reviewed source"
    );
    let source_glyph_sequences = [
        TITLE_MENU_START_SEQUENCE.as_slice(),
        TITLE_MENU_CONTINUE_SEQUENCE.as_slice(),
    ]
    .into_iter()
    .map(|sequence| {
        sequence[..sequence.len() - 1]
            .chunks_exact(2)
            .map(|pair| pair[0])
            .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>();

    Ok(GraphicTextConsumerEvidence::MaskedSpriteLabels {
        program_filename: "OPENING.COM".to_owned(),
        program_sha256: disk::sha256_hex(bytes),
        load_block_file_offset: TITLE_MENU_LOAD_FILE_OFFSET,
        load_block_runtime_address: TITLE_MENU_LOAD_FILE_OFFSET + COM_ORIGIN,
        load_block_decoded_instruction_count,
        file_read_call_file_offset: TITLE_MENU_FILE_READ_CALL_FILE_OFFSET,
        file_read_runtime_address: TITLE_MENU_FILE_READ_RUNTIME_ADDRESS,
        decompressor_call_file_offset: TITLE_MENU_DECOMPRESS_CALL_FILE_OFFSET,
        decompressor_runtime_address: TITLE_MENU_DECOMPRESS_RUNTIME_ADDRESS,
        decoded_copy_word_count_file_offset: TITLE_MENU_DECODED_COPY_WORD_COUNT_FILE_OFFSET,
        decoded_copy_word_count: TITLE_MENU_DECODED_COPY_WORD_COUNT,
        decoded_copy_byte_count: TITLE_MENU_DECODED_COPY_WORD_COUNT * 2,
        decoded_copy_call_file_offset: TITLE_MENU_DECODED_COPY_CALL_FILE_OFFSET,
        decoded_copy_runtime_address: TITLE_MENU_DECODED_COPY_RUNTIME_ADDRESS,
        precompose_call_file_offset: TITLE_MENU_PRECOMPOSE_CALL_FILE_OFFSET,
        precompose_runtime_address: TITLE_MENU_PRECOMPOSE_RUNTIME_ADDRESS,
        renderer_call_file_offsets: TITLE_MENU_RENDER_CALLS
            .iter()
            .map(|(file_offset, _)| *file_offset)
            .collect(),
        renderer_runtime_addresses: TITLE_MENU_RENDER_CALLS
            .iter()
            .map(|(_, runtime_address)| *runtime_address)
            .collect(),
        sequence_file_offsets: TITLE_MENU_SEQUENCE_FILE_OFFSETS.to_vec(),
        label_slot_counts: source_glyph_sequences.iter().map(Vec::len).collect(),
        source_glyph_sequences,
        stored_glyph_count: title_menu_glyphs::GLYPH_COUNT,
        selectable_glyph_count: title_menu_glyphs::GLYPH_COUNT - 1,
        color_plane_count: title_menu_glyphs::COLOR_PLANE_COUNT,
        mask_plane_count: title_menu_glyphs::MASK_PLANE_COUNT,
        isa_profile_id: v30::PROFILE_ID.to_owned(),
    })
}

fn verify_game_over_consumer(bytes: &[u8]) -> Result<GraphicTextConsumerEvidence> {
    let load = bytes
        .get(
            GAME_OVER_LOAD_BLOCK_FILE_OFFSET
                ..GAME_OVER_LOAD_BLOCK_FILE_OFFSET + GAME_OVER_LOAD_BLOCK_LEN,
        )
        .context("MAD game-over load block is truncated")?;
    ensure!(
        load.starts_with(&[0xb9, game_over_graphic::MADDAT_ENTRY_ID as u8, 0x00]),
        "MAD game-over MADDAT entry reference does not match"
    );
    let load_block_decoded_instruction_count = decode_exact_block(load, "MAD game-over load")?;
    verify_near_call(
        bytes,
        GAME_OVER_LOADER_CALL_FILE_OFFSET,
        GAME_OVER_LOADER_RUNTIME_ADDRESS,
        "MAD MADDAT loader",
    )?;

    let renderer = bytes
        .get(
            GAME_OVER_RENDERER_FILE_OFFSET
                ..GAME_OVER_RENDERER_FILE_OFFSET + GAME_OVER_RENDERER_BODY_LEN,
        )
        .context("MAD game-over renderer is truncated")?;
    ensure!(
        renderer.get(0x17..0x1a) == Some(&[0xb9, game_over_graphic::HEIGHT as u8, 0x00]),
        "MAD game-over renderer row count does not match"
    );
    ensure!(
        renderer.get(0x1d..0x20) == Some(&[0xb9, (game_over_graphic::WIDTH / 16) as u8, 0x00]),
        "MAD game-over renderer word width does not match"
    );
    let renderer_decoded_instruction_count =
        decode_complete_subroutine(renderer, "MAD game-over renderer")?;
    verify_near_call(
        bytes,
        GAME_OVER_RENDERER_CALL_FILE_OFFSET,
        GAME_OVER_RENDERER_FILE_OFFSET + COM_ORIGIN,
        "MAD game-over renderer",
    )?;

    Ok(GraphicTextConsumerEvidence::PlanarBlock {
        program_filename: "MAD.COM".to_owned(),
        program_sha256: disk::sha256_hex(bytes),
        load_block_file_offset: GAME_OVER_LOAD_BLOCK_FILE_OFFSET,
        load_block_runtime_address: GAME_OVER_LOAD_BLOCK_FILE_OFFSET + COM_ORIGIN,
        load_block_decoded_instruction_count,
        loader_call_file_offset: GAME_OVER_LOADER_CALL_FILE_OFFSET,
        loader_runtime_address: GAME_OVER_LOADER_RUNTIME_ADDRESS,
        renderer_call_file_offset: GAME_OVER_RENDERER_CALL_FILE_OFFSET,
        renderer_file_offset: GAME_OVER_RENDERER_FILE_OFFSET,
        renderer_runtime_address: GAME_OVER_RENDERER_FILE_OFFSET + COM_ORIGIN,
        renderer_decoded_instruction_count,
        bytes_per_row: game_over_graphic::WIDTH / 8,
        words_per_row: game_over_graphic::WIDTH / 16,
        isa_profile_id: v30::PROFILE_ID.to_owned(),
    })
}

fn verify_packed_asset(bytes: &[u8], expected_sha256: &str, label: &str) -> Result<()> {
    let actual = disk::sha256_hex(bytes);
    ensure!(
        actual == expected_sha256,
        "{label} packed SHA-256 differs from the reviewed source: expected {expected_sha256}, got {actual}"
    );
    Ok(())
}

fn verify_near_call(
    bytes: &[u8],
    file_offset: usize,
    expected_runtime_target: usize,
    label: &str,
) -> Result<()> {
    let encoded = bytes
        .get(file_offset..file_offset + 3)
        .with_context(|| format!("{label} call is truncated"))?;
    let decoded = decode_bytes(encoded).with_context(|| format!("decode {label} call"))?;
    ensure!(
        decoded.byte_len == 3 && matches!(decoded.instruction, Instruction::Call { .. }),
        "{label} is not a typed V30 near CALL"
    );
    let displacement = i16::from_le_bytes([encoded[1], encoded[2]]) as isize;
    let runtime_target = (file_offset + COM_ORIGIN + decoded.byte_len)
        .checked_add_signed(displacement)
        .context("near CALL target overflow")?;
    ensure!(
        runtime_target == expected_runtime_target,
        "{label} targets {runtime_target:#x}, expected {expected_runtime_target:#x}"
    );
    Ok(())
}

fn decode_complete_subroutine(bytes: &[u8], label: &str) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    let mut last = None;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..])
            .with_context(|| format!("decode V30 instruction at {label} byte {cursor:#x}"))?;
        cursor += decoded.byte_len;
        instruction_count += 1;
        last = Some(decoded.instruction);
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed {label} boundary"
    );
    ensure!(
        matches!(last, Some(Instruction::Ret { pop: 0 })),
        "{label} does not end in a typed V30 RET"
    );
    Ok(instruction_count)
}

fn decode_exact_block(bytes: &[u8], label: &str) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..])
            .with_context(|| format!("decode V30 instruction at {label} byte {cursor:#x}"))?;
        cursor += decoded.byte_len;
        instruction_count += 1;
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed {label} boundary"
    );
    Ok(instruction_count)
}

#[cfg(test)]
#[path = "graphic_text_catalog_tests.rs"]
mod graphic_text_catalog_tests;
