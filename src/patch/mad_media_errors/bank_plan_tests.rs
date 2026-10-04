use super::*;
use crate::source::GaijiCatalogEntry;

fn gaiji_catalog(count: usize) -> GaijiCatalog {
    GaijiCatalog {
        sha256: String::new(),
        file_size: 0,
        pointer_table_offset: 0,
        first_character_code: None,
        last_character_code: None,
        first_shift_jis_code: None,
        last_shift_jis_code: None,
        uniform_glyph_size: Some(34),
        reviewed_content_kind: crate::source::GaijiContentKind::PixelArtFragments,
        contains_japanese_text: false,
        glyphs: (0..count)
            .map(|index| GaijiCatalogEntry {
                index,
                character_code: 0x7622 + index as u16,
                shift_jis_code: 0xeba0 + index as u16,
                offset: index * 34,
                size: 34,
                sha256: String::new(),
            })
            .collect(),
    }
}

#[test]
fn shift_jis_supply_is_distinguished_from_external_glyph_demand() {
    assert!(is_shift_jis_encodable('A'));
    assert!(is_shift_jis_encodable('★'));
    assert!(!is_shift_jis_encodable('한'));
}

#[test]
fn mad_slots_never_replace_the_source_bank_or_persistent_consumer_codes() {
    let mut catalog = gaiji_catalog(175);
    for (index, glyph) in catalog.glyphs.iter_mut().enumerate() {
        let character_code = if index < 93 {
            0x7622 + index as u16
        } else {
            0x7721 + (index - 93) as u16
        };
        glyph.character_code = character_code;
        glyph.shift_jis_code =
            crate::game_data::gaiji::jis_row_cell_to_shift_jis(character_code).unwrap();
    }
    catalog.first_character_code = Some(FIRST_SOURCE_CHARACTER_CODE);
    catalog.last_character_code = Some(LAST_SOURCE_CHARACTER_CODE);
    let unused_source_indices = BTreeSet::from([
        154, 155, 156, 157, 158, 160, 161, 162, 163, 164, 165, 166, 167, 169, 170, 171, 172, 173,
        174,
    ]);
    let mad_com = catalog
        .glyphs
        .iter()
        .filter(|glyph| !unused_source_indices.contains(&glyph.index))
        .flat_map(|glyph| glyph.shift_jis_code.to_be_bytes())
        .collect::<Vec<_>>();

    let referenced = referenced_source_gaiji_codes(&mad_com, &catalog);
    let uninstalled = uninstalled_external_slots(&catalog).unwrap();
    let persistent_code = 0x777a;
    let persistent_bytes = crate::game_data::gaiji::jis_row_cell_to_shift_jis(persistent_code)
        .unwrap()
        .to_be_bytes();
    let blocked = referenced_slot_codes([persistent_bytes.as_slice()].into_iter(), &uninstalled);
    let available = uninstalled
        .iter()
        .copied()
        .filter(|slot| !blocked.contains(&slot.character_code))
        .collect::<Vec<_>>();

    assert_eq!(referenced.len(), 156);
    assert_eq!(uninstalled.len(), 13);
    assert_eq!(blocked, BTreeSet::from([persistent_code]));
    assert_eq!(available.len(), 12);
    assert!(available.iter().all(|slot| {
        !catalog
            .glyphs
            .iter()
            .any(|glyph| glyph.character_code == slot.character_code)
    }));
    assert!(available.iter().any(|slot| slot.character_code == 0x7621));
    assert!(available.iter().any(|slot| slot.character_code == 0x777e));
    assert!(
        !available
            .iter()
            .any(|slot| slot.character_code == persistent_code)
    );
}
