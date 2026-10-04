use super::*;

#[test]
fn source_verification_rejects_an_unrecognized_size() {
    let error = verify_source_bytes(&[0_u8; 16]).unwrap_err();
    assert!(error.to_string().contains("unsupported source size"));
}

#[test]
fn source_verification_rejects_an_unrecognized_revision() {
    let error = verify_source_bytes(&vec![0_u8; SOURCE_DISK_SIZE]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unsupported Disc Station Vol. 08 Disk 1 revision")
    );
}

#[test]
fn sha256_matches_a_published_empty_input_vector() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn fatfs_mount_copy_changes_only_later_bpb_fields_and_the_signature_slot() {
    let original = vec![0xe5; BOOT_SECTOR_SIZE];
    let mount_copy = fatfs_mount_copy(&original).unwrap();
    assert_eq!(
        &mount_copy[..FATFS_HIDDEN_SECTORS_OFFSET],
        &original[..FATFS_HIDDEN_SECTORS_OFFSET]
    );
    assert_eq!(
        &mount_copy[FATFS_HIDDEN_SECTORS_OFFSET..FATFS_TOTAL_SECTORS_32_OFFSET + 4],
        &[0; 8]
    );
    assert_eq!(
        &mount_copy[FATFS_TOTAL_SECTORS_32_OFFSET + 4..FATFS_SIGNATURE_OFFSET],
        &original[FATFS_TOTAL_SECTORS_32_OFFSET + 4..FATFS_SIGNATURE_OFFSET]
    );
    assert_eq!(
        &mount_copy[FATFS_SIGNATURE_OFFSET..FATFS_SIGNATURE_OFFSET + 2],
        &[0x55, 0xaa]
    );
    assert_eq!(
        &mount_copy[FATFS_SIGNATURE_OFFSET + 2..],
        &original[FATFS_SIGNATURE_OFFSET + 2..]
    );
}
