use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

use anyhow::{Context, Result, ensure};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::flink::{parse_flink_archive, repack_contiguous_flink_archive};
use crate::game_data::gcs::{
    HEADER_SIZE, PLANE_COUNT, SCREEN_HEIGHT, SCREEN_WIDTH, decode_gcs, encode_gcs,
};
use crate::game_data::planar::{decode_planar_indices, encode_planar_indices};
use crate::source::disk::sha256_hex;

use super::maddat::apply_repacked_maddat;

const MADDAT_ENTRY_ID: usize = 143;
const SOURCE_ENTRY_SHA256: &str =
    "04edd09ef7ac2c5a74881ee301ff1d8d26d3eac548b980153536ad0b0e5ecf90";
const ASSET_WIDTH: usize = 432;
const ASSET_HEIGHT: usize = 224;
const BACKGROUND_PALETTE_INDEX: u8 = 10;
const UNOWNED_GREEN_PALETTE_INDEX: u8 = 11;

const OPENING_PALETTE_RGB: [[u8; 3]; 16] = [
    [0, 0, 17],
    [17, 51, 170],
    [34, 136, 221],
    [68, 187, 238],
    [255, 136, 170],
    [119, 34, 17],
    [170, 85, 34],
    [204, 119, 51],
    [255, 170, 102],
    [255, 221, 119],
    [17, 119, 85],
    [85, 170, 85],
    [255, 204, 187],
    [136, 153, 170],
    [204, 221, 238],
    [255, 255, 255],
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PixelRegion {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
}

impl PixelRegion {
    const fn new(left: usize, top: usize, right: usize, bottom: usize) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    fn contains(self, x: usize, y: usize) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }

    fn pixel_count(self) -> usize {
        (self.right - self.left) * (self.bottom - self.top)
    }
}

