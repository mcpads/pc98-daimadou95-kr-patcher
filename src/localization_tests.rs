use super::*;
use crate::source::{LocalizationSummary, LocalizationTextItem, LocalizationTextReason};

fn item(
    id: &str,
    source_catalog: LocalizationSourceCatalog,
    disposition: LocalizationTextDisposition,
) -> LocalizationTextItem {
    LocalizationTextItem {
        id: id.to_owned(),
        source_filename: "SOURCE.COM".to_owned(),
        source_catalog,
        source_record_id: id.to_owned(),
        text: "日本語".to_owned(),
        disposition,
        reason: LocalizationTextReason::ConsumerReachable,
    }
}

fn catalog(items: Vec<LocalizationTextItem>, target_count: usize) -> LocalizationCatalog {
    LocalizationCatalog {
        summary: LocalizationSummary {
            declared_runtime_file_count: 0,
            audited_runtime_file_count: 0,
            declared_maddat_entry_count: 0,
            audited_maddat_entry_count: 0,
            identified_source_text_item_count: items.len(),
            target_text_item_count: target_count,
            excluded_text_item_count: items.len() - target_count,
            unresolved_text_item_count: 0,
        },
        files: Vec::new(),
        maddat_entries: Vec::new(),
        source_text: items,
    }
}

#[test]
fn assigns_every_target_once_and_never_assigns_excluded_text() {
    let source_catalogs = [
        LocalizationSourceCatalog::SelectDialogue,
        LocalizationSourceCatalog::OpeningText,
        LocalizationSourceCatalog::GraphicText,
        LocalizationSourceCatalog::EndingCredits,
        LocalizationSourceCatalog::EndingCurryResult,
        LocalizationSourceCatalog::EndingMealReaction,
        LocalizationSourceCatalog::EndingInterludeCaption,
        LocalizationSourceCatalog::EndingIngredientLabels,
        LocalizationSourceCatalog::EndingPasswords,
        LocalizationSourceCatalog::DosProgramText,
        LocalizationSourceCatalog::MadIndexedText,
    ];
    let mut items = source_catalogs
        .into_iter()
        .enumerate()
        .map(|(index, source_catalog)| {
            item(
                &format!("target-{index}"),
                source_catalog,
                LocalizationTextDisposition::Target,
            )
        })
        .collect::<Vec<_>>();
    items.push(item(
        "excluded",
        LocalizationSourceCatalog::DosProgramText,
        LocalizationTextDisposition::Excluded,
    ));

    let strategy = plan_localization(&catalog(items, source_catalogs.len())).unwrap();
    let assigned_ids = strategy
        .families
        .iter()
        .flat_map(|family| &family.target_ids)
        .collect::<BTreeSet<_>>();

    assert_eq!(strategy.summary.target_item_count, source_catalogs.len());
    assert_eq!(strategy.summary.assigned_item_count, source_catalogs.len());
    assert_eq!(strategy.summary.unassigned_item_count, 0);
    assert_eq!(assigned_ids.len(), source_catalogs.len());
    assert!(!assigned_ids.iter().any(|id| id.as_str() == "excluded"));
    assert_eq!(strategy.families.len(), source_catalogs.len());

    let mst_target = item(
        "mst-target",
        LocalizationSourceCatalog::MstText,
        LocalizationTextDisposition::Target,
    );
    assert!(plan_localization(&catalog(vec![mst_target], 1)).is_err());
}

#[test]
fn assigns_ending_credits_to_the_dedicated_font_path() {
    let strategy = plan_localization(&catalog(
        vec![
            item(
                "credit-role",
                LocalizationSourceCatalog::EndingCredits,
                LocalizationTextDisposition::Target,
            ),
            item(
                "indexed",
                LocalizationSourceCatalog::OpeningText,
                LocalizationTextDisposition::Target,
            ),
            item(
                "dos",
                LocalizationSourceCatalog::DosProgramText,
                LocalizationTextDisposition::Target,
            ),
            item(
                "graphic",
                LocalizationSourceCatalog::GraphicText,
                LocalizationTextDisposition::Target,
            ),
        ],
        4,
    ))
    .unwrap();
    let credits = strategy
        .families
        .iter()
        .find(|family| family.source_catalog == LocalizationSourceCatalog::EndingCredits)
        .unwrap();

    assert_eq!(
        credits.render_path,
        LocalizationRenderPath::EndingCreditFont
    );
    assert_eq!(
        credits.reinsertion_gate,
        LocalizationReinsertionGate::ReplaceEndingCreditGridAndFont
    );
    assert_eq!(
        strategy
            .render_paths
            .iter()
            .find(|path| path.path == LocalizationRenderPath::EndingCreditFont)
            .unwrap()
            .verified_glyph_capacity,
        Some(30)
    );
}

#[test]
fn rejects_a_target_summary_that_drifted_from_the_source_catalog() {
    let error = plan_localization(&catalog(
        vec![item(
            "dialogue",
            LocalizationSourceCatalog::SelectDialogue,
            LocalizationTextDisposition::Target,
        )],
        0,
    ))
    .unwrap_err()
    .to_string();

    assert!(error.contains("target summary differs"), "{error}");
}
