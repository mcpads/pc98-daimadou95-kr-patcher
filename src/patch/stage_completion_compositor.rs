use std::collections::BTreeSet;

use anyhow::{Result, ensure};

use crate::font::rasterize_pc98_character;
use crate::game_data::stage_completion_graphic::{HEIGHT, WIDTH};

const BACKGROUND: u8 = 0;
const OUTLINE: u8 = 1;
const DEEP_SHADOW: u8 = 8;
const HIGHLIGHT: u8 = 10;
const PRIMARY: u8 = 11;
const SHADOW: u8 = 12;
const SOURCE_PALETTE: [u8; 6] = [BACKGROUND, OUTLINE, DEEP_SHADOW, HIGHLIGHT, PRIMARY, SHADOW];

const GLYPH_SOURCE_SIZE: usize = 16;
const GLYPH_WIDTH: usize = 20;
const GLYPH_HEIGHT: usize = 24;
const GLYPH_COUNT: usize = 2;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct StageCompletionComposition {
    pub pixels: Vec<u8>,
}

pub(super) fn compose_stage_completion_graphic(
    source: &[u8],
    korean_text: &str,
) -> Result<StageCompletionComposition> {
    ensure!(
        source.len() == WIDTH * HEIGHT,
        "stage-completion compositor received {} pixels instead of {}",
        source.len(),
        WIDTH * HEIGHT
    );
    ensure!(
        [
            source[0],
            source[WIDTH - 1],
            source[(HEIGHT - 1) * WIDTH],
            source[WIDTH * HEIGHT - 1]
        ]
        .into_iter()
        .all(|pixel| pixel == BACKGROUND),
        "stage-completion source corners do not match the transparent background"
    );
    let source_palette = source.iter().copied().collect::<BTreeSet<_>>();
    ensure!(
        source_palette == SOURCE_PALETTE.into_iter().collect(),
        "stage-completion source palette differs from the reviewed six-color legend"
    );
    ensure!(
        korean_text.chars().count() == GLYPH_COUNT,
        "stage-completion text must contain exactly {GLYPH_COUNT} characters"
    );

    let mask = rasterize_scaled_text(korean_text)?;
    let mask_width = GLYPH_COUNT * GLYPH_WIDTH;
    let origin_x = (WIDTH - mask_width - 3) / 2;
    let origin_y = (HEIGHT - GLYPH_HEIGHT - 3) / 2;
    let mut pixels = vec![BACKGROUND; WIDTH * HEIGHT];

    paint_mask(
        &mut pixels,
        &mask,
        mask_width,
        origin_x + 3,
        origin_y + 3,
        DEEP_SHADOW,
    );
    paint_mask(
        &mut pixels,
        &mask,
        mask_width,
        origin_x + 2,
        origin_y + 2,
        SHADOW,
    );
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            paint_mask_signed(
                &mut pixels,
                &mask,
                mask_width,
                origin_x as isize + dx,
                origin_y as isize + dy,
                OUTLINE,
            )?;
        }
    }
    paint_mask(&mut pixels, &mask, mask_width, origin_x, origin_y, PRIMARY);
    paint_highlights(&mut pixels, &mask, mask_width, origin_x, origin_y);

    ensure!(
        pixels != source,
        "stage-completion compositor made no pixel changes"
    );
    Ok(StageCompletionComposition { pixels })
}

fn rasterize_scaled_text(text: &str) -> Result<Vec<bool>> {
    let width = GLYPH_COUNT * GLYPH_WIDTH;
    let mut mask = vec![false; width * GLYPH_HEIGHT];
    for (character_index, character) in text.chars().enumerate() {
        let glyph = rasterize_pc98_character(character)?;
        for y in 0..GLYPH_HEIGHT {
            let source_y = y * GLYPH_SOURCE_SIZE / GLYPH_HEIGHT;
            for x in 0..GLYPH_WIDTH {
                let source_x = x * GLYPH_SOURCE_SIZE / GLYPH_WIDTH;
                let set = glyph[source_y * 2 + source_x / 8] & (1 << (7 - source_x % 8)) != 0;
                mask[y * width + character_index * GLYPH_WIDTH + x] = set;
            }
        }
    }
    Ok(mask)
}

fn paint_mask(
    pixels: &mut [u8],
    mask: &[bool],
    mask_width: usize,
    origin_x: usize,
    origin_y: usize,
    color: u8,
) {
    for y in 0..GLYPH_HEIGHT {
        for x in 0..mask_width {
            if mask[y * mask_width + x] {
                pixels[(origin_y + y) * WIDTH + origin_x + x] = color;
            }
        }
    }
}

fn paint_mask_signed(
    pixels: &mut [u8],
    mask: &[bool],
    mask_width: usize,
    origin_x: isize,
    origin_y: isize,
    color: u8,
) -> Result<()> {
    ensure!(
        origin_x >= 0
            && origin_y >= 0
            && origin_x as usize + mask_width <= WIDTH
            && origin_y as usize + GLYPH_HEIGHT <= HEIGHT,
        "stage-completion outline exceeds the translated canvas"
    );
    paint_mask(
        pixels,
        mask,
        mask_width,
        origin_x as usize,
        origin_y as usize,
        color,
    );
    Ok(())
}

fn paint_highlights(
    pixels: &mut [u8],
    mask: &[bool],
    mask_width: usize,
    origin_x: usize,
    origin_y: usize,
) {
    for y in 0..GLYPH_HEIGHT {
        for x in 0..mask_width {
            if !mask[y * mask_width + x] || (y > 0 && mask[(y - 1) * mask_width + x]) {
                continue;
            }
            pixels[(origin_y + y) * WIDTH + origin_x + x] = HIGHLIGHT;
        }
    }
}

#[cfg(test)]
#[path = "stage_completion_compositor_tests.rs"]
mod stage_completion_compositor_tests;
