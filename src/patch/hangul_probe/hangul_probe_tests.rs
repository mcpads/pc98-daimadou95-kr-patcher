use expected_write::WritePlanError;

use super::{gaiji, mad_text};

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn expected_writes_change_only_the_glyph_bitmap_and_one_text_cell() {
    let mut gaiji_baseline = vec![0x5a; gaiji::BITMAP_FILE_OFFSET + 32];
    gaiji_baseline[gaiji::BITMAP_FILE_OFFSET..].fill(0);
    let patched_gaiji = gaiji::write_plan()
        .unwrap()
        .apply(&gaiji_baseline, None)
        .unwrap();

    let mut mad_baseline = vec![0x5a; mad_text::CELL_FILE_OFFSET + 2];
    mad_baseline[mad_text::CELL_FILE_OFFSET..].copy_from_slice(&[0xeb, 0xe1]);
    let patched_mad = mad_text::write_plan().apply(&mad_baseline, None).unwrap();

    assert_eq!(
        &patched_gaiji[..gaiji::BITMAP_FILE_OFFSET],
        &gaiji_baseline[..gaiji::BITMAP_FILE_OFFSET]
    );
    assert_ne!(
        &patched_gaiji[gaiji::BITMAP_FILE_OFFSET..],
        &gaiji_baseline[gaiji::BITMAP_FILE_OFFSET..]
    );
    assert_eq!(
        &patched_mad[..mad_text::CELL_FILE_OFFSET],
        &mad_baseline[..mad_text::CELL_FILE_OFFSET]
    );
    assert_eq!(&patched_mad[mad_text::CELL_FILE_OFFSET..], &[0xeb, 0xa0]);
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn expected_writes_reject_a_different_original_revision() {
    let wrong_gaiji = vec![0xff; gaiji::BITMAP_FILE_OFFSET + 32];
    let wrong_mad = vec![0xff; mad_text::CELL_FILE_OFFSET + 2];

    let gaiji_error = gaiji::write_plan()
        .unwrap()
        .apply(&wrong_gaiji, None)
        .unwrap_err();
    let mad_error = mad_text::write_plan().apply(&wrong_mad, None).unwrap_err();

    assert!(matches!(
        gaiji_error,
        WritePlanError::OriginalBytesMismatch { .. }
    ));
    assert!(matches!(
        mad_error,
        WritePlanError::OriginalBytesMismatch { .. }
    ));
}
