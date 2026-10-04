use super::*;

#[test]
fn original_preservation_forwards_every_arle_consumer_file_unchanged() {
    let files = apply_arle_asset_selection(
        b"MADDAT before Arle replacement",
        b"SELECT.COM before Arle replacement",
        b"MAD.COM before Arle replacement",
        b"ENDING.COM before Arle replacement",
        &ResolvedArleAssetSelection::PreserveOriginal,
    )
    .unwrap();

    assert_eq!(files.maddat, b"MADDAT before Arle replacement");
    assert_eq!(files.select_com, b"SELECT.COM before Arle replacement");
    assert_eq!(files.mad_com, b"MAD.COM before Arle replacement");
    assert_eq!(files.ending_com, b"ENDING.COM before Arle replacement");
    assert_eq!(files.report, ArleAssetBuildReport::OriginalPreserved);
}
