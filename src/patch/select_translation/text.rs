use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::game_data::indexed_text::{self, IndexedTextToken};

const COM_ORIGIN: usize = 0x100;
const SELECT_LINE_CELL_LIMIT: usize = 18;
const VOICE: u8 = 0xfa;
const PORTRAIT: u8 = 0xfd;
const LINE_BREAK: u8 = 0xfe;
const TERMINATOR: u8 = 0xff;

pub(super) struct SelectScriptTranslation {
    pub entry_id: String,
    pub spans: Vec<String>,
}

pub(super) struct SelectStageTranslation {
    pub stage_number: usize,
    pub scripts: Vec<SelectScriptTranslation>,
}

pub(super) struct PatchedSelectText {
    pub bytes: Vec<u8>,
    pub pointer_table_file_offset: usize,
    pub script_pool_file_offset: usize,
    pub script_pool_capacity: usize,
    pub script_pool_used: usize,
}

pub(super) fn replace_select_dialogue(
    bytes: &[u8],
    translations: &[SelectStageTranslation],
    codebooks: &[BTreeMap<char, u8>],
) -> Result<PatchedSelectText> {
    let dialogue = indexed_text::parse_select_dialogue(bytes)?;
    ensure!(
        translations.len() == dialogue.stages.len() && codebooks.len() == dialogue.stages.len(),
        "SELECT translation and font-bank counts must match its {} stages",
        dialogue.stages.len()
    );
    let script_count: usize = dialogue
        .stages
        .iter()
        .map(|stage| stage.scripts.len())
        .sum();
    let pointer_table_size = script_count * 2;
    let pointer_table_end = dialogue.pointer_table_file_offset + pointer_table_size;
    let script_pool_file_offset = dialogue
        .stages
        .first()
        .and_then(|stage| stage.scripts.first())
        .map(|script| script.file_offset)
        .context("SELECT dialogue catalog has no scripts")?;
    ensure!(
        script_pool_file_offset == pointer_table_end,
        "SELECT script pool no longer begins after its pointer table"
    );
    let script_pool_end = dialogue
        .stages
        .last()
        .and_then(|stage| stage.scripts.last())
        .map(|script| script.file_offset + script.bytes.len())
        .context("SELECT dialogue catalog has no final script")?;
    let script_pool_capacity = script_pool_end - script_pool_file_offset;
    let mut pointer_replacement = Vec::with_capacity(pointer_table_size);
    let mut pool_replacement = vec![TERMINATOR; script_pool_capacity];
    let mut cursor = 0_usize;

    for ((source_stage, translation_stage), codebook) in
        dialogue.stages.iter().zip(translations).zip(codebooks)
    {
        ensure!(
            translation_stage.stage_number == source_stage.stage_index + 1,
            "SELECT translation stage order changed at stage {}",
            source_stage.stage_index + 1
        );
        ensure!(
            translation_stage.scripts.len() == source_stage.scripts.len(),
            "SELECT stage {} supplies {} scripts for {} source phases",
            translation_stage.stage_number,
            translation_stage.scripts.len(),
            source_stage.scripts.len()
        );
        for (source_script, translation) in
            source_stage.scripts.iter().zip(&translation_stage.scripts)
        {
            let expected_id = format!(
                "select-dialogue:stage-{:02}-phase-{:02}",
                translation_stage.stage_number,
                source_script.phase_index + 1
            );
            ensure!(
                translation.entry_id == expected_id,
                "SELECT translation entry order changed: expected {expected_id}, got {}",
                translation.entry_id
            );
            let encoded = encode_script(source_script, translation, codebook)?;
            let end = cursor
                .checked_add(encoded.len())
                .context("SELECT translated script-pool cursor overflow")?;
            ensure!(
                end <= script_pool_capacity,
                "SELECT translated scripts require {end} bytes but the verified pool has {script_pool_capacity}"
            );
            let runtime_address = script_pool_file_offset
                .checked_add(cursor)
                .and_then(|offset| offset.checked_add(COM_ORIGIN))
                .context("SELECT translated script runtime address overflow")?;
            pointer_replacement.extend_from_slice(
                &u16::try_from(runtime_address)
                    .context("SELECT translated script runtime address exceeds 16 bits")?
                    .to_le_bytes(),
            );
            pool_replacement[cursor..end].copy_from_slice(&encoded);
            cursor = end;
        }
    }

    let plan = WritePlan::new()
        .region(ImageRegion {
            id: "select-dialogue-pointer-table".into(),
            range: dialogue.pointer_table_file_offset..pointer_table_end,
            kind: RegionKind::Metadata,
            reason: "stage and phase pointers into the repacked SELECT dialogue pool".into(),
        })
        .region(ImageRegion {
            id: "select-dialogue-script-pool".into(),
            range: script_pool_file_offset..script_pool_end,
            kind: RegionKind::Data,
            reason: "all translated SELECT scripts and a neutralized unused tail".into(),
        })
        .write(ExpectedWrite {
            id: "write-select-dialogue-pointers".into(),
            owner: "select-dialogue-compiler".into(),
            purpose: "point every SELECT stage and phase at its translated script".into(),
            offset: dialogue.pointer_table_file_offset,
            expected_original: bytes[dialogue.pointer_table_file_offset..pointer_table_end]
                .to_vec(),
            replacement: pointer_replacement.clone(),
            intent: WriteIntent::Metadata,
        })
        .write(ExpectedWrite {
            id: "write-select-dialogue-pool".into(),
            owner: "select-dialogue-compiler".into(),
            purpose: "replace all SELECT dialogue and clear unreachable source text".into(),
            offset: script_pool_file_offset,
            expected_original: bytes[script_pool_file_offset..script_pool_end].to_vec(),
            replacement: pool_replacement.clone(),
            intent: WriteIntent::Data,
        });
    let output = plan.apply(bytes, None)?;
    ensure!(
        output[dialogue.pointer_table_file_offset..pointer_table_end] == pointer_replacement,
        "SELECT dialogue pointers did not survive their Expected Write"
    );
    ensure!(
        output[script_pool_file_offset..script_pool_end] == pool_replacement,
        "SELECT dialogue pool did not survive its Expected Write"
    );
    ensure!(
        pool_replacement[cursor..]
            .iter()
            .all(|byte| *byte == TERMINATOR),
        "SELECT dialogue pool tail contains source text"
    );

    Ok(PatchedSelectText {
        bytes: output,
        pointer_table_file_offset: dialogue.pointer_table_file_offset,
        script_pool_file_offset,
        script_pool_capacity,
        script_pool_used: cursor,
    })
}

