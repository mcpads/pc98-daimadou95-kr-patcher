use std::collections::BTreeMap;

use super::*;
use crate::game_data::flink::{SIGNATURE, repack_contiguous_flink_archive};

fn archive(data: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0_u8; HEADER_SIZE + ENTRY_RECORD_SIZE];
    bytes[..SIGNATURE.len()].copy_from_slice(SIGNATURE);
    bytes[13..15].copy_from_slice(&1_u16.to_le_bytes());
    bytes[15] = 2;
    bytes[16..18].copy_from_slice(&(data.len() as u16).to_le_bytes());
    bytes[18..22].copy_from_slice(&22_u32.to_le_bytes());
    bytes.extend_from_slice(data);
    bytes
}

#[test]
fn repacked_maddat_resize_is_owned_and_matches_the_verified_candidate() {
    let source = archive(b"abc");
    let candidate =
        repack_contiguous_flink_archive(&source, &BTreeMap::from([(1, b"longer".to_vec())]), &[])
            .unwrap();

    let applied =
        apply_repacked_maddat(&source, &candidate.bytes, "fixture", "grow entry").unwrap();

    assert_eq!(applied, candidate.bytes);

    let shrink_candidate = repack_contiguous_flink_archive(
        &candidate.bytes,
        &BTreeMap::from([(1, b"x".to_vec())]),
        &[],
    )
    .unwrap();
    let shrunk = apply_repacked_maddat(
        &candidate.bytes,
        &shrink_candidate.bytes,
        "fixture",
        "shrink entry",
    )
    .unwrap();
    assert_eq!(shrunk, shrink_candidate.bytes);

    let mut wrong_candidate = candidate.bytes;
    wrong_candidate[15] ^= 0xff;
    assert!(apply_repacked_maddat(&source, &wrong_candidate, "fixture", "grow entry").is_err());
}
