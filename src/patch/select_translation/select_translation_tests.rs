use std::collections::BTreeMap;

use super::consumer::{
    STAGE_FONT_RESUME_SETUP_FILE_OFFSET, STAGE_FONT_SETUP_FILE_OFFSET,
    install_select_stage_font_loader,
};
use super::text::{SelectScriptTranslation, SelectStageTranslation, replace_select_dialogue};

const INTERPRETER_OFFSET: usize = 0x2082;
const POINTER_TABLE_OFFSET: usize = 0x42a8;
const SCRIPT_COUNT: usize = 54;

#[test]
fn select_stage_font_hook_is_typed_appended_and_revision_guarded() {
    let mut source = vec![0x90; 0x5000];
    source[STAGE_FONT_SETUP_FILE_OFFSET..STAGE_FONT_SETUP_FILE_OFFSET + 5]
        .copy_from_slice(&[0xfe, 0xc8, 0xb4, 0x00, 0x50]);
    source[STAGE_FONT_RESUME_SETUP_FILE_OFFSET..STAGE_FONT_RESUME_SETUP_FILE_OFFSET + 4]
        .copy_from_slice(&[0xb4, 0x00, 0x48, 0x50]);

    let patched = install_select_stage_font_loader(&source).unwrap();

    assert_eq!(patched.hook_file_offset, source.len());
    assert_eq!(patched.hook_runtime_address, source.len() + 0x100);
    assert_eq!(patched.copied_glyph_count, 240);
    assert_eq!(patched.bytes[STAGE_FONT_SETUP_FILE_OFFSET], 0xe9);
    assert_eq!(patched.bytes[STAGE_FONT_RESUME_SETUP_FILE_OFFSET], 0xe9);
    assert_eq!(patched.bytes.len(), source.len() + patched.hook_byte_size);
    assert_eq!(
        near_target(
            0x40c7,
            &patched.bytes[STAGE_FONT_SETUP_FILE_OFFSET..STAGE_FONT_SETUP_FILE_OFFSET + 3]
        ),
        patched.hook_runtime_address
    );
    assert_eq!(
        near_target(
            0x4117,
            &patched.bytes
                [STAGE_FONT_RESUME_SETUP_FILE_OFFSET..STAGE_FONT_RESUME_SETUP_FILE_OFFSET + 3]
        ),
        patched.resume_hook_runtime_address
    );

    let initial_wrapper = patched.hook_file_offset;
    assert_eq!(
        &patched.bytes[initial_wrapper..initial_wrapper + 5],
        &[0xfe, 0xc8, 0xb4, 0x00, 0x50]
    );
    assert_eq!(
        near_target(
            patched.hook_runtime_address + 5,
            &patched.bytes[initial_wrapper + 5..initial_wrapper + 8]
        ),
        patched.shared_loader_runtime_address
    );
    assert_eq!(
        near_target(
            patched.hook_runtime_address + 8,
            &patched.bytes[initial_wrapper + 8..initial_wrapper + 11]
        ),
        0x40cc
    );

    let resume_wrapper = patched.hook_file_offset + patched.resume_hook_runtime_address
        - patched.hook_runtime_address;
    assert_eq!(
        &patched.bytes[resume_wrapper..resume_wrapper + 4],
        &[0xb4, 0x00, 0x48, 0x50]
    );
    assert_eq!(
        near_target(
            patched.resume_hook_runtime_address + 4,
            &patched.bytes[resume_wrapper + 4..resume_wrapper + 7]
        ),
        patched.shared_loader_runtime_address
    );
    assert_eq!(
        near_target(
            patched.resume_hook_runtime_address + 7,
            &patched.bytes[resume_wrapper + 7..resume_wrapper + 10]
        ),
        0x411b
    );
    let shared_loader = patched.hook_file_offset + patched.shared_loader_runtime_address
        - patched.hook_runtime_address;
    assert_eq!(patched.bytes[shared_loader], 0x9c);
    assert_eq!(patched.bytes[patched.bytes.len() - 1], 0xc3);

    source[STAGE_FONT_SETUP_FILE_OFFSET + 3] ^= 1;
    assert!(install_select_stage_font_loader(&source).is_err());

    source[STAGE_FONT_SETUP_FILE_OFFSET + 3] ^= 1;
    source[STAGE_FONT_RESUME_SETUP_FILE_OFFSET + 1] ^= 1;
    assert!(install_select_stage_font_loader(&source).is_err());
}

