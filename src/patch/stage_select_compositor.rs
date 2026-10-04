use anyhow::{Context, Result, ensure};

use crate::game_data::gcs::{SCREEN_HEIGHT, SCREEN_WIDTH};

use super::graphic_text::{Rect, TextStyle, draw_centered_text};

const TOP_LABEL: Rect = Rect::new(32, 272, 240, 328);
const BOTTOM_LABEL: Rect = Rect::new(32, 328, 240, 384);
const BACKGROUND_PERIOD: usize = 32;
const PRIMARY: u8 = 1;
const OUTLINE: u8 = 7;
const SOURCE_TEXT_COLORS: [u8; 2] = [PRIMARY, OUTLINE];

pub(super) const EXPECTED_SOURCE_LABEL_PIXEL_COUNT: usize = 11_138;

pub(super) struct StageSelectComposition {
    pub pixels: Vec<u8>,
    pub erased_source_label_pixel_count: usize,
}

pub(super) fn compose_stage_select_graphic(
    source: &[u8],
    korean_text: &str,
) -> Result<StageSelectComposition> {
    ensure!(
        source.len() == SCREEN_WIDTH * SCREEN_HEIGHT,
        "stage-select compositor received {} pixels instead of {}",
        source.len(),
        SCREEN_WIDTH * SCREEN_HEIGHT
    );
    let mut words = korean_text.split_whitespace();
    let top_text = words
        .next()
        .context("stage-select translation has no upper label")?;
    let bottom_text = words
        .next()
        .context("stage-select translation has no lower label")?;
    ensure!(
        words.next().is_none(),
        "stage-select translation must contain exactly two label groups"
    );

    let mut pixels = source.to_vec();
    let erased_source_label_pixel_count = erase_source_label(source, &mut pixels, TOP_LABEL)?
        + erase_source_label(source, &mut pixels, BOTTOM_LABEL)?;
    ensure!(
        erased_source_label_pixel_count > 0,
        "stage-select source label mask is empty"
    );

    let style = TextStyle::outlined(3, PRIMARY, OUTLINE, 1, None);
    draw_centered_text(
        &mut pixels,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        TOP_LABEL,
        top_text,
        style,
    )?;
    draw_centered_text(
        &mut pixels,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        BOTTOM_LABEL,
        bottom_text,
        style,
    )?;

    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if source[y * SCREEN_WIDTH + x] != pixels[y * SCREEN_WIDTH + x] {
                ensure!(
                    TOP_LABEL.contains(x, y) || BOTTOM_LABEL.contains(x, y),
                    "stage-select compositor changed protected pixel ({x}, {y})"
                );
            }
        }
    }
    ensure!(pixels != source, "stage-select compositor made no changes");

    Ok(StageSelectComposition {
        pixels,
        erased_source_label_pixel_count,
    })
}

fn erase_source_label(source: &[u8], output: &mut [u8], rect: Rect) -> Result<usize> {
    let mut erased = 0;
    for y in rect.top..rect.bottom {
        for x in rect.left..rect.right {
            let offset = y * SCREEN_WIDTH + x;
            if !SOURCE_TEXT_COLORS.contains(&source[offset]) {
                continue;
            }
            output[offset] = background_sample(source, rect, x, y).with_context(|| {
                format!("stage-select background cannot be reconstructed at ({x}, {y})")
            })?;
            erased += 1;
        }
    }
    ensure!(
        (rect.top..rect.bottom).all(|y| (rect.left..rect.right)
            .all(|x| { !SOURCE_TEXT_COLORS.contains(&output[y * SCREEN_WIDTH + x]) })),
        "stage-select source label was not fully erased"
    );
    Ok(erased)
}

fn background_sample(source: &[u8], rect: Rect, x: usize, y: usize) -> Option<u8> {
    let horizontal_steps = (rect.right - rect.left).div_ceil(BACKGROUND_PERIOD);
    for step in 1..=horizontal_steps {
        let distance = step * BACKGROUND_PERIOD;
        for sample_x in [x.checked_sub(distance), x.checked_add(distance)]
            .into_iter()
            .flatten()
        {
            if rect.contains(sample_x, y) {
                let color = source[y * SCREEN_WIDTH + sample_x];
                if !SOURCE_TEXT_COLORS.contains(&color) {
                    return Some(color);
                }
            }
        }
    }

    let vertical_steps = rect.bottom - rect.top;
    for distance in 1..vertical_steps {
        for sample_y in [y.checked_sub(distance), y.checked_add(distance)]
            .into_iter()
            .flatten()
        {
            if rect.contains(x, sample_y) {
                let color = source[sample_y * SCREEN_WIDTH + x];
                if !SOURCE_TEXT_COLORS.contains(&color) {
                    return Some(color);
                }
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "stage_select_compositor_tests.rs"]
mod stage_select_compositor_tests;
