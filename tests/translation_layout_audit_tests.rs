//! User-owned supported-source layout gate for the tracked Korean corpus.

use std::env;
use std::path::Path;

use ds8_daimadou_builder::audit_translation_layout;

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn reviewed_corpus_fits_every_verified_hard_layout_boundary() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let report = audit_translation_layout(source.as_ref(), Path::new("assets/translations"))
        .expect("audit tracked translation layout");

    assert_eq!(report.target_item_count, 384);
    assert_eq!(report.segment_count, 63);
    assert!(report.human_review_ready);
    assert_eq!(report.measured_unit_count, 630);
    assert_eq!(report.hard_limit_unit_count, 568);
    assert_eq!(report.glyph_token_count, 78);
    assert_eq!(report.hard_violation_count, 0);
    assert!(report.verified_hard_limits_fit);
    assert_eq!(report.open_gates.len(), 2);
}
