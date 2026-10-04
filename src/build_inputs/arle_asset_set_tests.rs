use std::fs;

use super::*;

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn tracked_set_resolves_every_arle_surface() {
    let assets = ArleAssetSet::tracked().resolve().unwrap();

    for asset in [
        &assets.large_portrait,
        &assets.opening_poses,
        &assets.small_status,
        &assets.battle_sprites,
        &assets.ending_meal,
    ] {
        assert!(!asset.bytes().is_empty());
        assert_eq!(asset.sha256().len(), 64);
    }
}

#[test]
fn directory_set_reads_each_required_semantic_asset() {
    let directory = tempfile::tempdir().unwrap();
    for (index, filename) in ARLE_ASSET_FILENAMES.iter().enumerate() {
        fs::write(directory.path().join(filename), [index as u8]).unwrap();
    }

    let assets = ArleAssetSet::from_directory(directory.path())
        .resolve()
        .unwrap();

    assert_eq!(assets.large_portrait.bytes(), [0]);
    assert_eq!(assets.opening_poses.bytes(), [1]);
    assert_eq!(assets.small_status.bytes(), [2]);
    assert_eq!(assets.battle_sprites.bytes(), [3]);
    assert_eq!(assets.ending_meal.bytes(), [4]);
}

#[test]
fn directory_set_rejects_a_missing_surface_instead_of_mixing_sets() {
    let directory = tempfile::tempdir().unwrap();
    for filename in &ARLE_ASSET_FILENAMES[..4] {
        fs::write(directory.path().join(filename), b"present").unwrap();
    }

    let error = ArleAssetSet::from_directory(directory.path())
        .resolve()
        .unwrap_err();

    assert!(error.to_string().contains(ENDING_MEAL_FILENAME));
    assert!(error.to_string().contains("ending meal scenes"));
}

#[test]
fn original_preservation_resolves_without_an_asset_set() {
    let selection = ArleAssetSelection::preserve_original().resolve().unwrap();

    assert_eq!(selection, ResolvedArleAssetSelection::PreserveOriginal);
}
