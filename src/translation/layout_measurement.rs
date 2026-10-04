use anyhow::{Context, Result, ensure};
use encoding_rs::SHIFT_JIS;

use crate::source::LocalizationSourceCatalog;

use super::{
    TranslationContentPart, TranslationDraftEntry, TranslationEntry, TranslationLayoutFinding,
    TranslationLayoutFindingKind,
};

// The accepted source revision uses an 80-byte PC-98 graphics scanline and
// writes two bytes per indexed glyph. MAD callers choose their own x origin,
// so 40 is only the absolute screen boundary; their smaller boxes stay open.
const INDEXED_SCREEN_CELLS: usize = 40;
// MAD.COM menu group 03 is the six-row system menu. Runtime play confirms
// that its fixed window admits the original six indexed cells per row.
const MAD_SYSTEM_MENU_CELLS: usize = 6;
const DOS_SCREEN_COLUMNS: usize = 80;
// SELECT.COM $2082 starts at byte x=10, advances by four, and its renderer
// writes two bytes into an 80-byte scanline: positions 10 through 78 are 18 cells.
const SELECT_LINE_CELLS: usize = 18;
// ENDING.COM $2610 starts at byte x=32 and advances by two through x=78.
const ENDING_INTERLUDE_LINE_CELLS: usize = 24;
// ENDING.COM $3B28 selects one of three fixed 16-cell records per ingredient.
const ENDING_INGREDIENT_LABEL_CELLS: usize = 16;
// ENDING.COM $3B5A starts at byte x=24 and advances by two through x=78.
const ENDING_PASSWORD_LINE_CELLS: usize = 28;
// ENDING.COM $29B7 starts the selected evaluation text at byte x=20 and
// advances by two through x=78.
const ENDING_CURRY_RESULT_LINE_CELLS: usize = 30;
// ENDING.COM $2843 issues exactly three typed SI glyph loads to the 32x32
// renderer before the score-selected reaction animation.
const ENDING_MEAL_REACTION_CELLS: usize = 3;
// Each runtime-mutated date template has two digit cells followed by a
// two-byte source suffix and terminator. Korean owns one suffix cell and
// neutralizes the remaining byte.
const ENDING_DATE_SUFFIX_CELLS: usize = 1;
const ENDING_CREDIT_LINE_CELLS: usize = 20;

pub(super) struct EntryLayoutMeasurement {
    pub(super) units: Vec<MeasuredLayoutUnit>,
    pub(super) glyph_token_count: usize,
    pub(super) structural_findings: Vec<TranslationLayoutFinding>,
}

pub(super) struct MeasuredLayoutUnit {
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) start_column: usize,
    pub(super) source_cells: usize,
    pub(super) korean_cells: usize,
    pub(super) hard_limit_cells: Option<usize>,
}

pub(super) fn measure_translation_entry(
    segment_id: &str,
    source_catalog: LocalizationSourceCatalog,
    source: &TranslationEntry,
    draft: &TranslationDraftEntry,
) -> Result<EntryLayoutMeasurement> {
    ensure!(
        source.id == draft.id,
        "{} layout inputs name different entries",
        source.id
    );
    let text_pairs = paired_text_spans(source, draft)?;
    let mut structural_findings =
        embedded_line_break_findings(segment_id, source_catalog, &source.id, &text_pairs);
    let glyph_token_count = if source_catalog == LocalizationSourceCatalog::OpeningText {
        structural_findings.extend(opening_token_findings(
            segment_id,
            source_catalog,
            &source.id,
            &text_pairs,
        ));
        text_pairs.len()
    } else {
        0
    };

    let units = match source_catalog {
        LocalizationSourceCatalog::SelectDialogue => {
            measure_controlled_lines(source, &text_pairs, ControlledLineKind::Select)?
        }
        LocalizationSourceCatalog::OpeningText => {
            measure_controlled_lines(source, &text_pairs, ControlledLineKind::Opening)?
        }
        LocalizationSourceCatalog::EndingCurryResult => {
            measure_ending_curry_result(source, &text_pairs)?
        }
        LocalizationSourceCatalog::EndingMealCaptions => {
            measure_ending_meal_captions(source, &text_pairs)?
        }
        LocalizationSourceCatalog::MstText | LocalizationSourceCatalog::DosProgramText => {
            measure_dos_lines(source_catalog, source, &text_pairs)?
        }
        LocalizationSourceCatalog::MadIndexedText => measure_mad_indexed(source, &text_pairs),
        LocalizationSourceCatalog::EndingCredits => text_pairs
            .iter()
            .map(|pair| {
                measured_unit_with_limit(&pair.source, &pair.korean, 0, ENDING_CREDIT_LINE_CELLS)
            })
            .collect(),
        LocalizationSourceCatalog::GraphicText
        | LocalizationSourceCatalog::EndingInterludeCaption
        | LocalizationSourceCatalog::EndingMealReaction
        | LocalizationSourceCatalog::EndingIngredientLabels
        | LocalizationSourceCatalog::EndingPasswords => text_pairs
            .iter()
            .map(|pair| measured_unit(source_catalog, &pair.source, &pair.korean, 0))
            .collect(),
    };

    ensure!(
        !units.is_empty(),
        "{} translation layout produced no display units",
        source.id
    );
    Ok(EntryLayoutMeasurement {
        units,
        glyph_token_count,
        structural_findings,
    })
}

