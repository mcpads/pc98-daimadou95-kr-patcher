use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::gcs::{
    HEADER_SIZE, PLANE_COUNT, SCREEN_HEIGHT, SCREEN_WIDTH, decode_gcs, encode_gcs,
};
use crate::game_data::planar::{decode_planar_indices, encode_planar_indices};
use crate::game_data::title_menu_glyphs;
use crate::source::{
    GraphicTextConsumerEvidence, TITLE_MENU_PACKED_SHA256, disk, verify_title_menu_consumer,
};
use crate::translation::ReviewedTranslationCorpus;

use super::maddat::apply_repacked_maddat;
use super::title_compositor::compose_title_screen;
use super::title_logo::imagegen_title_logo_sha256;
use super::title_ribbon::imagegen_title_ribbon_sha256;

const TITLE_SEGMENT_ID: &str = "graphic-title";
const MAIN_ENTRY_ID: &str = "graphic-text:title:01";
const SUBTITLE_ENTRY_ID: &str = "graphic-text:title:02";
const COMPACT_TITLE_ENTRY_ID: &str = "graphic-text:title:03";
const MADDAT_ENTRY_ID: usize = 140;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct TitleTranslationFiles {
    pub maddat: Vec<u8>,
    pub opening_com: Vec<u8>,
    pub report: TitleTranslationReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct TitleTranslationReport {
    pub title_segment_id: String,
    pub maddat_entry_id: usize,
    pub main_text: String,
    pub subtitle_text: String,
    pub compact_title_text: String,
    pub packed_input_size: usize,
    pub packed_output_size: usize,
    pub changed_pixel_count: usize,
    pub erased_compact_title_pixel_count: usize,
    pub original_menu: OriginalTitleMenuReport,
    pub imagegen_logo_sha256: String,
    pub imagegen_ribbon_sha256: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct OriginalTitleMenuReport {
    pub maddat_entry_id: usize,
    pub labels: [String; 2],
    pub sequence_file_offsets: [usize; 2],
    pub packed_input_size: usize,
    pub packed_output_size: usize,
    pub packed_input_sha256: String,
    pub packed_output_sha256: String,
    pub opening_input_sha256: String,
    pub opening_output_sha256: String,
}

pub(crate) fn compile_title_translation(
    maddat: &[u8],
    opening_com: &[u8],
    corpus: &ReviewedTranslationCorpus,
) -> Result<TitleTranslationFiles> {
    ensure!(
        corpus.validation.human_review_ready,
        "title build requires a human-review-ready translation corpus"
    );
    let title_segment = corpus
        .segments
        .iter()
        .find(|segment| segment.protected.id == TITLE_SEGMENT_ID)
        .context("translation corpus is missing graphic-title")?;
    let main_text = title_segment.reinsertion_text(MAIN_ENTRY_ID)?;
    let subtitle_text = title_segment.reinsertion_text(SUBTITLE_ENTRY_ID)?;
    let compact_title_text = title_segment.reinsertion_text(COMPACT_TITLE_ENTRY_ID)?;

    let archive = parse_flink_archive(maddat)?;
    let source_entry = archive
        .entries
        .get(MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing title graphic entry 140")?;
    let source_menu_entry = archive
        .entries
        .get(title_menu_glyphs::MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing title-menu glyph bank entry 141")?;
    let menu_packed_input_size = source_menu_entry.bytes.len();
    let menu_packed_input_sha256 = disk::sha256_hex(source_menu_entry.bytes);
    ensure!(
        menu_packed_input_sha256 == TITLE_MENU_PACKED_SHA256,
        "title build requires the reviewed original START / CONTINUE glyph bank"
    );
    let menu_sequence_file_offsets = match verify_title_menu_consumer(opening_com)? {
        GraphicTextConsumerEvidence::MaskedSpriteLabels {
            sequence_file_offsets,
            ..
        } => {
            ensure!(
                sequence_file_offsets.len() == 2,
                "title menu consumer has {} label sequences instead of 2",
                sequence_file_offsets.len()
            );
            [sequence_file_offsets[0], sequence_file_offsets[1]]
        }
        _ => unreachable!("title menu verification returns masked-sprite evidence"),
    };

    let source_image = decode_gcs(source_entry.bytes)?;
    let source_pixels = decode_planar_indices(&source_image.planes, SCREEN_WIDTH, SCREEN_HEIGHT)?;
    let composition = compose_title_screen(
        &source_pixels,
        &main_text,
        &subtitle_text,
        &compact_title_text,
    )?;
    let pixels = composition.pixels;
    let changed_pixel_count = source_pixels
        .iter()
        .zip(&pixels)
        .filter(|(source, output)| source != output)
        .count();
    ensure!(
        changed_pixel_count > 0,
        "title compositor made no pixel changes"
    );

    let encoded_planes = encode_planar_indices(&pixels, SCREEN_WIDTH, SCREEN_HEIGHT, PLANE_COUNT)?;
    let encoded_planes: [Vec<u8>; PLANE_COUNT] = encoded_planes
        .try_into()
        .expect("four requested planes produce a four-element vector");
    let header: &[u8; HEADER_SIZE] = &source_image.header;
    let packed = encode_gcs(header, &encoded_planes)?;
    ensure!(
        packed.len() <= usize::from(u16::MAX),
        "translated title GCS is {} bytes and exceeds the FLINK u16 size field",
        packed.len()
    );

    let replacements = BTreeMap::from([(MADDAT_ENTRY_ID, packed.clone())]);
    let repacked = repack_contiguous_flink_archive(maddat, &replacements, &[])?;
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "title-maddat-repacker",
        "replace only the tracked Korean title GCS while preserving the original animated menu",
    )?;
    let checked_archive = parse_flink_archive(&checked_maddat)?;
    ensure!(
        checked_archive.entries[MADDAT_ENTRY_ID - 1].bytes == packed,
        "title GCS did not survive MADDAT repack"
    );
    let output_menu_entry = &checked_archive.entries[title_menu_glyphs::MADDAT_ENTRY_ID - 1];
    ensure!(
        output_menu_entry.bytes == source_menu_entry.bytes,
        "title build changed the original START / CONTINUE glyph bank"
    );
    let menu_packed_output_size = output_menu_entry.bytes.len();
    let menu_packed_output_sha256 = disk::sha256_hex(output_menu_entry.bytes);

    let opening_output = opening_com.to_vec();
    verify_title_menu_consumer(&opening_output)?;
    let opening_input_sha256 = disk::sha256_hex(opening_com);
    let opening_output_sha256 = disk::sha256_hex(&opening_output);
    ensure!(
        opening_input_sha256 == opening_output_sha256,
        "title build changed OPENING.COM while preserving START / CONTINUE"
    );

    Ok(TitleTranslationFiles {
        maddat: checked_maddat,
        opening_com: opening_output,
        report: TitleTranslationReport {
            title_segment_id: TITLE_SEGMENT_ID.into(),
            maddat_entry_id: MADDAT_ENTRY_ID,
            main_text,
            subtitle_text,
            compact_title_text,
            packed_input_size: source_entry.bytes.len(),
            packed_output_size: packed.len(),
            changed_pixel_count,
            erased_compact_title_pixel_count: composition.compact_title.erased_source_pixel_count,
            original_menu: OriginalTitleMenuReport {
                maddat_entry_id: title_menu_glyphs::MADDAT_ENTRY_ID,
                labels: ["START".into(), "CONTINUE".into()],
                sequence_file_offsets: menu_sequence_file_offsets,
                packed_input_size: menu_packed_input_size,
                packed_output_size: menu_packed_output_size,
                packed_input_sha256: menu_packed_input_sha256,
                packed_output_sha256: menu_packed_output_sha256,
                opening_input_sha256,
                opening_output_sha256,
            },
            imagegen_logo_sha256: imagegen_title_logo_sha256().into(),
            imagegen_ribbon_sha256: imagegen_title_ribbon_sha256().into(),
        },
    })
}

#[cfg(test)]
#[path = "title_translation_tests.rs"]
mod title_translation_tests;
