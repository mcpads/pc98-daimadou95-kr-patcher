use crate::build_inputs::ArleAssetSet;
use crate::game_data::ending_meal_scene::{
    OUTPUT_BANK_TILE_CAPACITY, decode_split_ending_meal_scenes, encode_split_ending_meal_scenes,
};

use super::*;

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn approved_four_scene_sheet_fits_as_two_complete_banks_and_round_trips() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_arle_ending_meal_asset(&inputs.ending_meal).unwrap();

    let encoded = encode_split_ending_meal_scenes(&asset.frames, BACKGROUND_PALETTE_INDEX).unwrap();
    let decoded = decode_split_ending_meal_scenes(
        &encoded.packed_tile_banks,
        std::array::from_fn(|index| encoded.map_records[index].as_slice()),
    )
    .unwrap();

    assert_eq!(decoded.frames, asset.frames);
    assert!(
        encoded
            .used_tile_counts
            .iter()
            .all(|count| *count <= OUTPUT_BANK_TILE_CAPACITY)
    );
}
