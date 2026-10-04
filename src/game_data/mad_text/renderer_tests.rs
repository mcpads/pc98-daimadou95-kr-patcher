use super::{FILE_OFFSET, verify_text_renderer};

#[test]
fn renderer_verification_fails_closed_on_a_missing_signature() {
    let bytes = vec![0_u8; FILE_OFFSET + 0x100];

    let error = verify_text_renderer(&bytes).unwrap_err();

    assert!(error.to_string().contains("renderer dispatch"));
}
