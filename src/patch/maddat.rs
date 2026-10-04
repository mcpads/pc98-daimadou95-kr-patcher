use anyhow::{Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, ResizePlan, WriteIntent, WritePlan};

use crate::game_data::flink::{ENTRY_RECORD_SIZE, HEADER_SIZE, parse_flink_archive};

pub(super) fn apply_repacked_maddat(
    baseline: &[u8],
    replacement: &[u8],
    owner: &str,
    purpose: &str,
) -> Result<Vec<u8>> {
    let baseline_archive = parse_flink_archive(baseline)?;
    let replacement_archive = parse_flink_archive(replacement)?;
    ensure!(
        baseline_archive.uninterpreted_header_byte == replacement_archive.uninterpreted_header_byte,
        "MADDAT repack changed the protected header byte"
    );
    let table_end = HEADER_SIZE + replacement_archive.entries.len() * ENTRY_RECORD_SIZE;

    let mut plan = WritePlan::new();
    if baseline.len() != replacement.len() {
        plan = plan.resize(ResizePlan {
            owner: owner.into(),
            purpose: purpose.into(),
            expected_input_len: baseline.len(),
            output_len: replacement.len(),
        });
    }
    plan = plan
        .region(ImageRegion {
            id: format!("{owner}-metadata"),
            range: 0..table_end,
            kind: RegionKind::Metadata,
            reason: "FLINK header and relocated entry table".into(),
        })
        .region(ImageRegion {
            id: format!("{owner}-payload"),
            range: table_end..replacement.len(),
            kind: RegionKind::Data,
            reason: "verified FLINK payload population".into(),
        })
        .write(ExpectedWrite {
            id: format!("{owner}-table-write"),
            owner: owner.into(),
            purpose: format!("{purpose}: serialize FLINK metadata"),
            offset: 0,
            expected_original: baseline[..table_end].to_vec(),
            replacement: replacement[..table_end].to_vec(),
            intent: WriteIntent::Metadata,
        })
        .write(ExpectedWrite {
            id: format!("{owner}-payload-write"),
            owner: owner.into(),
            purpose: format!("{purpose}: serialize verified payloads"),
            offset: table_end,
            expected_original: baseline[table_end..replacement.len().min(baseline.len())].to_vec(),
            replacement: replacement[table_end..].to_vec(),
            intent: WriteIntent::Data,
        });
    let output = plan.apply(baseline, None)?;
    ensure!(
        output == replacement,
        "Expected Write MADDAT result differs from verified repacker output"
    );
    Ok(output)
}

#[cfg(test)]
#[path = "maddat_tests.rs"]
mod maddat_tests;
