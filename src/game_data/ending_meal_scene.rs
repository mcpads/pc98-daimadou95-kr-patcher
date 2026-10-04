use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::compile_lz::{decode_complete_compile_lz, encode_compile_lz};

pub(crate) const MADDAT_ENTRY_ID: usize = 145;
pub(crate) const FRAME_COUNT: usize = 4;
pub(crate) const FRAME_WIDTH: usize = 256;
pub(crate) const FRAME_HEIGHT: usize = 176;
pub(crate) const TILE_WIDTH: usize = 16;
pub(crate) const TILE_HEIGHT: usize = 16;
pub(crate) const SOURCE_TILE_COUNT: usize = 231;
pub(crate) const OUTPUT_BANK_COUNT: usize = 2;
pub(crate) const OUTPUT_BANK_TILE_CAPACITY: usize = 248;
pub(crate) const OUTPUT_BANK_DECODED_SIZE: usize = OUTPUT_BANK_TILE_CAPACITY * TILE_RECORD_SIZE;
pub(crate) const OUTPUT_DECODED_SIZE: usize = OUTPUT_BANK_COUNT * OUTPUT_BANK_DECODED_SIZE;
pub(crate) const MAP_RECORD_SIZE: usize = TILES_PER_ROW * TILES_PER_COLUMN;

