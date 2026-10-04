use super::*;
use crate::game_data::title_menu_glyphs::{
    GLYPH_HEIGHT, GLYPH_WIDTH, MaskedTitleMenuGlyph, decode_title_menu_glyph_bank,
};

#[test]
#[ignore = "requires TITLE_TRANSLATION_MADDAT and TITLE_TRANSLATION_PREVIEW_PATH"]
fn render_source_and_korean_title_with_the_preserved_original_menu() {
    let maddat = std::fs::read(std::env::var("TITLE_TRANSLATION_MADDAT").unwrap()).unwrap();
    let archive = parse_flink_archive(&maddat).unwrap();
    let source_gcs = decode_gcs(archive.entries[MADDAT_ENTRY_ID - 1].bytes).unwrap();
    let source_pixels =
        decode_planar_indices(&source_gcs.planes, SCREEN_WIDTH, SCREEN_HEIGHT).unwrap();
    let mut source_runtime = source_pixels.clone();
    let source_menu =
        decode_title_menu_glyph_bank(archive.entries[title_menu_glyphs::MADDAT_ENTRY_ID - 1].bytes)
            .unwrap();
    paint_runtime_menu(
        &mut source_runtime,
        &source_menu.glyphs,
        &[0, 1, 2, 3, 1],
        &[4, 5, 6, 1, 7, 6, 8, 9],
    );

    let mut korean_runtime = compose_title_screen(
        &source_pixels,
        "대마도전략 이야기 '95",
        "대마도전략 이야기 '95",
        "대마도전략",
    )
    .unwrap()
    .pixels;
    paint_runtime_menu(
        &mut korean_runtime,
        &source_menu.glyphs,
        &[0, 1, 2, 3, 1],
        &[4, 5, 6, 1, 7, 6, 8, 9],
    );

    let gap = 8;
    let output_width = SCREEN_WIDTH * 2 + gap;
    let mut rgb = vec![255_u8; output_width * SCREEN_HEIGHT * 3];
    for (column, pixels) in [source_runtime, korean_runtime].into_iter().enumerate() {
        let origin_x = column * (SCREEN_WIDTH + gap);
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                let target = (y * output_width + origin_x + x) * 3;
                rgb[target..target + 3]
                    .copy_from_slice(&TITLE_PALETTE[pixels[y * SCREEN_WIDTH + x] as usize]);
            }
        }
    }
    let file =
        std::fs::File::create(std::env::var("TITLE_TRANSLATION_PREVIEW_PATH").unwrap()).unwrap();
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        output_width as u32,
        SCREEN_HEIGHT as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgb)
        .unwrap();
}

fn paint_runtime_menu(
    pixels: &mut [u8],
    glyphs: &[MaskedTitleMenuGlyph],
    start_sequence: &[usize],
    continue_sequence: &[usize],
) {
    paint_masked_sequence(pixels, glyphs, start_sequence, 0x10 * 8, 0x128);
    paint_masked_sequence(pixels, glyphs, continue_sequence, 0x28 * 8, 0x128);
    paint_masked_sequence(pixels, glyphs, &[10], 0x20 * 8, 0x140);
}

fn paint_masked_sequence(
    pixels: &mut [u8],
    glyphs: &[MaskedTitleMenuGlyph],
    sequence: &[usize],
    origin_x: usize,
    origin_y: usize,
) {
    for (slot, glyph_index) in sequence.iter().copied().enumerate() {
        for y in 0..GLYPH_HEIGHT {
            for x in 0..GLYPH_WIDTH {
                if let Some(color) = glyphs[glyph_index].pixels[y * GLYPH_WIDTH + x] {
                    pixels[(origin_y + y) * SCREEN_WIDTH + origin_x + slot * GLYPH_WIDTH + x] =
                        color;
                }
            }
        }
    }
}

const TITLE_PALETTE: [[u8; 3]; 16] = [
    [187, 0, 17],
    [238, 17, 68],
    [255, 102, 119],
    [255, 153, 170],
    [0, 0, 0],
    [68, 85, 204],
    [119, 204, 255],
    [136, 136, 136],
    [255, 255, 255],
    [255, 255, 170],
    [255, 238, 17],
    [221, 153, 17],
    [170, 204, 255],
    [255, 170, 204],
    [204, 170, 255],
    [170, 255, 204],
];
