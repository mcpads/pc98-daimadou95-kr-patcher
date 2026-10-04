use std::collections::BTreeMap;
use std::fs;
use std::io::BufWriter;

use super::*;
use crate::game_data::compile_lz::encode_compile_lz;
use crate::game_data::ending_credits::{LINES_PER_SCREEN, SCREEN_COUNT};

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn credit_compiler_replaces_only_role_lines_and_source_unused_glyphs() {
    let source_font = (0..FONT_GLYPH_COUNT)
        .flat_map(|index| vec![u8::try_from(index + 1).unwrap(); FONT_GLYPH_SIZE])
        .collect::<Vec<_>>();
    let packed_font = encode_compile_lz(&source_font).unwrap();
    let source_grid = synthetic_grid();
    let translations = reviewed_translations();

    let patched = compile_ending_credits(&packed_font, &source_grid, &translations).unwrap();

    assert_eq!(patched.report.translated_role_count, 10);
    assert_eq!(patched.report.unique_character_count, 23);
    assert_eq!(
        patched.report.font.profile_id,
        "neodunggeunmo-ending-credit-24x32"
    );
    let protected_line = ending_credit_line_offset(2, 2).unwrap();
    assert_eq!(
        &patched.text_grid[protected_line..protected_line + CELLS_PER_LINE],
        &source_grid[protected_line..protected_line + CELLS_PER_LINE]
    );
    let used_a = ending_credit_glyph_offset(1).unwrap();
    assert_eq!(
        &patched.decoded_font[used_a..used_a + FONT_GLYPH_SIZE],
        &source_font[used_a..used_a + FONT_GLYPH_SIZE]
    );
    assert_eq!(occupied_columns(&patched.text_grid, 1, 2), vec![8, 9, 10]);
    assert_eq!(occupied_columns(&patched.text_grid, 2, 1), vec![0, 1]);
    assert_eq!(occupied_columns(&patched.text_grid, 10, 1), vec![9, 10]);
}

#[test]
fn credit_compiler_rejects_incomplete_or_overwide_translations() {
    let source_font = vec![0; FONT_GLYPH_COUNT * FONT_GLYPH_SIZE];
    let packed_font = encode_compile_lz(&source_font).unwrap();
    let source_grid = synthetic_grid();
    let mut incomplete = reviewed_translations();
    incomplete.remove("ending-credit:screen-10-line-01");
    assert!(compile_ending_credits(&packed_font, &source_grid, &incomplete).is_err());

    let mut overwide = reviewed_translations();
    overwide.insert(
        "ending-credit:screen-01-line-02".into(),
        "가나다라마바사아자차카타파하가나다라마바사".into(),
    );
    assert!(compile_ending_credits(&packed_font, &source_grid, &overwide).is_err());
}

#[test]
#[ignore = "requires ENDING_CREDITS_MADDAT pointing to a user-owned extracted MADDAT"]
fn source_credit_bank_has_thirty_unused_slots_and_round_trips() {
    let path = std::env::var("ENDING_CREDITS_MADDAT").unwrap();
    let maddat = fs::read(path).unwrap();
    let archive = crate::game_data::flink::parse_flink_archive(&maddat).unwrap();
    let font = archive.entries[crate::game_data::ending_credits::FONT_ENTRY_ID - 1].bytes;
    let grid = archive.entries[crate::game_data::ending_credits::TEXT_ENTRY_ID - 1].bytes;

    let patched = compile_ending_credits(font, grid, &reviewed_translations()).unwrap();

    assert_eq!(patched.report.available_source_slot_count, 30);
    assert_eq!(patched.report.preserved_source_glyph_count, 35);
    assert_eq!(patched.report.unique_character_count, 23);
    assert_eq!(patched.text_grid.len(), 601);
    assert_eq!(patched.text_grid.last(), Some(&0x1a));
}

