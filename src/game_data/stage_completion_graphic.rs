use anyhow::{Context, Result, ensure};

use super::compile_lz::{decode_complete_compile_lz, encode_compile_lz};
use super::planar::{decode_planar_indices, encode_planar_indices};

pub(crate) const MADDAT_ENTRY_ID: usize = 152;
pub(crate) const WIDTH: usize = 48;
pub(crate) const HEIGHT: usize = 32;
pub(crate) const PLANE_COUNT: usize = 4;
pub(crate) const TILE_WIDTH: usize = 16;
pub(crate) const TILE_HEIGHT: usize = 16;
pub(crate) const TILE_IDS: [usize; 6] = [120, 121, 122, 123, 124, 125];

const TILE_COUNT: usize = 128;
const TILE_COLUMNS: usize = 3;
const PLANE_SIZE_PER_TILE: usize = TILE_WIDTH / 8 * TILE_HEIGHT;
const BYTES_PER_TILE: usize = PLANE_SIZE_PER_TILE * PLANE_COUNT;
const DECODED_SIZE: usize = TILE_COUNT * BYTES_PER_TILE;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct StageCompletionGraphic {
    pub packed_size: usize,
    pub decoded_tile_bank: Vec<u8>,
    pub pixels: Vec<u8>,
}

pub(crate) fn decode_stage_completion_graphic(input: &[u8]) -> Result<StageCompletionGraphic> {
    let stream = decode_complete_compile_lz(input)?;
    ensure!(
        stream.output.len() == DECODED_SIZE,
        "stage-completion tile bank has {} decoded bytes instead of {DECODED_SIZE}",
        stream.output.len()
    );
    let pixels = decode_surface(&stream.output)?;
    Ok(StageCompletionGraphic {
        packed_size: stream.packed_size,
        decoded_tile_bank: stream.output,
        pixels,
    })
}

pub(crate) fn encode_stage_completion_graphic(
    source: &StageCompletionGraphic,
    pixels: &[u8],
) -> Result<Vec<u8>> {
    ensure!(
        source.decoded_tile_bank.len() == DECODED_SIZE,
        "stage-completion source tile bank has {} decoded bytes instead of {DECODED_SIZE}",
        source.decoded_tile_bank.len()
    );
    ensure!(
        pixels.len() == WIDTH * HEIGHT,
        "stage-completion replacement has {} pixels instead of {}",
        pixels.len(),
        WIDTH * HEIGHT
    );

    let mut decoded_tile_bank = source.decoded_tile_bank.clone();
    for (tile_position, tile_id) in TILE_IDS.into_iter().enumerate() {
        let tile_pixels = extract_tile(pixels, tile_position);
        let encoded =
            encode_planar_indices(&tile_pixels, TILE_WIDTH, TILE_HEIGHT, PLANE_COUNT)?.concat();
        let start = tile_id
            .checked_mul(BYTES_PER_TILE)
            .context("stage-completion tile offset overflow")?;
        decoded_tile_bank[start..start + BYTES_PER_TILE].copy_from_slice(&encoded);
    }

    for tile_id in 0..TILE_COUNT {
        if TILE_IDS.contains(&tile_id) {
            continue;
        }
        let start = tile_id * BYTES_PER_TILE;
        ensure!(
            decoded_tile_bank[start..start + BYTES_PER_TILE]
                == source.decoded_tile_bank[start..start + BYTES_PER_TILE],
            "stage-completion encoder changed protected tile {tile_id}"
        );
    }

    let packed = encode_compile_lz(&decoded_tile_bank)?;
    let decoded = decode_stage_completion_graphic(&packed)?;
    ensure!(
        decoded.decoded_tile_bank == decoded_tile_bank && decoded.pixels == pixels,
        "stage-completion encoder failed its tile-bank pixel round trip"
    );
    Ok(packed)
}

fn decode_surface(decoded_tile_bank: &[u8]) -> Result<Vec<u8>> {
    let mut pixels = vec![0; WIDTH * HEIGHT];
    for (tile_position, tile_id) in TILE_IDS.into_iter().enumerate() {
        let start = tile_id
            .checked_mul(BYTES_PER_TILE)
            .context("stage-completion tile offset overflow")?;
        let tile = decoded_tile_bank
            .get(start..start + BYTES_PER_TILE)
            .with_context(|| format!("stage-completion tile {tile_id} is truncated"))?;
        let planes = tile
            .chunks_exact(PLANE_SIZE_PER_TILE)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let tile_pixels = decode_planar_indices(&planes, TILE_WIDTH, TILE_HEIGHT)?;
        let tile_x = tile_position % TILE_COLUMNS * TILE_WIDTH;
        let tile_y = tile_position / TILE_COLUMNS * TILE_HEIGHT;
        for y in 0..TILE_HEIGHT {
            let source = &tile_pixels[y * TILE_WIDTH..(y + 1) * TILE_WIDTH];
            let target_start = (tile_y + y) * WIDTH + tile_x;
            pixels[target_start..target_start + TILE_WIDTH].copy_from_slice(source);
        }
    }
    Ok(pixels)
}

fn extract_tile(pixels: &[u8], tile_position: usize) -> Vec<u8> {
    let tile_x = tile_position % TILE_COLUMNS * TILE_WIDTH;
    let tile_y = tile_position / TILE_COLUMNS * TILE_HEIGHT;
    let mut tile = Vec::with_capacity(TILE_WIDTH * TILE_HEIGHT);
    for y in 0..TILE_HEIGHT {
        let start = (tile_y + y) * WIDTH + tile_x;
        tile.extend_from_slice(&pixels[start..start + TILE_WIDTH]);
    }
    tile
}

#[cfg(test)]
#[path = "stage_completion_graphic_tests.rs"]
mod stage_completion_graphic_tests;
