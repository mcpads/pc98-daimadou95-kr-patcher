use crate::game_data::compile_lz::encode_compile_lz;
use crate::patch::field_unit_callout_compositor::{INTERIOR_PALETTE_INDEX, OUTLINE_PALETTE_INDEX};

use super::*;

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn replacement_changes_only_the_declared_consumer_tiles() {
    let mut source = vec![0_u8; 128 * BYTES_PER_TILE];
    for tile_id in 0..128 {
        source[tile_id * BYTES_PER_TILE..(tile_id + 1) * BYTES_PER_TILE].fill(tile_id as u8);
    }
    let composition = compose_field_unit_callout("모올~!").unwrap();
    let tile_ids = [125, 126, 127];
    let planned_tiles = tile_ids
        .into_iter()
        .enumerate()
        .map(|(column, tile_id)| (tile_id, composition_tile(&composition, column).unwrap()))
        .collect::<BTreeMap<_, _>>();

    let replaced = replace_planned_callout_tiles(&source, &planned_tiles).unwrap();

    for tile_id in 0..128 {
        let range = tile_id * BYTES_PER_TILE..(tile_id + 1) * BYTES_PER_TILE;
        if tile_ids.contains(&tile_id) {
            assert_ne!(replaced[range.clone()], source[range]);
        } else {
            assert_eq!(replaced[range.clone()], source[range]);
        }
    }
    let packed = encode_compile_lz(&replaced).unwrap();
    let decoded = decode_masked_sprite_bank(&packed).unwrap();
    for (tile_id, planned) in planned_tiles {
        assert_eq!(decoded.tiles[tile_id], planned);
    }
}

#[test]
fn changed_pixel_total_counts_a_shared_planned_tile_only_once() {
    let source = MaskedSpriteBank {
        packed_size: 0,
        command_count: 0,
        decoded: vec![0; 128 * BYTES_PER_TILE],
        tiles: (0..128)
            .map(|_| MaskedSpriteTile {
                opaque: vec![false; TILE_WIDTH * TILE_HEIGHT],
                palette_indices: vec![0; TILE_WIDTH * TILE_HEIGHT],
            })
            .collect(),
    };
    let mut planned = MaskedSpriteTile {
        opaque: vec![false; TILE_WIDTH * TILE_HEIGHT],
        palette_indices: vec![0; TILE_WIDTH * TILE_HEIGHT],
    };
    planned.opaque[0] = true;
    planned.palette_indices[0] = OUTLINE_PALETTE_INDEX;
    let planned_tiles = BTreeMap::from([(96, planned)]);

    assert_eq!(
        count_changed_planned_pixels(&source, &planned_tiles).unwrap(),
        1
    );
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn every_adopted_callout_uses_only_transparent_white_and_blue_roles() {
    for text in [
        "뿌요!",
        "그니까",
        "메롱!",
        "뻐끔~",
        "하오!",
        "후후후",
        "후우",
        "차아~!",
        "가오~!",
        "가~지!",
        "오잇스!",
        "피~쉬!",
        "모올~!",
        "쿠웅!",
        "이얏!",
        "얍~!",
    ] {
        let composition = compose_field_unit_callout(text).unwrap();
        let opaque = composition
            .opaque
            .iter()
            .zip(&composition.palette_indices)
            .filter_map(|(opaque, palette_index)| opaque.then_some(*palette_index))
            .collect::<BTreeSet<_>>();
        let transparent = composition
            .opaque
            .iter()
            .zip(&composition.palette_indices)
            .filter_map(|(opaque, palette_index)| (!opaque).then_some(*palette_index))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            opaque,
            [OUTLINE_PALETTE_INDEX, INTERIOR_PALETTE_INDEX]
                .into_iter()
                .collect(),
            "{text}"
        );
        assert_eq!(
            transparent,
            [TRANSPARENT_PALETTE_INDEX].into_iter().collect(),
            "{text}"
        );
    }
}
