use anyhow::{Context, Result, ensure};

use super::compile_lz::decode_complete_compile_lz;

pub(crate) const TILE_WIDTH: usize = 16;
pub(crate) const TILE_HEIGHT: usize = 16;
pub(crate) const MASK_PLANE_COUNT: usize = 1;
pub(crate) const COLOR_PLANE_COUNT: usize = 4;
pub(crate) const PLANE_SIZE: usize = TILE_WIDTH / 8 * TILE_HEIGHT;
pub(crate) const BYTES_PER_TILE: usize = PLANE_SIZE * (MASK_PLANE_COUNT + COLOR_PLANE_COUNT);

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MaskedSpriteBank {
    pub packed_size: usize,
    pub command_count: usize,
    pub decoded: Vec<u8>,
    pub tiles: Vec<MaskedSpriteTile>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MaskedSpriteTile {
    pub opaque: Vec<bool>,
    pub palette_indices: Vec<u8>,
}

pub(crate) fn decode_masked_sprite_bank(input: &[u8]) -> Result<MaskedSpriteBank> {
    let stream = decode_complete_compile_lz(input)?;
    ensure!(
        !stream.output.is_empty(),
        "masked sprite bank decodes to no tiles"
    );
    ensure!(
        stream.output.len().is_multiple_of(BYTES_PER_TILE),
        "masked sprite bank has {} decoded bytes, which is not a multiple of the {BYTES_PER_TILE}-byte mask-plus-color record",
        stream.output.len()
    );
    let tiles = stream
        .output
        .chunks_exact(BYTES_PER_TILE)
        .enumerate()
        .map(|(tile_id, record)| {
            decode_tile(record).with_context(|| format!("decode masked sprite tile {tile_id}"))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(MaskedSpriteBank {
        packed_size: stream.packed_size,
        command_count: stream.command_count,
        decoded: stream.output,
        tiles,
    })
}

pub(crate) fn encode_masked_sprite_tile(tile: &MaskedSpriteTile) -> Result<[u8; BYTES_PER_TILE]> {
    ensure!(
        tile.opaque.len() == TILE_WIDTH * TILE_HEIGHT,
        "masked sprite tile has {} opacity pixels instead of {}",
        tile.opaque.len(),
        TILE_WIDTH * TILE_HEIGHT
    );
    ensure!(
        tile.palette_indices.len() == TILE_WIDTH * TILE_HEIGHT,
        "masked sprite tile has {} palette pixels instead of {}",
        tile.palette_indices.len(),
        TILE_WIDTH * TILE_HEIGHT
    );

    let mut record = [0_u8; BYTES_PER_TILE];
    record[..PLANE_SIZE].fill(u8::MAX);
    for y in 0..TILE_HEIGHT {
        for x in 0..TILE_WIDTH {
            let pixel_offset = y * TILE_WIDTH + x;
            let palette_index = tile.palette_indices[pixel_offset];
            ensure!(
                palette_index < 1 << COLOR_PLANE_COUNT,
                "masked sprite pixel ({x}, {y}) uses palette index {palette_index} outside four planes"
            );
            let byte_offset = y * (TILE_WIDTH / 8) + x / 8;
            let bit = 1 << (7 - x % 8);
            if tile.opaque[pixel_offset] {
                record[byte_offset] &= !bit;
            }
            for plane_index in 0..COLOR_PLANE_COUNT {
                if palette_index & (1 << plane_index) != 0 {
                    record[PLANE_SIZE + plane_index * PLANE_SIZE + byte_offset] |= bit;
                }
            }
        }
    }
    Ok(record)
}

fn decode_tile(record: &[u8]) -> Result<MaskedSpriteTile> {
    ensure!(
        record.len() == BYTES_PER_TILE,
        "masked sprite tile has {} bytes instead of {BYTES_PER_TILE}",
        record.len()
    );
    let transparency_mask = &record[..PLANE_SIZE];
    let color_planes = record[PLANE_SIZE..]
        .chunks_exact(PLANE_SIZE)
        .collect::<Vec<_>>();
    ensure!(
        color_planes.len() == COLOR_PLANE_COUNT,
        "masked sprite tile has {} color planes instead of {COLOR_PLANE_COUNT}",
        color_planes.len()
    );

    let mut opaque = vec![false; TILE_WIDTH * TILE_HEIGHT];
    let mut palette_indices = vec![0; TILE_WIDTH * TILE_HEIGHT];
    for y in 0..TILE_HEIGHT {
        for x in 0..TILE_WIDTH {
            let byte_offset = y * (TILE_WIDTH / 8) + x / 8;
            let bit = 1 << (7 - x % 8);
            let pixel_offset = y * TILE_WIDTH + x;
            opaque[pixel_offset] = transparency_mask[byte_offset] & bit == 0;
            for (plane_index, plane) in color_planes.iter().enumerate() {
                if plane[byte_offset] & bit != 0 {
                    palette_indices[pixel_offset] |= 1 << plane_index;
                }
            }
        }
    }

    Ok(MaskedSpriteTile {
        opaque,
        palette_indices,
    })
}

#[cfg(test)]
#[path = "masked_sprite_tests.rs"]
mod masked_sprite_tests;
