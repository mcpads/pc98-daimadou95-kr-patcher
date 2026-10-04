use anyhow::{Context, Result, ensure};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CompileLzStream {
    pub packed_size: usize,
    pub command_count: usize,
    pub output: Vec<u8>,
}

pub(crate) fn decode_compile_lz(input: &[u8]) -> Result<CompileLzStream> {
    let mut cursor = 0usize;
    let mut output = Vec::new();
    let mut command_count = 0usize;

    loop {
        let command_offset = cursor;
        let command = *input
            .get(cursor)
            .context("Compile LZ stream has no terminating command")?;
        cursor += 1;
        command_count += 1;

        if command == 0 {
            return Ok(CompileLzStream {
                packed_size: cursor,
                command_count,
                output,
            });
        }

        if command < 0x80 {
            let literal_size = usize::from(command);
            let literal_end = cursor
                .checked_add(literal_size)
                .context("Compile LZ literal boundary overflow")?;
            let literal = input.get(cursor..literal_end).with_context(|| {
                format!("Compile LZ literal at {command_offset:#x} exceeds its packed entry")
            })?;
            output.extend_from_slice(literal);
            cursor = literal_end;
            continue;
        }

        let copy_size = usize::from(command & 0x7f) + 3;
        let distance = usize::from(*input.get(cursor).with_context(|| {
            format!("Compile LZ back-reference at {command_offset:#x} has no distance byte")
        })?) + 1;
        cursor += 1;

        for _ in 0..copy_size {
            let value = output
                .len()
                .checked_sub(distance)
                .and_then(|source| output.get(source).copied())
                .unwrap_or(0);
            output.push(value);
        }
    }
}

pub(crate) fn decode_complete_compile_lz(input: &[u8]) -> Result<CompileLzStream> {
    let stream = decode_compile_lz(input)?;
    ensure!(
        stream.packed_size == input.len(),
        "Compile LZ stream ends at {:#x}, leaving {} trailing bytes",
        stream.packed_size,
        input.len() - stream.packed_size
    );
    Ok(stream)
}

pub(crate) fn encode_compile_lz(input: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut cursor = 0usize;
    let mut literal_start = 0usize;

    while cursor < input.len() {
        let (distance, copy_size) = longest_copy(input, cursor);
        if copy_size >= 3 {
            append_literals(&mut output, &input[literal_start..cursor]);
            output.push(0x80 | u8::try_from(copy_size - 3).expect("copy size is at most 130"));
            output.push(u8::try_from(distance - 1).expect("copy distance is at most 256"));
            cursor += copy_size;
            literal_start = cursor;
        } else {
            cursor += 1;
        }
    }
    append_literals(&mut output, &input[literal_start..]);
    output.push(0);

    let decoded = decode_complete_compile_lz(&output)?;
    ensure!(
        decoded.output == input,
        "Compile LZ encoder failed its independent decoder round trip"
    );
    Ok(output)
}

fn longest_copy(input: &[u8], cursor: usize) -> (usize, usize) {
    let mut best_distance = 0usize;
    let mut best_size = 0usize;
    let max_distance = cursor.min(256);
    let max_size = (input.len() - cursor).min(130);

    for distance in 1..=max_distance {
        let mut size = 0usize;
        while size < max_size && input[cursor + size] == input[cursor + size - distance] {
            size += 1;
        }
        if size > best_size {
            best_distance = distance;
            best_size = size;
        }
    }
    (best_distance, best_size)
}

fn append_literals(output: &mut Vec<u8>, mut literals: &[u8]) {
    while !literals.is_empty() {
        let size = literals.len().min(0x7f);
        output.push(u8::try_from(size).expect("literal chunk is at most 127 bytes"));
        output.extend_from_slice(&literals[..size]);
        literals = &literals[size..];
    }
}

#[cfg(test)]
#[path = "compile_lz_tests.rs"]
mod compile_lz_tests;
