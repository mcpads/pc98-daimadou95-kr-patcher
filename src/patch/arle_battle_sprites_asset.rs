use std::io::Cursor;

use anyhow::{Context, Result, ensure};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::battle_sprite::{FRAME_COUNT, FRAME_HEIGHT, FRAME_WIDTH};

use super::arle_large_portrait_asset::RUNTIME_PALETTE;

pub(super) const BACKGROUND_PALETTE_INDEX: u8 = 0;

const SHEET_COLUMNS: usize = 5;
const SHEET_ROWS: usize = 2;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ArleBattleSpritesAsset {
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub sha256: String,
}

pub(super) fn load_arle_battle_sprites_asset(
    input: &ResolvedArleAsset,
) -> Result<ArleBattleSpritesAsset> {
    let rgb = decode_rgb_png(input.bytes())?;
    let sheet_width = FRAME_WIDTH * SHEET_COLUMNS;
    let sheet_height = FRAME_HEIGHT * SHEET_ROWS;
    ensure!(
        rgb.len() == sheet_width * sheet_height * 3,
        "Arle battle sprite RGB population is incomplete"
    );
    let mut frames = std::array::from_fn(|_| vec![0; FRAME_WIDTH * FRAME_HEIGHT]);
    for (frame_index, frame) in frames.iter_mut().enumerate() {
        let panel_left = frame_index % SHEET_COLUMNS * FRAME_WIDTH;
        let panel_top = frame_index / SHEET_COLUMNS * FRAME_HEIGHT;
        for y in 0..FRAME_HEIGHT {
            for x in 0..FRAME_WIDTH {
                let sheet_x = panel_left + x;
                let sheet_y = panel_top + y;
                let offset = (sheet_y * sheet_width + sheet_x) * 3;
                let color = [rgb[offset], rgb[offset + 1], rgb[offset + 2]];
                frame[y * FRAME_WIDTH + x] = palette_index(color).with_context(|| {
                    format!(
                        "Arle battle frame {frame_index} pixel ({x}, {y}) uses undeclared RGB {color:?}"
                    )
                })?;
            }
        }
    }
    validate_frame_contract(&frames)?;

    Ok(ArleBattleSpritesAsset {
        frames,
        sha256: input.sha256().to_owned(),
    })
}

fn decode_rgb_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().context("read Arle battle sprite PNG")?;
    let info = reader.info();
    ensure!(
        info.width as usize == FRAME_WIDTH * SHEET_COLUMNS
            && info.height as usize == FRAME_HEIGHT * SHEET_ROWS,
        "Arle battle sprite PNG is {}x{} instead of {}x{}",
        info.width,
        info.height,
        FRAME_WIDTH * SHEET_COLUMNS,
        FRAME_HEIGHT * SHEET_ROWS
    );
    let mut buffer = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut buffer)
        .context("decode Arle battle sprite PNG")?;
    let source = &buffer[..output.buffer_size()];
    let pixel_count = output.width as usize * output.height as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    match output.color_type {
        png::ColorType::Rgb => rgb.extend_from_slice(source),
        png::ColorType::Rgba => {
            for pixel in source.chunks_exact(4) {
                ensure!(
                    pixel[3] == u8::MAX,
                    "Arle battle sprite PNG must be fully opaque"
                );
                rgb.extend_from_slice(&pixel[..3]);
            }
        }
        other => anyhow::bail!("Arle battle sprite PNG decoded as unsupported {other:?}"),
    }
    ensure!(
        rgb.len() == pixel_count * 3,
        "Arle battle sprite PNG has an incomplete RGB buffer"
    );
    Ok(rgb)
}

fn palette_index(color: [u8; 3]) -> Result<u8> {
    RUNTIME_PALETTE
        .iter()
        .position(|candidate| *candidate == color)
        .map(|index| u8::try_from(index).expect("the runtime palette has 16 entries"))
        .context("RGB is not in the verified battle runtime palette")
}

fn validate_frame_contract(frames: &[Vec<u8>; FRAME_COUNT]) -> Result<()> {
    for (frame_index, frame) in frames.iter().enumerate() {
        ensure!(
            frame.len() == FRAME_WIDTH * FRAME_HEIGHT,
            "Arle battle frame {frame_index} has the wrong pixel population"
        );
        ensure!(
            frame.contains(&BACKGROUND_PALETTE_INDEX),
            "Arle battle frame {frame_index} has no black background"
        );
        ensure!(
            frame.iter().any(|pixel| *pixel != BACKGROUND_PALETTE_INDEX),
            "Arle battle frame {frame_index} has no foreground"
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "arle_battle_sprites_asset_tests.rs"]
mod arle_battle_sprites_asset_tests;
