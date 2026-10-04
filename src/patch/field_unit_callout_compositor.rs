use anyhow::{Context, Result, ensure};

use crate::font::rasterize_pc98_character;

pub(super) const WIDTH: usize = 48;
pub(super) const HEIGHT: usize = 16;
pub(super) const TRANSPARENT_PALETTE_INDEX: u8 = 0;
pub(super) const OUTLINE_PALETTE_INDEX: u8 = 1;
pub(super) const INTERIOR_PALETTE_INDEX: u8 = 9;

const GLYPH_WIDTH: usize = 16;
const GLYPH_HEIGHT: usize = 16;
const GLYPH_SPACING: usize = 1;
const OUTLINE_RADIUS: usize = 1;
const MAX_INTERIOR_WIDTH: usize = WIDTH - OUTLINE_RADIUS * 2;
const MAX_INTERIOR_HEIGHT: usize = HEIGHT - OUTLINE_RADIUS * 2;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct FieldUnitCalloutComposition {
    pub opaque: Vec<bool>,
    pub palette_indices: Vec<u8>,
    pub interior_width: usize,
    pub interior_height: usize,
}

struct CroppedGlyph {
    pixels: Vec<bool>,
    width: usize,
    height: usize,
}

pub(super) fn compose_field_unit_callout(korean_text: &str) -> Result<FieldUnitCalloutComposition> {
    ensure!(!korean_text.is_empty(), "field-unit callout text is empty");
    let glyphs = korean_text
        .chars()
        .map(cropped_glyph)
        .collect::<Result<Vec<_>>>()?;
    let interior_width = glyphs.iter().map(|glyph| glyph.width).sum::<usize>()
        + glyphs.len().saturating_sub(1) * GLYPH_SPACING;
    let interior_height = glyphs
        .iter()
        .map(|glyph| glyph.height)
        .max()
        .context("field-unit callout has no glyphs")?;
    ensure!(
        interior_width <= MAX_INTERIOR_WIDTH && interior_height <= MAX_INTERIOR_HEIGHT,
        "field-unit callout {korean_text:?} needs a {interior_width}x{interior_height} interior but only {MAX_INTERIOR_WIDTH}x{MAX_INTERIOR_HEIGHT} is available"
    );

    let mut interior = vec![false; interior_width * interior_height];
    let mut cursor_x = 0;
    for glyph in glyphs {
        let top = (interior_height - glyph.height) / 2;
        for y in 0..glyph.height {
            for x in 0..glyph.width {
                if glyph.pixels[y * glyph.width + x] {
                    interior[(top + y) * interior_width + cursor_x + x] = true;
                }
            }
        }
        cursor_x += glyph.width + GLYPH_SPACING;
    }

    let mut opaque = vec![false; WIDTH * HEIGHT];
    let mut palette_indices = vec![TRANSPARENT_PALETTE_INDEX; WIDTH * HEIGHT];
    let interior_origin_x = (WIDTH - (interior_width + OUTLINE_RADIUS * 2)) / 2 + OUTLINE_RADIUS;
    let interior_origin_y = (HEIGHT - (interior_height + OUTLINE_RADIUS * 2)) / 2 + OUTLINE_RADIUS;

    for y in 0..interior_height {
        for x in 0..interior_width {
            if !interior[y * interior_width + x] {
                continue;
            }
            for dy in -(OUTLINE_RADIUS as isize)..=OUTLINE_RADIUS as isize {
                for dx in -(OUTLINE_RADIUS as isize)..=OUTLINE_RADIUS as isize {
                    let target_x = (interior_origin_x + x) as isize + dx;
                    let target_y = (interior_origin_y + y) as isize + dy;
                    ensure!(
                        target_x >= 0
                            && target_y >= 0
                            && (target_x as usize) < WIDTH
                            && (target_y as usize) < HEIGHT,
                        "field-unit callout outline exceeds its 48x16 canvas"
                    );
                    let target = target_y as usize * WIDTH + target_x as usize;
                    opaque[target] = true;
                    palette_indices[target] = OUTLINE_PALETTE_INDEX;
                }
            }
        }
    }
    for y in 0..interior_height {
        for x in 0..interior_width {
            if interior[y * interior_width + x] {
                let target = (interior_origin_y + y) * WIDTH + interior_origin_x + x;
                opaque[target] = true;
                palette_indices[target] = INTERIOR_PALETTE_INDEX;
            }
        }
    }

    ensure!(
        opaque.iter().any(|pixel| *pixel) && opaque.iter().any(|pixel| !*pixel),
        "field-unit callout must contain foreground and transparent pixels"
    );
    ensure!(
        opaque
            .iter()
            .zip(&palette_indices)
            .all(|(is_opaque, palette_index)| if *is_opaque {
                [OUTLINE_PALETTE_INDEX, INTERIOR_PALETTE_INDEX].contains(palette_index)
            } else {
                *palette_index == TRANSPARENT_PALETTE_INDEX
            }),
        "field-unit callout palette roles are inconsistent with its mask"
    );

    Ok(FieldUnitCalloutComposition {
        opaque,
        palette_indices,
        interior_width,
        interior_height,
    })
}

fn cropped_glyph(character: char) -> Result<CroppedGlyph> {
    let bitmap = rasterize_pc98_character(character)?;
    let is_set =
        |x: usize, y: usize| bitmap[y * (GLYPH_WIDTH / 8) + x / 8] & (1 << (7 - x % 8)) != 0;
    let left = (0..GLYPH_WIDTH)
        .find(|x| (0..GLYPH_HEIGHT).any(|y| is_set(*x, y)))
        .with_context(|| format!("field-unit callout glyph {character:?} is empty"))?;
    let right = (0..GLYPH_WIDTH)
        .rfind(|x| (0..GLYPH_HEIGHT).any(|y| is_set(*x, y)))
        .expect("a glyph with a left edge has a right edge")
        + 1;
    let top = (0..GLYPH_HEIGHT)
        .find(|y| (0..GLYPH_WIDTH).any(|x| is_set(x, *y)))
        .expect("a nonempty glyph has a top edge");
    let bottom = (0..GLYPH_HEIGHT)
        .rfind(|y| (0..GLYPH_WIDTH).any(|x| is_set(x, *y)))
        .expect("a nonempty glyph has a bottom edge")
        + 1;
    let width = right - left;
    let height = bottom - top;
    let pixels = (top..bottom)
        .flat_map(|y| (left..right).map(move |x| is_set(x, y)))
        .collect();
    Ok(CroppedGlyph {
        pixels,
        width,
        height,
    })
}

#[cfg(test)]
#[path = "field_unit_callout_compositor_tests.rs"]
mod field_unit_callout_compositor_tests;