#[test]
#[ignore = "requires ENDING_CREDITS_MADDAT and ENDING_CREDITS_PREVIEW_PATH"]
fn render_source_and_korean_credit_roles_for_visual_review() {
    let maddat = fs::read(std::env::var("ENDING_CREDITS_MADDAT").unwrap()).unwrap();
    let output = std::env::var("ENDING_CREDITS_PREVIEW_PATH").unwrap();
    let archive = crate::game_data::flink::parse_flink_archive(&maddat).unwrap();
    let packed_source_font =
        archive.entries[crate::game_data::ending_credits::FONT_ENTRY_ID - 1].bytes;
    let source_font = decode_complete_compile_lz(packed_source_font)
        .unwrap()
        .output;
    let source_grid = archive.entries[crate::game_data::ending_credits::TEXT_ENTRY_ID - 1].bytes;
    let patched =
        compile_ending_credits(packed_source_font, source_grid, &reviewed_translations()).unwrap();

    const COLUMN_WIDTH: usize = CELLS_PER_LINE * crate::game_data::ending_credits::FONT_GLYPH_WIDTH;
    const GAP: usize = 32;
    const ROW_PITCH: usize = 40;
    let width = COLUMN_WIDTH * 2 + GAP;
    let height = ROLE_LINE_SPECS.len() * ROW_PITCH;
    let mut pixels = vec![0_u8; width * height];
    for (row, spec) in ROLE_LINE_SPECS.iter().enumerate() {
        let offset = ending_credit_line_offset(spec.screen_number, spec.line_number).unwrap();
        draw_credit_line(
            &mut pixels,
            width,
            row * ROW_PITCH,
            0,
            &source_font,
            &source_grid[offset..offset + CELLS_PER_LINE],
        );
        draw_credit_line(
            &mut pixels,
            width,
            row * ROW_PITCH,
            COLUMN_WIDTH + GAP,
            &patched.decoded_font,
            &patched.text_grid[offset..offset + CELLS_PER_LINE],
        );
    }

    let file = fs::File::create(output).unwrap();
    let mut encoder = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&pixels)
        .unwrap();
}

fn draw_credit_line(
    canvas: &mut [u8],
    canvas_width: usize,
    origin_y: usize,
    origin_x: usize,
    font: &[u8],
    indices: &[u8],
) {
    for (cell, &index) in indices.iter().enumerate() {
        if index == 0 {
            continue;
        }
        let glyph_offset = ending_credit_glyph_offset(index).unwrap();
        let glyph = &font[glyph_offset..glyph_offset + FONT_GLYPH_SIZE];
        for y in 0..crate::game_data::ending_credits::FONT_GLYPH_HEIGHT {
            for x in 0..crate::game_data::ending_credits::FONT_GLYPH_WIDTH {
                if glyph[y * 3 + x / 8] & (1 << (7 - x % 8)) == 0 {
                    continue;
                }
                let target = (origin_y + y) * canvas_width
                    + origin_x
                    + cell * crate::game_data::ending_credits::FONT_GLYPH_WIDTH
                    + x;
                canvas[target] = 0xff;
            }
        }
    }
}

fn synthetic_grid() -> Vec<u8> {
    let mut bytes = vec![0; SCREEN_COUNT * LINES_PER_SCREEN * CELLS_PER_LINE];
    for spec in ROLE_LINE_SPECS {
        let start_column = match (spec.screen_number, spec.line_number) {
            (1, 2) => 7,
            (10, 1) => 4,
            _ => 0,
        };
        let encoded = spec
            .source_text
            .chars()
            .map(source_index)
            .collect::<Vec<_>>();
        let offset = ending_credit_line_offset(spec.screen_number, spec.line_number).unwrap();
        bytes[offset + start_column..offset + start_column + encoded.len()]
            .copy_from_slice(&encoded);
    }
    let name_offset = ending_credit_line_offset(2, 2).unwrap();
    bytes[name_offset..name_offset + 4].copy_from_slice(&[7, 21, 20, 19]);
    bytes.push(0x1a);
    bytes
}

fn source_index(character: char) -> u8 {
    match character {
        ' ' => 0,
        'A'..='Z' => character as u8 - b'A' + 1,
        'a'..='z' => character as u8 - b'a' + 0x1b,
        _ => panic!("unsupported synthetic credit character {character:?}"),
    }
}

fn occupied_columns(grid: &[u8], screen_number: usize, line_number: usize) -> Vec<usize> {
    let offset = ending_credit_line_offset(screen_number, line_number).unwrap();
    grid[offset..offset + CELLS_PER_LINE]
        .iter()
        .enumerate()
        .filter_map(|(column, index)| (*index != 0).then_some(column))
        .collect()
}

fn reviewed_translations() -> BTreeMap<String, String> {
    [
        (1, 2, "제작진"),
        (2, 1, "기획"),
        (3, 1, "그래픽 디자인"),
        (4, 1, "프로그램"),
        (5, 1, "사운드"),
        (6, 1, "특별 감사"),
        (7, 1, "특별 감사"),
        (8, 1, "감독"),
        (9, 1, "프로듀서"),
        (10, 1, "제작"),
    ]
    .into_iter()
    .map(|(screen, line, text)| {
        (
            format!("ending-credit:screen-{screen:02}-line-{line:02}"),
            text.to_owned(),
        )
    })
    .collect()
}
