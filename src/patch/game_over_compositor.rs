use std::cmp::Reverse;

use anyhow::{Result, ensure};

use crate::game_data::game_over_graphic::{HEIGHT, WIDTH};

use super::graphic_text::{Rect, TextStyle, draw_centered_text};

const CANVAS: Rect = Rect::new(0, 0, WIDTH, HEIGHT);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GameOverPalette {
    pub background: u8,
    pub primary: u8,
    pub outline: Option<u8>,
    pub shadow: Option<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct GameOverComposition {
    pub pixels: Vec<u8>,
    pub palette: GameOverPalette,
}

pub(super) fn compose_game_over_graphic(
    source: &[u8],
    korean_text: &str,
) -> Result<GameOverComposition> {
    ensure!(
        source.len() == WIDTH * HEIGHT,
        "game-over compositor received {} pixels instead of {}",
        source.len(),
        WIDTH * HEIGHT
    );
    let palette = derive_palette(source)?;
    let mut pixels = vec![palette.background; source.len()];
    let style = if let Some(outline) = palette.outline {
        TextStyle::outlined(
            3,
            palette.primary,
            outline,
            1,
            palette.shadow.map(|color| (2, 2, color)),
        )
    } else {
        TextStyle::plain(3, palette.primary)
    };
    draw_centered_text(&mut pixels, WIDTH, HEIGHT, CANVAS, korean_text, style)?;
    ensure!(
        pixels != source,
        "game-over compositor made no pixel changes"
    );
    Ok(GameOverComposition { pixels, palette })
}

fn derive_palette(source: &[u8]) -> Result<GameOverPalette> {
    let corners = [
        source[0],
        source[WIDTH - 1],
        source[(HEIGHT - 1) * WIDTH],
        source[HEIGHT * WIDTH - 1],
    ];
    ensure!(
        corners.iter().all(|color| *color == corners[0]),
        "game-over source corners do not agree on a background color"
    );
    let background = corners[0];
    let mut counts = [0_usize; 16];
    for color in source {
        ensure!(
            *color < 16,
            "game-over source palette index exceeds four planes"
        );
        counts[usize::from(*color)] += 1;
    }
    let mut ink = (0_u8..16)
        .filter(|color| *color != background && counts[usize::from(*color)] > 0)
        .collect::<Vec<_>>();
    ink.sort_by_key(|color| (Reverse(counts[usize::from(*color)]), *color));
    let primary = *ink
        .first()
        .ok_or_else(|| anyhow::anyhow!("game-over source contains no text colors"))?;
    Ok(GameOverPalette {
        background,
        primary,
        outline: ink.get(1).copied(),
        shadow: ink.get(2).copied(),
    })
}

#[cfg(test)]
#[path = "game_over_compositor_tests.rs"]
mod game_over_compositor_tests;
