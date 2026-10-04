use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::compile_lz::{decode_complete_compile_lz, encode_compile_lz};
use super::planar::{decode_planar_indices, encode_planar_indices};

pub(crate) const TILE_MAP_ENTRY_ID: usize = 16;
pub(crate) const TILE_BANK_ENTRY_IDS: [usize; 2] = [38, 39];
pub(crate) const FRAME_COUNT: usize = 10;
pub(crate) const FIRST_BANK_FRAME_COUNT: usize = 4;
pub(crate) const FRAME_WIDTH: usize = 192;
pub(crate) const FRAME_HEIGHT: usize = 192;
pub(crate) const TILE_WIDTH: usize = 16;
pub(crate) const TILE_HEIGHT: usize = 16;
pub(crate) const TILE_BANK_CAPACITY: usize = 256;
pub(crate) const PLANE_COUNT: usize = 4;

const TILES_PER_ROW: usize = FRAME_WIDTH / TILE_WIDTH;
const TILES_PER_COLUMN: usize = FRAME_HEIGHT / TILE_HEIGHT;
const TILES_PER_FRAME: usize = TILES_PER_ROW * TILES_PER_COLUMN;
const PLANE_SIZE_PER_TILE: usize = TILE_WIDTH / 8 * TILE_HEIGHT;
const BYTES_PER_TILE: usize = PLANE_SIZE_PER_TILE * PLANE_COUNT;
const MAP_RECORD_SIZE: usize = 3 + TILES_PER_FRAME;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BattleSpriteSet {
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub used_tile_ids_by_bank: [BTreeSet<u8>; 2],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EncodedBattleSpriteSet {
    pub packed_tile_maps: Vec<u8>,
    pub packed_tile_banks: [Vec<u8>; 2],
    pub used_tile_counts: [usize; 2],
}

pub(crate) fn decode_battle_sprite_set(
    packed_tile_maps: &[u8],
    packed_tile_banks: [&[u8]; 2],
) -> Result<BattleSpriteSet> {
    let map_stream = decode_complete_compile_lz(packed_tile_maps)?;
    ensure!(
        map_stream.output.len() == FRAME_COUNT * MAP_RECORD_SIZE,
        "battle sprite maps decode to {} bytes instead of {}",
        map_stream.output.len(),
        FRAME_COUNT * MAP_RECORD_SIZE
    );
    let decoded_banks = packed_tile_banks
        .map(decode_complete_compile_lz)
        .into_iter()
        .collect::<Result<Vec<_>>>()?;
    for (bank_index, bank) in decoded_banks.iter().enumerate() {
        ensure!(
            bank.output.len() == TILE_BANK_CAPACITY * BYTES_PER_TILE,
            "battle sprite bank {bank_index} decodes to {} bytes instead of {}",
            bank.output.len(),
            TILE_BANK_CAPACITY * BYTES_PER_TILE
        );
    }

    let mut frames = Vec::with_capacity(FRAME_COUNT);
    let mut used_tile_ids_by_bank = std::array::from_fn(|_| BTreeSet::new());
    for frame_index in 0..FRAME_COUNT {
        let start = frame_index * MAP_RECORD_SIZE;
        let record = &map_stream.output[start..start + MAP_RECORD_SIZE];
        ensure!(
            record[..2] == [TILES_PER_ROW as u8, TILES_PER_COLUMN as u8],
            "battle sprite frame {frame_index} declares {}x{} tiles instead of {}x{}",
            record[0],
            record[1],
            TILES_PER_ROW,
            TILES_PER_COLUMN
        );
        let expected_bank = bank_for_frame(frame_index);
        ensure!(
            usize::from(record[2]) == expected_bank,
            "battle sprite frame {frame_index} selects bank {} instead of {expected_bank}",
            record[2]
        );
        frames.push(
            decode_frame(
                &decoded_banks[expected_bank].output,
                &record[3..],
                &mut used_tile_ids_by_bank[expected_bank],
            )
            .with_context(|| format!("decode battle sprite frame {frame_index}"))?,
        );
    }

    Ok(BattleSpriteSet {
        frames: frames
            .try_into()
            .expect("ten decoded records produce ten battle frames"),
        used_tile_ids_by_bank,
    })
}

pub(crate) fn encode_battle_sprite_set(
    frames: &[Vec<u8>; FRAME_COUNT],
    padding_palette_index: u8,
) -> Result<EncodedBattleSpriteSet> {
    ensure!(
        padding_palette_index < (1 << PLANE_COUNT),
        "battle sprite padding index exceeds the four-plane palette"
    );
    for (frame_index, frame) in frames.iter().enumerate() {
        ensure!(
            frame.len() == FRAME_WIDTH * FRAME_HEIGHT,
            "battle sprite frame {frame_index} has {} pixels instead of {}",
            frame.len(),
            FRAME_WIDTH * FRAME_HEIGHT
        );
        ensure!(
            frame.iter().all(|pixel| *pixel < (1 << PLANE_COUNT)),
            "battle sprite frame {frame_index} contains a palette index above 15"
        );
    }

    let required_tile_counts = count_unique_tiles_by_bank(frames)?;
    ensure!(
        required_tile_counts
            .iter()
            .all(|count| *count <= TILE_BANK_CAPACITY),
        "battle sprite banks require {required_tile_counts:?} unique tiles but each bank has {TILE_BANK_CAPACITY} slots"
    );

    let mut tiles_by_bank = [Vec::<Vec<u8>>::new(), Vec::<Vec<u8>>::new()];
    let mut tile_ids_by_bank = [
        BTreeMap::<Vec<u8>, u8>::new(),
        BTreeMap::<Vec<u8>, u8>::new(),
    ];
    let mut decoded_maps = Vec::with_capacity(FRAME_COUNT * MAP_RECORD_SIZE);
    for (frame_index, frame) in frames.iter().enumerate() {
        let bank = bank_for_frame(frame_index);
        decoded_maps.extend_from_slice(&[TILES_PER_ROW as u8, TILES_PER_COLUMN as u8, bank as u8]);
        for tile_position in 0..TILES_PER_FRAME {
            let tile_pixels = extract_tile(frame, tile_position);
            let tile_bytes =
                encode_planar_indices(&tile_pixels, TILE_WIDTH, TILE_HEIGHT, PLANE_COUNT)?.concat();
            let tile_id = if let Some(&tile_id) = tile_ids_by_bank[bank].get(&tile_bytes) {
                tile_id
            } else {
                ensure!(
                    tiles_by_bank[bank].len() < TILE_BANK_CAPACITY,
                    "battle sprite bank {bank} requires more than {TILE_BANK_CAPACITY} unique tiles at frame {frame_index}, position {tile_position}"
                );
                let tile_id = u8::try_from(tiles_by_bank[bank].len())
                    .expect("a battle sprite bank has at most 256 tiles");
                tile_ids_by_bank[bank].insert(tile_bytes.clone(), tile_id);
                tiles_by_bank[bank].push(tile_bytes);
                tile_id
            };
            decoded_maps.push(tile_id);
        }
    }
    let used_tile_counts = tiles_by_bank.each_ref().map(Vec::len);

    let padding_pixels = vec![padding_palette_index; TILE_WIDTH * TILE_HEIGHT];
    let padding_tile =
        encode_planar_indices(&padding_pixels, TILE_WIDTH, TILE_HEIGHT, PLANE_COUNT)?.concat();
    for tiles in &mut tiles_by_bank {
        while tiles.len() < TILE_BANK_CAPACITY {
            tiles.push(padding_tile.clone());
        }
    }
    let packed_tile_maps = encode_compile_lz(&decoded_maps)?;
    let packed_tile_banks = [
        encode_compile_lz(&tiles_by_bank[0].concat())?,
        encode_compile_lz(&tiles_by_bank[1].concat())?,
    ];

    let decoded = decode_battle_sprite_set(
        &packed_tile_maps,
        [&packed_tile_banks[0], &packed_tile_banks[1]],
    )?;
    ensure!(
        decoded.frames == *frames,
        "battle sprite encoder failed its frame-pixel round trip"
    );

    Ok(EncodedBattleSpriteSet {
        packed_tile_maps,
        packed_tile_banks,
        used_tile_counts,
    })
}

fn count_unique_tiles_by_bank(frames: &[Vec<u8>; FRAME_COUNT]) -> Result<[usize; 2]> {
    let mut unique_tiles = [BTreeSet::<Vec<u8>>::new(), BTreeSet::<Vec<u8>>::new()];
    for (frame_index, frame) in frames.iter().enumerate() {
        let bank = bank_for_frame(frame_index);
        for tile_position in 0..TILES_PER_FRAME {
            let tile_pixels = extract_tile(frame, tile_position);
            unique_tiles[bank].insert(
                encode_planar_indices(&tile_pixels, TILE_WIDTH, TILE_HEIGHT, PLANE_COUNT)?.concat(),
            );
        }
    }
    Ok(unique_tiles.each_ref().map(BTreeSet::len))
}

fn bank_for_frame(frame_index: usize) -> usize {
    usize::from(frame_index >= FIRST_BANK_FRAME_COUNT)
}

fn decode_frame(
    decoded_tile_bank: &[u8],
    tile_ids: &[u8],
    used_tile_ids: &mut BTreeSet<u8>,
) -> Result<Vec<u8>> {
    ensure!(
        tile_ids.len() == TILES_PER_FRAME,
        "battle sprite frame map has {} tile IDs instead of {TILES_PER_FRAME}",
        tile_ids.len()
    );
    let mut frame = vec![0; FRAME_WIDTH * FRAME_HEIGHT];
    for (tile_position, &tile_id) in tile_ids.iter().enumerate() {
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
#[path = "battle_sprite_tests.rs"]
mod battle_sprite_tests;
