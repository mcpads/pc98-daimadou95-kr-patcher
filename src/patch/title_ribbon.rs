use anyhow::{Result, ensure};

use super::graphic_text::Rect;
use super::title_imagegen_asset::HashBoundChromaAsset;

const RED: u8 = 0;
const BLACK: u8 = 4;
const LIGHT_BLUE: u8 = 6;
const YELLOW: u8 = 9;
const EXACT_TEXT: &str = "대마도전략 이야기 '95";
const ASSET_SHA256: &str = "26d49834c571e5f3d23229711bdcc2cbb048dee7c8195f748103e303dcb8ae16";
const ASSET_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/title/daimadou-title-ribbon.imagegen.png"
);
const ASSET: HashBoundChromaAsset = HashBoundChromaAsset::new(
    ASSET_PATH,
    ASSET_SHA256,
    "ImageGen title ribbon",
    classify_rgb,
);

pub(super) fn imagegen_title_ribbon_sha256() -> &'static str {
    ASSET_SHA256
}

pub(super) fn paint_imagegen_title_ribbon(
    pixels: &mut [u8],
    canvas_width: usize,
    canvas_height: usize,
    target: Rect,
    text: &str,
) -> Result<usize> {
    ensure!(
        text == EXACT_TEXT,
        "ImageGen title ribbon is bound to {EXACT_TEXT:?}, not {text:?}"
    );
    ASSET.paint(pixels, canvas_width, canvas_height, target)
}

fn classify_rgb([red, green, blue]: [u8; 3]) -> Option<u8> {
    let green_is_key =
        green > 96 && green > red.saturating_add(24) && green > blue.saturating_add(24);
    if green_is_key {
        None
    } else if blue > red.saturating_add(16) {
        Some(LIGHT_BLUE)
    } else if red > green.saturating_add(24) && red > blue.saturating_add(24) {
        Some(RED)
    } else if red < 112 && green < 112 && blue < 112 {
        Some(BLACK)
    } else {
        Some(YELLOW)
    }
}

#[cfg(test)]
#[path = "title_ribbon_tests.rs"]
mod title_ribbon_tests;
