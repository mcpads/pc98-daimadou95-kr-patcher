use super::{
    DisplaySiteKind, DisplaySiteSpec, ProgramSpec, RecordSpec, catalog_program, immediate,
};

#[test]
fn catalog_rejects_a_text_pointer_that_is_not_owned_by_its_consumer() {
    const DISPLAY: &[DisplaySiteSpec] = &[DisplaySiteSpec {
        kind: DisplaySiteKind::DirectAh09,
        file_offset: 3,
        bytes: &[0xb4, 0x09, 0xcd, 0x21],
    }];
    const RECORDS: &[RecordSpec] = &[RecordSpec {
        id: "message",
        runtime_address: 0x108,
        references: &[immediate(0, 3)],
    }];
    let spec = ProgramSpec {
        filename: "TEST.COM",
        display_sites: DISPLAY,
        records: RECORDS,
    };
    let mut bytes = vec![
        0xba, 0x09, 0x01, 0xb4, 0x09, 0xcd, 0x21, 0, b'O', b'K', b'$',
    ];

    let error = catalog_program(&bytes, spec).unwrap_err();
    assert!(error.to_string().contains("reference does not match"));

    bytes[1] = 0x08;
    let catalog = catalog_program(&bytes, spec).unwrap();
    assert_eq!(catalog.records[0].bytes, b"OK$");
}