const POSE_REGIONS: [PixelRegion; 3] = [
    PixelRegion::new(0, 0, 128, 224),
    PixelRegion::new(128, 0, 304, 208),
    PixelRegion::new(304, 0, 432, 192),
];
const PROTECTED_ACCESSORY_REGION: PixelRegion = PixelRegion::new(432, 0, 448, 32);

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ArleOpeningPosesFiles {
    pub maddat: Vec<u8>,
    pub report: ArleOpeningPosesReport,
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArleOpeningPosesReport {
    pub asset_sha256: String,
    pub maddat_entry_id: usize,
    pub pose_count: usize,
    pub packed_input_size: usize,
    pub packed_output_size: usize,
    pub changed_pixel_count: usize,
    pub protected_accessory_pixel_count: usize,
}

pub(crate) fn compile_arle_opening_poses(
    maddat: &[u8],
    asset_input: &ResolvedArleAsset,
) -> Result<ArleOpeningPosesFiles> {
    let archive = parse_flink_archive(maddat)?;
    let source_entry = archive
        .entries
        .get(MADDAT_ENTRY_ID - 1)
        .context("MADDAT is missing the Arle opening-pose GCS")?;
    ensure!(
        source_entry.id == MADDAT_ENTRY_ID,
        "Arle opening-pose entry ID differs from its table position"
    );
    ensure!(
        sha256_hex(source_entry.bytes) == SOURCE_ENTRY_SHA256,
        "Arle opening-pose source GCS differs from the verified entry"
    );

    let source_image = decode_gcs(source_entry.bytes)?;
    let source_pixels = decode_planar_indices(&source_image.planes, SCREEN_WIDTH, SCREEN_HEIGHT)?;
    let asset = load_opening_pose_asset(asset_input)?;
    let pixels = replace_opening_pose_pixels(&source_pixels, &asset.pixels)?;
    let changed_pixel_count = source_pixels
        .iter()
        .zip(&pixels)
        .filter(|(source, output)| source != output)
        .count();
    ensure!(
        changed_pixel_count > 0,
        "tracked Arle opening poses make no pixel changes"
    );

    let encoded_planes = encode_planar_indices(&pixels, SCREEN_WIDTH, SCREEN_HEIGHT, PLANE_COUNT)?;
    let encoded_planes: [Vec<u8>; PLANE_COUNT] = encoded_planes
        .try_into()
        .expect("four requested planes produce four encoded planes");
    let header: &[u8; HEADER_SIZE] = &source_image.header;
    let packed = encode_gcs(header, &encoded_planes)?;
    ensure!(
        packed.len() <= usize::from(u16::MAX),
        "Arle opening-pose GCS exceeds the FLINK u16 size field"
    );

    let replacements = BTreeMap::from([(MADDAT_ENTRY_ID, packed.clone())]);
    let repacked = repack_contiguous_flink_archive(maddat, &replacements, &[])?;
    ensure!(
        repacked.replaced_entry_ids == BTreeSet::from([MADDAT_ENTRY_ID]),
        "Arle opening-pose repack ownership differs from its one GCS entry"
    );
    let checked_maddat = apply_repacked_maddat(
        maddat,
        &repacked.bytes,
        "arle-opening-poses-maddat-repacker",
        "replace all three Arle prologue poses while preserving their adjacent accessory",
    )?;

    let candidate_archive = parse_flink_archive(&checked_maddat)?;
    let candidate_entry = &candidate_archive.entries[MADDAT_ENTRY_ID - 1];
    let candidate_image = decode_gcs(candidate_entry.bytes)?;
    let candidate_pixels =
        decode_planar_indices(&candidate_image.planes, SCREEN_WIDTH, SCREEN_HEIGHT)?;
    ensure!(
        candidate_pixels == pixels,
        "Arle opening-pose pixels did not survive the MADDAT repack"
    );
    for (source, candidate) in archive.entries.iter().zip(&candidate_archive.entries) {
        if source.id != MADDAT_ENTRY_ID {
            ensure!(
                source.bytes == candidate.bytes,
                "Arle opening-pose repack changed protected MADDAT entry {}",
                source.id
            );
        }
    }

    Ok(ArleOpeningPosesFiles {
        maddat: checked_maddat,
        report: ArleOpeningPosesReport {
            asset_sha256: asset.sha256,
            maddat_entry_id: MADDAT_ENTRY_ID,
            pose_count: POSE_REGIONS.len(),
            packed_input_size: source_entry.bytes.len(),
            packed_output_size: packed.len(),
            changed_pixel_count,
            protected_accessory_pixel_count: PROTECTED_ACCESSORY_REGION.pixel_count(),
        },
    })
}

struct OpeningPoseAsset {
    pixels: Vec<u8>,
    sha256: String,
}

fn load_opening_pose_asset(input: &ResolvedArleAsset) -> Result<OpeningPoseAsset> {
    let mut decoder = png::Decoder::new(Cursor::new(input.bytes()));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().context("read Arle opening-pose PNG")?;
    let info = reader.info();
    ensure!(
        info.width as usize == ASSET_WIDTH && info.height as usize == ASSET_HEIGHT,
        "Arle opening-pose PNG is {}x{} instead of {ASSET_WIDTH}x{ASSET_HEIGHT}",
        info.width,
        info.height
    );
    let mut buffer = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut buffer)
        .context("decode Arle opening-pose PNG")?;
    let source = &buffer[..output.buffer_size()];
    let pixel_count = ASSET_WIDTH * ASSET_HEIGHT;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    match output.color_type {
        png::ColorType::Rgb => rgb.extend_from_slice(source),
        png::ColorType::Rgba => {
            for pixel in source.chunks_exact(4) {
                ensure!(
                    pixel[3] == u8::MAX,
                    "Arle opening-pose PNG must be fully opaque"
                );
                rgb.extend_from_slice(&pixel[..3]);
            }
        }
        other => anyhow::bail!("Arle opening-pose PNG decoded as unsupported {other:?}"),
    }
    ensure!(
        rgb.len() == pixel_count * 3,
        "Arle opening-pose PNG has an incomplete RGB population"
    );

    let pixels = rgb
        .chunks_exact(3)
        .enumerate()
        .map(|(offset, color)| {
            OPENING_PALETTE_RGB
                .iter()
                .position(|candidate| candidate.as_slice() == color)
                .map(|index| u8::try_from(index).expect("the opening palette has 16 entries"))
                .with_context(|| {
                    let x = offset % ASSET_WIDTH;
                    let y = offset / ASSET_WIDTH;
                    format!("Arle opening-pose pixel ({x}, {y}) uses undeclared RGB {color:?}")
                })
        })
        .collect::<Result<Vec<_>>>()?;

    for (pose_index, region) in POSE_REGIONS.iter().copied().enumerate() {
        let pose_pixels = pixels_in_region(&pixels, ASSET_WIDTH, region);
        ensure!(
            pose_pixels.contains(&BACKGROUND_PALETTE_INDEX),
            "Arle opening pose {pose_index} has no background-key pixels"
        );
        ensure!(
            pose_pixels
                .iter()
                .any(|pixel| *pixel != BACKGROUND_PALETTE_INDEX),
            "Arle opening pose {pose_index} has no foreground pixels"
        );
        ensure!(
            !pose_pixels.contains(&UNOWNED_GREEN_PALETTE_INDEX),
            "Arle opening pose {pose_index} contains a non-transparent green matte"
        );
    }
    for y in 0..ASSET_HEIGHT {
        for x in 0..ASSET_WIDTH {
            if !POSE_REGIONS.iter().any(|region| region.contains(x, y)) {
                ensure!(
                    pixels[y * ASSET_WIDTH + x] == 0,
                    "unused Arle opening-pose sheet pixel ({x}, {y}) is not protected black"
                );
            }
        }
    }

    Ok(OpeningPoseAsset {
        pixels,
        sha256: input.sha256().to_owned(),
    })
}

fn replace_opening_pose_pixels(source: &[u8], asset: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        source.len() == SCREEN_WIDTH * SCREEN_HEIGHT,
        "Arle opening-pose compositor received the wrong GCS pixel population"
    );
    ensure!(
        asset.len() == ASSET_WIDTH * ASSET_HEIGHT,
        "Arle opening-pose compositor received the wrong asset pixel population"
    );

    let mut output = source.to_vec();
    for region in POSE_REGIONS {
        for y in region.top..region.bottom {
            let source_row = y * SCREEN_WIDTH;
            let asset_row = y * ASSET_WIDTH;
            output[source_row + region.left..source_row + region.right]
                .copy_from_slice(&asset[asset_row + region.left..asset_row + region.right]);
        }
    }
    for y in 0..SCREEN_HEIGHT {
        for x in 0..SCREEN_WIDTH {
            if !POSE_REGIONS.iter().any(|region| region.contains(x, y)) {
                ensure!(
                    output[y * SCREEN_WIDTH + x] == source[y * SCREEN_WIDTH + x],
                    "Arle opening-pose compositor changed protected pixel ({x}, {y})"
                );
            }
        }
    }
    Ok(output)
}

fn pixels_in_region(pixels: &[u8], stride: usize, region: PixelRegion) -> Vec<u8> {
    let mut output = Vec::with_capacity(region.pixel_count());
    for y in region.top..region.bottom {
        output.extend_from_slice(&pixels[y * stride + region.left..y * stride + region.right]);
    }
    output
}

#[cfg(test)]
#[path = "arle_opening_poses_tests.rs"]
mod arle_opening_poses_tests;
