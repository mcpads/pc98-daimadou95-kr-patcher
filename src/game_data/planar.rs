use anyhow::{Context, Result, ensure};

pub(crate) fn decode_planar_indices(
    planes: &[Vec<u8>],
    width: usize,
    height: usize,
) -> Result<Vec<u8>> {
    ensure!(
        !planes.is_empty() && planes.len() <= 8,
        "planar image must contain 1 through 8 planes"
    );
    ensure!(
        width > 0 && width.is_multiple_of(8),
        "planar width must be byte-aligned"
    );
    ensure!(height > 0, "planar height must be nonzero");
    let stride = width / 8;
    let plane_size = stride
        .checked_mul(height)
        .context("planar plane size overflow")?;
    for (plane_index, plane) in planes.iter().enumerate() {
        ensure!(
            plane.len() == plane_size,
            "planar plane {} has {} bytes instead of {plane_size}",
            plane_index + 1,
            plane.len()
        );
    }

    let mut pixels = vec![0_u8; width * height];
    for y in 0..height {
        for x in 0..width {
            let byte_offset = y * stride + x / 8;
            let mask = 1 << (7 - (x % 8));
            for (plane_index, plane) in planes.iter().enumerate() {
                if plane[byte_offset] & mask != 0 {
                    pixels[y * width + x] |= 1 << plane_index;
                }
            }
        }
    }
    Ok(pixels)
}

pub(crate) fn encode_planar_indices(
    pixels: &[u8],
    width: usize,
    height: usize,
    plane_count: usize,
) -> Result<Vec<Vec<u8>>> {
    ensure!(
        (1..=8).contains(&plane_count),
        "planar image must contain 1 through 8 planes"
    );
    ensure!(
        width > 0 && width.is_multiple_of(8),
        "planar width must be byte-aligned"
    );
    ensure!(height > 0, "planar height must be nonzero");
    ensure!(
        pixels.len() == width * height,
        "planar pixel population has {} entries instead of {}",
        pixels.len(),
        width * height
    );
    let max_index = (1_u16 << plane_count) - 1;
    ensure!(
        pixels.iter().all(|pixel| u16::from(*pixel) <= max_index),
        "planar pixel index exceeds {plane_count}-plane capacity"
    );

    let stride = width / 8;
    let mut planes = vec![vec![0_u8; stride * height]; plane_count];
    for y in 0..height {
        for x in 0..width {
            let pixel = pixels[y * width + x];
            let byte_offset = y * stride + x / 8;
            let mask = 1 << (7 - (x % 8));
            for (plane_index, plane) in planes.iter_mut().enumerate() {
                if pixel & (1 << plane_index) != 0 {
                    plane[byte_offset] |= mask;
                }
            }
        }
    }
    Ok(planes)
}

#[cfg(test)]
#[path = "planar_tests.rs"]
mod planar_tests;
