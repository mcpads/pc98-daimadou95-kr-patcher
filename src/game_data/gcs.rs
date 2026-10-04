use anyhow::{Context, Result, bail, ensure};

pub(crate) const SIGNATURE: &[u8; 9] = b"gcs v1.4\0";
pub(crate) const HEADER_SIZE: usize = 0x40;
pub(crate) const SCREEN_WIDTH: usize = 640;
pub(crate) const SCREEN_HEIGHT: usize = 400;
pub(crate) const PLANE_STRIDE: usize = SCREEN_WIDTH / 8;
pub(crate) const PLANE_SIZE: usize = PLANE_STRIDE * SCREEN_HEIGHT;
pub(crate) const PLANE_COUNT: usize = 4;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct GcsImage {
    pub header: [u8; HEADER_SIZE],
    pub planes: [Vec<u8>; PLANE_COUNT],
    pub command_counts: [usize; PLANE_COUNT],
    pub packed_size: usize,
}

#[derive(Clone, Copy)]
struct PlaneGeometry {
    stride: usize,
    height: usize,
}

impl PlaneGeometry {
    const SCREEN: Self = Self {
        stride: PLANE_STRIDE,
        height: SCREEN_HEIGHT,
    };

    fn size(self) -> Result<usize> {
        self.stride
            .checked_mul(self.height)
            .context("GCS plane size overflow")
    }
}

struct PlaneWriter {
    bytes: Vec<u8>,
    offset: usize,
    geometry: PlaneGeometry,
    finished: bool,
}

impl PlaneWriter {
    fn new(geometry: PlaneGeometry) -> Result<Self> {
        ensure!(geometry.stride > 0, "GCS plane stride must not be zero");
        ensure!(geometry.height > 0, "GCS plane height must not be zero");
        Ok(Self {
            bytes: vec![0; geometry.size()?],
            offset: 0,
            geometry,
            finished: false,
        })
    }

    fn write(&mut self, value: u8) -> Result<()> {
        ensure!(!self.finished, "GCS command writes past the end of a plane");
        self.bytes[self.offset] = value;

        let plane_size = self.geometry.size()?;
        self.offset += self.geometry.stride;
        if self.offset < plane_size {
            return Ok(());
        }

        let final_offset = plane_size + self.geometry.stride - 1;
        if self.offset == final_offset {
            self.finished = true;
            return Ok(());
        }

        self.offset = self.offset - plane_size + 1;
        Ok(())
    }

    fn read_written(&self, relative_offset: usize) -> Result<u8> {
        let source = self.offset.checked_sub(relative_offset).with_context(|| {
            format!(
                "GCS same-plane copy at {:#x} precedes the plane start",
                self.offset
            )
        })?;
        Ok(self.bytes[source])
    }
}

pub(crate) fn decode_gcs(input: &[u8]) -> Result<GcsImage> {
    ensure!(
        input.len() >= HEADER_SIZE,
        "GCS asset is shorter than its {HEADER_SIZE}-byte header"
    );
    ensure!(
        &input[..SIGNATURE.len()] == SIGNATURE,
        "GCS asset signature does not match"
    );

    let header: [u8; HEADER_SIZE] = input[..HEADER_SIZE]
        .try_into()
        .expect("the checked GCS header slice has a fixed size");
    let mut cursor = HEADER_SIZE;
    let mut planes: [Vec<u8>; PLANE_COUNT] = std::array::from_fn(|_| Vec::new());
    let mut command_counts = [0usize; PLANE_COUNT];

    for plane_index in 0..PLANE_COUNT {
        let decoded = decode_plane(input, cursor, &planes[..plane_index], PlaneGeometry::SCREEN)?;
        cursor = decoded.end_offset;
        command_counts[plane_index] = decoded.command_count;
        planes[plane_index] = decoded.bytes;
    }

    ensure!(
        cursor == input.len(),
        "GCS screen ends at {cursor:#x}, leaving {} trailing bytes",
        input.len() - cursor
    );

    Ok(GcsImage {
        header,
        planes,
        command_counts,
        packed_size: cursor,
    })
}

