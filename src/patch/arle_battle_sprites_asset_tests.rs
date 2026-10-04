use crate::build_inputs::ArleAssetSet;
use crate::game_data::battle_sprite::{decode_battle_sprite_set, encode_battle_sprite_set};

use super::*;

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn tracked_battle_input_fits_the_two_verified_runtime_banks() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_arle_battle_sprites_asset(&inputs.battle_sprites).unwrap();
    let encoded = encode_battle_sprite_set(&asset.frames, BACKGROUND_PALETTE_INDEX).unwrap();
    let decoded = decode_battle_sprite_set(
        &encoded.packed_tile_maps,
        [&encoded.packed_tile_banks[0], &encoded.packed_tile_banks[1]],
    )
    .unwrap();

    assert!(encoded.used_tile_counts.iter().all(|count| *count <= 256));
    assert_eq!(decoded.frames, asset.frames);
    assert_eq!(
        decoded
            .used_tile_ids_by_bank
            .each_ref()
            .map(|ids| ids.len()),
        encoded.used_tile_counts
    );
}
