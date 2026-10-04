use std::collections::{BTreeMap, BTreeSet};

use super::{IndexedFontGlyph, compile_indexed_font_bank, compile_mapped_indexed_font_bank};
use crate::game_data::indexed_text::FONT_DECODED_SIZE;

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn indexed_font_bank_preserves_reserved_slots_and_round_trips() {
    let source = vec![0x5a; FONT_DECODED_SIZE];
    let reserved = BTreeSet::from([0x00, 0x3b, 0x3f]);

    let bank =
        compile_indexed_font_bank(&source, ['가', ' ', '!', '가'], &reserved, "test-dialogue")
            .unwrap();

    assert_eq!(bank.codebook.len(), 3);
    assert!(
        bank.codebook
            .values()
            .all(|index| !reserved.contains(index))
    );
    for index in reserved {
        let offset = usize::from(index) * 32;
        assert_eq!(
            bank.decoded[offset..offset + 32],
            source[offset..offset + 32]
        );
    }
    assert!(!bank.packed.is_empty());
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn mapped_font_bank_keeps_shared_indices_and_accepts_custom_tiles() {
    let source = vec![0x5a; FONT_DECODED_SIZE];
    let codebook = BTreeMap::from([('가', 3), (' ', 8)]);
    let custom = BTreeMap::from([(
        9,
        IndexedFontGlyph {
            label: "large-title-top-left".to_owned(),
            bitmap: [0xa5; 32],
        },
    )]);

    let bank = compile_mapped_indexed_font_bank(
        &source,
        codebook.clone(),
        &custom,
        &BTreeSet::from([0xff]),
        "mapped-test",
    )
    .unwrap();

    assert_eq!(bank.codebook, codebook);
    assert_eq!(&bank.decoded[9 * 32..10 * 32], &[0xa5; 32]);
    assert_eq!(&bank.decoded[0xff * 32..0x100 * 32], &[0x5a; 32]);
}
