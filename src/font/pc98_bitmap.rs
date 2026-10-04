use std::collections::BTreeSet;
use std::sync::OnceLock;

use anyhow::{Context, Result, bail, ensure};
use fontdue::{Font, FontSettings};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const GLYPH_WIDTH: usize = 16;
const GLYPH_HEIGHT: usize = 16;
const GLYPH_BYTES: usize = 32;
pub(crate) const ENDING_CREDIT_GLYPH_WIDTH: usize = 24;
pub(crate) const ENDING_CREDIT_GLYPH_HEIGHT: usize = 32;
pub(crate) const ENDING_CREDIT_GLYPH_BYTES: usize = 96;
const PROFILE_SCHEMA: &str = "ds8_daimadou.font_profile";
/// The font and its license are not redistributed with this repository; they
/// are read from this directory and checked against the profile.
const FONT_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts");
const PROFILE_JSON: &str = include_str!("../../assets/fonts/neodunggeunmo-pc98-16x16.json");
const ENDING_CREDIT_PROFILE_JSON: &str =
    include_str!("../../assets/fonts/neodunggeunmo-ending-credit-24x32.json");

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct FontProvenance {
    pub profile_id: String,
    pub font_name: String,
    pub font_sha256: String,
    pub font_version: String,
    pub source: String,
    pub upstream_revision: String,
    pub license: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct FontRepertoireReport {
    pub provenance: FontProvenance,
    pub verified_character_count: usize,
    pub verified_hangul_syllable_count: usize,
}

#[derive(Debug, Deserialize)]
struct FontProfile {
    schema: String,
    id: String,
    font: String,
    font_sha256: String,
    font_version: String,
    cell_width: u16,
    cell_height: u16,
    font_size: u16,
    baseline_y: u8,
    vertical_fit: String,
    threshold: u8,
    horizontal_thickening: u8,
    rasterizer: String,
    license: String,
    source: String,
    upstream_revision: String,
}

struct EmbeddedFont {
    font: Font,
    profile: FontProfile,
}

static EMBEDDED_FONT: OnceLock<Result<EmbeddedFont, String>> = OnceLock::new();
static ENDING_CREDIT_FONT: OnceLock<Result<EmbeddedFont, String>> = OnceLock::new();

pub(crate) fn rasterize_hangul_syllable(character: char) -> Result<[u8; GLYPH_BYTES]> {
    ensure!(
        is_modern_hangul_syllable(character),
        "font input {character:?} is not a modern Hangul syllable"
    );
    rasterize_pc98_character(character)
}

pub(crate) fn rasterize_pc98_character(character: char) -> Result<[u8; GLYPH_BYTES]> {
    if character == ' ' {
        return Ok([0; GLYPH_BYTES]);
    }
    embedded_font()?.rasterize::<GLYPH_WIDTH, GLYPH_HEIGHT, GLYPH_BYTES>(character)
}

pub(crate) fn rasterize_ending_credit_character(
    character: char,
) -> Result<[u8; ENDING_CREDIT_GLYPH_BYTES]> {
    ensure!(
        is_modern_hangul_syllable(character),
        "ending-credit font input {character:?} is not a modern Hangul syllable"
    );
    ending_credit_font()?.rasterize::<
        ENDING_CREDIT_GLYPH_WIDTH,
        ENDING_CREDIT_GLYPH_HEIGHT,
        ENDING_CREDIT_GLYPH_BYTES,
    >(character)
}

pub(crate) fn verify_hangul_repertoire(
    characters: impl IntoIterator<Item = char>,
) -> Result<FontRepertoireReport> {
    let characters: BTreeSet<_> = characters.into_iter().collect();
    ensure!(
        characters
            .iter()
            .all(|character| is_modern_hangul_syllable(*character)),
        "Hangul repertoire contains a non-Hangul character"
    );
    verify_pc98_repertoire(characters)
}

pub(crate) fn verify_pc98_repertoire(
    characters: impl IntoIterator<Item = char>,
) -> Result<FontRepertoireReport> {
    let font = embedded_font()?;
    let characters: BTreeSet<_> = characters.into_iter().collect();
    for character in &characters {
        if *character != ' ' {
            font.rasterize::<GLYPH_WIDTH, GLYPH_HEIGHT, GLYPH_BYTES>(*character)?;
        }
    }
    Ok(FontRepertoireReport {
        provenance: font.provenance(),
        verified_character_count: characters.len(),
        verified_hangul_syllable_count: characters
            .iter()
            .filter(|character| is_modern_hangul_syllable(**character))
            .count(),
    })
}

pub(crate) fn verify_ending_credit_repertoire(
    characters: impl IntoIterator<Item = char>,
) -> Result<FontRepertoireReport> {
    let font = ending_credit_font()?;
    let characters: BTreeSet<_> = characters.into_iter().collect();
    ensure!(
        characters
            .iter()
            .all(|character| *character == ' ' || is_modern_hangul_syllable(*character)),
        "ending-credit repertoire contains a non-Hangul visible character"
    );
    for character in &characters {
        if *character != ' ' {
            font.rasterize::<
                ENDING_CREDIT_GLYPH_WIDTH,
                ENDING_CREDIT_GLYPH_HEIGHT,
                ENDING_CREDIT_GLYPH_BYTES,
            >(*character)?;
        }
    }
    Ok(FontRepertoireReport {
        provenance: font.provenance(),
        verified_character_count: characters.len(),
        verified_hangul_syllable_count: characters
            .iter()
            .filter(|character| is_modern_hangul_syllable(**character))
            .count(),
    })
}

fn embedded_font() -> Result<&'static EmbeddedFont> {
    match EMBEDDED_FONT.get_or_init(|| {
        EmbeddedFont::load(PROFILE_JSON, GLYPH_WIDTH, GLYPH_HEIGHT)
            .map_err(|error| format!("{error:#}"))
    }) {
        Ok(font) => Ok(font),
        Err(error) => bail!("load embedded font: {error}"),
    }
}

