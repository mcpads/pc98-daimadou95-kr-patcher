use std::collections::BTreeSet;

use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::source::{LocalizationCatalog, LocalizationSourceCatalog, LocalizationTextDisposition};

const FAMILY_ORDER: [LocalizationSourceCatalog; 12] = [
    LocalizationSourceCatalog::SelectDialogue,
    LocalizationSourceCatalog::OpeningText,
    LocalizationSourceCatalog::GraphicText,
    LocalizationSourceCatalog::EndingCredits,
    LocalizationSourceCatalog::EndingCurryResult,
    LocalizationSourceCatalog::EndingMealCaptions,
    LocalizationSourceCatalog::EndingMealReaction,
    LocalizationSourceCatalog::EndingInterludeCaption,
    LocalizationSourceCatalog::EndingIngredientLabels,
    LocalizationSourceCatalog::EndingPasswords,
    LocalizationSourceCatalog::DosProgramText,
    LocalizationSourceCatalog::MadIndexedText,
];

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationStrategy {
    pub summary: LocalizationStrategySummary,
    pub render_paths: Vec<LocalizationRenderPathSummary>,
    pub families: Vec<LocalizationFamilyPlan>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationStrategySummary {
    pub target_item_count: usize,
    pub assigned_item_count: usize,
    pub unassigned_item_count: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationRenderPathSummary {
    pub path: LocalizationRenderPath,
    pub target_item_count: usize,
    pub verified_glyph_capacity: Option<usize>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalizationFamilyPlan {
    pub source_catalog: LocalizationSourceCatalog,
    pub target_ids: Vec<String>,
    pub context: LocalizationContext,
    pub render_path: LocalizationRenderPath,
    pub glyph_supply: LocalizationGlyphSupply,
    pub source_layout: LocalizationSourceLayout,
    pub glyph_gate: LocalizationGlyphGate,
    pub reinsertion_gate: LocalizationReinsertionGate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationContext {
    SceneSpeakerAndSequence,
    OpeningSequence,
    GraphicSurface,
    EndingProgression,
    SystemMessageTrigger,
    UiOrBattleSpeaker,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationRenderPath {
    SharedIndexedFont,
    EndingCreditFont,
    DosSystemText,
    PlanarGraphics,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationGlyphSupply {
    SharedIndexedFont,
    EndingCreditFont,
    Pc98GaijiCandidate,
    DirectRasterGraphics,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationSourceLayout {
    PointerSelectedScript,
    ArchiveScript,
    ArchivePlanarGraphic,
    FixedEndingCreditGrid,
    FixedIndexedLine,
    FixedIndexedRecordsWithDynamicDate,
    FixedIndexedAnimationCaptions,
    FixedIndexedGlyphInstructions,
    FixedIndexedTable,
    PointerSelectedLinePool,
    DollarTerminatedDosText,
    IndexedPoolsAndBindings,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationGlyphGate {
    MeasureIndexedRepertoireAndLayout,
    MeasureEndingCreditRepertoireAndLayout,
    ProveGaijiResidencyAndMappedDemand,
    ProveGraphicRasterization,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalizationReinsertionGate {
    RelocatePointerSelectedScripts,
    RepackArchiveScript,
    ReencodeAndRepackArchiveGraphics,
    ReplaceEndingCreditGridAndFont,
    PreserveOrRelocateFixedIndexedLine,
    ReplaceFixedIndexedRecordsAndDateTemplates,
    ReplaceFixedIndexedAnimationCaptions,
    ReplaceFixedIndexedGlyphInstructions,
    ReplaceFixedIndexedTable,
    RelocatePointerSelectedLinePool,
    RelocateDosTextAndReferences,
    RelocateIndexedPoolsAndBindings,
}

pub fn plan_localization(catalog: &LocalizationCatalog) -> Result<LocalizationStrategy> {
    ensure!(
        catalog.summary.unresolved_text_item_count == 0,
        "cannot plan localization while the source catalog has unresolved text"
    );

    let targets = catalog
        .source_text
        .iter()
        .filter(|item| item.disposition == LocalizationTextDisposition::Target)
        .collect::<Vec<_>>();
    ensure!(
        targets.len() == catalog.summary.target_text_item_count,
        "localization target summary differs from the catalog population"
    );

    let unique_target_ids = targets
        .iter()
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    ensure!(
        unique_target_ids.len() == targets.len(),
        "localization strategy received duplicate target IDs"
    );

    for item in &targets {
        family_spec(item.source_catalog)
            .map_err(|error| anyhow::anyhow!("{} cannot be assigned: {error}", item.id))?;
    }

    let mut families = Vec::with_capacity(FAMILY_ORDER.len());
    for source_catalog in FAMILY_ORDER {
        let mut target_ids = targets
            .iter()
            .filter(|item| item.source_catalog == source_catalog)
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        if target_ids.is_empty() {
            continue;
        }
        target_ids.sort();
        let spec = family_spec(source_catalog)?;
        families.push(LocalizationFamilyPlan {
            source_catalog,
            target_ids,
            context: spec.context,
            render_path: spec.render_path,
            glyph_supply: spec.glyph_supply,
            source_layout: spec.source_layout,
            glyph_gate: spec.glyph_gate,
            reinsertion_gate: spec.reinsertion_gate,
        });
    }

    let assigned_item_count = families
        .iter()
        .map(|family| family.target_ids.len())
        .sum::<usize>();
    let unassigned_item_count = targets.len() - assigned_item_count;
    ensure!(
        unassigned_item_count == 0,
        "localization strategy left {unassigned_item_count} target items unassigned"
    );

    let render_paths = [
        (LocalizationRenderPath::SharedIndexedFont, Some(256)),
        (LocalizationRenderPath::EndingCreditFont, Some(30)),
        (LocalizationRenderPath::DosSystemText, Some(175)),
        (LocalizationRenderPath::PlanarGraphics, None),
    ]
    .into_iter()
    .map(
        |(path, verified_glyph_capacity)| LocalizationRenderPathSummary {
            path,
            target_item_count: families
                .iter()
                .filter(|family| family.render_path == path)
                .map(|family| family.target_ids.len())
                .sum(),
            verified_glyph_capacity,
        },
    )
    .collect::<Vec<_>>();
    ensure!(
        render_paths
            .iter()
            .all(|render_path| render_path.target_item_count > 0),
        "localization strategy contains an unused rendering path"
    );

    Ok(LocalizationStrategy {
        summary: LocalizationStrategySummary {
            target_item_count: targets.len(),
            assigned_item_count,
            unassigned_item_count,
        },
        render_paths,
        families,
    })
}

#[derive(Clone, Copy)]
struct FamilySpec {
    context: LocalizationContext,
    render_path: LocalizationRenderPath,
    glyph_supply: LocalizationGlyphSupply,
    source_layout: LocalizationSourceLayout,
    glyph_gate: LocalizationGlyphGate,
    reinsertion_gate: LocalizationReinsertionGate,
}

fn family_spec(source_catalog: LocalizationSourceCatalog) -> Result<FamilySpec> {
    use LocalizationContext as Context;
    use LocalizationGlyphGate as GlyphGate;
    use LocalizationGlyphSupply as GlyphSupply;
    use LocalizationReinsertionGate as ReinsertionGate;
    use LocalizationRenderPath as RenderPath;
    use LocalizationSourceCatalog as SourceCatalog;
    use LocalizationSourceLayout as SourceLayout;

    let spec = match source_catalog {
        SourceCatalog::SelectDialogue => FamilySpec {
            context: Context::SceneSpeakerAndSequence,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::PointerSelectedScript,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::RelocatePointerSelectedScripts,
        },
        SourceCatalog::OpeningText => FamilySpec {
            context: Context::OpeningSequence,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::ArchiveScript,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::RepackArchiveScript,
        },
        SourceCatalog::GraphicText => FamilySpec {
            context: Context::GraphicSurface,
            render_path: RenderPath::PlanarGraphics,
            glyph_supply: GlyphSupply::DirectRasterGraphics,
            source_layout: SourceLayout::ArchivePlanarGraphic,
            glyph_gate: GlyphGate::ProveGraphicRasterization,
            reinsertion_gate: ReinsertionGate::ReencodeAndRepackArchiveGraphics,
        },
        SourceCatalog::EndingCredits => FamilySpec {
            context: Context::EndingProgression,
            render_path: RenderPath::EndingCreditFont,
            glyph_supply: GlyphSupply::EndingCreditFont,
            source_layout: SourceLayout::FixedEndingCreditGrid,
            glyph_gate: GlyphGate::MeasureEndingCreditRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::ReplaceEndingCreditGridAndFont,
        },
        SourceCatalog::EndingInterludeCaption => FamilySpec {
            context: Context::EndingProgression,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::FixedIndexedLine,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::PreserveOrRelocateFixedIndexedLine,
        },
        SourceCatalog::EndingCurryResult => FamilySpec {
            context: Context::EndingProgression,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::FixedIndexedRecordsWithDynamicDate,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::ReplaceFixedIndexedRecordsAndDateTemplates,
        },
        SourceCatalog::EndingMealCaptions => FamilySpec {
            context: Context::EndingProgression,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::FixedIndexedAnimationCaptions,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::ReplaceFixedIndexedAnimationCaptions,
        },
        SourceCatalog::EndingMealReaction => FamilySpec {
            context: Context::EndingProgression,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::FixedIndexedGlyphInstructions,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::ReplaceFixedIndexedGlyphInstructions,
        },
        SourceCatalog::EndingIngredientLabels => FamilySpec {
            context: Context::EndingProgression,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::FixedIndexedTable,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::ReplaceFixedIndexedTable,
        },
        SourceCatalog::EndingPasswords => FamilySpec {
            context: Context::EndingProgression,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::PointerSelectedLinePool,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::RelocatePointerSelectedLinePool,
        },
        SourceCatalog::MstText => {
            bail!("MST shell output is outside the gameplay localization scope")
        }
        SourceCatalog::DosProgramText => FamilySpec {
            context: Context::SystemMessageTrigger,
            render_path: RenderPath::DosSystemText,
            glyph_supply: GlyphSupply::Pc98GaijiCandidate,
            source_layout: SourceLayout::DollarTerminatedDosText,
            glyph_gate: GlyphGate::ProveGaijiResidencyAndMappedDemand,
            reinsertion_gate: ReinsertionGate::RelocateDosTextAndReferences,
        },
        SourceCatalog::MadIndexedText => FamilySpec {
            context: Context::UiOrBattleSpeaker,
            render_path: RenderPath::SharedIndexedFont,
            glyph_supply: GlyphSupply::SharedIndexedFont,
            source_layout: SourceLayout::IndexedPoolsAndBindings,
            glyph_gate: GlyphGate::MeasureIndexedRepertoireAndLayout,
            reinsertion_gate: ReinsertionGate::RelocateIndexedPoolsAndBindings,
        },
    };
    Ok(spec)
}

#[cfg(test)]
#[path = "localization_tests.rs"]
mod localization_tests;