fn near_target(instruction_runtime_address: usize, bytes: &[u8]) -> usize {
    assert!(matches!(bytes[0], 0xe8 | 0xe9));
    let displacement = i16::from_le_bytes(bytes[1..3].try_into().unwrap());
    usize::try_from(instruction_runtime_address as isize + 3 + displacement as isize).unwrap()
}

#[test]
fn select_dialogue_repoints_all_phases_and_neutralizes_unused_pool_bytes() {
    let source = synthetic_select_com();
    let translations = (1..=9)
        .map(|stage_number| SelectStageTranslation {
            stage_number,
            scripts: (1..=6)
                .map(|phase_number| SelectScriptTranslation {
                    entry_id: format!(
                        "select-dialogue:stage-{stage_number:02}-phase-{phase_number:02}"
                    ),
                    spans: vec!["가".to_owned()],
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    let codebooks = vec![BTreeMap::from([('가', 0x10)]); 9];

    let patched = replace_select_dialogue(&source, &translations, &codebooks).unwrap();

    assert_eq!(patched.script_pool_capacity, SCRIPT_COUNT * 4);
    assert_eq!(patched.script_pool_used, SCRIPT_COUNT * 4);
    for index in 0..SCRIPT_COUNT {
        let pointer_offset = POINTER_TABLE_OFFSET + index * 2;
        let pointer = u16::from_le_bytes(
            patched.bytes[pointer_offset..pointer_offset + 2]
                .try_into()
                .unwrap(),
        );
        assert_eq!(
            usize::from(pointer),
            POINTER_TABLE_OFFSET + SCRIPT_COUNT * 2 + 0x100 + index * 4
        );
        let script_offset = patched.script_pool_file_offset + index * 4;
        assert_eq!(
            &patched.bytes[script_offset..script_offset + 4],
            &[0x01, 0x00, 0x10, 0xff]
        );
    }
}

fn synthetic_select_com() -> Vec<u8> {
    let script_pool_offset = POINTER_TABLE_OFFSET + SCRIPT_COUNT * 2;
    let mut bytes = vec![0x90; script_pool_offset + SCRIPT_COUNT * 4];
    bytes[INTERPRETER_OFFSET..INTERPRETER_OFFSET + 19].copy_from_slice(&[
        0x55, 0x2e, 0x8b, 0x1e, 0xa6, 0x43, 0x2e, 0x8a, 0x07, 0xbb, 0x44, 0x02, 0xfe, 0xc8, 0x74,
        0x03, 0xbb, 0xfe, 0x01,
    ]);
    bytes[INTERPRETER_OFFSET + 0x230] = 0xc3;
    for call_offset in [0x3ff9, 0x403f, 0x405b, 0x406a] {
        bytes[call_offset..call_offset + 3].copy_from_slice(&[0xe8, 0x00, 0x00]);
    }
    for index in 0..SCRIPT_COUNT {
        let file_offset = script_pool_offset + index * 4;
        let runtime_address = u16::try_from(file_offset + 0x100).unwrap();
        bytes[POINTER_TABLE_OFFSET + index * 2..POINTER_TABLE_OFFSET + index * 2 + 2]
            .copy_from_slice(&runtime_address.to_le_bytes());
        bytes[file_offset..file_offset + 4].copy_from_slice(&[0x01, 0x00, 0x00, 0xff]);
    }
    bytes
}