fn measure_mad_indexed(source: &TranslationEntry, pairs: &[TextPair]) -> Vec<MeasuredLayoutUnit> {
    let fixed_system_menu = source
        .consumer_contexts
        .iter()
        .any(|context| context.id == "menu-group-03");
    pairs
        .iter()
        .map(|pair| {
            if fixed_system_menu {
                measured_unit_with_limit(&pair.source, &pair.korean, 0, MAD_SYSTEM_MENU_CELLS)
            } else {
                measured_unit(
                    LocalizationSourceCatalog::MadIndexedText,
                    &pair.source,
                    &pair.korean,
                    0,
                )
            }
        })
        .collect()
}

pub(super) fn family_hard_limit(source_catalog: LocalizationSourceCatalog) -> Option<usize> {
    match source_catalog {
        LocalizationSourceCatalog::SelectDialogue => Some(SELECT_LINE_CELLS),
        LocalizationSourceCatalog::EndingInterludeCaption => Some(ENDING_INTERLUDE_LINE_CELLS),
        LocalizationSourceCatalog::EndingIngredientLabels => Some(ENDING_INGREDIENT_LABEL_CELLS),
        LocalizationSourceCatalog::EndingPasswords => Some(ENDING_PASSWORD_LINE_CELLS),
        LocalizationSourceCatalog::EndingCurryResult => Some(ENDING_CURRY_RESULT_LINE_CELLS),
        LocalizationSourceCatalog::EndingMealCaptions => None,
        LocalizationSourceCatalog::EndingMealReaction => Some(ENDING_MEAL_REACTION_CELLS),
        LocalizationSourceCatalog::EndingCredits => Some(ENDING_CREDIT_LINE_CELLS),
        LocalizationSourceCatalog::MstText | LocalizationSourceCatalog::DosProgramText => {
            Some(DOS_SCREEN_COLUMNS)
        }
        LocalizationSourceCatalog::MadIndexedText => Some(INDEXED_SCREEN_CELLS),
        LocalizationSourceCatalog::OpeningText | LocalizationSourceCatalog::GraphicText => None,
    }
}

fn measure_ending_meal_captions(
    source: &TranslationEntry,
    pairs: &[TextPair],
) -> Result<Vec<MeasuredLayoutUnit>> {
    let limits: &[usize] = match source.source_record_id.as_str() {
        "before-eating" => &[7],
        "ordinary-reaction" => &[6],
        "excellent-reaction" | "bad-reaction" => &[10],
        "legendary-reaction" => &[9, 12],
        other => anyhow::bail!("unknown ENDING meal-caption record {other}"),
    };
    ensure!(
        pairs.len() == limits.len(),
        "ENDING meal-caption {} has {} text spans but {} fixed limits",
        source.id,
        pairs.len(),
        limits.len()
    );
    Ok(pairs
        .iter()
        .zip(limits)
        .map(|(pair, limit)| measured_unit_with_limit(&pair.source, &pair.korean, 0, *limit))
        .collect())
}

struct TextPair {
    source: String,
    korean: String,
}

fn paired_text_spans(
    source: &TranslationEntry,
    draft: &TranslationDraftEntry,
) -> Result<Vec<TextPair>> {
    let source_texts = source
        .content
        .iter()
        .filter_map(|part| match part {
            TranslationContentPart::Text { source_text, .. } => Some(source_text),
            _ => None,
        })
        .collect::<Vec<_>>();
    ensure!(
        source_texts.len() == draft.korean_text.len(),
        "{} layout span count differs from the protected entry",
        source.id
    );
    source_texts
        .into_iter()
        .zip(&draft.korean_text)
        .map(|(source, korean)| {
            Ok(TextPair {
                source: source.clone(),
                korean: korean
                    .as_ref()
                    .with_context(|| format!("{} has an untranslated layout span", draft.id))?
                    .clone(),
            })
        })
        .collect()
}

