use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

const LARGE_PORTRAIT_FILENAME: &str = "large-portrait.pc98.png";
const OPENING_POSES_FILENAME: &str = "opening-poses.pc98.png";
const SMALL_STATUS_FILENAME: &str = "small-status.pc98.png";
const BATTLE_SPRITES_FILENAME: &str = "battle-sprites.pc98.png";
const ENDING_MEAL_FILENAME: &str = "ending-meal.pc98.png";

pub const ARLE_ASSET_FILENAMES: [&str; 5] = [
    LARGE_PORTRAIT_FILENAME,
    OPENING_POSES_FILENAME,
    SMALL_STATUS_FILENAME,
    BATTLE_SPRITES_FILENAME,
    ENDING_MEAL_FILENAME,
];

const TRACKED_LARGE_PORTRAIT_SHA256: &str =
    "efdd0537c914a1c31a52c3e879c8b7c89efd1d3b6ecac0aceb2e29186723ecc4";
const TRACKED_OPENING_POSES_SHA256: &str =
    "ffd941b0d5bb7f7d3ddd475b2ff670473f6544591984c02126aa5d23293c8e04";
const TRACKED_SMALL_STATUS_SHA256: &str =
    "a6644d7868d6ad60488db7f6bbf62381d99f73dfb039398d5b80cf929b5d20b0";
const TRACKED_BATTLE_SPRITES_SHA256: &str =
    "abc6f393708efa54511b7a8cb1404979d576fce03c31f8e813475293be347cc7";
const TRACKED_ENDING_MEAL_SHA256: &str =
    "ab7dc91b5f0a8f3fc97ddc7171468ac82b237562fff52d9bf29170449888760e";

/// The tracked Arle set is not redistributed with this repository. It is read
/// from this directory when a build selects it and must match the hashes above.
const TRACKED_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/characters/arle");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArleAssetSet {
    source: ArleAssetSetSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArleAssetSelection {
    replacement: Option<ArleAssetSet>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ArleAssetSetSource {
    Tracked,
    Directory(PathBuf),
}

impl ArleAssetSet {
    pub fn tracked() -> Self {
        Self {
            source: ArleAssetSetSource::Tracked,
        }
    }

    pub fn from_directory(directory: impl Into<PathBuf>) -> Self {
        Self {
            source: ArleAssetSetSource::Directory(directory.into()),
        }
    }

    pub fn directory(&self) -> Option<&Path> {
        match &self.source {
            ArleAssetSetSource::Tracked => None,
            ArleAssetSetSource::Directory(directory) => Some(directory),
        }
    }

    pub(crate) fn resolve(&self) -> Result<ResolvedArleAssetSet> {
        match &self.source {
            ArleAssetSetSource::Tracked => Ok(ResolvedArleAssetSet {
                large_portrait: resolve_tracked_asset(
                    "large portrait",
                    LARGE_PORTRAIT_FILENAME,
                    TRACKED_LARGE_PORTRAIT_SHA256,
                )?,
                opening_poses: resolve_tracked_asset(
                    "opening poses",
                    OPENING_POSES_FILENAME,
                    TRACKED_OPENING_POSES_SHA256,
                )?,
                small_status: resolve_tracked_asset(
                    "small status portrait",
                    SMALL_STATUS_FILENAME,
                    TRACKED_SMALL_STATUS_SHA256,
                )?,
                battle_sprites: resolve_tracked_asset(
                    "battle sprites",
                    BATTLE_SPRITES_FILENAME,
                    TRACKED_BATTLE_SPRITES_SHA256,
                )?,
                ending_meal: resolve_tracked_asset(
                    "ending meal scenes",
                    ENDING_MEAL_FILENAME,
                    TRACKED_ENDING_MEAL_SHA256,
                )?,
            }),
            ArleAssetSetSource::Directory(directory) => {
                ensure!(
                    directory.is_dir(),
                    "Arle asset set path is not a directory: {}",
                    directory.display()
                );
                Ok(ResolvedArleAssetSet {
                    large_portrait: resolve_directory_asset(
                        directory,
                        "large portrait",
                        LARGE_PORTRAIT_FILENAME,
                    )?,
                    opening_poses: resolve_directory_asset(
                        directory,
                        "opening poses",
                        OPENING_POSES_FILENAME,
                    )?,
                    small_status: resolve_directory_asset(
                        directory,
                        "small status portrait",
                        SMALL_STATUS_FILENAME,
                    )?,
                    battle_sprites: resolve_directory_asset(
                        directory,
                        "battle sprites",
                        BATTLE_SPRITES_FILENAME,
                    )?,
                    ending_meal: resolve_directory_asset(
                        directory,
                        "ending meal scenes",
                        ENDING_MEAL_FILENAME,
                    )?,
                })
            }
        }
    }
}

impl ArleAssetSelection {
    pub fn preserve_original() -> Self {
        Self { replacement: None }
    }

    pub fn tracked() -> Self {
        Self::replace(ArleAssetSet::tracked())
    }

    pub fn from_directory(directory: impl Into<PathBuf>) -> Self {
        Self::replace(ArleAssetSet::from_directory(directory))
    }

    pub fn replace(asset_set: ArleAssetSet) -> Self {
        Self {
            replacement: Some(asset_set),
        }
    }

    pub fn replacement(&self) -> Option<&ArleAssetSet> {
        self.replacement.as_ref()
    }

    pub(crate) fn resolve(&self) -> Result<ResolvedArleAssetSelection> {
        self.replacement
            .as_ref()
            .map(ArleAssetSet::resolve)
            .transpose()
            .map(|replacement| match replacement {
                Some(asset_set) => ResolvedArleAssetSelection::Replace(Box::new(asset_set)),
                None => ResolvedArleAssetSelection::PreserveOriginal,
            })
    }
}

impl Default for ArleAssetSet {
    fn default() -> Self {
        Self::tracked()
    }
}

impl fmt::Display for ArleAssetSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            ArleAssetSetSource::Tracked => formatter.write_str("tracked assets/characters/arle"),
            ArleAssetSetSource::Directory(directory) => {
                write!(formatter, "{}", directory.display())
            }
        }
    }
}

