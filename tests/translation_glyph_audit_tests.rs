//! User-owned supported-source glyph-demand gate for the tracked Korean corpus.

use std::env;
use std::path::Path;

use ds8_daimadou_builder::{LocalizationRenderPath, audit_translation_glyphs};

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image and the build inputs under assets/"]
fn reviewed_corpus_reports_the_current_physical_slot_shortfalls() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let report = audit_translation_glyphs(source.as_ref(), Path::new("assets/translations"))
        .expect("audit tracked translation glyph demand");

    assert_eq!(report.target_item_count, 384);
    assert_eq!(report.segment_count, 63);
    assert!(report.human_review_ready);
    assert_eq!(
        report.font.provenance.profile_id,
        "neodunggeunmo-pc98-16x16"
    );
    assert_eq!(
        report.font.provenance.font_sha256,
        "d61b60eccb731f8ca9c7da582e4a05a94db66b570471809950aa9a7261b941d6"
    );
    assert!(report.font.verified_hangul_syllable_count > 0);
    assert_eq!(
        report.ending_credit_font.provenance.profile_id,
        "neodunggeunmo-ending-credit-24x32"
    );
    assert_eq!(report.ending_credit_font.verified_hangul_syllable_count, 23);

    let shared = report
        .render_paths
        .iter()
        .find(|report| report.render_path == LocalizationRenderPath::SharedIndexedFont)
        .unwrap();
    assert_eq!(shared.demand.verified_physical_capacity, Some(256));
    assert_eq!(
        shared.demand.physical_capacity_headroom,
        shared
            .demand
            .verified_physical_capacity
            .zip(shared.demand.slot_glyph_count)
            .map(|(capacity, demand)| capacity as isize - demand as isize)
    );
    assert_eq!(shared.demand.physical_capacity_fit, Some(false));

    let credits = report
        .render_paths
        .iter()
        .find(|report| report.render_path == LocalizationRenderPath::EndingCreditFont)
        .unwrap();
    assert_eq!(credits.demand.slot_glyph_count, Some(23));
    assert_eq!(credits.demand.verified_physical_capacity, Some(30));
    assert_eq!(credits.demand.physical_capacity_headroom, Some(7));
    assert_eq!(credits.demand.physical_capacity_fit, Some(true));

    let dos = report
        .render_paths
        .iter()
        .find(|report| report.render_path == LocalizationRenderPath::DosSystemText)
        .unwrap();
    assert_eq!(dos.demand.verified_physical_capacity, Some(175));
    assert_eq!(
        dos.demand.physical_capacity_headroom,
        dos.demand
            .verified_physical_capacity
            .zip(dos.demand.slot_glyph_count)
            .map(|(capacity, demand)| capacity as isize - demand as isize)
    );
    assert_eq!(dos.demand.physical_capacity_fit, Some(true));

    let graphics = report
        .render_paths
        .iter()
        .find(|report| report.render_path == LocalizationRenderPath::PlanarGraphics)
        .unwrap();
    assert_eq!(graphics.entry_count, 25);
    assert_eq!(graphics.text_span_count, 25);
    assert_eq!(graphics.demand.slot_glyph_count, None);
    assert_eq!(graphics.demand.verified_physical_capacity, None);
}