pub(crate) fn encode_gcs(
    header: &[u8; HEADER_SIZE],
    planes: &[Vec<u8>; PLANE_COUNT],
) -> Result<Vec<u8>> {
    for (plane_index, plane) in planes.iter().enumerate() {
        ensure!(
            plane.len() == PLANE_SIZE,
            "GCS plane {} has {} bytes instead of {PLANE_SIZE}",
            plane_index + 1,
            plane.len()
        );
    }

    let ordered_planes: [Vec<u8>; PLANE_COUNT] = std::array::from_fn(|plane_index| {
        plane_in_storage_order(&planes[plane_index], PlaneGeometry::SCREEN)
    });
    let mut output = header.to_vec();
    for plane_index in 0..PLANE_COUNT {
        encode_plane(
            &ordered_planes[plane_index],
            &ordered_planes[..plane_index],
            &mut output,
        );
    }

    let decoded = decode_gcs(&output)?;
    ensure!(
        decoded.header == *header,
        "GCS encoder changed the protected header"
    );
    for (plane_index, (decoded_plane, expected_plane)) in
        decoded.planes.iter().zip(planes).enumerate()
    {
        if let Some(offset) = decoded_plane
            .iter()
            .zip(expected_plane)
            .position(|(actual, expected)| actual != expected)
        {
            bail!(
                "GCS encoder pixel round trip differs in plane {} at {offset:#x}: encoded {:02X}, expected {:02X}",
                plane_index + 1,
                decoded_plane[offset],
                expected_plane[offset]
            );
        }
    }
    Ok(output)
}

fn plane_in_storage_order(plane: &[u8], geometry: PlaneGeometry) -> Vec<u8> {
    let mut ordered = Vec::with_capacity(plane.len());
    for column in 0..geometry.stride {
        for row in 0..geometry.height {
            ordered.push(plane[row * geometry.stride + column]);
        }
    }
    ordered
}

#[derive(Clone)]
struct EncodedRun {
    kind: u8,
    count: usize,
    output_len: usize,
    parameters: Vec<u8>,
}

impl EncodedRun {
    fn encoded_len(&self) -> usize {
        general_count_len(self.count) + self.parameters.len()
    }

    fn savings(&self) -> isize {
        self.output_len as isize - self.encoded_len() as isize
    }
}

fn encode_plane(plane: &[u8], previous_planes: &[Vec<u8>], output: &mut Vec<u8>) {
    let mut cursor = 0usize;
    while cursor < plane.len() {
        if let Some(run) = best_run(plane, previous_planes, cursor) {
            append_general_command(output, run.kind, run.count);
            output.extend_from_slice(&run.parameters);
            cursor += run.output_len;
            continue;
        }

        let literal_start = cursor;
        cursor += 1;
        while cursor < plane.len()
            && cursor - literal_start < 255
            && best_run(plane, previous_planes, cursor).is_none()
        {
            cursor += 1;
        }
        append_literal(output, &plane[literal_start..cursor]);
    }
}

fn best_run(plane: &[u8], previous_planes: &[Vec<u8>], cursor: usize) -> Option<EncodedRun> {
    let remaining = plane.len() - cursor;
    let mut candidates = Vec::new();
    let max_general = remaining.min(511);

    let value = plane[cursor];
    if value == 0 {
        candidates.push(run_of_value(0x0, plane, cursor, 0, max_general, Vec::new()));
    }
    if value == 0xff {
        candidates.push(run_of_value(
            0x1,
            plane,
            cursor,
            0xff,
            max_general,
            Vec::new(),
        ));
    }
    candidates.push(run_of_value(
        0xc,
        plane,
        cursor,
        value,
        max_general,
        vec![value],
    ));
    for (plane_index, previous) in previous_planes.iter().enumerate() {
        let same_kind = 0x4 + u8::try_from(plane_index * 2).expect("GCS has only four planes");
        candidates.push(run_against_plane(
            same_kind,
            plane,
            previous,
            cursor,
            false,
            max_general,
        ));
        candidates.push(run_against_plane(
            same_kind + 1,
            plane,
            previous,
            cursor,
            true,
            max_general,
        ));
    }
    if remaining >= 4 {
        candidates.push(pattern_run(0xe, plane, cursor, 2));
    }
    if remaining >= 8 {
        candidates.push(pattern_run(0xf, plane, cursor, 4));
    }

    candidates
        .into_iter()
        .filter(|candidate| candidate.output_len > 0 && candidate.savings() > 0)
        .max_by_key(|candidate| {
            (
                candidate.savings(),
                candidate.output_len,
                0xff - candidate.kind,
            )
        })
}

