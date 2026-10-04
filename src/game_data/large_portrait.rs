use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::compile_lz::{decode_complete_compile_lz, encode_compile_lz};
use super::planar::{decode_planar_indices, encode_planar_indices};

pub(crate) const TILE_BANK_ENTRY_ID: usize = 101;
pub(crate) const TILE_MAP_ENTRY_ID: usize = 129;
pub(crate) const FRAME_COUNT: usize = 4;
pub(crate) const FRAME_WIDTH: usize = 128;
pub(crate) const FRAME_HEIGHT: usize = 128;
pub(crate) const TILE_WIDTH: usize = 16;
pub(crate) const TILE_HEIGHT: usize = 16;
pub(crate) const SOURCE_TILE_COUNT: usize = 128;
pub(crate) const TILE_BANK_CAPACITY: usize = 256;
pub(crate) const PLANE_COUNT: usize = 4;

const TILES_PER_ROW: usize = FRAME_WIDTH / TILE_WIDTH;
const TILES_PER_COLUMN: usize = FRAME_HEIGHT / TILE_HEIGHT;
const TILES_PER_FRAME: usize = TILES_PER_ROW * TILES_PER_COLUMN;
const PLANE_SIZE_PER_TILE: usize = TILE_WIDTH / 8 * TILE_HEIGHT;
const BYTES_PER_TILE: usize = PLANE_SIZE_PER_TILE * PLANE_COUNT;
const MAP_RECORD_SIZE: usize = 2 + TILES_PER_FRAME;
pub(crate) const DECODED_TILE_BANK_SIZE: usize = TILE_BANK_CAPACITY * BYTES_PER_TILE;
pub(crate) const DECODED_TILE_MAP_SIZE: usize = FRAME_COUNT * MAP_RECORD_SIZE;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct LargePortraitSet {
    pub packed_tile_bank_size: usize,
    pub packed_tile_map_size: usize,
    pub decoded_tile_bank: Vec<u8>,
    pub decoded_tile_maps: Vec<u8>,
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub used_tile_ids: BTreeSet<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EncodedLargePortraitSet {
    pub packed_tile_bank: Vec<u8>,
    pub packed_tile_maps: Vec<u8>,
    pub used_tile_count: usize,
}

pub(crate) fn decode_large_portrait_set(
    packed_tile_bank: &[u8],
    packed_tile_maps: &[u8],
) -> Result<LargePortraitSet> {
    let tile_stream = decode_complete_compile_lz(packed_tile_bank)?;
    let map_stream = decode_complete_compile_lz(packed_tile_maps)?;
    ensure!(
        tile_stream.output.len() % BYTES_PER_TILE == 0,
        "large portrait tile bank decoded to a partial tile: {} bytes",
        tile_stream.output.len()
    );
    let tile_count = tile_stream.output.len() / BYTES_PER_TILE;
    ensure!(
        [SOURCE_TILE_COUNT, TILE_BANK_CAPACITY].contains(&tile_count),
        "large portrait tile bank contains {tile_count} tiles instead of {SOURCE_TILE_COUNT} or {TILE_BANK_CAPACITY}"
    );
    ensure!(
        map_stream.output.len() == DECODED_TILE_MAP_SIZE,
        "large portrait map decoded to {} bytes instead of {}",
        map_stream.output.len(),
        DECODED_TILE_MAP_SIZE
    );

    let mut frames = Vec::with_capacity(FRAME_COUNT);
    let mut used_tile_ids = BTreeSet::new();
    for frame_index in 0..FRAME_COUNT {
        let start = frame_index * MAP_RECORD_SIZE;
        let record = &map_stream.output[start..start + MAP_RECORD_SIZE];
        ensure!(
            record[..2] == [TILES_PER_ROW as u8, TILES_PER_COLUMN as u8],
            "large portrait frame {frame_index} declares {}x{} tiles instead of {}x{}",
            record[0],
            record[1],
            TILES_PER_ROW,
            TILES_PER_COLUMN
        );
        let frame = decode_frame(
            &tile_stream.output,
            tile_count,
            &record[2..],
            &mut used_tile_ids,
        )
        .with_context(|| format!("decode large portrait frame {frame_index}"))?;
        frames.push(frame);
    }

    Ok(LargePortraitSet {
        packed_tile_bank_size: tile_stream.packed_size,
        packed_tile_map_size: map_stream.packed_size,
        decoded_tile_bank: tile_stream.output,
        decoded_tile_maps: map_stream.output,
        frames: frames
            .try_into()
            .expect("four decoded records produce four portrait frames"),
        used_tile_ids,
    })
}

pub(crate) fn encode_large_portrait_set(
    frames: &[Vec<u8>; FRAME_COUNT],
    padding_palette_index: u8,
) -> Result<EncodedLargePortraitSet> {
    ensure!(
        padding_palette_index < (1 << PLANE_COUNT),
        "large portrait padding index exceeds the four-plane palette"
    );
    for (frame_index, frame) in frames.iter().enumerate() {
        ensure!(
            frame.len() == FRAME_WIDTH * FRAME_HEIGHT,
            "large portrait frame {frame_index} has {} pixels instead of {}",
            frame.len(),
            FRAME_WIDTH * FRAME_HEIGHT
        );
        ensure!(
            frame.iter().all(|pixel| *pixel < (1 << PLANE_COUNT)),
            "large portrait frame {frame_index} contains a palette index above 15"
        );
    }

    let mut tiles_by_bytes = BTreeMap::<Vec<u8>, u8>::new();
    let mut tiles = Vec::<Vec<u8>>::new();
    let mut decoded_maps = Vec::with_capacity(DECODED_TILE_MAP_SIZE);
    for (frame_index, frame) in frames.iter().enumerate() {
        decoded_maps.extend_from_slice(&[TILES_PER_ROW as u8, TILES_PER_COLUMN as u8]);
        for tile_position in 0..TILES_PER_FRAME {
            let tile_pixels = extract_tile(frame, tile_position);
            let tile_bytes =
                encode_planar_indices(&tile_pixels, TILE_WIDTH, TILE_HEIGHT, PLANE_COUNT)?.concat();
            let tile_id = if let Some(&tile_id) = tiles_by_bytes.get(&tile_bytes) {
                tile_id
            } else {
                ensure!(
                    tiles.len() < TILE_BANK_CAPACITY,
                    "large portrait frames require more than {TILE_BANK_CAPACITY} unique tiles at frame {frame_index}, position {tile_position}"
                );
                let tile_id = u8::try_from(tiles.len())
                    .expect("the large portrait bank has at most 256 tiles");
                tiles_by_bytes.insert(tile_bytes.clone(), tile_id);
                tiles.push(tile_bytes);
                tile_id
            };
            decoded_maps.push(tile_id);
        }
    }
    let used_tile_count = tiles.len();

    let padding_pixels = vec![padding_palette_index; TILE_WIDTH * TILE_HEIGHT];
    let padding_tile =
        encode_planar_indices(&padding_pixels, TILE_WIDTH, TILE_HEIGHT, PLANE_COUNT)?.concat();
    while tiles.len() < TILE_BANK_CAPACITY {
        tiles.push(padding_tile.clone());
    }
    let decoded_tile_bank = tiles.concat();
    let packed_tile_bank = encode_compile_lz(&decoded_tile_bank)?;
    let packed_tile_maps = encode_compile_lz(&decoded_maps)?;

    let decoded = decode_large_portrait_set(&packed_tile_bank, &packed_tile_maps)?;
    ensure!(
        decoded.frames == *frames,
        "large portrait encoder failed its frame-pixel round trip"
    );
    ensure!(
        decoded.used_tile_ids.len() == used_tile_count,
        "large portrait encoder used {} mapped tiles after assigning {used_tile_count}",
        decoded.used_tile_ids.len()
    );

    Ok(EncodedLargePortraitSet {
        packed_tile_bank,
        packed_tile_maps,
        used_tile_count,
    })
}

fn decode_frame(
    decoded_tile_bank: &[u8],
    tile_count: usize,
    tile_ids: &[u8],
    used_tile_ids: &mut BTreeSet<u8>,
) -> Result<Vec<u8>> {
    ensure!(
        tile_ids.len() == TILES_PER_FRAME,
        "large portrait frame map has {} tile IDs instead of {TILES_PER_FRAME}",
        tile_ids.len()
    );
    let mut frame = vec![0; FRAME_WIDTH * FRAME_HEIGHT];
    for (tile_position, &tile_id) in tile_ids.iter().enumerate() {
        ensure!(
            usize::from(tile_id) < tile_count,
            "large portrait frame references tile {tile_id} outside 0..{tile_count}"
        );
        used_tile_ids.insert(tile_id);
        let start = usize::from(tile_id) * BYTES_PER_TILE;
        let tile = &decoded_tile_bank[start..start + BYTES_PER_TILE];
        let planes = tile
            .chunks_exact(PLANE_SIZE_PER_TILE)
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>();
        let tile_pixels = decode_planar_indices(&planes, TILE_WIDTH, TILE_HEIGHT)?;
        let tile_x = tile_position % TILES_PER_ROW * TILE_WIDTH;
        let tile_y = tile_position / TILES_PER_ROW * TILE_HEIGHT;
        for y in 0..TILE_HEIGHT {
            let source = &tile_pixels[y * TILE_WIDTH..(y + 1) * TILE_WIDTH];
            let target_start = (tile_y + y) * FRAME_WIDTH + tile_x;
            frame[target_start..target_start + TILE_WIDTH].copy_from_slice(source);
        }
    }
    Ok(frame)
}

fn extract_tile(frame: &[u8], tile_position: usize) -> Vec<u8> {
    let tile_x = tile_position % TILES_PER_ROW * TILE_WIDTH;
    let tile_y = tile_position / TILES_PER_ROW * TILE_HEIGHT;
    let mut tile = Vec::with_capacity(TILE_WIDTH * TILE_HEIGHT);
    for y in 0..TILE_HEIGHT {
        let start = (tile_y + y) * FRAME_WIDTH + tile_x;
        tile.extend_from_slice(&frame[start..start + TILE_WIDTH]);
    }
    tile
}

#[cfg(test)]
#[path = "large_portrait_tests.rs"]
mod large_portrait_tests;
