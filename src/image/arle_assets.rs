use anyhow::Result;

use crate::build_inputs::ResolvedArleAssetSelection;
use crate::patch::{
    ArleBattleSpritesReport, ArleEndingMealReport, ArleLargePortraitReport, ArleOpeningPosesReport,
    ArleSmallStatusReport, compile_arle_battle_sprites, compile_arle_ending_meal,
    compile_arle_large_portrait, compile_arle_opening_poses, compile_arle_small_status,
};

#[derive(Debug, Eq, PartialEq)]
pub enum ArleAssetBuildReport {
    OriginalPreserved,
    Replaced(Box<ArleAssetReplacementReport>),
}

impl ArleAssetBuildReport {
    pub fn replacement(&self) -> Option<&ArleAssetReplacementReport> {
        match self {
            Self::OriginalPreserved => None,
            Self::Replaced(report) => Some(report),
        }
    }

    pub fn original_is_preserved(&self) -> bool {
        matches!(self, Self::OriginalPreserved)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ArleAssetReplacementReport {
    pub portrait: ArleLargePortraitReport,
    pub opening_poses: ArleOpeningPosesReport,
    pub small_status: ArleSmallStatusReport,
    pub battle_sprites: ArleBattleSpritesReport,
    pub ending_meal: ArleEndingMealReport,
}

pub(crate) struct ArleAssetFiles {
    pub maddat: Vec<u8>,
    pub select_com: Vec<u8>,
    pub mad_com: Vec<u8>,
    pub ending_com: Vec<u8>,
    pub report: ArleAssetBuildReport,
}

pub(crate) fn apply_arle_asset_selection(
    maddat: &[u8],
    select_com: &[u8],
    mad_com: &[u8],
    ending_com: &[u8],
    selection: &ResolvedArleAssetSelection,
) -> Result<ArleAssetFiles> {
    let ResolvedArleAssetSelection::Replace(assets) = selection else {
        return Ok(ArleAssetFiles {
            maddat: maddat.to_vec(),
            select_com: select_com.to_vec(),
            mad_com: mad_com.to_vec(),
            ending_com: ending_com.to_vec(),
            report: ArleAssetBuildReport::OriginalPreserved,
        });
    };

    let portrait =
        compile_arle_large_portrait(maddat, select_com, mad_com, &assets.large_portrait)?;
    let opening_poses = compile_arle_opening_poses(&portrait.maddat, &assets.opening_poses)?;
    let small_status = compile_arle_small_status(
        &opening_poses.maddat,
        &portrait.mad_com,
        &assets.small_status,
    )?;
    let battle_sprites = compile_arle_battle_sprites(&small_status.maddat, &assets.battle_sprites)?;
    let ending_meal =
        compile_arle_ending_meal(&battle_sprites.maddat, ending_com, &assets.ending_meal)?;

    Ok(ArleAssetFiles {
        maddat: ending_meal.maddat,
        select_com: portrait.select_com,
        mad_com: small_status.mad_com,
        ending_com: ending_meal.ending_com,
        report: ArleAssetBuildReport::Replaced(Box::new(ArleAssetReplacementReport {
            portrait: portrait.report,
            opening_poses: opening_poses.report,
            small_status: small_status.report,
            battle_sprites: battle_sprites.report,
            ending_meal: ending_meal.report,
        })),
    })
}

#[cfg(test)]
#[path = "arle_assets_tests.rs"]
mod arle_assets_tests;
