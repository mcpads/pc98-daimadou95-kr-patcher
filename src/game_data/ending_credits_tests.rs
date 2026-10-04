use super::{
    EndingCreditGrid, EndingCreditLine, EndingCreditMotionTrack, EndingCreditPoint,
    EndingCreditScreen, ROLE_LINE_SPECS, ending_credit_glyph_offset, ending_credit_line_offset,
    parse_ending_credit_grid, parse_ending_credit_motion_track, reconstruct_ending_credit_grid,
    reconstruct_ending_credit_motion_track, role_line_spec,
};

#[test]
fn credit_grid_preserves_fixed_cells_and_dos_eof() {
    let mut source = vec![0; 600];
    source[20..25].copy_from_slice(&[0x13, 0x14, 0x01, 0x06, 0x06]);
    source.push(0x1a);

    let grid = parse_ending_credit_grid(&source).unwrap();
    assert_eq!(grid.screens.len(), 10);
    assert_eq!(grid.screens[0].lines[1].text, "STAFF");
    assert_eq!(reconstruct_ending_credit_grid(&grid).unwrap(), source);
}

#[test]
fn credit_grid_rejects_unknown_glyphs_and_wrong_shape() {
    let mut unknown = vec![0; 601];
    unknown[0] = 0x35;
    unknown[600] = 0x1a;
    assert!(
        parse_ending_credit_grid(&unknown)
            .unwrap_err()
            .to_string()
            .contains("unresolved glyph")
    );
    assert!(parse_ending_credit_grid(&[0; 600]).is_err());

    let invalid = EndingCreditGrid {
        screens: vec![EndingCreditScreen {
            lines: vec![EndingCreditLine {
                glyph_indices: [0; 20],
                text: String::new(),
            }],
        }],
    };
    assert!(reconstruct_ending_credit_grid(&invalid).is_err());
}

#[test]
fn credit_motion_track_preserves_points_and_terminator() {
    let points = vec![EndingCreditPoint { x: 12, y: 34 }; 190];
    let track = EndingCreditMotionTrack {
        points,
        terminator: 0x0280,
    };
    let source = reconstruct_ending_credit_motion_track(&track).unwrap();
    let parsed = parse_ending_credit_motion_track(&source).unwrap();
    assert_eq!(parsed, track);
    assert_eq!(source.len(), 762);
}

#[test]
fn credit_motion_track_rejects_early_or_trailing_terminators() {
    assert!(parse_ending_credit_motion_track(&[0x80, 0x02]).is_err());

    let mut source = Vec::new();
    for _ in 0..190 {
        source.extend_from_slice(&[1, 0, 2, 0]);
    }
    source.extend_from_slice(&[0x80, 0x02, 0, 0]);
    assert!(
        parse_ending_credit_motion_track(&source)
            .unwrap_err()
            .to_string()
            .contains("bytes after")
    );
}

#[test]
fn role_lines_bind_the_ten_reviewed_credit_records() {
    assert_eq!(ROLE_LINE_SPECS.len(), 10);
    assert_eq!(
        ROLE_LINE_SPECS[0].target_id(),
        "ending-credit:screen-01-line-02"
    );
    assert_eq!(ROLE_LINE_SPECS[9].source_text, "Presented By");
    assert_eq!(ending_credit_line_offset(1, 2).unwrap(), 20);
    assert_eq!(ending_credit_line_offset(10, 1).unwrap(), 540);
    assert_eq!(ending_credit_glyph_offset(1).unwrap(), 0);
    assert_eq!(ending_credit_glyph_offset(65).unwrap(), 64 * 96);
    assert_eq!(role_line_spec(6, 1).unwrap().source_text, "SPECIAL THANKS");
    assert!(role_line_spec(6, 2).is_none());
    assert!(ending_credit_line_offset(0, 1).is_err());
    assert!(ending_credit_glyph_offset(0).is_err());
}
