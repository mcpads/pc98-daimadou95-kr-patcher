use std::io::Cursor;

use anyhow::{Context, Result, ensure};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::ending_meal_scene::{FRAME_COUNT, FRAME_HEIGHT, FRAME_WIDTH};

pub(super) const BACKGROUND_PALETTE_INDEX: u8 = 1;
pub(super) const EFFECT_PALETTE_INDEX: u8 = 14;

const BASE_PALETTE: [[u8; 3]; 14] = [
    [0, 0, 0],
    [255, 255, 255],
    [85, 102, 204],
    [153, 153, 238],
    [221, 204, 255],
    [255, 34, 68],
    [255, 170, 204],
    [255, 170, 153],
    [255, 221, 187],
    [187, 34, 0],
    [204, 119, 34],
    [221, 153, 34],
    [255, 204, 34],
    [255, 255, 136],
];
const EFFECT_COLORS: [[u8; 3]; FRAME_COUNT] = [
    [136, 170, 238],
    [136, 170, 238],
    [119, 119, 17],
    [238, 204, 221],
];

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ArleEndingMealAsset {
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub sha256: String,
}

pub(super) fn load_arle_ending_meal_asset(
    input: &ResolvedArleAsset,
) -> Result<ArleEndingMealAsset> {
    let rgb = decode_rgb_png(input.bytes())?;
    let sheet_width = FRAME_WIDTH * FRAME_COUNT;
    ensure!(
        rgb.len() == sheet_width * FRAME_HEIGHT * 3,
        "Arle ending meal RGB population is incomplete"
    );
    let mut frames = std::array::from_fn(|_| vec![0; FRAME_WIDTH * FRAME_HEIGHT]);
    for (frame_index, frame) in frames.iter_mut().enumerate() {
        for y in 0..FRAME_HEIGHT {
            for x in 0..FRAME_WIDTH {
                let sheet_x = frame_index * FRAME_WIDTH + x;
                let offset = (y * sheet_width + sheet_x) * 3;
                let color = [rgb[offset], rgb[offset + 1], rgb[offset + 2]];
                frame[y * FRAME_WIDTH + x] = palette_index(frame_index, color).with_context(|| {
                    format!(
                        "Arle ending meal frame {frame_index} pixel ({x}, {y}) uses undeclared RGB {color:?}"
                    )
                })?;
            }
        }
    }
    validate_frame_contract(&frames)?;

    Ok(ArleEndingMealAsset {
        frames,
        sha256: input.sha256().to_owned(),
    })
}

fn decode_rgb_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().context("read Arle ending meal PNG")?;
    let info = reader.info();
    ensure!(
        info.width as usize == FRAME_WIDTH * FRAME_COUNT && info.height as usize == FRAME_HEIGHT,
        "Arle ending meal PNG is {}x{} instead of {}x{}",
        info.width,
        info.height,
        FRAME_WIDTH * FRAME_COUNT,
        FRAME_HEIGHT
    );
    let mut buffer = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut buffer)
        .context("decode Arle ending meal PNG")?;
    let source = &buffer[..output.buffer_size()];
    let pixel_count = output.width as usize * output.height as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    match output.color_type {
        png::ColorType::Rgb => rgb.extend_from_slice(source),
        png::ColorType::Rgba => {
            for pixel in source.chunks_exact(4) {
                ensure!(pixel[3] == u8::MAX, "Arle ending meal PNG must be opaque");
                rgb.extend_from_slice(&pixel[..3]);
            }
        }
        other => anyhow::bail!("Arle ending meal PNG decoded as unsupported {other:?}"),
    }
    ensure!(
        rgb.len() == pixel_count * 3,
        "Arle ending meal PNG has an incomplete RGB buffer"
    );
    Ok(rgb)
}

fn palette_index(frame_index: usize, color: [u8; 3]) -> Result<u8> {
    if let Some(index) = BASE_PALETTE
        .iter()
        .position(|candidate| *candidate == color)
    {
        return Ok(u8::try_from(index).expect("the base ending palette has 14 entries"));
    }
    if color == EFFECT_COLORS[frame_index] {
        return Ok(EFFECT_PALETTE_INDEX);
    }
    anyhow::bail!("RGB is not in the frame-specific ending runtime palette")
}

fn validate_frame_contract(frames: &[Vec<u8>; FRAME_COUNT]) -> Result<()> {
    for (frame_index, frame) in frames.iter().enumerate() {
        ensure!(
            frame.len() == FRAME_WIDTH * FRAME_HEIGHT,
            "Arle ending meal frame {frame_index} has the wrong pixel population"
        );
        for &(x, y) in &[
            (0, 0),
            (FRAME_WIDTH - 1, 0),
            (0, FRAME_HEIGHT - 1),
            (FRAME_WIDTH - 1, FRAME_HEIGHT - 1),
        ] {
            ensure!(
                frame[y * FRAME_WIDTH + x] == BACKGROUND_PALETTE_INDEX,
                "Arle ending meal frame {frame_index} corner ({x}, {y}) is not background index {BACKGROUND_PALETTE_INDEX}"
            );
        }
        ensure!(
            frame.iter().any(|pixel| *pixel != BACKGROUND_PALETTE_INDEX),
            "Arle ending meal frame {frame_index} has no foreground"
        );
        ensure!(
            frame.contains(&0),
            "Arle ending meal frame {frame_index} has no black outline pixels"
        );
        if frame_index == 0 {
            ensure!(
                !frame.contains(&EFFECT_PALETTE_INDEX),
                "Arle ending meal before-eating frame unexpectedly uses the reaction color"
            );
        } else {
            ensure!(
                frame.contains(&EFFECT_PALETTE_INDEX),
                "Arle ending meal reaction frame {frame_index} has no state-colored effect"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "arle_ending_meal_asset_tests.rs"]
mod arle_ending_meal_asset_tests;
