use std::ops::Range;

use anyhow::{Result, bail, ensure};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ImageDifference {
    offset: usize,
    expected: Vec<u8>,
    replacement: Vec<u8>,
}

pub(crate) fn derive_reconstruction_diff(
    source: &[u8],
    candidate: &[u8],
    protected_range: Range<usize>,
) -> Result<Vec<ImageDifference>> {
    ensure!(
        source.len() == candidate.len(),
        "candidate image size changed: expected {}, got {}",
        source.len(),
        candidate.len()
    );
    ensure!(
        protected_range.start <= protected_range.end && protected_range.end <= source.len(),
        "protected image range lies outside the source: {protected_range:?}"
    );
    ensure!(
        source[protected_range.clone()] == candidate[protected_range.clone()],
        "candidate changed protected image range: {protected_range:?}"
    );

    let mut differences = Vec::new();
    let mut offset = 0;
    while offset < source.len() {
        if source[offset] == candidate[offset] {
            offset += 1;
            continue;
        }
        let start = offset;
        while offset < source.len() && source[offset] != candidate[offset] {
            offset += 1;
        }
        differences.push(ImageDifference {
            offset: start,
            expected: source[start..offset].to_vec(),
            replacement: candidate[start..offset].to_vec(),
        });
    }
    Ok(differences)
}

pub(crate) fn apply_reconstruction_diff(
    source: &[u8],
    differences: &[ImageDifference],
) -> Result<Vec<u8>> {
    let mut previous_end = 0;
    for difference in differences {
        ensure!(
            !difference.expected.is_empty(),
            "reconstruction diff contains an empty difference"
        );
        ensure!(
            difference.expected.len() == difference.replacement.len(),
            "reconstruction diff changes the image length at offset {}",
            difference.offset
        );
        ensure!(
            difference.offset >= previous_end,
            "reconstruction diff is overlapping or out of order at offset {}",
            difference.offset
        );
        let end = difference
            .offset
            .checked_add(difference.expected.len())
            .ok_or_else(|| anyhow::anyhow!("reconstruction difference range overflow"))?;
        let observed = source.get(difference.offset..end).ok_or_else(|| {
            anyhow::anyhow!(
                "reconstruction difference lies outside the source at offset {}",
                difference.offset
            )
        })?;
        if observed != difference.expected {
            bail!(
                "reconstruction diff source mismatch at offset {}",
                difference.offset
            );
        }
        previous_end = end;
    }

    let mut output = source.to_vec();
    for difference in differences {
        let end = difference.offset + difference.replacement.len();
        output[difference.offset..end].copy_from_slice(&difference.replacement);
    }
    Ok(output)
}

#[cfg(test)]
#[path = "reconstruction_tests.rs"]
mod reconstruction_tests;