fn embedded_line_break_findings(
    segment_id: &str,
    source_catalog: LocalizationSourceCatalog,
    entry_id: &str,
    pairs: &[TextPair],
) -> Vec<TranslationLayoutFinding> {
    pairs
        .iter()
        .enumerate()
        .filter(|(_, pair)| pair.korean.contains(['\r', '\n']))
        .map(|(index, pair)| TranslationLayoutFinding {
            kind: TranslationLayoutFindingKind::EmbeddedLineBreak,
            source_catalog,
            segment_id: segment_id.to_owned(),
            entry_id: entry_id.to_owned(),
            unit_index: index,
            start_column: 0,
            source_cells: pair.source.chars().count(),
            korean_cells: pair.korean.chars().count(),
            hard_limit_cells: None,
            source_text: pair.source.clone(),
            korean_text: pair.korean.clone(),
        })
        .collect()
}

fn opening_token_findings(
    segment_id: &str,
    source_catalog: LocalizationSourceCatalog,
    entry_id: &str,
    pairs: &[TextPair],
) -> Vec<TranslationLayoutFinding> {
    pairs
        .iter()
        .enumerate()
        .filter(|(_, pair)| pair.korean.chars().count() != 1)
        .map(|(index, pair)| TranslationLayoutFinding {
            kind: TranslationLayoutFindingKind::GlyphTokenWidth,
            source_catalog,
            segment_id: segment_id.to_owned(),
            entry_id: entry_id.to_owned(),
            unit_index: index,
            start_column: 0,
            source_cells: pair.source.chars().count(),
            korean_cells: pair.korean.chars().count(),
            hard_limit_cells: Some(1),
            source_text: pair.source.clone(),
            korean_text: pair.korean.clone(),
        })
        .collect()
}

#[derive(Clone, Copy)]
enum ControlledLineKind {
    Select,
    Opening,
    EndingCurryResult,
}

fn measure_ending_curry_result(
    source: &TranslationEntry,
    pairs: &[TextPair],
) -> Result<Vec<MeasuredLayoutUnit>> {
    let dynamic_decimal_cells = source.content.iter().find_map(|part| match part {
        TranslationContentPart::DynamicDecimal { cell_count } => Some(*cell_count),
        _ => None,
    });
    if let Some(cell_count) = dynamic_decimal_cells {
        ensure!(
            cell_count == 2 && pairs.len() == 1,
            "{} dynamic date must have two digit cells and one suffix span",
            source.id
        );
        let pair = &pairs[0];
        return Ok(vec![measured_unit_with_limit(
            &pair.source,
            &pair.korean,
            0,
            ENDING_DATE_SUFFIX_CELLS,
        )]);
    }
    measure_controlled_lines(source, pairs, ControlledLineKind::EndingCurryResult)
}

fn measure_controlled_lines(
    source: &TranslationEntry,
    pairs: &[TextPair],
    kind: ControlledLineKind,
) -> Result<Vec<MeasuredLayoutUnit>> {
    let mut pair_index = 0;
    let mut current = LineAccumulator::default();
    let mut units = Vec::new();
    for part in &source.content {
        match part {
            TranslationContentPart::Text { .. } => {
                let pair = pairs
                    .get(pair_index)
                    .context("layout text pair ended before protected content")?;
                pair_index += 1;
                current.source.push_str(&pair.source);
                current.korean.push_str(&pair.korean);
            }
            TranslationContentPart::IndexedLineBreak { .. }
            | TranslationContentPart::LineBreak
            | TranslationContentPart::Terminator => {
                flush_indexed_line(&mut current, &mut units, kind);
            }
            TranslationContentPart::Portrait { .. }
                if matches!(kind, ControlledLineKind::Select) =>
            {
                flush_indexed_line(&mut current, &mut units, kind);
            }
            TranslationContentPart::OpeningMode { .. }
            | TranslationContentPart::OpeningScroll
            | TranslationContentPart::OpeningEnd
                if matches!(kind, ControlledLineKind::Opening) =>
            {
                flush_indexed_line(&mut current, &mut units, kind);
            }
            _ => {}
        }
    }
    flush_indexed_line(&mut current, &mut units, kind);
    ensure!(
        pair_index == pairs.len(),
        "{} layout did not consume every protected text span",
        source.id
    );
    Ok(units)
}

#[derive(Default)]
struct LineAccumulator {
    source: String,
    korean: String,
    start_column: usize,
}

fn flush_indexed_line(
    current: &mut LineAccumulator,
    units: &mut Vec<MeasuredLayoutUnit>,
    kind: ControlledLineKind,
) {
    if current.source.is_empty() && current.korean.is_empty() {
        return;
    }
    let source_catalog = match kind {
        ControlledLineKind::Select => LocalizationSourceCatalog::SelectDialogue,
        ControlledLineKind::Opening => LocalizationSourceCatalog::OpeningText,
        ControlledLineKind::EndingCurryResult => LocalizationSourceCatalog::EndingCurryResult,
    };
    units.push(measured_unit(
        source_catalog,
        &current.source,
        &current.korean,
        current.start_column,
    ));
    *current = LineAccumulator::default();
}

