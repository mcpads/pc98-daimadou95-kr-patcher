use anyhow::{Context, Result, ensure};

use crate::source::{
    DosDisplayToken, IndexedTextToken, LocalizationSourceCatalog, LocalizationTextItem,
    OpeningTextToken, SourceStructureReport,
};

use super::segments::mad_bindings;
use super::{
    TranslationByteRecord, TranslationConsumerContext, TranslationContentPart, TranslationEntry,
    TranslationGraphicAsset, TranslationSourceEvidence, TranslationStatus,
};

pub(super) fn translation_entry(
    item: &LocalizationTextItem,
    source: &SourceStructureReport,
) -> Result<TranslationEntry> {
    let entry = TranslationEntry {
        id: item.id.clone(),
        source_filename: item.source_filename.clone(),
        source_record_id: item.source_record_id.clone(),
        evidence: evidence_for(item, source)?,
        consumer_contexts: contexts_for(item, source)?,
        content: content_for(item, source)?,
        status: TranslationStatus::Untranslated,
        notes: None,
    };
    validate_protected_entry(&entry)?;
    Ok(entry)
}

fn evidence_for(
    item: &LocalizationTextItem,
    source: &SourceStructureReport,
) -> Result<TranslationSourceEvidence> {
    let mut byte_records = Vec::new();
    let mut graphic_asset = None;
    match item.source_catalog {
        LocalizationSourceCatalog::SelectDialogue => {
            let record = source
                .select_dialogue
                .stages
                .iter()
                .flat_map(|stage| &stage.scripts)
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no SELECT source record", item.id))?;
            byte_records.push(TranslationByteRecord {
                id: record.id.clone(),
                file_offset: record.file_offset,
                byte_size: record.byte_size,
                sha256: record.sha256.clone(),
                raw_hex: record.raw_hex.clone(),
            });
        }
        LocalizationSourceCatalog::OpeningText => {
            byte_records.push(TranslationByteRecord {
                id: item.source_record_id.clone(),
                file_offset: maddat_entry_offset(source, source.opening_text.maddat_entry_id)?,
                byte_size: source.opening_text.byte_size,
                sha256: source.opening_text.sha256.clone(),
                raw_hex: source.opening_text.raw_hex.clone(),
            });
        }
        LocalizationSourceCatalog::GraphicText => {
            let surface_id = item
                .source_record_id
                .split_once(':')
                .map(|(surface_id, _)| surface_id)
                .context("graphic-text record ID has no surface prefix")?;
            let surface = source
                .graphic_text
                .surfaces
                .iter()
                .find(|surface| surface.id == surface_id)
                .with_context(|| format!("{} has no graphic source surface", item.id))?;
            graphic_asset = Some(TranslationGraphicAsset {
                maddat_entry_id: surface.maddat_entry_id,
                entry_file_offset: maddat_entry_offset(source, surface.maddat_entry_id)?,
                packed_size: surface.packed_size,
                packed_sha256: surface.packed_sha256.clone(),
                decoded_size: surface.decoded_size,
                decoded_sha256: surface.decoded_sha256.clone(),
                width: surface.width,
                height: surface.height,
                plane_count: surface.plane_count,
            });
        }
        LocalizationSourceCatalog::EndingInterludeCaption => {
            let record = &source.ending_interlude_caption;
            byte_records.push(TranslationByteRecord {
                id: item.source_record_id.clone(),
                file_offset: record.file_offset,
                byte_size: record.byte_size,
                sha256: record.sha256.clone(),
                raw_hex: record.raw_hex.clone(),
            });
        }
        LocalizationSourceCatalog::EndingCurryResult => {
            if let Some(record) = source
                .ending_curry_result
                .messages
                .iter()
                .find(|record| record.id == item.source_record_id)
            {
                byte_records.push(TranslationByteRecord {
                    id: record.id.clone(),
                    file_offset: record.file_offset,
                    byte_size: record.byte_size,
                    sha256: record.sha256.clone(),
                    raw_hex: record.raw_hex.clone(),
                });
            } else {
                let record = [
                    &source.ending_curry_result.month,
                    &source.ending_curry_result.day,
                ]
                .into_iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING curry-result record", item.id))?;
                byte_records.push(TranslationByteRecord {
                    id: record.id.clone(),
                    file_offset: record.file_offset,
                    byte_size: record.byte_size,
                    sha256: record.sha256.clone(),
                    raw_hex: record.raw_hex.clone(),
                });
            }
        }
        LocalizationSourceCatalog::EndingMealCaptions => {
            let record = source
                .ending_meal_captions
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING meal-caption record", item.id))?;
            byte_records.push(TranslationByteRecord {
                id: record.id.clone(),
                file_offset: record.file_offset,
                byte_size: record.byte_size,
                sha256: record.sha256.clone(),
                raw_hex: record.raw_hex.clone(),
            });
        }
        LocalizationSourceCatalog::EndingMealReaction => {
            ensure!(
                item.source_record_id == source.ending_meal_reaction.id,
                "{} names an unknown ENDING meal-reaction record",
                item.id
            );
            for load in &source.ending_meal_reaction.glyph_loads {
                byte_records.push(TranslationByteRecord {
                    id: format!("{}:slot-{:02}", item.source_record_id, load.slot_number),
                    file_offset: load.file_offset,
                    byte_size: load.byte_size,
                    sha256: load.sha256.clone(),
                    raw_hex: load.raw_hex.clone(),
                });
            }
        }
        LocalizationSourceCatalog::EndingIngredientLabels => {
            let record = source
                .ending_ingredient_labels
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING ingredient record", item.id))?;
            byte_records.push(TranslationByteRecord {
                id: record.id.clone(),
                file_offset: record.file_offset,
                byte_size: record.byte_size,
                sha256: record.sha256.clone(),
                raw_hex: record.raw_hex.clone(),
            });
        }
        LocalizationSourceCatalog::EndingPasswords => {
            let record = source
                .ending_passwords
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING password record", item.id))?;
            byte_records.push(TranslationByteRecord {
                id: record.id.clone(),
                file_offset: record.file_offset,
                byte_size: record.byte_size,
                sha256: record.sha256.clone(),
                raw_hex: record.raw_hex.clone(),
            });
        }
        LocalizationSourceCatalog::MstText => {
            let record = source
                .mst_text
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no MST source record", item.id))?;
            byte_records.push(TranslationByteRecord {
                id: record.id.clone(),
                file_offset: record.file_offset,
                byte_size: record.byte_size,
                sha256: record.sha256.clone(),
                raw_hex: record.raw_hex.clone(),
            });
        }
        LocalizationSourceCatalog::DosProgramText => {
            let program = source
                .dos_program_text
                .programs
                .iter()
                .find(|program| program.filename == item.source_filename)
                .with_context(|| format!("{} has no DOS source program", item.id))?;
            let record = program
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no DOS source record", item.id))?;
            byte_records.push(TranslationByteRecord {
                id: record.id.clone(),
                file_offset: record.file_offset,
                byte_size: record.byte_size,
                sha256: record.sha256.clone(),
                raw_hex: record.raw_hex.clone(),
            });
        }
        LocalizationSourceCatalog::MadIndexedText => {
            if item.source_record_id == "result-banner" {
                let binding = source
                    .mad_indexed_text
                    .bindings
                    .iter()
                    .find(|binding| binding.id == item.source_record_id)
                    .context("MAD result banner binding is missing")?;
                for record_id in &binding.string_ids {
                    let record = source
                        .mad_indexed_text
                        .strings
                        .iter()
                        .find(|record| &record.id == record_id)
                        .with_context(|| format!("result banner record {record_id} is missing"))?;
                    byte_records.push(TranslationByteRecord {
                        id: record.id.clone(),
                        file_offset: record.file_offset,
                        byte_size: record.byte_size,
                        sha256: record.sha256.clone(),
                        raw_hex: record.raw_hex.clone(),
                    });
                }
            } else {
                let record = source
                    .mad_indexed_text
                    .strings
                    .iter()
                    .find(|record| record.id == item.source_record_id)
                    .with_context(|| format!("{} has no MAD indexed source record", item.id))?;
                byte_records.push(TranslationByteRecord {
                    id: record.id.clone(),
                    file_offset: record.file_offset,
                    byte_size: record.byte_size,
                    sha256: record.sha256.clone(),
                    raw_hex: record.raw_hex.clone(),
                });
            }
        }
        LocalizationSourceCatalog::EndingCredits => {
            let (_, line) = ending_credit_line(item, source)?;
            byte_records.push(TranslationByteRecord {
                id: item.source_record_id.clone(),
                file_offset: line.file_offset,
                byte_size: line.byte_size,
                sha256: line.sha256.clone(),
                raw_hex: line.raw_hex.clone(),
            });
        }
    }
    Ok(TranslationSourceEvidence {
        byte_records,
        graphic_asset,
    })
}

