mod mad;
mod select;

use anyhow::Result;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ExpandedLargePortraitConsumers {
    pub select_com: Vec<u8>,
    pub mad_com: Vec<u8>,
    pub select_hook_file_offset: usize,
    pub select_hook_runtime_address: usize,
    pub select_hook_byte_size: usize,
    pub select_portrait_segment_byte_size: usize,
    pub mad_bank_allocation_file_offset: usize,
}

pub(super) fn expand_large_portrait_consumers(
    select_com: &[u8],
    mad_com: &[u8],
    packed_tile_bank_size: usize,
    packed_tile_map_size: usize,
) -> Result<ExpandedLargePortraitConsumers> {
    let select = select::install_expanded_portrait_bank(
        select_com,
        packed_tile_bank_size,
        packed_tile_map_size,
    )?;
    let mad = mad::expand_portrait_bank_allocation(mad_com)?;

    Ok(ExpandedLargePortraitConsumers {
        select_com: select.bytes,
        mad_com: mad,
        select_hook_file_offset: select.hook_file_offset,
        select_hook_runtime_address: select.hook_runtime_address,
        select_hook_byte_size: select.hook_byte_size,
        select_portrait_segment_byte_size: select.portrait_segment_byte_size,
        mad_bank_allocation_file_offset: mad::MAD_BANK_ALLOCATION_FILE_OFFSET,
    })
}
