use std::fs;

use super::*;
use crate::build_inputs::{ARLE_ASSET_FILENAMES, ArleAssetSet};
use crate::game_data::large_portrait::{
    TILE_BANK_CAPACITY, decode_large_portrait_set, encode_large_portrait_set,
};

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn tracked_asset_is_an_exact_pc98_palette_sheet_with_four_health_states() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_arle_large_portrait_asset(&inputs.large_portrait).unwrap();

    assert_eq!(asset.sha256, inputs.large_portrait.sha256());
    assert_eq!(asset.frames.len(), FRAME_COUNT);
    assert!(asset.frames.iter().all(|frame| {
        frame
            .iter()
            .all(|pixel| usize::from(*pixel) < RUNTIME_PALETTE.len())
    }));
}

#[test]
fn undeclared_rgb_is_not_silently_quantized_during_the_build() {
    let error = palette_index([1, 2, 3]).unwrap_err();

    assert!(
        error
            .to_string()
            .contains("verified portrait runtime palette")
    );
}

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn selected_directory_asset_uses_the_same_png_contract() {
    let tracked = ArleAssetSet::tracked().resolve().unwrap();
    let directory = tempfile::tempdir().unwrap();
    for (filename, asset) in ARLE_ASSET_FILENAMES.iter().zip([
        &tracked.large_portrait,
        &tracked.opening_poses,
        &tracked.small_status,
        &tracked.battle_sprites,
        &tracked.ending_meal,
    ]) {
        fs::write(directory.path().join(filename), asset.bytes()).unwrap();
    }
    fs::write(directory.path().join(ARLE_ASSET_FILENAMES[0]), b"not a PNG").unwrap();

    let selected = ArleAssetSet::from_directory(directory.path())
        .resolve()
        .unwrap();
    let error = load_arle_large_portrait_asset(&selected.large_portrait).unwrap_err();

    assert!(error.to_string().contains("read Arle large portrait PNG"));
}

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn independent_health_portraits_fit_the_expanded_consumer_bank() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_arle_large_portrait_asset(&inputs.large_portrait).unwrap();
    let encoded = encode_large_portrait_set(&asset.frames, BACKGROUND_PALETTE_INDEX).unwrap();
    let decoded =
        decode_large_portrait_set(&encoded.packed_tile_bank, &encoded.packed_tile_maps).unwrap();

    assert!(
        encoded.used_tile_count <= TILE_BANK_CAPACITY,
        "portrait maps require {} tiles, but the expanded consumers address only {TILE_BANK_CAPACITY}",
        encoded.used_tile_count
    );
    assert!(decoded.decoded_tile_maps.chunks_exact(66).all(|record| {
        record[2..]
            .iter()
            .all(|tile_id| usize::from(*tile_id) < TILE_BANK_CAPACITY)
    }));
}