const PLANE_COUNT: usize = 4;
const TILES_PER_ROW: usize = FRAME_WIDTH / TILE_WIDTH;
const TILES_PER_COLUMN: usize = FRAME_HEIGHT / TILE_HEIGHT;
const TILE_HEADER: [u8; 4] = [2, 0, 16, 0];
const TILE_PAYLOAD_SIZE: usize = TILE_WIDTH / 8 * TILE_HEIGHT * PLANE_COUNT;
const TILE_RECORD_SIZE: usize = TILE_HEADER.len() + TILE_PAYLOAD_SIZE;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SourceEndingMealScenes {
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub used_tile_ids: BTreeSet<u8>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SplitEndingMealScenes {
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub used_tile_ids_by_bank: [BTreeSet<u8>; OUTPUT_BANK_COUNT],
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct EncodedEndingMealScenes {
    pub packed_tile_banks: Vec<u8>,
    pub map_records: [Vec<u8>; FRAME_COUNT],
    pub used_tile_counts: [usize; OUTPUT_BANK_COUNT],
}

pub(crate) fn decode_source_ending_meal_scenes(
    packed_tile_bank: &[u8],
    map_records: [&[u8]; FRAME_COUNT],
) -> Result<SourceEndingMealScenes> {
    let stream = decode_complete_compile_lz(packed_tile_bank)?;
    ensure!(
        stream.output.len() == SOURCE_TILE_COUNT * TILE_RECORD_SIZE,
        "ending meal source bank decodes to {} bytes instead of {} records of {TILE_RECORD_SIZE} bytes",
        stream.output.len(),
        SOURCE_TILE_COUNT
    );
    verify_tile_headers(&stream.output)?;

    let mut used_tile_ids = BTreeSet::new();
    let frames = map_records
        .into_iter()
        .enumerate()
        .map(|(frame_index, map)| {
            decode_frame(&stream.output, SOURCE_TILE_COUNT, map, &mut used_tile_ids)
                .with_context(|| format!("decode source ending meal frame {frame_index}"))
        })
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .expect("four ending meal maps produce four frames");

    Ok(SourceEndingMealScenes {
        frames,
        used_tile_ids,
    })
}

pub(crate) fn decode_split_ending_meal_scenes(
    packed_tile_banks: &[u8],
    map_records: [&[u8]; FRAME_COUNT],
) -> Result<SplitEndingMealScenes> {
    let stream = decode_complete_compile_lz(packed_tile_banks)?;
    ensure!(
        stream.output.len() == OUTPUT_DECODED_SIZE,
        "ending meal split banks decode to {} bytes instead of {OUTPUT_DECODED_SIZE}",
        stream.output.len()
    );
    verify_tile_headers(&stream.output)?;

    let mut used_tile_ids_by_bank = std::array::from_fn(|_| BTreeSet::new());
    let frames = map_records
        .into_iter()
        .enumerate()
        .map(|(frame_index, map)| {
            let bank_index = bank_for_frame(frame_index);
            let bank_start = bank_index * OUTPUT_BANK_DECODED_SIZE;
            decode_frame(
                &stream.output[bank_start..bank_start + OUTPUT_BANK_DECODED_SIZE],
                OUTPUT_BANK_TILE_CAPACITY,
                map,
                &mut used_tile_ids_by_bank[bank_index],
            )
            .with_context(|| format!("decode split ending meal frame {frame_index}"))
        })
        .collect::<Result<Vec<_>>>()?
        .try_into()
        .expect("four ending meal maps produce four frames");

    Ok(SplitEndingMealScenes {
        frames,
        used_tile_ids_by_bank,
    })
}

pub(crate) fn encode_split_ending_meal_scenes(
    frames: &[Vec<u8>; FRAME_COUNT],
    padding_palette_index: u8,
) -> Result<EncodedEndingMealScenes> {
    ensure!(
        padding_palette_index < 1 << PLANE_COUNT,
        "ending meal padding index exceeds the four-plane palette"
    );
    for (frame_index, frame) in frames.iter().enumerate() {
        ensure!(
            frame.len() == FRAME_WIDTH * FRAME_HEIGHT,
            "ending meal frame {frame_index} has {} pixels instead of {}",
            frame.len(),
            FRAME_WIDTH * FRAME_HEIGHT
        );
        ensure!(
            frame.iter().all(|pixel| *pixel < 1 << PLANE_COUNT),
            "ending meal frame {frame_index} contains a palette index above 15"
        );
    }

    let mut tiles_by_bank = std::array::from_fn::<_, OUTPUT_BANK_COUNT, _>(|_| Vec::new());
    let mut tile_ids_by_bank =
        std::array::from_fn::<_, OUTPUT_BANK_COUNT, _>(|_| BTreeMap::<Vec<u8>, u8>::new());
    let mut map_records = std::array::from_fn(|_| Vec::with_capacity(MAP_RECORD_SIZE));

    for (frame_index, frame) in frames.iter().enumerate() {
        let bank_index = bank_for_frame(frame_index);
        for tile_position in 0..MAP_RECORD_SIZE {
            let tile_pixels = extract_tile(frame, tile_position);
            let tile_record = encode_tile_record(&tile_pixels)?.to_vec();
            let tile_id = if let Some(&tile_id) = tile_ids_by_bank[bank_index].get(&tile_record) {
                tile_id
            } else {
                ensure!(
                    tiles_by_bank[bank_index].len() < OUTPUT_BANK_TILE_CAPACITY,
                    "ending meal bank {bank_index} requires more than {OUTPUT_BANK_TILE_CAPACITY} unique tiles at frame {frame_index}, position {tile_position}"
                );
                let tile_id = u8::try_from(tiles_by_bank[bank_index].len())
                    .expect("an ending meal bank has fewer than 256 records");
                tile_ids_by_bank[bank_index].insert(tile_record.clone(), tile_id);
                tiles_by_bank[bank_index].push(tile_record);
                tile_id
            };
            map_records[frame_index].push(tile_id);
        }
    }

    let used_tile_counts = tiles_by_bank.each_ref().map(Vec::len);
    for (bank_index, tiles) in tiles_by_bank.iter_mut().enumerate() {
        let (ordered_tiles, remapped_ids) = order_tiles_for_local_lz(std::mem::take(tiles));
        *tiles = ordered_tiles;
        for (frame_index, map) in map_records.iter_mut().enumerate() {
            if bank_for_frame(frame_index) == bank_index {
                for tile_id in map {
                    *tile_id = remapped_ids[usize::from(*tile_id)];
                }
            }
        }
    }
    let padding_tile =
        encode_tile_record(&vec![padding_palette_index; TILE_WIDTH * TILE_HEIGHT])?.to_vec();
    for tiles in &mut tiles_by_bank {
        while tiles.len() < OUTPUT_BANK_TILE_CAPACITY {
            tiles.push(padding_tile.clone());
        }
    }
    let decoded_tile_banks = tiles_by_bank
        .into_iter()
        .flatten()
        .flatten()
        .collect::<Vec<_>>();
    ensure!(
        decoded_tile_banks.len() == OUTPUT_DECODED_SIZE,
        "ending meal split bank encoder produced the wrong decoded size"
    );
    let packed_tile_banks = encode_compile_lz(&decoded_tile_banks)?;

    let decoded = decode_split_ending_meal_scenes(
        &packed_tile_banks,
        std::array::from_fn(|index| map_records[index].as_slice()),
    )?;
    ensure!(
        decoded.frames == *frames,
        "ending meal split bank encoder failed its frame-pixel round trip"
    );
    ensure!(
        decoded.used_tile_ids_by_bank.each_ref().map(BTreeSet::len) == used_tile_counts,
        "ending meal split bank encoder changed its used tile population"
    );

    Ok(EncodedEndingMealScenes {
        packed_tile_banks,
        map_records,
        used_tile_counts,
    })
}

fn bank_for_frame(frame_index: usize) -> usize {
    usize::from(frame_index >= FRAME_COUNT / OUTPUT_BANK_COUNT)
}

fn order_tiles_for_local_lz(tiles: Vec<Vec<u8>>) -> (Vec<Vec<u8>>, Vec<u8>) {
    if tiles.is_empty() {
        return (tiles, Vec::new());
    }

    let mut remaining = (0..tiles.len()).collect::<Vec<_>>();
    remaining.sort_by(|left, right| tiles[*left].cmp(&tiles[*right]));
    let first = remaining.remove(0);
    let mut order = vec![first];
    while !remaining.is_empty() {
        let current = *order.last().expect("the tile order has a first record");
        let next_position = remaining
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                tile_bit_difference(&tiles[current], &tiles[**left])
                    .cmp(&tile_bit_difference(&tiles[current], &tiles[**right]))
                    .then_with(|| tiles[**left].cmp(&tiles[**right]))
            })
            .map(|(position, _)| position)
            .expect("a nonempty remaining set has a closest tile");
        order.push(remaining.remove(next_position));
    }

    let mut remapped_ids = vec![0; tiles.len()];
    let ordered_tiles = order
        .into_iter()
        .enumerate()
        .map(|(new_id, old_id)| {
            remapped_ids[old_id] =
                u8::try_from(new_id).expect("an ending meal bank has fewer than 256 records");
            tiles[old_id].clone()
        })
        .collect();
    (ordered_tiles, remapped_ids)
}