fn encode_script(
    source: &indexed_text::SelectDialogueScript<'_>,
    translation: &SelectScriptTranslation,
    codebook: &BTreeMap<char, u8>,
) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    output.extend_from_slice(&source.initial_portrait);
    let mut span_index = 0_usize;
    let mut in_source_glyph_run = false;
    let mut line_cells = 0_usize;
    for token in &source.tokens {
        match token {
            IndexedTextToken::Glyph { .. } => {
                if !in_source_glyph_run {
                    let text = translation.spans.get(span_index).with_context(|| {
                        format!(
                            "SELECT entry {} is missing Korean span {}",
                            translation.entry_id,
                            span_index + 1
                        )
                    })?;
                    for character in text.chars() {
                        let index = codebook.get(&character).copied().with_context(|| {
                            format!(
                                "SELECT entry {} codebook is missing {character:?}",
                                translation.entry_id
                            )
                        })?;
                        ensure!(
                            index < VOICE,
                            "SELECT entry {} maps {character:?} to reserved control {index:02X}",
                            translation.entry_id
                        );
                        output.push(index);
                        line_cells += 1;
                        ensure!(
                            line_cells <= SELECT_LINE_CELL_LIMIT,
                            "SELECT entry {} exceeds its {SELECT_LINE_CELL_LIMIT}-cell line",
                            translation.entry_id
                        );
                    }
                    span_index += 1;
                    in_source_glyph_run = true;
                }
            }
            IndexedTextToken::Voice(parameter) => {
                in_source_glyph_run = false;
                output.extend_from_slice(&[VOICE, *parameter]);
            }
            IndexedTextToken::Portrait(parameters) => {
                in_source_glyph_run = false;
                line_cells = 0;
                output.push(PORTRAIT);
                output.extend_from_slice(parameters);
            }
            IndexedTextToken::LineBreak(parameters) => {
                in_source_glyph_run = false;
                line_cells = 0;
                output.push(LINE_BREAK);
                output.extend_from_slice(parameters);
            }
            IndexedTextToken::Terminator => {
                in_source_glyph_run = false;
                output.push(TERMINATOR);
            }
        }
    }
    ensure!(
        span_index == translation.spans.len(),
        "SELECT entry {} has {} Korean spans for {span_index} source text runs",
        translation.entry_id,
        translation.spans.len()
    );
    ensure!(
        output.last() == Some(&TERMINATOR),
        "SELECT entry {} did not retain its terminator",
        translation.entry_id
    );
    Ok(output)
}
