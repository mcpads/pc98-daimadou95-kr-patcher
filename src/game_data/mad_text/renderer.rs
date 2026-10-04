use anyhow::{Context, Result, ensure};
use v30::{Instruction, decode_bytes};

pub(crate) const FILE_OFFSET: usize = 0x80e0;
pub(crate) const RUNTIME_ADDRESS: usize = FILE_OFFSET + 0x100;
pub(crate) const CALL_SITE_FILE_OFFSET: usize = 0x0e2b;
const BODY_LEN: usize = 0x4e;

const DISPATCH_SIGNATURE: &[u8] = &[
    0xfc, 0xb2, 0xe1, 0x8b, 0xf3, 0xb9, 0x00, 0xa0, 0x8e, 0xc1, 0x8b, 0xcf, 0xad, 0x3d, 0x24, 0x24,
    0x74, 0x3b, 0x3d, 0x24, 0x30, 0x74, 0x2b, 0x3c, 0x24, 0x75, 0x05,
];
const CALL_SIGNATURE: &[u8] = &[0xe8, 0xb2, 0x72];

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RendererEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub call_site_file_offset: usize,
    pub decoded_instruction_count: usize,
}

pub(crate) fn verify_text_renderer(bytes: &[u8]) -> Result<RendererEvidence> {
    ensure_bytes(
        bytes,
        FILE_OFFSET,
        DISPATCH_SIGNATURE,
        "MAD text renderer dispatch",
    )?;
    ensure_bytes(
        bytes,
        CALL_SITE_FILE_OFFSET,
        CALL_SIGNATURE,
        "MAD fixed-text renderer call",
    )?;

    let body = bytes
        .get(FILE_OFFSET..FILE_OFFSET + BODY_LEN)
        .context("MAD text renderer body is truncated")?;
    let decoded_instruction_count = decode_complete_block(body)?;
    let call = decode_bytes(CALL_SIGNATURE).context("decode MAD fixed-text renderer call")?;
    ensure!(
        matches!(call.instruction, Instruction::Call { .. }),
        "MAD fixed-text renderer call is not a typed V30 CALL"
    );

    Ok(RendererEvidence {
        file_offset: FILE_OFFSET,
        runtime_address: RUNTIME_ADDRESS,
        call_site_file_offset: CALL_SITE_FILE_OFFSET,
        decoded_instruction_count,
    })
}

fn decode_complete_block(bytes: &[u8]) -> Result<usize> {
    let mut cursor = 0;
    let mut instruction_count = 0;
    let mut last_instruction = None;
    while cursor < bytes.len() {
        let decoded = decode_bytes(&bytes[cursor..])
            .with_context(|| format!("decode V30 instruction at renderer byte {cursor:#x}"))?;
        cursor += decoded.byte_len;
        instruction_count += 1;
        last_instruction = Some(decoded.instruction);
    }
    ensure!(
        cursor == bytes.len(),
        "V30 decoder crossed renderer boundary"
    );
    ensure!(
        matches!(last_instruction, Some(Instruction::Ret { pop: 0 })),
        "MAD text renderer body does not end in a typed V30 RET"
    );
    Ok(instruction_count)
}

fn ensure_bytes(bytes: &[u8], offset: usize, expected: &[u8], label: &str) -> Result<()> {
    ensure!(
        bytes.get(offset..offset + expected.len()) == Some(expected),
        "{label} does not match at file offset {offset:#x}"
    );
    Ok(())
}

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod renderer_tests;
