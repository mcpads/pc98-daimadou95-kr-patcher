use anyhow::{Result, ensure};

use crate::game_data::gcs::{SCREEN_HEIGHT, SCREEN_WIDTH};

use super::graphic_text::{Rect, TextStyle, draw_centered_text};

const BACKGROUND: u8 = 9;
const TEXT: u8 = 11;
const COMPACT_TITLE: Rect = Rect::new(248, 256, 392, 284);

#[derive(Debug, Eq, PartialEq)]
pub(super) struct CompactTitleCompositionReport {
    pub erased_source_pixel_count: usize,
}

pub(super) fn compose_compact_title(
    source: &[u8],
    output: &mut [u8],
    text: &str,
) -> Result<CompactTitleCompositionReport> {
    ensure!(
        source.len() == SCREEN_WIDTH * SCREEN_HEIGHT && output.len() == source.len(),
        "compact-title compositor requires two complete 640x400 indexed screens"
    );
    let mut erased_source_pixel_count = 0;
    for y in COMPACT_TITLE.top..COMPACT_TITLE.bottom {
        for x in COMPACT_TITLE.left..COMPACT_TITLE.right {
            let offset = y * SCREEN_WIDTH + x;
            ensure!(
                matches!(source[offset], BACKGROUND | TEXT),
                "compact Romanized title contains unexpected palette index {} at ({x}, {y})",
                source[offset]
            );
            if source[offset] == TEXT {
                erased_source_pixel_count += 1;
            }
            output[offset] = BACKGROUND;
        }
    }
    ensure!(
        erased_source_pixel_count > 0,
        "compact-title compositor found no source title pixels"
    );
    draw_centered_text(
        output,
        SCREEN_WIDTH,
        SCREEN_HEIGHT,
        COMPACT_TITLE,
        text,
        TextStyle::plain(1, TEXT),
    )?;
    Ok(CompactTitleCompositionReport {
        erased_source_pixel_count,
    })
}

pub(super) fn is_compact_title_pixel(x: usize, y: usize) -> bool {
    COMPACT_TITLE.contains(x, y)
}

#[cfg(test)]
#[path = "title_compact_label_tests.rs"]
mod title_compact_label_tests;
