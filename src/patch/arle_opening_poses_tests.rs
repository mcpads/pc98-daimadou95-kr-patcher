use super::*;
use crate::build_inputs::ArleAssetSet;

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn tracked_asset_covers_each_opening_pose_region() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_opening_pose_asset(&inputs.opening_poses).unwrap();

    assert_eq!(asset.sha256, inputs.opening_poses.sha256());
    for region in POSE_REGIONS {
        let pixels = pixels_in_region(&asset.pixels, ASSET_WIDTH, region);
        assert!(pixels.contains(&BACKGROUND_PALETTE_INDEX));
        assert!(
            pixels
                .iter()
                .any(|pixel| *pixel != BACKGROUND_PALETTE_INDEX)
        );
        assert!(!pixels.contains(&UNOWNED_GREEN_PALETTE_INDEX));
    }
}

#[test]
#[ignore = "requires the tracked Arle PNG set in assets/characters/arle/"]
fn pose_replacement_preserves_every_pixel_outside_the_three_consumed_regions() {
    let inputs = ArleAssetSet::tracked().resolve().unwrap();
    let asset = load_opening_pose_asset(&inputs.opening_poses).unwrap();
    let source = vec![7; SCREEN_WIDTH * SCREEN_HEIGHT];
    let output = replace_opening_pose_pixels(&source, &asset.pixels).unwrap();

    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            let offset = y * SCREEN_WIDTH + x;
            if POSE_REGIONS.iter().any(|region| region.contains(x, y)) {
                assert_eq!(output[offset], asset.pixels[y * ASSET_WIDTH + x]);
            } else {
                assert_eq!(output[offset], source[offset]);
            }
        }
    }
    assert_eq!(
        pixels_in_region(&output, SCREEN_WIDTH, PROTECTED_ACCESSORY_REGION),
        pixels_in_region(&source, SCREEN_WIDTH, PROTECTED_ACCESSORY_REGION)
    );
}
