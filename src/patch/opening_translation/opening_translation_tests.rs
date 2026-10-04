use super::consumer::{FONT_LOAD_FILE_OFFSET, select_opening_font_bank};
use super::{VERIFIED_DISPLAY_RUN_LENGTHS, validate_opening_display_runs};
use crate::game_data::opening_script::OpeningDisplayRun;

#[test]
fn opening_font_consumer_patch_is_typed_and_revision_guarded() {
    let mut source = vec![0_u8; FONT_LOAD_FILE_OFFSET + 3];
    source[FONT_LOAD_FILE_OFFSET..FONT_LOAD_FILE_OFFSET + 3].copy_from_slice(&[0xb8, 0x06, 0x00]);

    let patched = select_opening_font_bank(&source).unwrap();

    assert_eq!(
        &patched.bytes[FONT_LOAD_FILE_OFFSET..FONT_LOAD_FILE_OFFSET + 3],
        &[0xb8, 0x9b, 0x00]
    );
    source[FONT_LOAD_FILE_OFFSET + 1] = 7;
    assert!(select_opening_font_bank(&source).is_err());
}

#[test]
fn opening_dialogue_must_finish_inside_each_display_run() {
    let runs = display_runs();
    let broken = "카레 재료가 정말 하나도 없어! 후쿠진즈케는 잔뜩 있는데! 큰일이야! 으으음... 좋아! 카레 재료를 찾으러 가자! 힘내서 꼭 해내자! 응!"
        .chars()
        .collect::<Vec<_>>();

    let error = validate_opening_display_runs(&broken, &runs).unwrap_err();

    assert!(error.to_string().contains("run 0 ends mid-sentence"));
}

#[test]
fn opening_dialogue_accepts_independently_completed_display_runs() {
    let runs = display_runs();
    let fitted = [
        "카레 재료가 정말 하나도 없어!  ",
        "행운절임은   잔뜩 있는데!    ",
        "큰일이야! 음...",
        "좋아! 재료를  찾으러 가자!   ",
        "힘낼 거야! 응!  ",
    ]
    .concat()
    .chars()
    .collect::<Vec<_>>();

    validate_opening_display_runs(&fitted, &runs).unwrap();
}

fn display_runs() -> Vec<OpeningDisplayRun> {
    let mut start = 0;
    VERIFIED_DISPLAY_RUN_LENGTHS
        .into_iter()
        .enumerate()
        .map(|(index, length)| {
            let end = start + length;
            let line_break_offsets = match index {
                0 => vec![10],
                1 => vec![8],
                3 => vec![9],
                _ => vec![],
            };
            let run = OpeningDisplayRun {
                glyph_range: start..end,
                line_break_offsets,
            };
            start = end;
            run
        })
        .collect()
}