fn run_of_value(
    kind: u8,
    plane: &[u8],
    cursor: usize,
    value: u8,
    max_len: usize,
    parameters: Vec<u8>,
) -> EncodedRun {
    let output_len = plane[cursor..]
        .iter()
        .take(max_len)
        .take_while(|byte| **byte == value)
        .count();
    EncodedRun {
        kind,
        count: output_len,
        output_len,
        parameters,
    }
}

fn run_against_plane(
    kind: u8,
    plane: &[u8],
    previous: &[u8],
    cursor: usize,
    invert: bool,
    max_len: usize,
) -> EncodedRun {
    let mut output_len = 0usize;
    while output_len < max_len {
        let expected = if invert {
            !previous[cursor + output_len]
        } else {
            previous[cursor + output_len]
        };
        if plane[cursor + output_len] != expected {
            break;
        }
        output_len += 1;
    }
    EncodedRun {
        kind,
        count: output_len,
        output_len,
        parameters: Vec::new(),
    }
}

fn pattern_run(kind: u8, plane: &[u8], cursor: usize, pattern_len: usize) -> EncodedRun {
    let max_count = ((plane.len() - cursor) / pattern_len).min(511);
    let mut count = 1usize;
    while count < max_count {
        let start = cursor + count * pattern_len;
        if plane[start..start + pattern_len] != plane[cursor..cursor + pattern_len] {
            break;
        }
        count += 1;
    }
    EncodedRun {
        kind,
        count,
        output_len: count * pattern_len,
        parameters: plane[cursor..cursor + pattern_len].to_vec(),
    }
}

fn append_literal(output: &mut Vec<u8>, literal: &[u8]) {
    debug_assert!(!literal.is_empty() && literal.len() <= 255);
    if literal.len() <= 15 {
        output.push(0x20 | u8::try_from(literal.len()).expect("literal length is at most 15"));
    } else {
        output.extend_from_slice(&[
            0x20,
            u8::try_from(literal.len()).expect("literal length is at most 255"),
        ]);
    }
    output.extend_from_slice(literal);
}

fn append_general_command(output: &mut Vec<u8>, kind: u8, count: usize) {
    debug_assert!(kind != 2 && (1..=511).contains(&count));
    if (2..=15).contains(&count) {
        output.push((kind << 4) | u8::try_from(count).expect("short count is at most 15"));
    } else if count <= 255 {
        output.extend_from_slice(&[
            kind << 4,
            u8::try_from(count).expect("extended count is at most 255"),
        ]);
    } else {
        output.extend_from_slice(&[
            (kind << 4) | 1,
            u8::try_from(count - 256).expect("long count is at most 511"),
        ]);
    }
}

fn general_count_len(count: usize) -> usize {
    if (2..=15).contains(&count) { 1 } else { 2 }
}

struct DecodedPlane {
    bytes: Vec<u8>,
    end_offset: usize,
    command_count: usize,
}

