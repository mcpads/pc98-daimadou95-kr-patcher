use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::compile_lz::{decode_complete_compile_lz, encode_compile_lz};
use super::planar::{decode_planar_indices, encode_planar_indices};

pub(crate) const TILE_BANK_ENTRY_ID: usize = 73;
pub(crate) const FRAME_COUNT: usize = 4;
pub(crate) const FRAME_WIDTH: usize = 64;
pub(crate) const FRAME_HEIGHT: usize = 64;
pub(crate) const TILE_WIDTH: usize = 16;
pub(crate) const TILE_HEIGHT: usize = 16;
pub(crate) const SOURCE_TILE_COUNT: usize = 32;
pub(crate) const TILE_BANK_CAPACITY: usize = 64;
pub(crate) const PLANE_COUNT: usize = 4;
pub(crate) const MAP_RECORD_SIZE: usize = 18;

const TILES_PER_ROW: usize = FRAME_WIDTH / TILE_WIDTH;
const TILES_PER_COLUMN: usize = FRAME_HEIGHT / TILE_HEIGHT;
const TILES_PER_FRAME: usize = TILES_PER_ROW * TILES_PER_COLUMN;
const PLANE_SIZE_PER_TILE: usize = TILE_WIDTH / 8 * TILE_HEIGHT;
const BYTES_PER_TILE: usize = PLANE_SIZE_PER_TILE * PLANE_COUNT;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SmallStatusSet {
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub used_tile_ids: BTreeSet<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EncodedSmallStatusSet {
    pub packed_tile_bank: Vec<u8>,
    pub map_records: [Vec<u8>; FRAME_COUNT],
    pub used_tile_count: usize,
}

pub(crate) fn decode_small_status_set(
    packed_tile_bank: &[u8],
    map_records: [&[u8]; FRAME_COUNT],
) -> Result<SmallStatusSet> {
    let tile_stream = decode_complete_compile_lz(packed_tile_bank)?;
    ensure!(
        tile_stream.output.len() % BYTES_PER_TILE == 0,
        "small status tile bank decoded to a partial tile: {} bytes",
        tile_stream.output.len()
    );
    let tile_count = tile_stream.output.len() / BYTES_PER_TILE;
    ensure!(
        [SOURCE_TILE_COUNT, TILE_BANK_CAPACITY].contains(&tile_count),
        "small status tile bank contains {tile_count} tiles instead of {SOURCE_TILE_COUNT} or {TILE_BANK_CAPACITY}"
    );

    let mut frames = Vec::with_capacity(FRAME_COUNT);
    let mut used_tile_ids = BTreeSet::new();
    for (frame_index, record) in map_records.into_iter().enumerate() {
        ensure!(
            record.len() == MAP_RECORD_SIZE,
            "small status frame {frame_index} map has {} bytes instead of {MAP_RECORD_SIZE}",
            record.len()
        );
        ensure!(
            record[..2] == [TILES_PER_ROW as u8, TILES_PER_COLUMN as u8],
            "small status frame {frame_index} declares {}x{} tiles instead of {}x{}",
            record[0],
            record[1],
            TILES_PER_ROW,
            TILES_PER_COLUMN
        );
        frames.push(
            decode_frame(
                &tile_stream.output,
                tile_count,
                &record[2..],
                &mut used_tile_ids,
            )
            .with_context(|| format!("decode small status frame {frame_index}"))?,
        );
    }

    Ok(SmallStatusSet {
        frames: frames
            .try_into()
            .expect("four decoded records produce four status frames"),
        used_tile_ids,
    })
}

pub(crate) fn encode_small_status_set(
    frames: &[Vec<u8>; FRAME_COUNT],
    padding_palette_index: u8,
) -> Result<EncodedSmallStatusSet> {
    ensure!(
        padding_palette_index < (1 << PLANE_COUNT),
        "small status padding index exceeds the four-plane palette"
    );
    for (frame_index, frame) in frames.iter().enumerate() {
        ensure!(
            frame.len() == FRAME_WIDTH * FRAME_HEIGHT,
            "small status frame {frame_index} has {} pixels instead of {}",
            frame.len(),
            FRAME_WIDTH * FRAME_HEIGHT
        );
        ensure!(
            frame.iter().all(|pixel| *pixel < (1 << PLANE_COUNT)),
            "small status frame {frame_index} contains a palette index above 15"
        );
    }

    let mut tiles_by_bytes = BTreeMap::<Vec<u8>, u8>::new();
    let mut tiles = Vec::<Vec<u8>>::new();
    let mut map_records = Vec::with_capacity(FRAME_COUNT);
    for (frame_index, frame) in frames.iter().enumerate() {
        let mut record = vec![TILES_PER_ROW as u8, TILES_PER_COLUMN as u8];
        for tile_position in 0..TILES_PER_FRAME {
            let tile_pixels = extract_tile(frame, tile_position);
            let tile_bytes =
                encode_planar_indices(&tile_pixels, TILE_WIDTH, TILE_HEIGHT, PLANE_COUNT)?.concat();
            let tile_id = if let Some(&tile_id) = tiles_by_bytes.get(&tile_bytes) {
                tile_id
            } else {
                ensure!(
                    tiles.len() < TILE_BANK_CAPACITY,
                    "small status frames require more than {TILE_BANK_CAPACITY} unique tiles at frame {frame_index}, position {tile_position}"
                );
                let tile_id =
                    u8::try_from(tiles.len()).expect("the small status bank has at most 64 tiles");
                tiles_by_bytes.insert(tile_bytes.clone(), tile_id);
                tiles.push(tile_bytes);
                tile_id
            };
            record.push(tile_id);
        }
        map_records.push(record);
    }
    let used_tile_count = tiles.len();

    let padding_pixels = vec![padding_palette_index; TILE_WIDTH * TILE_HEIGHT];
    let padding_tile =
        encode_planar_indices(&padding_pixels, TILE_WIDTH, TILE_HEIGHT, PLANE_COUNT)?.concat();
    while tiles.len() < TILE_BANK_CAPACITY {
        tiles.push(padding_tile.clone());
    }
    let packed_tile_bank = encode_compile_lz(&tiles.concat())?;
    let map_records: [Vec<u8>; FRAME_COUNT] = map_records
        .try_into()
        .expect("four frames produce four status maps");

    let decoded = decode_small_status_set(
        &packed_tile_bank,
        std::array::from_fn(|index| map_records[index].as_slice()),
    )?;
    ensure!(
        decoded.frames == *frames,
        "small status encoder failed its frame-pixel round trip"
    );

    Ok(EncodedSmallStatusSet {
        packed_tile_bank,
        map_records,
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
        "small status frame map has {} tile IDs instead of {TILES_PER_FRAME}",
        tile_ids.len()
    );
    let mut frame = vec![0; FRAME_WIDTH * FRAME_HEIGHT];
    for (tile_position, &tile_id) in tile_ids.iter().enumerate() {
        ensure!(
            usize::from(tile_id) < tile_count,
            "small status frame references tile {tile_id} outside 0..{tile_count}"
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
#[path = "small_status_tests.rs"]
mod small_status_tests;
