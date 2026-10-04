use anyhow::{Result, ensure};

use super::gcs::{
    GcsImage, HEADER_SIZE, PLANE_COUNT, SCREEN_HEIGHT, SCREEN_WIDTH, decode_gcs, encode_gcs,
};
use super::planar::{decode_planar_indices, encode_planar_indices};

pub(crate) const MADDAT_ENTRY_ID: usize = 5;

pub(crate) struct StageSelectGraphic {
    pub header: [u8; HEADER_SIZE],
    pub pixels: Vec<u8>,
}

pub(crate) fn decode_stage_select_graphic(input: &[u8]) -> Result<StageSelectGraphic> {
    let image = decode_gcs(input)?;
    let pixels = decode_planar_indices(&image.planes, SCREEN_WIDTH, SCREEN_HEIGHT)?;
    Ok(StageSelectGraphic {
        header: image.header,
        pixels,
    })
}

pub(crate) fn encode_stage_select_graphic(
    source: &StageSelectGraphic,
    pixels: &[u8],
) -> Result<Vec<u8>> {
    ensure!(
        source.pixels.len() == SCREEN_WIDTH * SCREEN_HEIGHT,
        "stage-select source has the wrong pixel population"
    );
    let planes = encode_planar_indices(pixels, SCREEN_WIDTH, SCREEN_HEIGHT, PLANE_COUNT)?;
    let planes: [Vec<u8>; PLANE_COUNT] = planes
        .try_into()
        .expect("four requested planes produce four GCS planes");
    let output = encode_gcs(&source.header, &planes)?;
    let decoded = decode_gcs(&output)?;
    verify_round_trip(&decoded, pixels)?;
    Ok(output)
}

fn verify_round_trip(decoded: &GcsImage, expected_pixels: &[u8]) -> Result<()> {
    ensure!(
        decoded.header.len() == HEADER_SIZE,
        "stage-select GCS header has the wrong size"
    );
    let pixels = decode_planar_indices(&decoded.planes, SCREEN_WIDTH, SCREEN_HEIGHT)?;
    ensure!(
        pixels == expected_pixels,
        "stage-select GCS pixel semantics changed during encoding"
    );
    Ok(())
}

#[cfg(test)]
#[path = "stage_select_graphic_tests.rs"]
mod stage_select_graphic_tests;
