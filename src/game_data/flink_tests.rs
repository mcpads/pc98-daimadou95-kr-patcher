use super::*;

fn archive_with_entries(entries: &[(usize, &[u8])], len: usize) -> Vec<u8> {
    let mut archive = vec![0_u8; len];
    archive[..SIGNATURE.len()].copy_from_slice(SIGNATURE);
    archive[13..15].copy_from_slice(&(entries.len() as u16).to_le_bytes());
    archive[15] = 2;
    for (index, (offset, data)) in entries.iter().enumerate() {
        let record_offset = HEADER_SIZE + index * ENTRY_RECORD_SIZE;
        archive[record_offset..record_offset + 2]
            .copy_from_slice(&(data.len() as u16).to_le_bytes());
        archive[record_offset + 2..record_offset + ENTRY_RECORD_SIZE]
            .copy_from_slice(&(*offset as u32).to_le_bytes());
        archive[*offset..*offset + data.len()].copy_from_slice(data);
    }
    archive
}

#[test]
fn entry_ids_start_at_one_and_records_select_their_declared_ranges() {
    let archive_bytes = archive_with_entries(&[(32, b"abc"), (40, b"WXYZ")], 48);

    let archive = parse_flink_archive(&archive_bytes).unwrap();

    assert_eq!(archive.uninterpreted_header_byte, 2);
    assert_eq!(
        archive.entries,
        vec![
            FlinkEntry {
                id: 1,
                offset: 32,
                bytes: b"abc",
            },
            FlinkEntry {
                id: 2,
                offset: 40,
                bytes: b"WXYZ",
            },
        ]
    );
}

#[test]
fn entry_outside_the_archive_is_rejected() {
    let mut archive = archive_with_entries(&[], 24);
    archive[13..15].copy_from_slice(&1_u16.to_le_bytes());
    archive[16..18].copy_from_slice(&8_u16.to_le_bytes());
    archive[18..22].copy_from_slice(&20_u32.to_le_bytes());

    let error = parse_flink_archive(&archive).unwrap_err();

    assert!(error.to_string().contains("entry 1 lies outside"));
}

#[test]
fn repack_updates_sizes_and_offsets_without_changing_other_entry_payloads() {
    let archive_bytes = archive_with_entries(&[(28, b"abc"), (31, b"WXYZ")], 35);
    let replacements = BTreeMap::from([(1, b"longer".to_vec())]);

    let repacked = repack_contiguous_flink_archive(&archive_bytes, &replacements, &[]).unwrap();
    let archive = parse_flink_archive(&repacked.bytes).unwrap();

    assert_eq!(repacked.replaced_entry_ids, BTreeSet::from([1]));
    assert!(repacked.appended_entry_ids.is_empty());
    assert_eq!(repacked.input_size, 35);
    assert_eq!(repacked.output_size, 38);
    assert_eq!(archive.entries[0].offset, 28);
    assert_eq!(archive.entries[0].bytes, b"longer");
    assert_eq!(archive.entries[1].offset, 34);
    assert_eq!(archive.entries[1].bytes, b"WXYZ");
    assert_eq!(archive.uninterpreted_header_byte, 2);
}

#[test]
fn repack_can_append_a_new_entry_without_reusing_an_existing_asset() {
    let archive_bytes = archive_with_entries(&[(28, b"abc"), (31, b"WXYZ")], 35);

    let repacked = repack_contiguous_flink_archive(
        &archive_bytes,
        &BTreeMap::from([(2, b"Q".to_vec())]),
        &[b"font-bank".to_vec()],
    )
    .unwrap();
    let archive = parse_flink_archive(&repacked.bytes).unwrap();

    assert_eq!(archive.entries.len(), 3);
    assert_eq!(archive.entries[0].bytes, b"abc");
    assert_eq!(archive.entries[1].bytes, b"Q");
    assert_eq!(archive.entries[2].bytes, b"font-bank");
    assert_eq!(archive.entries[0].offset, 34);
    assert_eq!(repacked.replaced_entry_ids, BTreeSet::from([2]));
    assert_eq!(repacked.appended_entry_ids, BTreeSet::from([3]));
}

#[test]
fn repack_rejects_archives_with_unowned_gaps_or_unknown_replacements() {
    let gapped = archive_with_entries(&[(32, b"abc"), (40, b"WXYZ")], 44);
    let gap_error =
        repack_contiguous_flink_archive(&gapped, &BTreeMap::from([(1, vec![1])]), &[]).unwrap_err();
    assert!(gap_error.to_string().contains("contiguous in table order"));

    let contiguous = archive_with_entries(&[(22, b"abc")], 25);
    let id_error =
        repack_contiguous_flink_archive(&contiguous, &BTreeMap::from([(2, vec![1])]), &[])
            .unwrap_err();
    assert!(id_error.to_string().contains("outside 1..=1"));
}
