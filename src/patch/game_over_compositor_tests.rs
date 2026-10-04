use super::*;

fn source_fixture() -> Vec<u8> {
    let mut pixels = vec![0_u8; WIDTH * HEIGHT];
    for y in 16..48 {
        for x in 32..256 {
            pixels[y * WIDTH + x] = match x % 7 {
                0..=3 => 5,
                4..=5 => 4,
                _ => 1,
            };
        }
    }
    pixels
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn game_over_composition_uses_the_source_palette_and_canvas() {
    let source = source_fixture();
    let composition = compose_game_over_graphic(&source, "게임 오버").unwrap();

    assert_eq!(
        composition.palette,
        GameOverPalette {
            background: 0,
            primary: 5,
            outline: Some(4),
            shadow: Some(1),
        }
    );
    assert_eq!(composition.pixels.len(), WIDTH * HEIGHT);
    assert_eq!(composition.pixels[0], 0);
    assert!(composition.pixels.contains(&5));
}

#[test]
fn game_over_composition_rejects_an_ambiguous_background() {
    let mut source = source_fixture();
    source[WIDTH - 1] = 7;

    let error = compose_game_over_graphic(&source, "게임 오버").unwrap_err();

    assert!(error.to_string().contains("corners"));
}
