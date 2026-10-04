use anyhow::{Context, Result, bail, ensure};

const CONTROL_PREFIX: u8 = b'$';

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RendererTextToken {
    DisplayControl(u8),
    LineBreak,
    SingleByte(u8),
    DoubleByte([u8; 2]),
    Terminator,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ParsedTextStream {
    pub byte_len: usize,
    pub tokens: Vec<RendererTextToken>,
}

pub(crate) fn parse_renderer_text_stream(bytes: &[u8]) -> Result<ParsedTextStream> {
    let mut cursor = 0;
    let mut tokens = Vec::new();

    loop {
        let first = *bytes
            .get(cursor)
            .with_context(|| "renderer text stream has no $$ terminator")?;
        if first == CONTROL_PREFIX {
            let code = *bytes
                .get(cursor + 1)
                .with_context(|| format!("truncated renderer control at byte {cursor:#x}"))?;
            cursor += 2;
            match code {
                b'$' => {
                    tokens.push(RendererTextToken::Terminator);
                    return Ok(ParsedTextStream {
                        byte_len: cursor,
                        tokens,
                    });
                }
                b'0' => tokens.push(RendererTextToken::LineBreak),
                b'1'..=b'8' => tokens.push(RendererTextToken::DisplayControl(code)),
                _ => bail!(
                    "unsupported renderer control ${} at byte {:#x}",
                    char::from(code),
                    cursor - 2
                ),
            }
            continue;
        }

        if is_single_byte(first) {
            tokens.push(RendererTextToken::SingleByte(first));
            cursor += 1;
            continue;
        }

        ensure!(
            is_renderer_double_byte_lead(first),
            "unsupported renderer text byte {first:02X} at byte {cursor:#x}"
        );
        let second = *bytes
            .get(cursor + 1)
            .with_context(|| format!("truncated Shift-JIS character at byte {cursor:#x}"))?;
        ensure!(
            is_shift_jis_trail(second),
            "invalid Shift-JIS trailing byte {second:02X} at byte {cursor:#x}"
        );
        tokens.push(RendererTextToken::DoubleByte([first, second]));
        cursor += 2;
    }
}

pub(crate) fn reconstruct_renderer_text_stream(tokens: &[RendererTextToken]) -> Result<Vec<u8>> {
    ensure!(
        matches!(tokens.last(), Some(RendererTextToken::Terminator)),
        "renderer token stream must end with a terminator"
    );
    ensure!(
        !tokens[..tokens.len() - 1]
            .iter()
            .any(|token| matches!(token, RendererTextToken::Terminator)),
        "renderer token stream contains an early terminator"
    );

    let mut bytes = Vec::new();
    for token in tokens {
        match token {
            RendererTextToken::DisplayControl(code) => {
                ensure!(
                    (b'1'..=b'8').contains(code),
                    "renderer display control must be $1 through $8"
                );
                bytes.extend_from_slice(&[CONTROL_PREFIX, *code]);
            }
            RendererTextToken::LineBreak => bytes.extend_from_slice(b"$0"),
            RendererTextToken::SingleByte(byte) => {
                ensure!(
                    is_single_byte(*byte) && *byte != CONTROL_PREFIX,
                    "unsupported renderer single byte {byte:02X}"
                );
                bytes.push(*byte);
            }
            RendererTextToken::DoubleByte(pair) => {
                ensure!(
                    is_renderer_double_byte_lead(pair[0]) && is_shift_jis_trail(pair[1]),
                    "unsupported renderer double-byte character {:02X}{:02X}",
                    pair[0],
                    pair[1]
                );
                bytes.extend_from_slice(pair);
            }
            RendererTextToken::Terminator => bytes.extend_from_slice(b"$$"),
        }
    }
    Ok(bytes)
}

fn is_single_byte(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte) || (0xa1..=0xdf).contains(&byte)
}

fn is_renderer_double_byte_lead(byte: u8) -> bool {
    (0x81..=0x9f).contains(&byte) || (0xe0..=0xee).contains(&byte)
}

fn is_shift_jis_trail(byte: u8) -> bool {
    ((0x40..=0x7e).contains(&byte) || (0x80..=0xfc).contains(&byte)) && byte != 0x7f
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod stream_tests;
