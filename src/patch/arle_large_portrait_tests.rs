use crate::build_inputs::ArleAssetSet;
use crate::game_data::large_portrait::{
    TILE_BANK_CAPACITY, decode_large_portrait_set, encode_large_portrait_set,
};

use super::*;

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn tracked_pc98_input_compiles_as_four_independent_health_states() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_arle_large_portrait_asset(&inputs.large_portrait).unwrap();
    let encoded = encode_large_portrait_set(&asset.frames, BACKGROUND_PALETTE_INDEX).unwrap();
    let decoded =
        decode_large_portrait_set(&encoded.packed_tile_bank, &encoded.packed_tile_maps).unwrap();

    assert!(encoded.used_tile_count <= TILE_BANK_CAPACITY);
    assert_eq!(decoded.frames, asset.frames);
    assert_eq!(decoded.used_tile_ids.len(), encoded.used_tile_count);
}
