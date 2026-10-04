use super::*;

#[test]
fn protected_control_hex_is_decoded_without_text_interpretation() {
    assert_eq!(decode_hex("1b5b324a").unwrap(), [0x1b, 0x5b, 0x32, 0x4a]);
    assert!(decode_hex("1b5").is_err());
}