fn tile_bit_difference(left: &[u8], right: &[u8]) -> u32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| (left ^ right).count_ones())
        .sum()
}

fn verify_tile_headers(decoded: &[u8]) -> Result<()> {
    ensure!(
        decoded.len().is_multiple_of(TILE_RECORD_SIZE),
        "ending meal bank ends in a partial tile record"
    );
    for (tile_id, record) in decoded.chunks_exact(TILE_RECORD_SIZE).enumerate() {
        ensure!(
            record[..TILE_HEADER.len()] == TILE_HEADER,
            "ending meal tile {tile_id} has an invalid 2-byte-wide, 16-row header"
        );
    }
    Ok(())
}

fn decode_frame(
    decoded_bank: &[u8],
    tile_count: usize,
    map: &[u8],
    used_tile_ids: &mut BTreeSet<u8>,
) -> Result<Vec<u8>> {
    ensure!(
        map.len() == MAP_RECORD_SIZE,
        "ending meal map has {} tile IDs instead of {MAP_RECORD_SIZE}",
        map.len()
    );
    let mut frame = vec![0; FRAME_WIDTH * FRAME_HEIGHT];
    for (tile_position, &tile_id) in map.iter().enumerate() {
        ensure!(
            usize::from(tile_id) < tile_count,
            "ending meal map selects tile {tile_id} outside its {tile_count}-record bank"
        );
        used_tile_ids.insert(tile_id);
        let start = usize::from(tile_id) * TILE_RECORD_SIZE;
        let tile = decode_tile_record(&decoded_bank[start..start + TILE_RECORD_SIZE])?;
        let tile_x = tile_position % TILES_PER_ROW * TILE_WIDTH;
        let tile_y = tile_position / TILES_PER_ROW * TILE_HEIGHT;
        for y in 0..TILE_HEIGHT {
            let source = &tile[y * TILE_WIDTH..(y + 1) * TILE_WIDTH];
            let target = (tile_y + y) * FRAME_WIDTH + tile_x;
            frame[target..target + TILE_WIDTH].copy_from_slice(source);
        }
    }
    Ok(frame)
}

fn decode_tile_record(record: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        record.len() == TILE_RECORD_SIZE && record[..TILE_HEADER.len()] == TILE_HEADER,
        "ending meal tile record does not match the verified 132-byte structure"
    );
    let payload = &record[TILE_HEADER.len()..];
    let mut pixels = vec![0; TILE_WIDTH * TILE_HEIGHT];
    for y in 0..TILE_HEIGHT {
        for x in 0..TILE_WIDTH {
            let byte_column = x / 8;
            let bit = 1 << (7 - x % 8);
            for plane_index in 0..PLANE_COUNT {
                let byte_offset =
                    y * (TILE_WIDTH / 8 * PLANE_COUNT) + byte_column * PLANE_COUNT + plane_index;
                if payload[byte_offset] & bit != 0 {
                    pixels[y * TILE_WIDTH + x] |= 1 << plane_index;
                }
            }
        }
    }
    Ok(pixels)
}

fn encode_tile_record(pixels: &[u8]) -> Result<[u8; TILE_RECORD_SIZE]> {
    ensure!(
        pixels.len() == TILE_WIDTH * TILE_HEIGHT,
        "ending meal tile has {} pixels instead of {}",
        pixels.len(),
        TILE_WIDTH * TILE_HEIGHT
    );
    ensure!(
        pixels.iter().all(|pixel| *pixel < 1 << PLANE_COUNT),
        "ending meal tile contains a palette index above 15"
    );
    let mut record = [0_u8; TILE_RECORD_SIZE];
    record[..TILE_HEADER.len()].copy_from_slice(&TILE_HEADER);
    for y in 0..TILE_HEIGHT {
        for x in 0..TILE_WIDTH {
            let pixel = pixels[y * TILE_WIDTH + x];
            let byte_column = x / 8;
            let bit = 1 << (7 - x % 8);
            for plane_index in 0..PLANE_COUNT {
                if pixel & (1 << plane_index) != 0 {
                    let byte_offset = TILE_HEADER.len()
                        + y * (TILE_WIDTH / 8 * PLANE_COUNT)
                        + byte_column * PLANE_COUNT
                        + plane_index;
                    record[byte_offset] |= bit;
                }
            }
        }
    }
    Ok(record)
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
#[path = "ending_meal_scene_tests.rs"]
mod ending_meal_scene_tests;