fn contexts_for(
    item: &LocalizationTextItem,
    source: &SourceStructureReport,
) -> Result<Vec<TranslationConsumerContext>> {
    let contexts = match item.source_catalog {
        LocalizationSourceCatalog::SelectDialogue => {
            let record = source
                .select_dialogue
                .stages
                .iter()
                .flat_map(|stage| &stage.scripts)
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no SELECT context", item.id))?;
            vec![TranslationConsumerContext {
                id: record.id.clone(),
                role: format!(
                    "stage dialogue sequence; initial portrait {:02x}:{:02x}",
                    record.initial_portrait[0], record.initial_portrait[1]
                ),
            }]
        }
        LocalizationSourceCatalog::OpeningText => vec![TranslationConsumerContext {
            id: "opening-sequence".to_owned(),
            role: "timed opening text and sprite sequence".to_owned(),
        }],
        LocalizationSourceCatalog::GraphicText => {
            let surface_id = item
                .source_record_id
                .split_once(':')
                .map(|(surface_id, _)| surface_id)
                .context("graphic-text record ID has no surface prefix")?;
            let surface = source
                .graphic_text
                .surfaces
                .iter()
                .find(|surface| surface.id == surface_id)
                .with_context(|| format!("{} has no graphic context", item.id))?;
            vec![TranslationConsumerContext {
                id: surface.id.clone(),
                role: format!(
                    "{}x{} {}-plane graphic surface",
                    surface.width, surface.height, surface.plane_count
                ),
            }]
        }
        LocalizationSourceCatalog::EndingInterludeCaption => {
            vec![TranslationConsumerContext {
                id: "ending-interlude".to_owned(),
                role: "ending progression interlude caption".to_owned(),
            }]
        }
        LocalizationSourceCatalog::EndingCurryResult => {
            if let Some(record) = source
                .ending_curry_result
                .messages
                .iter()
                .find(|record| record.id == item.source_record_id)
            {
                vec![TranslationConsumerContext {
                    id: record.id.clone(),
                    role: format!(
                        "ending current-date curry evaluation selected by result class {}",
                        record.result_class
                    ),
                }]
            } else {
                let record = [
                    &source.ending_curry_result.month,
                    &source.ending_curry_result.day,
                ]
                .into_iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING date context", item.id))?;
                vec![TranslationConsumerContext {
                    id: record.id.clone(),
                    role: format!(
                        "runtime-mutated ending date {:?} suffix after two decimal cells",
                        record.unit
                    ),
                }]
            }
        }
        LocalizationSourceCatalog::EndingMealCaptions => {
            let record = source
                .ending_meal_captions
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING meal-caption context", item.id))?;
            vec![TranslationConsumerContext {
                id: record.id.clone(),
                role: format!(
                    "fixed progressive ending caption command stream with {} editable phase(s); renderer {:#x}",
                    record.spans.len(),
                    source
                        .ending_meal_captions
                        .consumer
                        .glyph_renderer_runtime_address
                ),
            }]
        }
        LocalizationSourceCatalog::EndingMealReaction => {
            ensure!(
                item.source_record_id == source.ending_meal_reaction.id,
                "{} has no ENDING meal-reaction context",
                item.id
            );
            vec![TranslationConsumerContext {
                id: item.source_record_id.clone(),
                role: format!(
                    "three fixed typed V30 glyph loads rendered progressively at 32x32 before the score-selected facial reaction; renderer {:#x}",
                    source
                        .ending_meal_reaction
                        .consumer
                        .glyph_renderer_runtime_address
                ),
            }]
        }
        LocalizationSourceCatalog::EndingIngredientLabels => {
            let record = source
                .ending_ingredient_labels
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING ingredient context", item.id))?;
            vec![TranslationConsumerContext {
                id: record.id.clone(),
                role: format!(
                    "ending interlude input-selected ingredient label; slot {}, variant {}",
                    record.group_number, record.variant_number
                ),
            }]
        }
        LocalizationSourceCatalog::EndingPasswords => {
            let record = source
                .ending_passwords
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING password context", item.id))?;
            vec![TranslationConsumerContext {
                id: record.id.clone(),
                role: format!("ending selector value {}", record.selector_value),
            }]
        }
        LocalizationSourceCatalog::MstText => {
            let record = source
                .mst_text
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no MST context", item.id))?;
            vec![TranslationConsumerContext {
                id: record.id.clone(),
                role: record.consumer.clone(),
            }]
        }
        LocalizationSourceCatalog::DosProgramText => {
            let program = source
                .dos_program_text
                .programs
                .iter()
                .find(|program| program.filename == item.source_filename)
                .with_context(|| format!("{} has no DOS program context", item.id))?;
            let record = program
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no DOS record context", item.id))?;
            vec![TranslationConsumerContext {
                id: format!("{}:{}", program.filename, record.id),
                role: format!(
                    "DOS AH=09h text with {} verified reference(s)",
                    record.references.len()
                ),
            }]
        }
        LocalizationSourceCatalog::MadIndexedText => mad_bindings(item, source)?
            .into_iter()
            .map(|binding| TranslationConsumerContext {
                id: binding.id.clone(),
                role: binding.consumer.clone(),
            })
            .collect(),
        LocalizationSourceCatalog::EndingCredits => {
            let (screen, line) = ending_credit_line(item, source)?;
            vec![TranslationConsumerContext {
                id: item.source_record_id.clone(),
                role: format!(
                    "ending credit role label on screen {}, line {}; 20 cells rendered through MADDAT {} with the 24x32 glyph consumer at {:#x}",
                    screen.screen_number,
                    line.line_number,
                    source.ending_credits.text.maddat_entry_id,
                    source
                        .ending_credits
                        .consumer
                        .glyph_renderer_runtime_address
                ),
            }]
        }
    };
    ensure!(!contexts.is_empty(), "{} has no consumer context", item.id);
    Ok(contexts)
}

