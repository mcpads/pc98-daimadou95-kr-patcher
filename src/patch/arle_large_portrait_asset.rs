use std::io::Cursor;

use anyhow::{Context, Result, ensure};

use crate::build_inputs::ResolvedArleAsset;
use crate::game_data::large_portrait::{FRAME_COUNT, FRAME_HEIGHT, FRAME_WIDTH};

pub(super) const BACKGROUND_PALETTE_INDEX: u8 = 4;
pub(super) const RUNTIME_PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],
    [255, 255, 255],
    [34, 68, 187],
    [255, 0, 102],
    [119, 204, 0],
    [0, 153, 68],
    [255, 204, 170],
    [34, 221, 255],
    [187, 0, 17],
    [34, 136, 221],
    [238, 255, 17],
    [255, 170, 34],
    [204, 119, 34],
    [255, 153, 170],
    [136, 136, 136],
    [204, 204, 204],
];

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ArleLargePortraitAsset {
    pub frames: [Vec<u8>; FRAME_COUNT],
    pub sha256: String,
}

pub(super) fn load_arle_large_portrait_asset(
    input: &ResolvedArleAsset,
) -> Result<ArleLargePortraitAsset> {
    let rgb = decode_rgb_png(input.bytes())?;
    let sheet_width = FRAME_WIDTH * FRAME_COUNT;
    ensure!(
        rgb.len() == sheet_width * FRAME_HEIGHT * 3,
        "Arle large portrait RGB population is incomplete"
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
                        "Arle large portrait frame {frame_index} pixel ({x}, {y}) uses undeclared RGB {color:?}"
                    )
                })?;
            }
        }
    }
    validate_frame_contract(&frames)?;

    Ok(ArleLargePortraitAsset {
        frames,
        sha256: input.sha256().to_owned(),
    })
}

fn decode_rgb_png(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .context("read Arle large portrait PNG")?;
    let info = reader.info();
    ensure!(
        info.width as usize == FRAME_WIDTH * FRAME_COUNT && info.height as usize == FRAME_HEIGHT,
        "Arle large portrait PNG is {}x{} instead of {}x{}",
        info.width,
        info.height,
        FRAME_WIDTH * FRAME_COUNT,
        FRAME_HEIGHT
    );
    let mut buffer = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut buffer)
        .context("decode Arle large portrait PNG")?;
    let source = &buffer[..output.buffer_size()];
    let pixel_count = output.width as usize * output.height as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    match output.color_type {
        png::ColorType::Rgb => rgb.extend_from_slice(source),
        png::ColorType::Rgba => {
            for pixel in source.chunks_exact(4) {
                ensure!(
                    pixel[3] == u8::MAX,
                    "Arle large portrait PNG must be fully opaque"
                );
                rgb.extend_from_slice(&pixel[..3]);
            }
        }
        other => anyhow::bail!("Arle large portrait PNG decoded as unsupported {other:?}"),
    }
    ensure!(
        rgb.len() == pixel_count * 3,
        "Arle large portrait PNG has an incomplete RGB buffer"
    );
    Ok(rgb)
}

fn palette_index(color: [u8; 3]) -> Result<u8> {
    RUNTIME_PALETTE
        .iter()
        .position(|candidate| *candidate == color)
        .map(|index| u8::try_from(index).expect("the runtime palette has 16 entries"))
        .context("RGB is not in the verified portrait runtime palette")
}

fn validate_frame_contract(frames: &[Vec<u8>; FRAME_COUNT]) -> Result<()> {
    for (frame_index, frame) in frames.iter().enumerate() {
        ensure!(
            frame.len() == FRAME_WIDTH * FRAME_HEIGHT,
            "Arle large portrait frame {frame_index} has the wrong pixel population"
        );
        for &(x, y) in &[
            (0, 0),
            (FRAME_WIDTH - 1, 0),
            (0, FRAME_HEIGHT - 1),
            (FRAME_WIDTH - 1, FRAME_HEIGHT - 1),
        ] {
            ensure!(
                frame[y * FRAME_WIDTH + x] == BACKGROUND_PALETTE_INDEX,
                "Arle large portrait frame {frame_index} corner ({x}, {y}) is not background index {BACKGROUND_PALETTE_INDEX}"
            );
        }
        ensure!(
            frame.iter().any(|pixel| *pixel != BACKGROUND_PALETTE_INDEX),
            "Arle large portrait frame {frame_index} has no foreground"
        );
    }

    Ok(())
}

#[cfg(test)]
#[path = "arle_large_portrait_asset_tests.rs"]
mod arle_large_portrait_asset_tests;
