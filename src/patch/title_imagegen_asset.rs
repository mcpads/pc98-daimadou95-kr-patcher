use std::io::Cursor;

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

use super::graphic_text::Rect;

pub(super) struct HashBoundChromaAsset {
    path: &'static str,
    expected_sha256: &'static str,
    label: &'static str,
    classify: fn([u8; 3]) -> Option<u8>,
}

impl HashBoundChromaAsset {
    pub(super) const fn new(
        path: &'static str,
        expected_sha256: &'static str,
        label: &'static str,
        classify: fn([u8; 3]) -> Option<u8>,
    ) -> Self {
        Self {
            path,
            expected_sha256,
            label,
            classify,
        }
    }

    pub(super) fn paint(
        &self,
        pixels: &mut [u8],
        canvas_width: usize,
        canvas_height: usize,
        target: Rect,
    ) -> Result<usize> {
        paint_hash_bound_chroma_asset(self, pixels, canvas_width, canvas_height, target)
    }
}

fn paint_hash_bound_chroma_asset(
    asset: &HashBoundChromaAsset,
    pixels: &mut [u8],
    canvas_width: usize,
    canvas_height: usize,
    target: Rect,
) -> Result<usize> {
    let HashBoundChromaAsset {
        path,
        expected_sha256,
        label,
        classify,
    } = asset;
    let bytes = &std::fs::read(path).with_context(|| format!("{label} is unavailable: {path}"))?;
    ensure!(
        pixels.len() == canvas_width * canvas_height,
        "{label} canvas has {} pixels instead of {}",
        pixels.len(),
        canvas_width * canvas_height
    );
    ensure!(
        target.right <= canvas_width && target.bottom <= canvas_height,
        "{label} target lies outside the canvas"
    );

    let actual_sha256 = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    ensure!(
        actual_sha256 == *expected_sha256,
        "{label} hash changed: expected {expected_sha256}, got {actual_sha256}"
    );
    let image = decode_image(bytes, label)?;
    let bounds = image
        .foreground_bounds(*classify)
        .with_context(|| format!("{label} contains no foreground pixels"))?;
    let source_width = bounds.right - bounds.left;
    let source_height = bounds.bottom - bounds.top;
    let target_width = target.right - target.left;
    let target_height = target.bottom - target.top;
    let (draw_width, draw_height) =
        fit_inside(source_width, source_height, target_width, target_height);
    let left = target.left + (target_width - draw_width) / 2;
    let top = target.top + (target_height - draw_height) / 2;
    let mut painted = 0;

    for y in 0..draw_height {
        let source_y = bounds.top + y * source_height / draw_height;
        for x in 0..draw_width {
            let source_x = bounds.left + x * source_width / draw_width;
            if let Some(color) = image.palette_index(source_x, source_y, *classify) {
                pixels[(top + y) * canvas_width + left + x] = color;
                painted += 1;
            }
        }
    }
    ensure!(painted > 0, "{label} painted no pixels");
    Ok(painted)
}

struct ChromaImage {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
}

impl ChromaImage {
    fn palette_index(&self, x: usize, y: usize, classify: fn([u8; 3]) -> Option<u8>) -> Option<u8> {
        let offset = (y * self.width + x) * 3;
        classify([self.rgb[offset], self.rgb[offset + 1], self.rgb[offset + 2]])
    }

    fn foreground_bounds(&self, classify: fn([u8; 3]) -> Option<u8>) -> Option<Rect> {
        let mut left = self.width;
        let mut top = self.height;
        let mut right = 0;
        let mut bottom = 0;
        for y in 0..self.height {
            for x in 0..self.width {
                if self.palette_index(x, y, classify).is_some() {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x + 1);
                    bottom = bottom.max(y + 1);
                }
            }
        }
        (left < right && top < bottom).then(|| Rect::new(left, top, right, bottom))
    }
}

fn decode_image(bytes: &[u8], label: &str) -> Result<ChromaImage> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("read {label} PNG header"))?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buffer)
        .with_context(|| format!("decode {label} PNG"))?;
    let source = &buffer[..info.buffer_size()];
    let pixel_count = info.width as usize * info.height as usize;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    match info.color_type {
        png::ColorType::Rgb => rgb.extend_from_slice(source),
        png::ColorType::Rgba => {
            for pixel in source.chunks_exact(4) {
                rgb.extend_from_slice(&pixel[..3]);
            }
        }
        png::ColorType::Grayscale => {
            for &value in source {
                rgb.extend_from_slice(&[value, value, value]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for pixel in source.chunks_exact(2) {
                rgb.extend_from_slice(&[pixel[0], pixel[0], pixel[0]]);
            }
        }
        png::ColorType::Indexed => {
            anyhow::bail!("expanded {label} unexpectedly remained indexed")
        }
    }
    ensure!(
        rgb.len() == pixel_count * 3,
        "decoded {label} has an incomplete RGB buffer"
    );
    Ok(ChromaImage {
        width: info.width as usize,
        height: info.height as usize,
        rgb,
    })
}

fn fit_inside(
    source_width: usize,
    source_height: usize,
    target_width: usize,
    target_height: usize,
) -> (usize, usize) {
    if source_width * target_height > source_height * target_width {
        (
            target_width,
            (source_height * target_width / source_width).max(1),
        )
    } else {
        (
            (source_width * target_height / source_height).max(1),
            target_height,
        )
    }
}
