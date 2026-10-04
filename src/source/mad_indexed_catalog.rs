use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::game_data::indexed_text;
use crate::source::disk;

const COM_ORIGIN: usize = 0x100;

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadIndexedTextCatalog {
    pub mad_com_sha256: String,
    pub file_size: usize,
    pub renderer: MadIndexedRendererEvidence,
    pub pools: Vec<MadIndexedTextPool>,
    pub strings: Vec<MadIndexedString>,
    pub bindings: Vec<MadIndexedBinding>,
    pub unbound_string_ids: Vec<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadIndexedRendererEvidence {
    pub file_offset: usize,
    pub runtime_address: usize,
    pub decoded_instruction_count: usize,
    pub isa_profile_id: String,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadIndexedTextPool {
    pub id: String,
    pub runtime_start: usize,
    pub runtime_end: usize,
    pub file_start: usize,
    pub file_end: usize,
    pub string_count: usize,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadIndexedString {
    pub id: String,
    pub runtime_address: usize,
    pub file_offset: usize,
    pub byte_size: usize,
    pub sha256: String,
    pub raw_hex: String,
    pub text: Option<String>,
    pub contains_japanese: bool,
    pub bound_to_consumer: bool,
    pub glyphs: Vec<MadIndexedGlyph>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadIndexedGlyph {
    pub index: u8,
    pub text: Option<String>,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct MadIndexedBinding {
    pub id: String,
    pub consumer: String,
    pub reference_file_offsets: Vec<usize>,
    pub string_ids: Vec<String>,
    pub content_kind: MadIndexedBindingContentKind,
    pub display_text: Option<String>,
    pub contains_japanese: bool,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MadIndexedBindingContentKind {
    TextLines,
    CompositeGlyphBanner,
}

struct BindingRequest {
    id: String,
    consumer: String,
    reference_file_offsets: Vec<usize>,
    runtime_address: usize,
    line_count: usize,
}

pub(super) fn catalog_mad_indexed_text(bytes: &[u8]) -> Result<MadIndexedTextCatalog> {
    let renderer = indexed_text::verify_mad_indexed_renderer(bytes)?;
    let (pools, mut strings) = catalog_string_pools(bytes)?;
    let requests = catalog_binding_requests(bytes)?;
    let string_index: BTreeMap<_, _> = strings
        .iter()
        .enumerate()
        .map(|(index, string)| (string.runtime_address, index))
        .collect();
    let mut bound_addresses = BTreeSet::new();
    let mut bindings = Vec::with_capacity(requests.len());

    for request in requests {
        let mut runtime_address = request.runtime_address;
        let mut string_ids = Vec::with_capacity(request.line_count);
        for _ in 0..request.line_count {
            let string_index = *string_index.get(&runtime_address).with_context(|| {
                format!(
                    "MAD indexed-text binding {} selects unknown string {runtime_address:#x}",
                    request.id
                )
            })?;
            let string = &strings[string_index];
            bound_addresses.insert(runtime_address);
            string_ids.push(string.id.clone());
            runtime_address += string.byte_size;
        }
        let (content_kind, display_text) = if request.id == "result-banner" {
            ensure!(
                string_ids == ["mad-indexed-81c2", "mad-indexed-81d1"],
                "MAD result banner no longer selects the reviewed glyph fragments"
            );
            (
                MadIndexedBindingContentKind::CompositeGlyphBanner,
                Some("ステージクリア".to_owned()),
            )
        } else {
            let lines = string_ids
                .iter()
                .map(|id| {
                    strings
                        .iter()
                        .find(|string| &string.id == id)
                        .and_then(|string| string.text.as_deref())
                })
                .collect::<Option<Vec<_>>>();
            (
                MadIndexedBindingContentKind::TextLines,
                lines.map(|lines| lines.join("\n")),
            )
        };
        let contains_japanese = display_text
            .as_deref()
            .is_some_and(|text| text.chars().any(is_japanese_character));
        bindings.push(MadIndexedBinding {
            id: request.id,
            consumer: request.consumer,
            reference_file_offsets: request.reference_file_offsets,
            string_ids,
            content_kind,
            display_text,
            contains_japanese,
        });
    }

    for string in &mut strings {
        string.bound_to_consumer = bound_addresses.contains(&string.runtime_address);
    }
    let unbound_string_ids: Vec<_> = strings
        .iter()
        .filter(|string| !string.bound_to_consumer)
        .map(|string| string.id.clone())
        .collect();
    ensure!(
        unbound_string_ids == ["mad-indexed-7d1b"],
        "MAD indexed-text pool consumer coverage changed: {unbound_string_ids:?}"
    );

    Ok(MadIndexedTextCatalog {
        mad_com_sha256: disk::sha256_hex(bytes),
        file_size: bytes.len(),
        renderer: MadIndexedRendererEvidence {
            file_offset: renderer.file_offset,
            runtime_address: renderer.runtime_address,
            decoded_instruction_count: renderer.decoded_instruction_count,
            isa_profile_id: v30::PROFILE_ID.to_owned(),
        },
        pools,
        strings,
        bindings,
        unbound_string_ids,
    })
}

fn catalog_string_pools(bytes: &[u8]) -> Result<(Vec<MadIndexedTextPool>, Vec<MadIndexedString>)> {
    const POOLS: &[(&str, usize, usize)] = &[
        ("numeric-glyphs", 0x74f9, 0x750d),
        ("battle-and-menu-text", 0x7975, 0x80eb),
        ("system-and-terrain-text", 0x813c, 0x81e0),
    ];
    let mut pools = Vec::with_capacity(POOLS.len());
    let mut strings = Vec::new();

    for (id, runtime_start, runtime_end) in POOLS {
        let file_start = runtime_start - COM_ORIGIN;
        let file_end = runtime_end - COM_ORIGIN;
        let pool_bytes = bytes
            .get(file_start..file_end)
            .with_context(|| format!("MAD indexed-text pool {id} is truncated"))?;
        let mut cursor = 0;
        let first_string_index = strings.len();
        while cursor < pool_bytes.len() {
            let line = indexed_text::parse_indexed_lines(&pool_bytes[cursor..], 1)
                .with_context(|| format!("parse MAD indexed-text pool {id} at {cursor:#x}"))?;
            ensure!(
                indexed_text::reconstruct_indexed_lines(&line.lines)? == line.bytes,
                "MAD indexed string does not reconstruct in pool {id}"
            );
            let runtime_address = runtime_start + cursor;
            let glyphs: Vec<_> = line.lines[0]
                .glyphs
                .iter()
                .map(|glyph| MadIndexedGlyph {
                    index: glyph.index,
                    text: glyph.text.map(str::to_owned),
                })
                .collect();
            let text = glyphs
                .iter()
                .map(|glyph| glyph.text.as_deref())
                .collect::<Option<String>>();
            let contains_japanese = text
                .as_deref()
                .is_some_and(|text| text.chars().any(is_japanese_character));
            strings.push(MadIndexedString {
                id: format!("mad-indexed-{runtime_address:04x}"),
                runtime_address,
                file_offset: runtime_address - COM_ORIGIN,
                byte_size: line.byte_len,
                sha256: disk::sha256_hex(line.bytes),
                raw_hex: encode_hex(line.bytes),
                text,
                contains_japanese,
                bound_to_consumer: false,
                glyphs,
            });
            cursor += line.byte_len;
        }
        ensure!(
            cursor == pool_bytes.len(),
            "MAD indexed-text pool {id} does not end on an FF boundary"
        );
        pools.push(MadIndexedTextPool {
            id: (*id).to_owned(),
            runtime_start: *runtime_start,
            runtime_end: *runtime_end,
            file_start,
            file_end,
            string_count: strings.len() - first_string_index,
        });
    }
    Ok((pools, strings))
}

fn catalog_binding_requests(bytes: &[u8]) -> Result<Vec<BindingRequest>> {
    let mut requests = Vec::new();

    add_pointer_table(
        bytes,
        &mut requests,
        "terrain",
        "terrain label lookup",
        0x1730,
        9,
    )?;
    add_pointer_table(
        bytes,
        &mut requests,
        "unit-name",
        "unit name lookup",
        0x43cd,
        20,
    )?;
    add_pointer_table(
        bytes,
        &mut requests,
        "health-label",
        "health bucket lookup",
        0x4531,
        5,
    )?;
    add_pointer_table(
        bytes,
        &mut requests,
        "magic-label",
        "magic bucket lookup",
        0x453b,
        6,
    )?;
    add_pointer_table(
        bytes,
        &mut requests,
        "progress-label",
        "progress bucket lookup",
        0x4547,
        7,
    )?;
    add_menu_table(bytes, &mut requests, "menu-action", 0x4274, 16)?;
    add_menu_table(bytes, &mut requests, "menu-group", 0x42d4, 16)?;
    add_pointer_count_table(
        bytes,
        &mut requests,
        "attack",
        "unit attack callout table",
        0x9495,
        38,
        |index| format!("attack-unit-{:02}-variant-{}", index / 2 + 1, index % 2 + 1),
    )?;
    add_pointer_count_table(
        bytes,
        &mut requests,
        "damage",
        "unit damage response table",
        0x952d,
        38,
        |index| format!("damage-unit-{:02}-variant-{}", index / 2 + 1, index % 2 + 1),
    )?;
    add_pointer_count_table(
        bytes,
        &mut requests,
        "actor-health",
        "active unit health-state table",
        0x95c5,
        50,
        |index| format!("actor-unit-{:02}-health-{}", index / 5 + 1, index % 5),
    )?;
    add_pointer_count_table(
        bytes,
        &mut requests,
        "opponent-health",
        "opposing unit health-state table",
        0x968d,
        50,
        |index| format!("opponent-unit-{:02}-health-{}", index / 5 + 1, index % 5),
    )?;
    add_pointer_count_table(
        bytes,
        &mut requests,
        "defeat",
        "unit defeat callout table",
        0x9755,
        19,
        |index| format!("defeat-unit-{:02}", index + 1),
    )?;

    for digit in 0..10 {
        let runtime_address = 0x74f9 + digit * 2;
        verify_mov_si(bytes, 0x455f, 0x74f9)?;
        verify_mov_si(bytes, 0x4577, 0x74f9)?;
        requests.push(BindingRequest {
            id: format!("numeric-glyph-{digit}"),
            consumer: "two-digit indexed number renderer".to_owned(),
            reference_file_offsets: vec![0x455f, 0x4577],
            runtime_address,
            line_count: 1,
        });
    }

    const DIRECT: &[(&str, &str, usize, usize, &[usize])] = &[
        ("carbuncle-name", "map object label", 0x7975, 1, &[0x16b7]),
        (
            "empty-meter",
            "empty progress label",
            0x7a4d,
            1,
            &[0x16c3, 0x16cf, 0x1840],
        ),
        ("friendly-side", "map side label", 0x8178, 1, &[0x1754]),
        ("enemy-side", "map side label", 0x817c, 1, &[0x175c]),
        ("spring-terrain", "map terrain label", 0x818e, 1, &[0x17cd]),
        (
            "spell-healing",
            "spell command",
            0x7a07,
            1,
            &[0x3740, 0xa536],
        ),
        (
            "spell-large-healing",
            "spell command",
            0x7a0d,
            1,
            &[0x3977, 0xa4e3],
        ),
        ("spell-brain-dumbed", "spell command", 0x7a15, 1, &[0x39dd]),
        ("spell-jugem", "spell command", 0x7a1d, 1, &[0x399a, 0xa57c]),
        ("spell-bayoen", "spell command", 0x7a22, 1, &[0x39b9]),
        ("unit-rank-e", "unit status rank", 0x81b2, 1, &[0x442d]),
        ("magic-recovery", "map notification", 0x815c, 1, &[0x5d8e]),
        ("whole-map", "map panel title", 0x8167, 1, &[0x36b5]),
        ("hurry", "turn notification", 0x816f, 1, &[0x9d7f]),
        ("friendly-turn", "turn notification", 0x81b4, 1, &[0x59b8]),
        ("enemy-turn", "turn notification", 0x81bb, 1, &[0x59c3]),
        ("spring-captured", "map notification", 0x814a, 1, &[0x5eb8]),
        ("spring-lost", "map notification", 0x8153, 1, &[0x5ec3]),
        ("retry", "battle result menu", 0x813c, 1, &[0x60bf]),
        (
            "return-to-title",
            "battle result menu",
            0x8141,
            1,
            &[0x60cb],
        ),
        (
            "result-banner",
            "two-line composite glyph banner",
            0x81c2,
            2,
            &[0x6177],
        ),
    ];
    for (id, consumer, runtime_address, line_count, references) in DIRECT {
        for reference in *references {
            verify_mov_si(bytes, *reference, *runtime_address)?;
        }
        requests.push(BindingRequest {
            id: (*id).to_owned(),
            consumer: (*consumer).to_owned(),
            reference_file_offsets: references.to_vec(),
            runtime_address: *runtime_address,
            line_count: *line_count,
        });
    }

    Ok(requests)
}

fn add_pointer_table(
    bytes: &[u8],
    requests: &mut Vec<BindingRequest>,
    id_prefix: &str,
    consumer: &str,
    table_file_offset: usize,
    record_count: usize,
) -> Result<()> {
    for index in 0..record_count {
        let reference_file_offset = table_file_offset + index * 2;
        let runtime_address = read_u16(bytes, reference_file_offset)? as usize;
        requests.push(BindingRequest {
            id: format!("{id_prefix}-{index:02}"),
            consumer: consumer.to_owned(),
            reference_file_offsets: vec![reference_file_offset],
            runtime_address,
            line_count: 1,
        });
    }
    Ok(())
}

fn add_menu_table(
    bytes: &[u8],
    requests: &mut Vec<BindingRequest>,
    id_prefix: &str,
    table_file_offset: usize,
    record_count: usize,
) -> Result<()> {
    for index in 0..record_count {
        let record_offset = table_file_offset + index * 6;
        let line_count = read_u16(bytes, record_offset + 2)? as usize;
        let runtime_address = read_u16(bytes, record_offset + 4)? as usize;
        if line_count == 0 && runtime_address == 0 {
            continue;
        }
        ensure!(
            line_count > 0 && runtime_address >= COM_ORIGIN,
            "MAD menu table has a partial text record at {record_offset:#x}"
        );
        requests.push(BindingRequest {
            id: format!("{id_prefix}-{index:02}"),
            consumer: "menu selection table".to_owned(),
            reference_file_offsets: vec![record_offset + 4],
            runtime_address,
            line_count,
        });
    }
    Ok(())
}

fn add_pointer_count_table<F>(
    bytes: &[u8],
    requests: &mut Vec<BindingRequest>,
    _id_prefix: &str,
    consumer: &str,
    table_file_offset: usize,
    record_count: usize,
    id: F,
) -> Result<()>
where
    F: Fn(usize) -> String,
{
    for index in 0..record_count {
        let record_offset = table_file_offset + index * 4;
        let runtime_address = read_u16(bytes, record_offset)? as usize;
        let line_count = read_u16(bytes, record_offset + 2)? as usize;
        ensure!(
            runtime_address >= COM_ORIGIN && line_count > 0,
            "MAD pointer/count table has an empty record at {record_offset:#x}"
        );
        requests.push(BindingRequest {
            id: id(index),
            consumer: consumer.to_owned(),
            reference_file_offsets: vec![record_offset],
            runtime_address,
            line_count,
        });
    }
    Ok(())
}

fn verify_mov_si(bytes: &[u8], file_offset: usize, runtime_address: usize) -> Result<()> {
    let pointer = u16::try_from(runtime_address)
        .context("MAD indexed string address does not fit a 16-bit COM pointer")?
        .to_le_bytes();
    ensure!(
        bytes.get(file_offset..file_offset + 3) == Some(&[0xbe, pointer[0], pointer[1]]),
        "MAD indexed string reference does not match at {file_offset:#x}"
    );
    Ok(())
}

fn read_u16(bytes: &[u8], file_offset: usize) -> Result<u16> {
    let raw: [u8; 2] = bytes
        .get(file_offset..file_offset + 2)
        .with_context(|| format!("MAD field is truncated at file offset {file_offset:#x}"))?
        .try_into()
        .expect("a two-byte range converts to a two-byte array");
    Ok(u16::from_le_bytes(raw))
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

fn is_japanese_character(character: char) -> bool {
    matches!(character,
        '\u{3040}'..='\u{30ff}'
            | '\u{31f0}'..='\u{31ff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{4e00}'..='\u{9fff}'
            | '\u{ff66}'..='\u{ff9f}'
    )
}
