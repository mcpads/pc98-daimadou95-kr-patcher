use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::game_data::ending_curry_result::{
    DATE_DIGIT_CELL_COUNT, DATE_TEMPLATE_BYTE_SIZE, DAY_TEMPLATE_FILE_OFFSET,
    MESSAGE_POOL_END_FILE_OFFSET, MESSAGE_POOL_FILE_OFFSET, MONTH_TEMPLATE_FILE_OFFSET,
    parse_ending_curry_result,
};

const LINE_BREAK: u8 = 0xfe;
const TERMINATOR: u8 = 0xff;
const RESULT_LINE_CELL_LIMIT: usize = 30;

#[derive(Debug, Eq, PartialEq)]
pub struct EndingCurryResultPatchReport {
    pub date_templates_file_offset: usize,
    pub date_templates_byte_size: usize,
    pub message_pool_file_offset: usize,
    pub message_pool_capacity: usize,
    pub message_pool_used: usize,
    pub message_count: usize,
}

#[derive(Debug)]
pub(super) struct PatchedEndingCurryResult {
    pub bytes: Vec<u8>,
    pub report: EndingCurryResultPatchReport,
}

pub(super) fn replace_ending_curry_result(
    bytes: &[u8],
    translations: &BTreeMap<String, Vec<String>>,
    codebook: &BTreeMap<char, u8>,
) -> Result<PatchedEndingCurryResult> {
    let catalog = parse_ending_curry_result(bytes)?;
    ensure!(
        translations.len() == catalog.messages.len() + 2,
        "ENDING curry-result translation supplies {} records for {} date and message records",
        translations.len(),
        catalog.messages.len() + 2
    );

    ensure!(
        catalog.month.file_offset == MONTH_TEMPLATE_FILE_OFFSET
            && catalog.day.file_offset == DAY_TEMPLATE_FILE_OFFSET
            && catalog.month.file_offset + DATE_TEMPLATE_BYTE_SIZE == catalog.day.file_offset,
        "ENDING curry-result date templates are no longer contiguous"
    );
    let date_templates_file_offset = catalog.month.file_offset;
    let date_templates_end = catalog.day.file_offset + DATE_TEMPLATE_BYTE_SIZE;
    let mut date_replacement = encode_date_template(
        catalog.month.initial_digit_indices,
        translated_spans(translations, "month-suffix")?,
        codebook,
        "month-suffix",
    )?
    .to_vec();
    date_replacement.extend_from_slice(&encode_date_template(
        catalog.day.initial_digit_indices,
        translated_spans(translations, "day-suffix")?,
        codebook,
        "day-suffix",
    )?);

    let message_pool_capacity = MESSAGE_POOL_END_FILE_OFFSET - MESSAGE_POOL_FILE_OFFSET;
    let mut message_replacement = vec![TERMINATOR; message_pool_capacity];
    let mut message_pool_used = 0;
    let mut records = catalog.messages.iter().collect::<Vec<_>>();
    records.sort_by_key(|record| record.file_offset);
    for (index, record) in records.iter().enumerate() {
        let record_end = records
            .get(index + 1)
            .map_or(MESSAGE_POOL_END_FILE_OFFSET, |next| next.file_offset);
        ensure!(
            record.file_offset + record.bytes.len() == record_end,
            "ENDING curry-result class {} does not own its complete fixed record",
            record.result_class
        );
    }
    for record in &catalog.messages {
        let id = format!("result-class-{:02}", record.result_class + 1);
        let lines = translated_spans(translations, &id)?;
        ensure!(
            lines.len() == record.lines.len(),
            "ENDING curry-result {id} supplies {} lines for {} protected lines",
            lines.len(),
            record.lines.len()
        );
        let encoded = encode_message(lines, &record.line_break_counts, codebook, &id)?;
        ensure!(
            encoded.len() <= record.bytes.len(),
            "ENDING curry-result {id} requires {} bytes but its fixed record has {}",
            encoded.len(),
            record.bytes.len()
        );
        let start = record.file_offset - MESSAGE_POOL_FILE_OFFSET;
        message_replacement[start..start + encoded.len()].copy_from_slice(&encoded);
        message_pool_used += encoded.len();
    }

    let plan = WritePlan::new()
        .region(ImageRegion {
            id: "ending-curry-result-date-templates".into(),
            range: date_templates_file_offset..date_templates_end,
            kind: RegionKind::Data,
            reason: "runtime-mutated decimal date cells and Korean month/day suffixes".into(),
        })
        .region(ImageRegion {
            id: "ending-curry-result-message-pool".into(),
            range: MESSAGE_POOL_FILE_OFFSET..MESSAGE_POOL_END_FILE_OFFSET,
            kind: RegionKind::Data,
            reason: "score-selected fixed ending curry evaluation records".into(),
        })
        .write(ExpectedWrite {
            id: "write-ending-curry-result-date-templates".into(),
            owner: "ending-curry-result-compiler".into(),
            purpose: "preserve the two runtime decimal cells and replace both date suffixes".into(),
            offset: date_templates_file_offset,
            expected_original: bytes[date_templates_file_offset..date_templates_end].to_vec(),
            replacement: date_replacement.clone(),
            intent: WriteIntent::Data,
        })
        .write(ExpectedWrite {
            id: "write-ending-curry-result-message-pool".into(),
            owner: "ending-curry-result-compiler".into(),
            purpose:
                "replace all four score-selected evaluation records and neutralize their tails"
                    .into(),
            offset: MESSAGE_POOL_FILE_OFFSET,
            expected_original: bytes[MESSAGE_POOL_FILE_OFFSET..MESSAGE_POOL_END_FILE_OFFSET]
                .to_vec(),
            replacement: message_replacement.clone(),
            intent: WriteIntent::Data,
        });
    let output = plan.apply(bytes, None)?;
    ensure!(
        output[date_templates_file_offset..date_templates_end] == date_replacement,
        "ENDING curry-result date templates did not survive their Expected Write"
    );
    ensure!(
        output[MESSAGE_POOL_FILE_OFFSET..MESSAGE_POOL_END_FILE_OFFSET] == message_replacement,
        "ENDING curry-result message pool did not survive its Expected Write"
    );

    Ok(PatchedEndingCurryResult {
        bytes: output,
        report: EndingCurryResultPatchReport {
            date_templates_file_offset,
            date_templates_byte_size: date_replacement.len(),
            message_pool_file_offset: MESSAGE_POOL_FILE_OFFSET,
            message_pool_capacity,
            message_pool_used,
            message_count: catalog.messages.len(),
        },
    })
}

