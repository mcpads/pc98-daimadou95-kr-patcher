use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};

use crate::source::{
    LocalizationSourceCatalog, LocalizationTextItem, MadIndexedBinding, SourceStructureReport,
};

pub(super) fn segment_id_for(
    item: &LocalizationTextItem,
    source: &SourceStructureReport,
) -> Result<String> {
    Ok(match item.source_catalog {
        LocalizationSourceCatalog::SelectDialogue => {
            let script = source
                .select_dialogue
                .stages
                .iter()
                .flat_map(|stage| &stage.scripts)
                .find(|script| script.id == item.source_record_id)
                .with_context(|| format!("{} names an unknown SELECT script", item.id))?;
            let stage = source
                .select_dialogue
                .stages
                .iter()
                .find(|stage| {
                    stage
                        .scripts
                        .iter()
                        .any(|candidate| candidate.id == script.id)
                })
                .expect("a selected script belongs to one stage");
            format!("select-stage-{:02}", stage.stage_number)
        }
        LocalizationSourceCatalog::OpeningText => "opening-sequence".to_owned(),
        LocalizationSourceCatalog::EndingCredits => "ending-credits".to_owned(),
        LocalizationSourceCatalog::GraphicText => {
            let surface_id = item
                .source_record_id
                .split_once(':')
                .map(|(surface_id, _)| surface_id)
                .context("graphic-text record ID has no surface prefix")?;
            format!("graphic-{surface_id}")
        }
        LocalizationSourceCatalog::EndingCurryResult => "ending-curry-result".to_owned(),
        LocalizationSourceCatalog::EndingMealCaptions => "ending-meal-captions".to_owned(),
        LocalizationSourceCatalog::EndingMealReaction => "ending-meal-reaction".to_owned(),
        LocalizationSourceCatalog::EndingInterludeCaption => "ending-interlude".to_owned(),
        LocalizationSourceCatalog::EndingIngredientLabels => {
            "ending-interlude-ingredients".to_owned()
        }
        LocalizationSourceCatalog::EndingPasswords => "ending-passwords".to_owned(),
        LocalizationSourceCatalog::MstText => {
            bail!("MST shell output is outside the gameplay localization scope")
        }
        LocalizationSourceCatalog::DosProgramText => format!(
            "dos-program-{}",
            safe_component(item.source_filename.trim_end_matches(".COM"))
        ),
        LocalizationSourceCatalog::MadIndexedText => mad_segment_id(item, source)?,
    })
}

fn mad_segment_id(item: &LocalizationTextItem, source: &SourceStructureReport) -> Result<String> {
    let bindings = mad_bindings(item, source)?;
    if item.source_record_id == "result-banner" {
        return Ok("mad-results".to_owned());
    }

    let battle_units = bindings
        .iter()
        .filter_map(|binding| battle_unit(&binding.id))
        .collect::<BTreeSet<_>>();
    if battle_units.len() > 1 {
        return Ok("mad-battle-shared".to_owned());
    }
    if let Some(unit) = battle_units.first() {
        return Ok(format!("mad-battle-unit-{unit:02}"));
    }

    let groups = bindings
        .iter()
        .map(|binding| mad_non_battle_group(&binding.id))
        .collect::<Result<BTreeSet<_>>>()?;
    if groups.len() > 1 {
        return Ok("mad-shared-ui".to_owned());
    }
    Ok(groups
        .first()
        .with_context(|| format!("{} has no MAD context group", item.id))?
        .to_string())
}

fn battle_unit(binding_id: &str) -> Option<u8> {
    [
        "attack-unit-",
        "damage-unit-",
        "actor-unit-",
        "opponent-unit-",
        "defeat-unit-",
    ]
    .into_iter()
    .find_map(|prefix| {
        binding_id
            .strip_prefix(prefix)
            .and_then(|suffix| suffix.split('-').next())
            .and_then(|unit| unit.parse().ok())
    })
}

fn mad_non_battle_group(binding_id: &str) -> Result<&'static str> {
    if binding_id.starts_with("unit-name-") || binding_id == "carbuncle-name" {
        Ok("mad-unit-names")
    } else if binding_id.starts_with("health-label-")
        || binding_id.starts_with("magic-label-")
        || binding_id.starts_with("progress-label-")
        || binding_id == "empty-meter"
    {
        Ok("mad-status-labels")
    } else if binding_id.starts_with("menu-action-")
        || binding_id.starts_with("menu-group-")
        || binding_id.starts_with("spell-")
    {
        Ok("mad-commands")
    } else if matches!(binding_id, "retry" | "return-to-title" | "result-banner") {
        Ok("mad-results")
    } else if binding_id.starts_with("terrain-")
        || matches!(
            binding_id,
            "friendly-side"
                | "enemy-side"
                | "spring-terrain"
                | "magic-recovery"
                | "whole-map"
                | "hurry"
                | "friendly-turn"
                | "enemy-turn"
                | "spring-captured"
                | "spring-lost"
                | "unit-rank-e"
        )
    {
        Ok("mad-map-and-system")
    } else {
        bail!("MAD binding {binding_id} has no semantic translation segment")
    }
}

pub(super) fn mad_bindings<'a>(
    item: &LocalizationTextItem,
    source: &'a SourceStructureReport,
) -> Result<Vec<&'a MadIndexedBinding>> {
    let bindings: Vec<&MadIndexedBinding> = if item.source_record_id == "result-banner" {
        source
            .mad_indexed_text
            .bindings
            .iter()
            .filter(|binding| binding.id == item.source_record_id)
            .collect()
    } else {
        source
            .mad_indexed_text
            .bindings
            .iter()
            .filter(|binding| binding.string_ids.contains(&item.source_record_id))
            .collect()
    };
    ensure!(
        !bindings.is_empty(),
        "{} has no MAD consumer binding",
        item.id
    );
    Ok(bindings)
}

pub(super) fn ensure_safe_segment_id(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty()
            && id
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
        "unsafe translation segment ID {id:?}"
    );
    Ok(())
}

fn safe_component(value: &str) -> String {
    value
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() {
                (byte as char).to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
