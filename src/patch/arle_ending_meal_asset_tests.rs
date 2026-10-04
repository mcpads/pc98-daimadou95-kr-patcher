use super::*;
use crate::build_inputs::ArleAssetSet;

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn tracked_sheet_supplies_four_complete_runtime_frames() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_arle_ending_meal_asset(&inputs.ending_meal).unwrap();

    assert_eq!(asset.sha256, inputs.ending_meal.sha256());
    assert_eq!(asset.frames.len(), FRAME_COUNT);
    assert!(
        asset
            .frames
            .iter()
            .all(|frame| frame.len() == FRAME_WIDTH * FRAME_HEIGHT)
    );
    assert!(!asset.frames[0].contains(&EFFECT_PALETTE_INDEX));
    assert!(
        asset.frames[1..]
            .iter()
            .all(|frame| frame.contains(&EFFECT_PALETTE_INDEX))
    );
}
