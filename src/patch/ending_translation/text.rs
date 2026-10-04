use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};

use crate::game_data::indexed_text::{parse_ending_interlude_caption, parse_ending_passwords};

const COM_ORIGIN: usize = 0x100;
const INTERLUDE_CELL_LIMIT: usize = 24;
const PASSWORD_CELL_LIMIT: usize = 28;
const TERMINATOR: u8 = 0xff;

pub(super) struct PatchedEndingText {
    pub bytes: Vec<u8>,
    pub interlude_file_offset: usize,
    pub pointer_table_file_offset: usize,
    pub password_pool_file_offset: usize,
    pub password_pool_capacity: usize,
    pub password_pool_used: usize,
}

pub(super) fn replace_ending_text(
    bytes: &[u8],
    interlude_text: &str,
    password_texts: &[String],
    codebook: &BTreeMap<char, u8>,
) -> Result<PatchedEndingText> {
    let interlude = parse_ending_interlude_caption(bytes)?;
    let passwords = parse_ending_passwords(bytes)?;
    ensure!(
        password_texts.len() == passwords.records.len(),
        "ENDING translation supplies {} passwords for {} selector records",
        password_texts.len(),
        passwords.records.len()
    );

    let encoded_interlude = encode_line(interlude_text, codebook, "ending interlude")?;
    ensure!(
        interlude_text.chars().count() <= INTERLUDE_CELL_LIMIT,
        "ENDING interlude uses {} cells but its consumer allows {INTERLUDE_CELL_LIMIT}",
        interlude_text.chars().count()
    );
    ensure!(
        encoded_interlude.len() <= interlude.bytes.len(),
        "ENDING interlude requires {} bytes but its fixed field has {}",
        encoded_interlude.len(),
        interlude.bytes.len()
    );
    let mut interlude_replacement = vec![TERMINATOR; interlude.bytes.len()];
    interlude_replacement[..encoded_interlude.len()].copy_from_slice(&encoded_interlude);

    let pool_file_offset = passwords
        .records
        .iter()
        .map(|record| record.file_offset)
        .min()
        .context("ENDING password catalog has no records")?;
    let pool_capacity = passwords
        .selector_file_offset
        .checked_sub(pool_file_offset)
        .context("ENDING password selector lies before its text pool")?;
    let mut pool_replacement = vec![TERMINATOR; pool_capacity];
    let mut pointer_replacement = Vec::with_capacity(password_texts.len() * 2);
    let mut cursor = 0_usize;
    for (selection_index, text) in password_texts.iter().enumerate() {
        let cells = text.chars().count();
        ensure!(
            cells <= PASSWORD_CELL_LIMIT,
            "ENDING password {} uses {cells} cells but its consumer allows {PASSWORD_CELL_LIMIT}",
            selection_index + 1
        );
        let encoded = encode_line(
            text,
            codebook,
            &format!("ending password {}", selection_index + 1),
        )?;
        let end = cursor
            .checked_add(encoded.len())
            .context("ENDING password pool cursor overflow")?;
        ensure!(
            end <= pool_capacity,
            "ENDING password pool requires {end} bytes but has {pool_capacity}"
        );
        let runtime_address = pool_file_offset
            .checked_add(cursor)
            .and_then(|offset| offset.checked_add(COM_ORIGIN))
            .context("ENDING password runtime address overflow")?;
        pointer_replacement.extend_from_slice(
            &u16::try_from(runtime_address)
                .context("ENDING password runtime address exceeds 16 bits")?
                .to_le_bytes(),
        );
        pool_replacement[cursor..end].copy_from_slice(&encoded);
        cursor = end;
    }

    let pointer_table_size = passwords.records.len() * 2;
    let pointer_table_end = passwords.pointer_table_file_offset + pointer_table_size;
    let pool_end = pool_file_offset + pool_capacity;
    let plan = WritePlan::new()
        .region(ImageRegion {
            id: "ending-interlude-text".into(),
            range: interlude.file_offset..interlude.file_offset + interlude.bytes.len(),
            kind: RegionKind::Data,
            reason: "fixed indexed ending interlude caption".into(),
        })
        .region(ImageRegion {
            id: "ending-password-pointer-table".into(),
            range: passwords.pointer_table_file_offset..pointer_table_end,
            kind: RegionKind::Metadata,
            reason: "selector-ordered pointers into the repacked ending password pool".into(),
        })
        .region(ImageRegion {
            id: "ending-password-text-pool".into(),
            range: pool_file_offset..pool_end,
            kind: RegionKind::Data,
            reason: "repacked indexed ending password strings and neutral tail".into(),
        })
        .write(ExpectedWrite {
            id: "write-ending-interlude-text".into(),
            owner: "ending-indexed-text-compiler".into(),
            purpose: "replace the ending interlude caption with tracked Korean text".into(),
            offset: interlude.file_offset,
            expected_original: interlude.bytes.to_vec(),
            replacement: interlude_replacement,
            intent: WriteIntent::Data,
        })
        .write(ExpectedWrite {
            id: "write-ending-password-pointers".into(),
            owner: "ending-indexed-text-compiler".into(),
            purpose: "point each ending password selector at its repacked Korean line".into(),
            offset: passwords.pointer_table_file_offset,
            expected_original: bytes[passwords.pointer_table_file_offset..pointer_table_end]
                .to_vec(),
            replacement: pointer_replacement.clone(),
            intent: WriteIntent::Metadata,
        })
        .write(ExpectedWrite {
            id: "write-ending-password-pool".into(),
            owner: "ending-indexed-text-compiler".into(),
            purpose: "replace the complete ending password pool and clear unreachable source text"
                .into(),
            offset: pool_file_offset,
            expected_original: bytes[pool_file_offset..pool_end].to_vec(),
            replacement: pool_replacement.clone(),
            intent: WriteIntent::Data,
        });
    let output = plan.apply(bytes, None)?;

    ensure!(
        output[interlude.file_offset..interlude.file_offset + encoded_interlude.len()]
            == encoded_interlude,
        "ENDING interlude did not survive its Expected Write"
    );
    ensure!(
        output[passwords.pointer_table_file_offset..pointer_table_end] == pointer_replacement,
        "ENDING password pointers did not survive their Expected Write"
    );
    ensure!(
        output[pool_file_offset..pool_end] == pool_replacement,
        "ENDING password pool did not survive its Expected Write"
    );
    ensure!(
        pool_replacement[cursor..]
            .iter()
            .all(|byte| *byte == TERMINATOR),
        "ENDING password pool tail contains source text"
    );

    Ok(PatchedEndingText {
        bytes: output,
        interlude_file_offset: interlude.file_offset,
        pointer_table_file_offset: passwords.pointer_table_file_offset,
        password_pool_file_offset: pool_file_offset,
        password_pool_capacity: pool_capacity,
        password_pool_used: cursor,
    })
}

fn encode_line(text: &str, codebook: &BTreeMap<char, u8>, label: &str) -> Result<Vec<u8>> {
    let mut encoded = Vec::with_capacity(text.chars().count() + 1);
    for character in text.chars() {
        let index = codebook
            .get(&character)
            .copied()
            .with_context(|| format!("{label} codebook is missing {character:?}"))?;
        ensure!(
            index != TERMINATOR,
            "{label} maps {character:?} to the reserved terminator"
        );
        encoded.push(index);
    }
    encoded.push(TERMINATOR);
    Ok(encoded)
}
