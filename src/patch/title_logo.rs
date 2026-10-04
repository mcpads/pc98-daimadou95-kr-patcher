use anyhow::{Result, ensure};

use super::graphic_text::Rect;
use super::title_imagegen_asset::HashBoundChromaAsset;

const BLACK: u8 = 4;
const DARK_BLUE: u8 = 5;
const RED: u8 = 0;
const EXACT_TEXT: &str = "대마도전략 이야기 '95";
const ASSET_SHA256: &str = "b0dc12c96a15184dab7ebcb788918e0c80486f47f1564d6fc093ad3153c79722";
const ASSET_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/title/daimadou-title-logo.imagegen.png"
);
const ASSET: HashBoundChromaAsset = HashBoundChromaAsset::new(
    ASSET_PATH,
    ASSET_SHA256,
    "ImageGen title logo",
    classify_rgb,
);

pub(super) fn imagegen_title_logo_sha256() -> &'static str {
    ASSET_SHA256
}

pub(super) fn paint_imagegen_title_logo(
    pixels: &mut [u8],
    canvas_width: usize,
    canvas_height: usize,
    target: Rect,
    text: &str,
) -> Result<usize> {
    ensure!(
        text == EXACT_TEXT,
        "ImageGen title logo is bound to {EXACT_TEXT:?}, not {text:?}"
    );
    ASSET.paint(pixels, canvas_width, canvas_height, target)
}

fn classify_rgb([red, green, blue]: [u8; 3]) -> Option<u8> {
    let green_is_key =
        green > 96 && green > red.saturating_add(24) && green > blue.saturating_add(24);
    if green_is_key {
        None
    } else if red > blue.saturating_add(24) {
        Some(RED)
    } else if blue > red.saturating_add(16) {
        Some(DARK_BLUE)
    } else {
        Some(BLACK)
    }
}

#[cfg(test)]
#[path = "title_logo_tests.rs"]
mod title_logo_tests;
