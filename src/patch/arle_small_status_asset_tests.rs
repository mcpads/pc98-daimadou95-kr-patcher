use crate::build_inputs::ArleAssetSet;
use crate::game_data::small_status::{decode_small_status_set, encode_small_status_set};

use super::*;

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn tracked_status_input_compiles_as_four_independent_frames() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_arle_small_status_asset(&inputs.small_status).unwrap();
    let encoded = encode_small_status_set(&asset.frames, BACKGROUND_PALETTE_INDEX).unwrap();
    let decoded = decode_small_status_set(
        &encoded.packed_tile_bank,
        std::array::from_fn(|index| encoded.map_records[index].as_slice()),
    )
    .unwrap();

    assert!(encoded.used_tile_count <= 64);
    assert_eq!(decoded.frames, asset.frames);
    assert_eq!(decoded.used_tile_ids.len(), encoded.used_tile_count);
}
