use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::game_data::ending_meal_captions::{
    COM_ORIGIN, GLYPH_RENDERER_CALL_FILE_OFFSET, GLYPH_RENDERER_RUNTIME_ADDRESS,
    INTERPRETER_FILE_OFFSET, INTERPRETER_SIGNATURE, parse_ending_meal_captions,
};

#[test]
fn replaces_every_caption_slot_without_changing_animation_commands() {
    let source = synthetic_ending_com();
    let source_catalog = parse_ending_meal_captions(&source).unwrap();
    let translations = translations();
    let codebook = codebook_for(&translations);

    let patched = replace_ending_meal_captions(&source, &translations, &codebook).unwrap();

    assert_eq!(patched.report.record_count, 5);
    assert_eq!(patched.report.span_count, 6);
    assert_eq!(patched.report.glyph_slot_count, 54);
    let slot_offsets = source_catalog
        .records
        .iter()
        .flat_map(|record| &record.spans)
        .flat_map(|span| span.slot_file_offsets.iter().copied())
        .collect::<BTreeSet<_>>();
    for record in &source_catalog.records {
        let record_end = record.file_offset + record.bytes.len();
        for (relative_offset, (&patched_byte, &source_byte)) in patched.bytes
            [record.file_offset..record_end]
            .iter()
            .zip(&source[record.file_offset..record_end])
            .enumerate()
        {
            let file_offset = record.file_offset + relative_offset;
            if !slot_offsets.contains(&file_offset) {
                assert_eq!(
                    patched_byte, source_byte,
                    "animation command changed at {file_offset:#x}"
                );
            }
        }
    }
    for record in &source_catalog.records {
        let translated_spans = &translations[record.id];
        for (span, translation) in record.spans.iter().zip(translated_spans) {
            let expected = encode_centered_span(
                translation,
                span.slot_file_offsets.len(),
                &codebook,
                record.id,
                0,
            )
            .unwrap();
            assert_eq!(
                span.slot_file_offsets
                    .iter()
                    .map(|offset| patched.bytes[*offset])
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}

#[test]
fn rejects_a_translation_that_exceeds_its_fixed_visible_slots() {
    let source = synthetic_ending_com();
    let mut translations = translations();
    translations.insert("ordinary-reaction".into(), vec!["가나다라마바사".into()]);
    let codebook = codebook_for(&translations);

    assert!(
        replace_ending_meal_captions(&source, &translations, &codebook)
            .unwrap_err()
            .to_string()
            .contains("requires 1..=6")
    );
}

fn translations() -> BTreeMap<String, Vec<String>> {
    BTreeMap::from([
        ("before-eating".into(), vec!["잘 먹겠습니다".into()]),
        ("ordinary-reaction".into(), vec!["맛있어!!".into()]),
        ("excellent-reaction".into(), vec!["엄청 맛있어!!!".into()]),
        ("bad-reaction".into(), vec!["..........".into()]),
        (
            "legendary-reaction".into(),
            vec!["…이것이…".into(), "전설의 카레~!!!!".into()],
        ),
    ])
}

fn codebook_for(translations: &BTreeMap<String, Vec<String>>) -> BTreeMap<char, u8> {
    translations
        .values()
        .flatten()
        .flat_map(|text| text.chars())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(|(index, character)| (character, 0x60 + u8::try_from(index).unwrap()))
        .collect()
}

fn synthetic_ending_com() -> Vec<u8> {
    let mut bytes = vec![0x90; 0x41a7];
    bytes[INTERPRETER_FILE_OFFSET..INTERPRETER_FILE_OFFSET + INTERPRETER_SIGNATURE.len()]
        .copy_from_slice(INTERPRETER_SIGNATURE);
    let displacement = i16::try_from(
        GLYPH_RENDERER_RUNTIME_ADDRESS as isize
            - (GLYPH_RENDERER_CALL_FILE_OFFSET + COM_ORIGIN + 3) as isize,
    )
    .unwrap()
    .to_le_bytes();
    bytes[GLYPH_RENDERER_CALL_FILE_OFFSET..GLYPH_RENDERER_CALL_FILE_OFFSET + 3].copy_from_slice(&[
        0xe8,
        displacement[0],
        displacement[1],
    ]);

    let records = [
        (
            0x404e,
            0x4074,
            vec![vec![0x10, 0x06, 0x07, 0x12, 0x0c, 0xc2, 0x24]],
        ),
        (
            0x4074,
            0x40b2,
            vec![vec![0x40, 0x10, 0x14, 0x10, 0xa1, 0xa1]],
        ),
        (
            0x40b2,
            0x40f4,
            vec![vec![
                0x16, 0xc0, 0xc2, 0x40, 0x10, 0x14, 0x10, 0xa1, 0xa1, 0xa1,
            ]],
        ),
        (0x40f4, 0x412d, vec![vec![0xe0; 10]]),
        (
            0x412d,
            0x41a7,
            vec![
                vec![0xe0, 0xe0, 0xe0, 0x42, 0x3e, 0x03, 0xe0, 0xe0, 0xe0],
                vec![
                    0x37, 0x2f, 0x34, 0x26, 0x48, 0x52, 0x8e, 0xc2, 0xa1, 0xa1, 0xa1, 0xa1,
                ],
            ],
        ),
    ];
    let slot_offsets = [
        vec![vec![0x4057, 0x405a, 0x405d, 0x4060, 0x4063, 0x4066, 0x4069]],
        vec![vec![0x4081, 0x4084, 0x4087, 0x408a, 0x408d, 0x4090]],
        vec![vec![
            0x40b9, 0x40bb, 0x40bd, 0x40bf, 0x40c1, 0x40c3, 0x40c5, 0x40c7, 0x40c9, 0x40cb,
        ]],
        vec![vec![
            0x40fb, 0x40ff, 0x4103, 0x4107, 0x410b, 0x410f, 0x4113, 0x4117, 0x411b, 0x411f,
        ]],
        vec![
            vec![
                0x4133, 0x4136, 0x4139, 0x413c, 0x413f, 0x4142, 0x4145, 0x4148, 0x414b,
            ],
            vec![
                0x4158, 0x415a, 0x415c, 0x415e, 0x4160, 0x4162, 0x4164, 0x4166, 0x4168, 0x416a,
                0x416c, 0x416e,
            ],
        ],
    ];
    for ((record_index, (start, end, span_glyphs)), spans) in
        records.into_iter().enumerate().zip(slot_offsets)
    {
        bytes[start..end].fill(0x1e);
        bytes[start] = [0x01, 0x02, 0x04, 0x03, 0x02][record_index];
        bytes[end - 1] = 0x28;
        if record_index == 4 {
            bytes[0x4156] = 0x0b;
            bytes[0x4155] = 0x04;
        }
        for (glyphs, offsets) in span_glyphs.into_iter().zip(spans) {
            for (glyph, offset) in glyphs.into_iter().zip(offsets) {
                bytes[offset - 1] = 0x0a;
                bytes[offset] = glyph;
            }
        }
    }
    bytes
}