fn ending_credit_font() -> Result<&'static EmbeddedFont> {
    match ENDING_CREDIT_FONT.get_or_init(|| {
        EmbeddedFont::load(
            ENDING_CREDIT_PROFILE_JSON,
            ENDING_CREDIT_GLYPH_WIDTH,
            ENDING_CREDIT_GLYPH_HEIGHT,
        )
        .map_err(|error| format!("{error:#}"))
    }) {
        Ok(font) => Ok(font),
        Err(error) => bail!("load ending-credit embedded font: {error}"),
    }
}

impl EmbeddedFont {
    fn load(profile_json: &str, cell_width: usize, cell_height: usize) -> Result<Self> {
        let profile: FontProfile =
            serde_json::from_str(profile_json).context("parse embedded font profile")?;
        let font_bytes = read_font_input(&profile.font)?;
        let font_license = String::from_utf8(read_font_input(&profile.license)?)
            .context("font license is not UTF-8")?;
        verify_profile(
            &profile,
            cell_width,
            cell_height,
            &font_bytes,
            &font_license,
        )?;
        let font = Font::from_bytes(font_bytes.as_slice(), FontSettings::default())
            .map_err(|error| anyhow::anyhow!("parse embedded font: {error}"))?;
        Ok(Self { font, profile })
    }

    fn provenance(&self) -> FontProvenance {
        FontProvenance {
            profile_id: self.profile.id.clone(),
            font_name: self.profile.font.clone(),
            font_sha256: self.profile.font_sha256.clone(),
            font_version: self.profile.font_version.clone(),
            source: self.profile.source.clone(),
            upstream_revision: self.profile.upstream_revision.clone(),
            license: self.profile.license.clone(),
        }
    }

