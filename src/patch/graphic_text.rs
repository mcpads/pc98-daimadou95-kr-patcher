use anyhow::{Result, ensure};

use crate::font::rasterize_pc98_character;

#[derive(Clone, Copy)]
pub(super) struct Rect {
    pub left: usize,
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
}

impl Rect {
    pub const fn new(left: usize, top: usize, right: usize, bottom: usize) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn contains(self, x: usize, y: usize) -> bool {
        (self.left..self.right).contains(&x) && (self.top..self.bottom).contains(&y)
    }

    pub fn inset(self, amount: usize) -> Self {
        Self::new(
            self.left + amount,
            self.top + amount,
            self.right - amount,
            self.bottom - amount,
        )
    }
}

#[derive(Clone, Copy)]
pub(super) struct TextStyle {
    scale: usize,
    color: u8,
    outline: Option<(u8, isize)>,
    shadow: Option<(isize, isize, u8)>,
}

impl TextStyle {
    pub const fn plain(scale: usize, color: u8) -> Self {
        Self {
            scale,
            color,
            outline: None,
            shadow: None,
        }
    }

    pub const fn outlined(
        scale: usize,
        color: u8,
        outline_color: u8,
        outline_radius: isize,
        shadow: Option<(isize, isize, u8)>,
    ) -> Self {
        Self {
            scale,
            color,
            outline: Some((outline_color, outline_radius)),
            shadow,
        }
    }
}

pub(super) fn draw_centered_text(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    rect: Rect,
    text: &str,
    style: TextStyle,
) -> Result<()> {
    ensure!(
        pixels.len() == width * height,
        "graphic text canvas has {} pixels instead of {}",
        pixels.len(),
        width * height
    );
    ensure!(
        rect.right <= width && rect.bottom <= height,
        "graphic text rectangle exceeds its canvas"
    );
    let rendered = rasterize_text(text, style.scale)?;
    ensure!(
        rendered.width <= rect.right - rect.left - 8
            && rendered.height <= rect.bottom - rect.top - 8,
        "graphic text {text:?} does not fit its composition rectangle"
    );
    let x = rect.left + (rect.right - rect.left - rendered.width) / 2;
    let y = rect.top + (rect.bottom - rect.top - rendered.height) / 2;
    if let Some((dx, dy, color)) = style.shadow {
        blit_text(
            pixels,
            width,
            height,
            x as isize + dx,
            y as isize + dy,
            &rendered,
            color,
        )?;
    }
    if let Some((color, radius)) = style.outline {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx == 0 && dy == 0 {
                    continue;
                }
                blit_text(
                    pixels,
                    width,
                    height,
                    x as isize + dx,
                    y as isize + dy,
                    &rendered,
                    color,
                )?;
            }
        }
    }
    blit_text(
        pixels,
        width,
        height,
        x as isize,
        y as isize,
        &rendered,
        style.color,
    )
}

struct RasterizedText {
    width: usize,
    height: usize,
    pixels: Vec<bool>,
}

fn rasterize_text(text: &str, scale: usize) -> Result<RasterizedText> {
    ensure!(!text.is_empty(), "graphic text must not be empty");
    ensure!(scale > 0, "graphic text scale must be nonzero");
    let width = text.chars().count() * 16 * scale;
    let height = 16 * scale;
    let mut pixels = vec![false; width * height];
    for (character_index, character) in text.chars().enumerate() {
        let bitmap = rasterize_pc98_character(character)?;
        for source_y in 0..16 {
            for source_x in 0..16 {
                if bitmap[source_y * 2 + source_x / 8] & (1 << (7 - source_x % 8)) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let x = character_index * 16 * scale + source_x * scale + dx;
                        let y = source_y * scale + dy;
                        pixels[y * width + x] = true;
                    }
                }
            }
        }
    }
    Ok(RasterizedText {
        width,
        height,
        pixels,
    })
}

fn blit_text(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    origin_x: isize,
    origin_y: isize,
    rendered: &RasterizedText,
    color: u8,
) -> Result<()> {
    for y in 0..rendered.height {
        for x in 0..rendered.width {
            if !rendered.pixels[y * rendered.width + x] {
                continue;
            }
            let target_x = origin_x + x as isize;
            let target_y = origin_y + y as isize;
            ensure!(
                target_x >= 0
                    && target_y >= 0
                    && (target_x as usize) < width
                    && (target_y as usize) < height,
                "graphic glyph writes outside the canvas"
            );
            pixels[target_y as usize * width + target_x as usize] = color;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "graphic_text_tests.rs"]
mod graphic_text_tests;
