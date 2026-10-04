mod fixed_slots;
mod renderer;
mod stream;

pub(crate) use fixed_slots::{FixedTextSlot, parse_fixed_text_slots};
pub(crate) use renderer::{RendererEvidence, verify_text_renderer};
pub(crate) use stream::RendererTextToken;

use anyhow::Result;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct MadTextCatalog<'a> {
    pub renderer: RendererEvidence,
    pub fixed_slots: Vec<FixedTextSlot<'a>>,
}

pub(crate) fn parse_mad_text_catalog(bytes: &[u8]) -> Result<MadTextCatalog<'_>> {
    Ok(MadTextCatalog {
        renderer: verify_text_renderer(bytes)?,
        fixed_slots: parse_fixed_text_slots(bytes)?,
    })
}
