use super::*;
use crate::patch::mad_media_errors::bank_plan::GaijiSlot;

#[test]
fn output_encoding_preserves_controls_and_uses_declared_gaiji_codes() {
    let codebook = BTreeMap::from([(
        '한',
        GaijiSlot {
            character_code: 0x7622,
            shift_jis_code: 0xeba0,
        },
    )]);
    let parts = vec![
        DosOutputPart::Bytes(vec![0x1b, b'[', b'2', b'J']),
        DosOutputPart::Text("A한".to_owned()),
        DosOutputPart::Bytes(vec![b'\r', b'\n']),
        DosOutputPart::Terminator,
    ];

    assert_eq!(
        encode_output_parts("fixture", &parts, &codebook).unwrap(),
        [0x1b, b'[', b'2', b'J', b'A', 0xeb, 0xa0, b'\r', b'\n', b'$']
    );
}

#[test]
fn output_encoding_rejects_unmapped_hangul_and_early_terminators() {
    let no_mapping = vec![
        DosOutputPart::Text("한".to_owned()),
        DosOutputPart::Terminator,
    ];
    assert!(encode_output_parts("fixture", &no_mapping, &BTreeMap::new()).is_err());

    let early = vec![DosOutputPart::Terminator, DosOutputPart::Terminator];
    assert!(encode_output_parts("fixture", &early, &BTreeMap::new()).is_err());
}