fn decode_plane(
    input: &[u8],
    start_offset: usize,
    previous_planes: &[Vec<u8>],
    geometry: PlaneGeometry,
) -> Result<DecodedPlane> {
    let mut cursor = start_offset;
    let mut writer = PlaneWriter::new(geometry)?;
    let mut command_count = 0usize;

    while !writer.finished {
        let command_offset = cursor;
        let command = read_byte(input, &mut cursor, "command")?;
        command_count += 1;
        let kind = command >> 4;
        let repeats = if kind == 2 {
            literal_count(command, input, &mut cursor)?
        } else {
            general_count(command, input, &mut cursor)?
        };
        let repeats = if repeats == 0 { 65_536 } else { repeats };

        match kind {
            0 => repeat_until_finished(repeats, &mut writer, |_| Ok(0))?,
            1 => repeat_until_finished(repeats, &mut writer, |_| Ok(0xff))?,
            2 => repeat_until_finished(repeats, &mut writer, |_| {
                read_byte(input, &mut cursor, "literal")
            })?,
            3 => repeat_until_finished(repeats, &mut writer, |writer| writer.read_written(1))?,
            4..=9 => {
                let referenced_plane = usize::from((kind - 4) / 2);
                let invert = kind % 2 == 1;
                let source = previous_planes.get(referenced_plane).with_context(|| {
                    format!(
                        "GCS command {kind:X} at {command_offset:#x} references unavailable plane {}",
                        referenced_plane + 1
                    )
                })?;
                repeat_until_finished(repeats, &mut writer, |writer| {
                    let value = source[writer.offset];
                    Ok(if invert { !value } else { value })
                })?;
            }
            0xA => repeat_until_finished(repeats, &mut writer, |writer| writer.read_written(2))?,
            0xB => {
                let even = read_byte(input, &mut cursor, "paired-run even byte")?;
                for _ in 0..repeats {
                    let odd = read_byte(input, &mut cursor, "paired-run odd byte")?;
                    writer.write(even)?;
                    if writer.finished {
                        break;
                    }
                    writer.write(odd)?;
                    if writer.finished {
                        break;
                    }
                }
            }
            0xC => {
                let value = read_byte(input, &mut cursor, "repeated byte")?;
                repeat_until_finished(repeats, &mut writer, |_| Ok(value))?;
            }
            0xD => {
                let mut value = read_byte(input, &mut cursor, "rotating byte")?;
                let rotation = if matches!(value, 0x55 | 0xaa) { 1 } else { 2 };
                repeat_until_finished(repeats, &mut writer, |_| {
                    let current = value;
                    value = value.rotate_left(rotation);
                    Ok(current)
                })?;
            }
            0xE => {
                let pattern = [
                    read_byte(input, &mut cursor, "two-byte pattern")?,
                    read_byte(input, &mut cursor, "two-byte pattern")?,
                ];
                write_pattern(repeats, &pattern, &mut writer)?;
            }
            0xF => {
                let pattern = [
                    read_byte(input, &mut cursor, "four-byte pattern")?,
                    read_byte(input, &mut cursor, "four-byte pattern")?,
                    read_byte(input, &mut cursor, "four-byte pattern")?,
                    read_byte(input, &mut cursor, "four-byte pattern")?,
                ];
                write_pattern(repeats, &pattern, &mut writer)?;
            }
            _ => bail!("unreachable GCS command kind"),
        }
    }

    Ok(DecodedPlane {
        bytes: writer.bytes,
        end_offset: cursor,
        command_count,
    })
}

fn general_count(command: u8, input: &[u8], cursor: &mut usize) -> Result<usize> {
    let low = usize::from(command & 0x0f);
    if low > 1 {
        return Ok(low);
    }
    let extra = usize::from(read_byte(input, cursor, "extended repeat count")?);
    Ok(low * 0x100 + extra)
}

fn literal_count(command: u8, input: &[u8], cursor: &mut usize) -> Result<usize> {
    let low = usize::from(command & 0x0f);
    if low != 0 {
        return Ok(low);
    }
    Ok(usize::from(read_byte(
        input,
        cursor,
        "extended literal count",
    )?))
}

fn read_byte(input: &[u8], cursor: &mut usize, role: &str) -> Result<u8> {
    let offset = *cursor;
    let byte = *input
        .get(offset)
        .with_context(|| format!("GCS {role} at {offset:#x} exceeds the packed asset"))?;
    *cursor += 1;
    Ok(byte)
}

fn repeat_until_finished(
    repeats: usize,
    writer: &mut PlaneWriter,
    mut value: impl FnMut(&PlaneWriter) -> Result<u8>,
) -> Result<()> {
    for _ in 0..repeats {
        let byte = value(writer)?;
        writer.write(byte)?;
        if writer.finished {
            break;
        }
    }
    Ok(())
}

fn write_pattern(repeats: usize, pattern: &[u8], writer: &mut PlaneWriter) -> Result<()> {
    for _ in 0..repeats {
        for &byte in pattern {
            writer.write(byte)?;
            if writer.finished {
                return Ok(());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "gcs_tests.rs"]
mod gcs_tests;