impl fmt::Display for ArleAssetSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.replacement {
            Some(asset_set) => asset_set.fmt(formatter),
            None => formatter.write_str("original game assets (preserved)"),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ResolvedArleAssetSelection {
    PreserveOriginal,
    Replace(Box<ResolvedArleAssetSet>),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ResolvedArleAssetSet {
    pub large_portrait: ResolvedArleAsset,
    pub opening_poses: ResolvedArleAsset,
    pub small_status: ResolvedArleAsset,
    pub battle_sprites: ResolvedArleAsset,
    pub ending_meal: ResolvedArleAsset,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ResolvedArleAsset {
    bytes: Vec<u8>,
    sha256: String,
}

impl ResolvedArleAsset {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

fn resolve_tracked_asset(
    role: &str,
    filename: &str,
    expected_sha256: &str,
) -> Result<ResolvedArleAsset> {
    let path = Path::new(TRACKED_DIRECTORY).join(filename);
    let bytes = fs::read(&path).with_context(|| {
        format!(
            "tracked Arle {role} asset is unavailable: {}",
            path.display()
        )
    })?;
    let asset = resolved_asset(bytes);
    ensure!(
        asset.sha256 == expected_sha256,
        "tracked Arle {role} asset hash changed: expected {expected_sha256}, got {}",
        asset.sha256
    );
    Ok(asset)
}

fn resolve_directory_asset(
    directory: &Path,
    role: &str,
    filename: &str,
) -> Result<ResolvedArleAsset> {
    let path = directory.join(filename);
    let bytes = fs::read(&path).with_context(|| {
        format!(
            "read Arle {role} asset {} from the selected asset set",
            path.display()
        )
    })?;
    Ok(resolved_asset(bytes))
}

fn resolved_asset(bytes: Vec<u8>) -> ResolvedArleAsset {
    let sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    ResolvedArleAsset { bytes, sha256 }
}

#[cfg(test)]
#[path = "arle_asset_set_tests.rs"]
mod arle_asset_set_tests;
