use super::*;

#[test]
fn unit_demands_share_banks_only_within_the_real_slot_limit() {
    let base = ('a'..='j').collect::<BTreeSet<_>>();
    let demands = vec![
        UnitDemand {
            unit_number: 2,
            characters: ['가', '나', '다'].into_iter().collect(),
        },
        UnitDemand {
            unit_number: 3,
            characters: ['다', '라'].into_iter().collect(),
        },
    ];

    let groups = pack_unit_demands(demands, &base, 4).unwrap();

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].unit_numbers, [2, 3]);
    assert_eq!(groups[0].characters.len(), 4);
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn composite_character_tiles_reconstruct_a_two_times_bitmap() {
    let source = rasterize_pc98_character('가').unwrap();
    let tiles = scale_character_tiles('가').unwrap();

    for source_y in 0..16 {
        for source_x in 0..16 {
            let source_set = source[source_y * 2 + source_x / 8] & (0x80 >> (source_x % 8)) != 0;
            for scaled_y in source_y * 2..source_y * 2 + 2 {
                for scaled_x in source_x * 2..source_x * 2 + 2 {
                    let tile = (scaled_y / 16) * 2 + scaled_x / 16;
                    let tile_x = scaled_x % 16;
                    let tile_y = scaled_y % 16;
                    let scaled_set =
                        tiles[tile][tile_y * 2 + tile_x / 8] & (0x80 >> (tile_x % 8)) != 0;
                    assert_eq!(scaled_set, source_set);
                }
            }
        }
    }
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn composite_banner_reports_two_tile_columns_per_text_cell() {
    let base_codebook = BTreeMap::from([(' ', 0)]);

    let banner = compile_result_banner("가 나", &base_codebook).unwrap();

    assert_eq!(banner.tile_columns, 6);
    assert_eq!(banner.top.len(), 7);
    assert_eq!(banner.bottom.len(), 7);
    assert_eq!(banner.top.last(), Some(&TERMINATOR_INDEX));
    assert_eq!(banner.bottom.last(), Some(&TERMINATOR_INDEX));
}
