use anyhow::{Result, ensure};

use crate::game_data::gcs::{SCREEN_HEIGHT, SCREEN_WIDTH};

use super::graphic_text::Rect;
use super::title_compact_label::{
    CompactTitleCompositionReport, compose_compact_title, is_compact_title_pixel,
};
use super::title_logo::paint_imagegen_title_logo;
use super::title_ribbon::paint_imagegen_title_ribbon;

const BLACK: u8 = 4;
const RED: u8 = 0;
const YELLOW: u8 = 9;

const MAIN_EDITABLE: Rect = Rect::new(56, 104, 584, 256);
const SUBTITLE_EDITABLE: Rect = Rect::new(184, 32, 528, 120);
const MAIN_LOGO: Rect = Rect::new(64, 112, 576, 244);

const SUBTITLE_OUTER: [(usize, usize); 7] = [
    (184, 56),
    (480, 32),
    (520, 44),
    (528, 80),
    (504, 100),
    (224, 120),
    (184, 104),
];
const SUBTITLE_MIDDLE: [(usize, usize); 7] = [
    (192, 59),
    (480, 37),
    (514, 48),
    (521, 77),
    (500, 94),
    (225, 114),
    (192, 100),
];
const SUBTITLE_INNER: [(usize, usize); 7] = [
    (199, 62),
    (479, 42),
    (508, 52),
    (514, 74),
    (497, 89),
    (226, 108),
    (199, 97),
];

#[derive(Debug)]
pub(super) struct TitleScreenComposition {
    pub pixels: Vec<u8>,
    pub compact_title: CompactTitleCompositionReport,
}

pub(super) fn compose_title_screen(
    source: &[u8],
    main_text: &str,
    subtitle_text: &str,
    compact_title_text: &str,
) -> Result<TitleScreenComposition> {
    ensure!(
        source.len() == SCREEN_WIDTH * SCREEN_HEIGHT,
        "title compositor received {} pixels instead of {}",
        source.len(),
        SCREEN_WIDTH * SCREEN_HEIGHT
    );
    let mut pixels = source.to_vec();

    fill_polygon(&mut pixels, &SUBTITLE_OUTER, BLACK);
    fill_polygon(&mut pixels, &SUBTITLE_MIDDLE, RED);
    fill_polygon(&mut pixels, &SUBTITLE_INNER, YELLOW);
    paint_imagegen_title_ribbon(
        &mut pixels,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        SUBTITLE_EDITABLE,
        subtitle_text,
    )?;

    fill_jagged_panel(&mut pixels, MAIN_EDITABLE, BLACK);
    fill_jagged_panel(&mut pixels, MAIN_EDITABLE.inset(3), RED);
    fill_jagged_panel(&mut pixels, MAIN_EDITABLE.inset(7), YELLOW);
    paint_imagegen_title_logo(
        &mut pixels,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        MAIN_LOGO,
        main_text,
    )?;

    let compact_title = compose_compact_title(source, &mut pixels, compact_title_text)?;

    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if source[y * SCREEN_WIDTH + x] != pixels[y * SCREEN_WIDTH + x] {
                ensure!(
                    MAIN_EDITABLE.contains(x, y)
                        || SUBTITLE_EDITABLE.contains(x, y)
                        || is_compact_title_pixel(x, y),
                    "title compositor changed protected pixel ({x}, {y})"
                );
            }
        }
    }
    Ok(TitleScreenComposition {
        pixels,
        compact_title,
    })
}

fn fill_jagged_panel(pixels: &mut [u8], rect: Rect, color: u8) {
    for y in rect.top..rect.bottom {
        for x in rect.left..rect.right {
            let horizontal_notch = if ((x - rect.left) / 16).is_multiple_of(2) {
                0
            } else {
                8
            };
            let vertical_notch = if ((y - rect.top) / 16).is_multiple_of(2) {
                0
            } else {
                8
            };
            if y >= rect.top + horizontal_notch
                && y < rect.bottom - horizontal_notch
                && x >= rect.left + vertical_notch
                && x < rect.right - vertical_notch
            {
                pixels[y * SCREEN_WIDTH + x] = color;
            }
        }
    }
}

fn fill_polygon(pixels: &mut [u8], points: &[(usize, usize)], color: u8) {
    let left = points.iter().map(|(x, _)| *x).min().unwrap_or(0);
    let right = points.iter().map(|(x, _)| *x).max().unwrap_or(0);
    let top = points.iter().map(|(_, y)| *y).min().unwrap_or(0);
    let bottom = points.iter().map(|(_, y)| *y).max().unwrap_or(0);
    for y in top..bottom {
        for x in left..right {
            if point_is_inside_polygon(x, y, points) {
                pixels[y * SCREEN_WIDTH + x] = color;
            }
        }
    }
}

fn point_is_inside_polygon(x: usize, y: usize, points: &[(usize, usize)]) -> bool {
    let x = x as f64 + 0.5;
    let y = y as f64 + 0.5;
    let mut inside = false;
    let mut previous = points[points.len() - 1];
    for &current in points {
        let (x1, y1) = (previous.0 as f64, previous.1 as f64);
        let (x2, y2) = (current.0 as f64, current.1 as f64);
        if (y1 > y) != (y2 > y) && x < (x2 - x1) * (y - y1) / (y2 - y1) + x1 {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[cfg(test)]
#[path = "title_compositor_tests.rs"]
mod title_compositor_tests;