    fn rasterize<const WIDTH: usize, const HEIGHT: usize, const BYTE_SIZE: usize>(
        &self,
        character: char,
    ) -> Result<[u8; BYTE_SIZE]> {
        ensure!(
            WIDTH.is_multiple_of(8) && BYTE_SIZE == WIDTH / 8 * HEIGHT,
            "font target {WIDTH}x{HEIGHT} has an invalid 1bpp byte size {BYTE_SIZE}"
        );
        ensure!(
            usize::from(self.profile.cell_width) == WIDTH
                && usize::from(self.profile.cell_height) == HEIGHT,
            "font profile {} targets {}x{} instead of {WIDTH}x{HEIGHT}",
            self.profile.id,
            self.profile.cell_width,
            self.profile.cell_height
        );
        ensure!(
            self.font.has_glyph(character),
            "embedded font has no glyph for {character:?} (U+{:04X})",
            character as u32
        );

        let (metrics, coverage) = self
            .font
            .rasterize(character, f32::from(self.profile.font_size));
        if metrics.width == 0 || metrics.height == 0 || coverage.is_empty() {
            bail!(
                "embedded font rendered no pixels for {character:?} (U+{:04X})",
                character as u32
            );
        }
        ensure!(
            metrics.width <= WIDTH && metrics.height <= HEIGHT,
            "glyph {character:?} is {}x{} and does not fit {WIDTH}x{HEIGHT}",
            metrics.width,
            metrics.height
        );

        let left = (WIDTH - metrics.width) / 2;
        let baseline_top =
            i32::from(self.profile.baseline_y) - (metrics.ymin + metrics.height as i32);
        let maximum_top = (HEIGHT - metrics.height) as i32;
        let top = baseline_top.clamp(0, maximum_top);
        let bottom = top + metrics.height as i32;
        ensure!(
            top >= 0 && bottom <= HEIGHT as i32,
            "glyph {character:?} target rows {top}..{bottom} do not fit after {:?}",
            self.profile.vertical_fit,
        );

        let mut bitmap = [0_u8; BYTE_SIZE];
        for source_y in 0..metrics.height {
            for source_x in 0..metrics.width {
                if coverage[source_y * metrics.width + source_x] < self.profile.threshold {
                    continue;
                }
                let x = left + source_x;
                let y = top as usize + source_y;
                for extra_x in 0..=usize::from(self.profile.horizontal_thickening) {
                    let thickened_x = x + extra_x;
                    if thickened_x < WIDTH {
                        bitmap[y * (WIDTH / 8) + thickened_x / 8] |= 1 << (7 - (thickened_x % 8));
                    }
                }
            }
        }
        ensure!(
            bitmap.iter().any(|byte| *byte != 0),
            "embedded font rendered an empty 1bpp glyph for {character:?}"
        );
        Ok(bitmap)
    }
}

fn read_font_input(filename: &str) -> Result<Vec<u8>> {
    let path = std::path::Path::new(FONT_DIRECTORY).join(filename);
    std::fs::read(&path).with_context(|| format!("font input is unavailable: {}", path.display()))
}

fn verify_profile(
    profile: &FontProfile,
    cell_width: usize,
    cell_height: usize,
    font_bytes: &[u8],
    font_license: &str,
) -> Result<()> {
    ensure!(
        profile.schema == PROFILE_SCHEMA,
        "unsupported embedded font profile schema {:?}",
        profile.schema
    );
    ensure!(
        profile.font == "NeoDunggeunmo.ttf",
        "embedded font profile names an unexpected font"
    );
    ensure!(
        profile.license == "NeoDunggeunmo-OFL.txt"
            && font_license.contains("SIL OPEN FONT LICENSE"),
        "embedded font profile has no matching OFL license"
    );
    ensure!(
        profile.rasterizer == "fontdue 0.9.3",
        "embedded font profile names an unexpected rasterizer"
    );
    ensure!(
        profile.vertical_fit == "baseline_then_clamp",
        "embedded font profile names an unexpected vertical-fit policy"
    );
    let actual_sha256 = Sha256::digest(font_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    ensure!(
        actual_sha256 == profile.font_sha256,
        "embedded font SHA-256 {actual_sha256} does not match profile {}",
        profile.font_sha256
    );
    ensure!(
        usize::from(profile.cell_width) == cell_width
            && usize::from(profile.cell_height) == cell_height,
        "embedded font profile {} declares {}x{} instead of {cell_width}x{cell_height}",
        profile.id,
        profile.cell_width,
        profile.cell_height
    );
    ensure!(
        (1..=profile.cell_height).contains(&profile.font_size),
        "embedded font size is outside its declared cell height"
    );
    ensure!(
        u16::from(profile.baseline_y) <= profile.cell_height,
        "embedded font baseline is outside the glyph cell"
    );
    ensure!(
        profile.threshold > 0,
        "embedded font threshold must be nonzero"
    );
    ensure!(
        profile.horizontal_thickening <= 2,
        "embedded font horizontal thickening exceeds two pixels"
    );
    Ok(())
}

fn is_modern_hangul_syllable(character: char) -> bool {
    matches!(character as u32, 0xAC00..=0xD7A3)
}

#[cfg(test)]
#[path = "pc98_bitmap_tests.rs"]
mod pc98_bitmap_tests;
