use super::*;

#[test]
fn adjacent_changes_form_one_difference_and_reproduce_candidate() {
    let source = b"abcdefgh";
    let candidate = b"abXYZfgh";
    let differences = derive_reconstruction_diff(source, candidate, 0..2).unwrap();

    assert_eq!(
        differences,
        vec![ImageDifference {
            offset: 2,
            expected: b"cde".to_vec(),
            replacement: b"XYZ".to_vec(),
        }]
    );
    assert_eq!(
        apply_reconstruction_diff(source, &differences).unwrap(),
        candidate
    );
}

#[test]
fn protected_change_is_rejected() {
    let error = derive_reconstruction_diff(b"source", b"Source", 0..1).unwrap_err();
    assert!(error.to_string().contains("protected image range"));
}

#[test]
fn source_mismatch_is_rejected_before_any_difference_is_applied() {
    let differences = vec![ImageDifference {
        offset: 1,
        expected: b"b".to_vec(),
        replacement: b"X".to_vec(),
    }];
    let error = apply_reconstruction_diff(b"axc", &differences).unwrap_err();
    assert!(error.to_string().contains("source mismatch"));
}
