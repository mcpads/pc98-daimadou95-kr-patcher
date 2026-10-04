use std::io::Cursor;

use anyhow::{Context, Result, ensure};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::small_status::{FRAME_COUNT, FRAME_HEIGHT, FRAME_WIDTH};

use super::arle_large_portrait_asset::{BACKGROUND_PALETTE_INDEX, RUNTIME_PALETTE};

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ArleSmallStatusAsset {
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub sha256: String,
}

pub(super) fn load_arle_small_status_asset(
    input: &ResolvedArleAsset,
) -> Result<ArleSmallStatusAsset> {
    let rgb = decode_rgb_png(input.bytes())?;
    let sheet_width = FRAME_WIDTH * FRAME_COUNT;
    ensure!(
        rgb.len() == sheet_width * FRAME_HEIGHT * 3,
        "Arle small status RGB population is incomplete"
    );
    let mut frames = std::array::from_fn(|_| vec![0; FRAME_WIDTH * FRAME_HEIGHT]);
    for (frame_index, frame) in frames.iter_mut().enumerate() {
        for y in 0..FRAME_HEIGHT {
            for x in 0..FRAME_WIDTH {
                let sheet_x = frame_index * FRAME_WIDTH + x;
                let offset = (y * sheet_width + sheet_x) * 3;
                let color = [rgb[offset], rgb[offset + 1], rgb[offset + 2]];
                frame[y * FRAME_WIDTH + x] = palette_index(color).with_context(|| {
                    format!(
                        "Arle small status frame {frame_index} pixel ({x}, {y}) uses undeclared RGB {color:?}"
                    )
                })?;
            }
        }
    }
    validate_frame_contract(&frames)?;

    Ok(ArleSmallStatusAsset {
        frames,
        sha256: input.sha256().to_owned(),
    })
}

fn decode_rgb_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().context("read Arle small status PNG")?;
    let info = reader.info();
    ensure!(
        info.width as usize == FRAME_WIDTH * FRAME_COUNT && info.height as usize == FRAME_HEIGHT,
        "Arle small status PNG is {}x{} instead of {}x{}",
        info.width,
        info.height,
        FRAME_WIDTH * FRAME_COUNT,
        FRAME_HEIGHT
    );
    let mut buffer = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut buffer)
        .context("decode Arle small status PNG")?;
    let source = &buffer[..output.buffer_size()];
    let pixel_count = output.width as usize * output.height as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    match output.color_type {
        png::ColorType::Rgb => rgb.extend_from_slice(source),
        png::ColorType::Rgba => {
            for pixel in source.chunks_exact(4) {
                ensure!(
                    pixel[3] == u8::MAX,
                    "Arle small status PNG must be fully opaque"
                );
                rgb.extend_from_slice(&pixel[..3]);
            }
        }
        other => anyhow::bail!("Arle small status PNG decoded as unsupported {other:?}"),
    }
    ensure!(
        rgb.len() == pixel_count * 3,
        "Arle small status PNG has an incomplete RGB buffer"
    );
    Ok(rgb)
}

fn palette_index(color: [u8; 3]) -> Result<u8> {
    RUNTIME_PALETTE
        .iter()
        .position(|candidate| *candidate == color)
        .map(|index| u8::try_from(index).expect("the runtime palette has 16 entries"))
        .context("RGB is not in the verified status runtime palette")
}

fn validate_frame_contract(frames: &[Vec<u8>; FRAME_COUNT]) -> Result<()> {
    for (frame_index, frame) in frames.iter().enumerate() {
        ensure!(
            frame.len() == FRAME_WIDTH * FRAME_HEIGHT,
            "Arle small status frame {frame_index} has the wrong pixel population"
        );
        ensure!(
            frame.contains(&BACKGROUND_PALETTE_INDEX),
            "Arle small status frame {frame_index} has no background-key pixels"
        );
        ensure!(
            frame.iter().any(|pixel| *pixel != BACKGROUND_PALETTE_INDEX),
            "Arle small status frame {frame_index} has no foreground"
        );
    }
    ensure!(
        frames[0] != frames[1] && frames[1] != frames[2] && frames[2] != frames[3],
        "Arle small status expressions are not independent"
    );
    Ok(())
}

#[cfg(test)]
#[path = "arle_small_status_asset_tests.rs"]
mod arle_small_status_asset_tests;
