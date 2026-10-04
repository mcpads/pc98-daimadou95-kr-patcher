use anyhow::{Context, Result, ensure};

use super::compile_lz::{decode_complete_compile_lz, encode_compile_lz};
use super::planar::{decode_planar_indices, encode_planar_indices};

pub(crate) const MADDAT_ENTRY_ID: usize = 153;
pub(crate) const WIDTH: usize = 288;
pub(crate) const HEIGHT: usize = 64;
pub(crate) const PLANE_COUNT: usize = 4;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct GameOverGraphic {
    pub packed_size: usize,
    pub planar_bytes: Vec<u8>,
    pub pixels: Vec<u8>,
}

pub(crate) fn decode_game_over_graphic(input: &[u8]) -> Result<GameOverGraphic> {
    let stream = decode_complete_compile_lz(input)?;
    let plane_size = (WIDTH / 8)
        .checked_mul(HEIGHT)
        .context("game-over plane size overflow")?;
    ensure!(
        stream.output.len() == plane_size * PLANE_COUNT,
        "game-over graphic has {} decoded bytes instead of {}",
        stream.output.len(),
        plane_size * PLANE_COUNT
    );
    let planes = stream
        .output
        .chunks_exact(plane_size)
        .map(<[u8]>::to_vec)
        .collect::<Vec<_>>();
    let pixels = decode_planar_indices(&planes, WIDTH, HEIGHT)?;
    Ok(GameOverGraphic {
        packed_size: stream.packed_size,
        planar_bytes: stream.output,
        pixels,
    })
}

pub(crate) fn encode_game_over_graphic(pixels: &[u8]) -> Result<Vec<u8>> {
    let planes = encode_planar_indices(pixels, WIDTH, HEIGHT, PLANE_COUNT)?;
    let planar_bytes = planes.concat();
    let packed = encode_compile_lz(&planar_bytes)?;
    let decoded = decode_game_over_graphic(&packed)?;
    ensure!(
        decoded.planar_bytes == planar_bytes && decoded.pixels == pixels,
        "game-over encoder failed its planar pixel round trip"
    );
    Ok(packed)
}

#[cfg(test)]
#[path = "game_over_graphic_tests.rs"]
mod game_over_graphic_tests;