fn measure_dos_lines(
    source_catalog: LocalizationSourceCatalog,
    source: &TranslationEntry,
    pairs: &[TextPair],
) -> Result<Vec<MeasuredLayoutUnit>> {
    let mut pair_index = 0;
    let mut current = LineAccumulator::default();
    let mut units = Vec::new();
    for part in &source.content {
        match part {
            TranslationContentPart::Text { .. } => {
                let pair = pairs
                    .get(pair_index)
                    .context("DOS layout text pair ended before protected content")?;
                pair_index += 1;
                current.source.push_str(&pair.source);
                current.korean.push_str(&pair.korean);
            }
            TranslationContentPart::AnsiControl { raw_hex } => {
                if let Some(column) = ansi_cursor_column(raw_hex)? {
                    flush_dos_line(source_catalog, &mut current, &mut units);
                    current.start_column = column;
                }
            }
            TranslationContentPart::CarriageReturn
            | TranslationContentPart::LineFeed
            | TranslationContentPart::Terminator => {
                flush_dos_line(source_catalog, &mut current, &mut units);
            }
            _ => {}
        }
    }
    flush_dos_line(source_catalog, &mut current, &mut units);
    ensure!(
        pair_index == pairs.len(),
        "{} DOS layout did not consume every protected text span",
        source.id
    );
    Ok(units)
}

fn flush_dos_line(
    source_catalog: LocalizationSourceCatalog,
    current: &mut LineAccumulator,
    units: &mut Vec<MeasuredLayoutUnit>,
) {
    if current.source.is_empty() && current.korean.is_empty() {
        current.start_column = 0;
        return;
    }
    units.push(measured_unit(
        source_catalog,
        &current.source,
        &current.korean,
        current.start_column,
    ));
    *current = LineAccumulator::default();
}

fn measured_unit(
    source_catalog: LocalizationSourceCatalog,
    source_text: &str,
    korean_text: &str,
    start_column: usize,
) -> MeasuredLayoutUnit {
    let (source_cells, korean_cells) = match source_catalog {
        LocalizationSourceCatalog::MstText | LocalizationSourceCatalog::DosProgramText => (
            dos_display_columns(source_text),
            dos_display_columns(korean_text),
        ),
        _ => (source_text.chars().count(), korean_text.chars().count()),
    };
    let hard_limit_cells =
        family_hard_limit(source_catalog).map(|limit| limit.saturating_sub(start_column));
    MeasuredLayoutUnit {
        source_text: source_text.to_owned(),
        korean_text: korean_text.to_owned(),
        start_column,
        source_cells,
        korean_cells,
        hard_limit_cells,
    }
}

fn measured_unit_with_limit(
    source_text: &str,
    korean_text: &str,
    start_column: usize,
    hard_limit_cells: usize,
) -> MeasuredLayoutUnit {
    MeasuredLayoutUnit {
        source_text: source_text.to_owned(),
        korean_text: korean_text.to_owned(),
        start_column,
        source_cells: source_text.chars().count(),
        korean_cells: korean_text.chars().count(),
        hard_limit_cells: Some(hard_limit_cells),
    }
}

fn dos_display_columns(text: &str) -> usize {
    text.chars()
        .map(|character| {
            let mut utf8 = [0_u8; 4];
            let value = character.encode_utf8(&mut utf8);
            let (encoded, _, had_errors) = SHIFT_JIS.encode(value);
            if had_errors { 2 } else { encoded.len() }
        })
        .sum()
}

fn ansi_cursor_column(raw_hex: &str) -> Result<Option<usize>> {
    ensure!(
        raw_hex.len().is_multiple_of(2),
        "ANSI control hex has an odd length"
    );
    let bytes = (0..raw_hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&raw_hex[index..index + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("ANSI control contains invalid hex")?;
    if bytes.len() < 3 || bytes[..2] != [0x1b, b'['] || !matches!(bytes.last(), Some(b'H' | b'f')) {
        return Ok(None);
    }
    let parameters = std::str::from_utf8(&bytes[2..bytes.len() - 1])
        .context("ANSI cursor parameters are not ASCII")?;
    let column = parameters
        .split(';')
        .nth(1)
        .filter(|value| !value.is_empty())
        .unwrap_or("1")
        .parse::<usize>()
        .context("ANSI cursor column is not a number")?;
    ensure!(column > 0, "ANSI cursor column must be one-based");
    Ok(Some(column - 1))
}

#[cfg(test)]
#[path = "layout_measurement_tests.rs"]
mod layout_measurement_tests;