fn translated_spans<'a>(
    translations: &'a BTreeMap<String, Vec<String>>,
    id: &str,
) -> Result<&'a [String]> {
    translations
        .get(id)
        .map(Vec::as_slice)
        .with_context(|| format!("ENDING curry-result translation is missing {id}"))
}

fn encode_date_template(
    digits: [u8; DATE_DIGIT_CELL_COUNT],
    suffix: &[String],
    codebook: &BTreeMap<char, u8>,
    id: &str,
) -> Result<[u8; DATE_TEMPLATE_BYTE_SIZE]> {
    ensure!(
        suffix.len() == 1 && suffix[0].chars().count() == 1,
        "ENDING curry-result {id} must contain one suffix cell"
    );
    let suffix_index = encode_character(
        suffix[0]
            .chars()
            .next()
            .expect("one suffix character was checked"),
        codebook,
        id,
    )?;
    Ok([digits[0], digits[1], suffix_index, TERMINATOR, TERMINATOR])
}

fn encode_message(
    lines: &[String],
    line_break_counts: &[usize],
    codebook: &BTreeMap<char, u8>,
    id: &str,
) -> Result<Vec<u8>> {
    ensure!(
        lines.len() == line_break_counts.len() + 1,
        "ENDING curry-result {id} line-break structure changed"
    );
    let mut encoded = Vec::new();
    for (line_index, line) in lines.iter().enumerate() {
        let cell_count = line.chars().count();
        ensure!(
            cell_count > 0 && cell_count <= RESULT_LINE_CELL_LIMIT,
            "ENDING curry-result {id} line {} uses {cell_count} cells but its consumer requires 1..={RESULT_LINE_CELL_LIMIT}",
            line_index + 1
        );
        for character in line.chars() {
            encoded.push(encode_character(character, codebook, id)?);
        }
        if let Some(count) = line_break_counts.get(line_index) {
            encoded.extend(std::iter::repeat_n(LINE_BREAK, *count));
        }
    }
    encoded.push(TERMINATOR);
    Ok(encoded)
}

fn encode_character(character: char, codebook: &BTreeMap<char, u8>, id: &str) -> Result<u8> {
    let index = codebook
        .get(&character)
        .copied()
        .with_context(|| format!("ENDING curry-result {id} codebook is missing {character:?}"))?;
    ensure!(
        !matches!(index, LINE_BREAK | TERMINATOR),
        "ENDING curry-result {id} maps {character:?} to a control byte"
    );
    Ok(index)
}

#[cfg(test)]
#[path = "curry_result_tests.rs"]
mod curry_result_tests;
