//! User-owned supported-source human-review packet gate.

use std::env;
use std::path::Path;

use ds8_daimadou_builder::report_translation_review;

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn review_queue_joins_every_tracked_draft_to_protected_context() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let report = report_translation_review(source.as_ref(), Path::new("assets/translations"))
        .expect("report tracked translation review queue");

    assert_eq!(report.target_item_count, 384);
    assert_eq!(report.segment_count, 63);
    assert!(report.human_review_ready);
    assert_eq!(report.human_decision_segment_count, 21);
    assert_eq!(report.human_decision_entry_count, 226);
    assert_eq!(report.ready_segment_count, 42);
    assert_eq!(report.ready_entry_count, 158);
    assert_eq!(report.segments.len(), 63);
    assert!(report.segments.iter().all(|segment| {
        !segment.notes.is_empty()
            && segment.entries.iter().all(|entry| {
                !entry.consumer_roles.is_empty()
                    && !entry.text.is_empty()
                    && entry
                        .text
                        .iter()
                        .all(|pair| !pair.source.is_empty() && !pair.korean.is_empty())
            })
    }));
}
