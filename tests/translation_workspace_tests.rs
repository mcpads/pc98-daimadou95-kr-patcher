//! User-owned supported-source translation workspace gate.

use std::collections::BTreeSet;
use std::env;
use std::fs;

use ds8_daimadou_builder::{
    TranslationSegment, TranslationWorkspaceIndex, prepare_translation_workspace,
    validate_translation_workspace,
};

#[test]
#[ignore = "requires DS8_DISK1 pointing to a user-owned supported source image"]
fn workspace_covers_the_target_population_and_rejects_protected_drift() {
    let source = env::var_os("DS8_DISK1").expect("set DS8_DISK1 to the supported Disk 1 HDM");
    let directory = tempfile::tempdir().unwrap();
    let workspace = directory.path().join("translations");

    let prepared = prepare_translation_workspace(source.as_ref(), &workspace).unwrap();
    assert_eq!(prepared.target_item_count, 384);
    assert!(prepared.segment_count > 1);
    assert!(
        prepare_translation_workspace(source.as_ref(), &workspace)
            .unwrap_err()
            .to_string()
            .contains("refusing to overwrite")
    );

    let validated = validate_translation_workspace(source.as_ref(), &workspace).unwrap();
    assert_eq!(validated.target_item_count, 384);
    assert_eq!(validated.segment_count, prepared.segment_count);
    assert_eq!(validated.untranslated_item_count, 384);

    let index: TranslationWorkspaceIndex =
        serde_json::from_slice(&fs::read(workspace.join("index.json")).unwrap()).unwrap();
    assert_eq!(index.segment_count, index.segments.len());
    assert_eq!(
        index
            .segments
            .iter()
            .map(|segment| segment.entry_count)
            .sum::<usize>(),
        384
    );
    assert_eq!(
        index
            .segments
            .iter()
            .map(|segment| segment.id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        index.segment_count
    );

    let first_path = workspace.join(&index.segments[0].path);
    let mut first: TranslationSegment =
        serde_json::from_slice(&fs::read(&first_path).unwrap()).unwrap();
    first.entries[0].source_record_id.push_str("-tampered");
    let mut tampered = serde_json::to_vec_pretty(&first).unwrap();
    tampered.push(b'\n');
    fs::write(&first_path, tampered).unwrap();

    let error = validate_translation_workspace(source.as_ref(), &workspace)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("protected translation fields differ"),
        "{error}"
    );
}