fn content_for(
    item: &LocalizationTextItem,
    source: &SourceStructureReport,
) -> Result<Vec<TranslationContentPart>> {
    let mut parts = Vec::new();
    match item.source_catalog {
        LocalizationSourceCatalog::SelectDialogue => {
            let record = source
                .select_dialogue
                .stages
                .iter()
                .flat_map(|stage| &stage.scripts)
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no SELECT content", item.id))?;
            for token in &record.tokens {
                match token {
                    IndexedTextToken::Glyph { text, .. } => push_source_text(&mut parts, text),
                    IndexedTextToken::Voice { parameter } => {
                        parts.push(TranslationContentPart::Voice {
                            parameter: *parameter,
                        })
                    }
                    IndexedTextToken::Portrait { parameters } => {
                        parts.push(TranslationContentPart::Portrait {
                            parameters: *parameters,
                        });
                    }
                    IndexedTextToken::LineBreak { parameters } => {
                        parts.push(TranslationContentPart::IndexedLineBreak {
                            parameters: *parameters,
                        });
                    }
                    IndexedTextToken::Terminator => {
                        parts.push(TranslationContentPart::Terminator);
                    }
                }
            }
        }
        LocalizationSourceCatalog::OpeningText => {
            for token in &source.opening_text.tokens {
                match token {
                    OpeningTextToken::Mode { value } => {
                        parts.push(TranslationContentPart::OpeningMode { value: *value });
                    }
                    OpeningTextToken::Sprite {
                        glyph_index,
                        position,
                    } => parts.push(TranslationContentPart::OpeningSprite {
                        glyph_index: *glyph_index,
                        position: *position,
                    }),
                    OpeningTextToken::Glyph { text, .. } => push_source_text(&mut parts, text),
                    OpeningTextToken::LineBreak => {
                        parts.push(TranslationContentPart::LineBreak);
                    }
                    OpeningTextToken::Scroll => {
                        parts.push(TranslationContentPart::OpeningScroll);
                    }
                    OpeningTextToken::Yield => {
                        parts.push(TranslationContentPart::OpeningYield);
                    }
                    OpeningTextToken::End => parts.push(TranslationContentPart::OpeningEnd),
                }
            }
        }
        LocalizationSourceCatalog::MstText => {
            let record = source
                .mst_text
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no MST content", item.id))?;
            append_dos_content(&mut parts, &record.tokens);
        }
        LocalizationSourceCatalog::EndingCurryResult => {
            if let Some(record) = source
                .ending_curry_result
                .messages
                .iter()
                .find(|record| record.id == item.source_record_id)
            {
                for (line_index, line) in record.lines.iter().enumerate() {
                    push_source_text(&mut parts, line);
                    if let Some(count) = record.line_break_counts.get(line_index) {
                        for _ in 0..*count {
                            parts.push(TranslationContentPart::LineBreak);
                        }
                    }
                }
                parts.push(TranslationContentPart::Terminator);
            } else {
                let record = [
                    &source.ending_curry_result.month,
                    &source.ending_curry_result.day,
                ]
                .into_iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING date content", item.id))?;
                parts.push(TranslationContentPart::DynamicDecimal {
                    cell_count: record.initial_digit_indices.len(),
                });
                push_source_text(&mut parts, &record.suffix_text);
                parts.push(TranslationContentPart::Terminator);
            }
        }
        LocalizationSourceCatalog::EndingMealCaptions => {
            let record = source
                .ending_meal_captions
                .records
                .iter()
                .find(|record| record.id == item.source_record_id)
                .with_context(|| format!("{} has no ENDING meal-caption content", item.id))?;
            for span in &record.spans {
                parts.push(TranslationContentPart::Text {
                    source_text: span.text.clone(),
                    korean_text: None,
                });
            }
        }
        LocalizationSourceCatalog::EndingMealReaction => {
            ensure!(
                item.source_record_id == source.ending_meal_reaction.id,
                "{} has no ENDING meal-reaction content",
                item.id
            );
            push_source_text(&mut parts, &source.ending_meal_reaction.text);
        }
        LocalizationSourceCatalog::DosProgramText => {
            let record = source
                .dos_program_text
                .programs
                .iter()
                .find(|program| program.filename == item.source_filename)
                .and_then(|program| {
                    program
                        .records
                        .iter()
                        .find(|record| record.id == item.source_record_id)
                })
                .with_context(|| format!("{} has no DOS content", item.id))?;
            append_dos_content(&mut parts, &record.tokens);
        }
        LocalizationSourceCatalog::GraphicText
        | LocalizationSourceCatalog::EndingCredits
        | LocalizationSourceCatalog::EndingInterludeCaption
        | LocalizationSourceCatalog::EndingIngredientLabels
        | LocalizationSourceCatalog::EndingPasswords
        | LocalizationSourceCatalog::MadIndexedText => {
            push_source_text(&mut parts, &item.text);
            if !matches!(
                item.source_catalog,
                LocalizationSourceCatalog::GraphicText
                    | LocalizationSourceCatalog::EndingCredits
                    | LocalizationSourceCatalog::EndingIngredientLabels
            ) && item.source_record_id != "result-banner"
            {
                parts.push(TranslationContentPart::Terminator);
            }
        }
    }
    ensure!(
        parts
            .iter()
            .any(|part| matches!(part, TranslationContentPart::Text { .. })),
        "{} has no editable text span",
        item.id
    );
    Ok(parts)
}

