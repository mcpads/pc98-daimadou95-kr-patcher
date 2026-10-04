use super::*;

#[test]
fn map_patch_owns_only_the_four_verified_arle_records() {
    let mut baseline = vec![0xa5; MAD_MAP_FILE_OFFSETS[3] + MAP_RECORD_SIZE + 16];
    for (offset, record) in MAD_MAP_FILE_OFFSETS.iter().zip(SOURCE_MAP_RECORDS) {
        baseline[*offset..*offset + MAP_RECORD_SIZE].copy_from_slice(&record);
    }
    let replacements = std::array::from_fn(|frame_index| {
        let mut record = vec![4, 4];
        record.extend(std::iter::repeat_n(
            u8::try_from(frame_index).unwrap(),
            MAP_RECORD_SIZE - 2,
        ));
        record
    });

    let output = replace_small_status_maps(&baseline, &replacements).unwrap();

    for (offset, replacement) in MAD_MAP_FILE_OFFSETS.iter().zip(&replacements) {
        assert_eq!(
            &output[*offset..*offset + MAP_RECORD_SIZE],
            replacement.as_slice()
        );
    }
    for (index, (&before, &after)) in baseline.iter().zip(&output).enumerate() {
        if !MAD_MAP_FILE_OFFSETS
            .iter()
            .any(|offset| (*offset..*offset + MAP_RECORD_SIZE).contains(&index))
        {
            assert_eq!(before, after);
        }
    }
}
