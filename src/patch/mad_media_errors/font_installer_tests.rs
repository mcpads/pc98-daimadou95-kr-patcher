use super::*;
use crate::patch::mad_media_errors::bank_plan::GaijiSlot;

fn codebook(character_codes: &[u16]) -> GaijiCodebook {
    character_codes
        .iter()
        .copied()
        .enumerate()
        .map(|(position, character_code)| {
            let character = char::from_u32(0xac00 + position as u32).unwrap();
            (
                character,
                GaijiSlot {
                    character_code,
                    shift_jis_code: crate::game_data::gaiji::jis_row_cell_to_shift_jis(
                        character_code,
                    )
                    .unwrap(),
                },
            )
        })
        .collect()
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn mad_entry_installs_its_bank_within_the_remaining_com_space() {
    let original_entry = assemble_original_entry().unwrap();
    let mut bytes = vec![0_u8; 64_325];
    bytes[..ENTRY_WIDTH].copy_from_slice(original_entry.bytes());
    let codebook = codebook(&(0x7622..0x763a).collect::<Vec<_>>());

    let patched = install_mad_error_font_at_entry(&bytes, &codebook).unwrap();

    assert_eq!(patched.hook_file_offset, bytes.len());
    assert_eq!(patched.bytes[0], 0xe9);
    assert_eq!(
        patched.bytes.len(),
        bytes.len()
            + patched.hook_byte_size
            + codebook.len() * 2
            + codebook.len() * GLYPH_RECORD_SIZE
    );
    assert!(patched.bytes.len() + COM_ORIGIN <= usize::from(u16::MAX) + 1);
    assert_eq!(
        &patched.bytes[patched.bytes.len() - GLYPH_RECORD_SIZE..][..2],
        [0, 0]
    );
}

#[test]
#[ignore = "requires NeoDunggeunmo.ttf and NeoDunggeunmo-OFL.txt in assets/fonts/"]
fn mad_installer_keeps_gapped_external_character_codes() {
    let original_entry = assemble_original_entry().unwrap();
    let mut bytes = vec![0_u8; 64_325];
    bytes[..ENTRY_WIDTH].copy_from_slice(original_entry.bytes());

    let codebook = codebook(&[0x7621, 0x775e]);
    let patched = install_mad_error_font_at_entry(&bytes, &codebook).unwrap();
    let slot_codes_offset = bytes.len() + patched.hook_byte_size;

    assert_eq!(
        &patched.bytes[slot_codes_offset..slot_codes_offset + 4],
        [0x21, 0x76, 0x5e, 0x77]
    );
}