fn ending_credit_line<'a>(
    item: &LocalizationTextItem,
    source: &'a SourceStructureReport,
) -> Result<(
    &'a crate::source::EndingCreditScreen,
    &'a crate::source::EndingCreditLine,
)> {
    for screen in &source.ending_credits.text.screens {
        for line in &screen.lines {
            let record_id = format!(
                "screen-{:02}-line-{:02}",
                screen.screen_number, line.line_number
            );
            if record_id == item.source_record_id {
                ensure!(
                    line.requires_localization
                        && line.reviewed_role_text.as_deref() == Some(item.text.as_str()),
                    "{} does not bind a reviewed ending-credit role line",
                    item.id
                );
                return Ok((screen, line));
            }
        }
    }
    anyhow::bail!("{} has no ending-credit source record", item.id)
}

fn append_dos_content(parts: &mut Vec<TranslationContentPart>, tokens: &[DosDisplayToken]) {
    for token in tokens {
        match token {
            DosDisplayToken::AnsiControl { raw_hex } => {
                parts.push(TranslationContentPart::AnsiControl {
                    raw_hex: raw_hex.clone(),
                });
            }
            DosDisplayToken::CarriageReturn => {
                parts.push(TranslationContentPart::CarriageReturn);
            }
            DosDisplayToken::LineFeed => parts.push(TranslationContentPart::LineFeed),
            DosDisplayToken::SingleByte { text, .. } | DosDisplayToken::ShiftJis { text, .. } => {
                push_source_text(parts, text)
            }
            DosDisplayToken::Terminator => parts.push(TranslationContentPart::Terminator),
        }
    }
}

