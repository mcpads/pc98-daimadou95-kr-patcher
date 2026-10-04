use anyhow::{Context, Result, bail, ensure};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DosTextToken {
    AnsiControl(Vec<u8>),
    CarriageReturn,
    LineFeed,
    SingleByte(u8),
    DoubleByte([u8; 2]),
    Terminator,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParsedDosText {
    pub byte_len: usize,
    pub tokens: Vec<DosTextToken>,
}

pub(crate) fn parse_dos_text(bytes: &[u8]) -> Result<ParsedDosText> {
    let mut cursor = 0;
    let mut tokens = Vec::new();
    loop {
        let first = *bytes
            .get(cursor)
            .context("DOS display string has no $ terminator")?;
        match first {
            b'$' => {
                cursor += 1;
                tokens.push(DosTextToken::Terminator);
                return Ok(ParsedDosText {
                    byte_len: cursor,
                    tokens,
                });
            }
            0x1b => {
                let end = parse_ansi_control_end(bytes, cursor)?;
                tokens.push(DosTextToken::AnsiControl(bytes[cursor..end].to_vec()));
                cursor = end;
            }
            b'\r' => {
                tokens.push(DosTextToken::CarriageReturn);
                cursor += 1;
            }
            b'\n' => {
                tokens.push(DosTextToken::LineFeed);
                cursor += 1;
            }
            byte if is_single_byte(byte) => {
                tokens.push(DosTextToken::SingleByte(byte));
                cursor += 1;
            }
            lead if is_shift_jis_lead(lead) => {
                let trail = *bytes.get(cursor + 1).with_context(|| {
                    format!("truncated Shift-JIS character at byte {cursor:#x}")
                })?;
                ensure!(
                    is_shift_jis_trail(trail),
                    "invalid Shift-JIS trailing byte {trail:02X} at byte {cursor:#x}"
                );
                tokens.push(DosTextToken::DoubleByte([lead, trail]));
                cursor += 2;
            }
            _ => bail!("unsupported DOS display byte {first:02X} at byte {cursor:#x}"),
        }
    }
}

pub(crate) fn reconstruct_dos_text(tokens: &[DosTextToken]) -> Result<Vec<u8>> {
    ensure!(
        matches!(tokens.last(), Some(DosTextToken::Terminator)),
        "DOS display token stream must end with a terminator"
    );
    ensure!(
        !tokens[..tokens.len() - 1]
            .iter()
            .any(|token| matches!(token, DosTextToken::Terminator)),
        "DOS display token stream contains an early terminator"
    );

    let mut bytes = Vec::new();
    for token in tokens {
        match token {
            DosTextToken::AnsiControl(control) => {
                ensure!(
                    parse_ansi_control_end(control, 0)? == control.len(),
                    "ANSI display control contains trailing bytes"
                );
                bytes.extend_from_slice(control);
            }
            DosTextToken::CarriageReturn => bytes.push(b'\r'),
            DosTextToken::LineFeed => bytes.push(b'\n'),
            DosTextToken::SingleByte(byte) => {
                ensure!(is_single_byte(*byte), "unsupported DOS single byte");
                bytes.push(*byte);
            }
            DosTextToken::DoubleByte(pair) => {
                ensure!(
                    is_shift_jis_lead(pair[0]) && is_shift_jis_trail(pair[1]),
                    "unsupported DOS double-byte character {:02X}{:02X}",
                    pair[0],
                    pair[1]
                );
                bytes.extend_from_slice(pair);
            }
            DosTextToken::Terminator => bytes.push(b'$'),
        }
    }
    Ok(bytes)
}

fn parse_ansi_control_end(bytes: &[u8], start: usize) -> Result<usize> {
    ensure!(
        bytes.get(start..start + 2) == Some(&[0x1b, b'[']),
        "unsupported ANSI display control at byte {start:#x}"
    );
    let mut cursor = start + 2;
    loop {
        let byte = *bytes
            .get(cursor)
            .with_context(|| format!("unterminated ANSI display control at byte {start:#x}"))?;
        cursor += 1;
        if (0x40..=0x7e).contains(&byte) {
            return Ok(cursor);
        }
        ensure!(
            (0x20..=0x3f).contains(&byte),
            "invalid ANSI display control byte {byte:02X} at byte {:#x}",
            cursor - 1
        );
    }
}

fn is_single_byte(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte) || (0xa1..=0xdf).contains(&byte)
}

fn is_shift_jis_lead(byte: u8) -> bool {
    (0x81..=0x9f).contains(&byte) || (0xe0..=0xef).contains(&byte)
}

fn is_shift_jis_trail(byte: u8) -> bool {
    ((0x40..=0x7e).contains(&byte) || (0x80..=0xfc).contains(&byte)) && byte != 0x7f
}

#[cfg(test)]
#[path = "dos_text_tests.rs"]
mod dos_text_tests;