fn push_source_text(parts: &mut Vec<TranslationContentPart>, text: &str) {
    if let Some(TranslationContentPart::Text { source_text, .. }) = parts.last_mut() {
        source_text.push_str(text);
    } else {
        parts.push(TranslationContentPart::Text {
            source_text: text.to_owned(),
            korean_text: None,
        });
    }
}

fn maddat_entry_offset(source: &SourceStructureReport, entry_id: usize) -> Result<usize> {
    source
        .maddat
        .entries
        .iter()
        .find(|entry| entry.id == entry_id)
        .map(|entry| entry.offset)
        .with_context(|| format!("MADDAT entry {entry_id} is missing"))
}

fn validate_protected_entry(entry: &TranslationEntry) -> Result<()> {
    ensure!(
        !entry.evidence.byte_records.is_empty() ^ entry.evidence.graphic_asset.is_some(),
        "{} must have byte-record evidence or one graphic asset",
        entry.id
    );
    for record in &entry.evidence.byte_records {
        ensure!(
            record.raw_hex.len() == record.byte_size * 2,
            "{} source record {} raw length differs from its byte size",
            entry.id,
            record.id
        );
        ensure!(
            record.sha256.len() == 64,
            "{} source record {} has an invalid SHA-256",
            entry.id,
            record.id
        );
    }
    if let Some(asset) = &entry.evidence.graphic_asset {
        ensure!(
            asset.packed_sha256.len() == 64 && asset.decoded_sha256.len() == 64,
            "{} graphic evidence has an invalid SHA-256",
            entry.id
        );
    }
    ensure!(
        !entry.consumer_contexts.is_empty(),
        "{} has no consumer context",
        entry.id
    );
    Ok(())
}
