use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use ds8_daimadou_builder::{
    ArleAssetBuildReport, ArleAssetSelection, ArleAssetSet, TranslationReviewOutcome,
    audit_translation_glyphs, audit_translation_layout,
    build_arle_large_portrait_image_with_arle_assets,
    build_full_translation_image_with_arle_asset_selection, build_game_over_translation_image,
    build_hangul_probe_image, build_indexed_translation_image,
    build_ingame_translation_image_with_arle_asset_selection, build_opening_translation_image,
    build_standalone_image, build_title_translation_image, default_arle_large_portrait_output_path,
    default_full_translation_output_path, default_game_over_translation_output_path,
    default_hangul_probe_output_path, default_indexed_translation_output_path,
    default_ingame_translation_output_path, default_opening_translation_output_path,
    default_output_path, default_title_translation_output_path, prepare_translation_drafts,
    prepare_translation_workspace, record_translation_segment_review, report_translation_review,
    survey_source_path, validate_translation_drafts, validate_translation_workspace,
    verify_source_path,
};

#[derive(Debug, Parser)]
#[command(
    name = "ds8-daimadou-builder",
    about = "Build Daimadou Senryaku Monogatari '95 from Disc Station Vol. 08 Disk 1"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Verify that a disk is the supported Disc Station Vol. 08 Disk 1 image.
    VerifySource {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
    },
    /// Print the verified source structure, localization catalog, and strategy as JSON.
    SurveySource {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
    },
    /// Create a protected, semantically segmented translation workspace.
    PrepareTranslations {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        output_dir: PathBuf,
    },
    /// Validate translation edits against the current protected source baseline.
    ValidateTranslations {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        workspace: PathBuf,
    },
    /// Create a source-free, versionable Korean translation draft overlay.
    PrepareTranslationDrafts {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        output_dir: PathBuf,
    },
    /// Validate Korean draft overlays against the current supported source.
    ValidateTranslationDrafts {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(long)]
        require_human_review_ready: bool,
    },
    /// Record an independent review of one tracked translation segment.
    RecordTranslationReview {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(long, value_name = "SEMANTIC_ID")]
        segment: String,
        #[arg(long, value_enum)]
        outcome: ReviewOutcomeArgument,
        #[arg(long = "note", required = true, value_name = "EVIDENCE")]
        notes: Vec<String>,
    },
    /// Audit reviewed Korean glyph demand against each rendering path's physical capacity.
    AuditTranslationGlyphs {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Audit reviewed Korean text against verified screen and token layout boundaries.
    AuditTranslationLayout {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Report the protected source, Korean draft, context, and decisions for human review.
    ReportTranslationReview {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(long)]
        human_decisions_only: bool,
        #[arg(long)]
        json: bool,
    },
    /// Build a standalone game disk without modifying the supplied source image.
    Build {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "OUTPUT.HDM", default_value_os_t = default_output_path())]
        output: PathBuf,
    },
    /// Build the tracked four-expression Arle large portrait into MADDAT 101 and 129.
    BuildArleLargePortrait {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        /// Load one complete five-file Arle asset set from this directory.
        #[arg(long, value_name = "DIRECTORY")]
        arle_assets: Option<PathBuf>,
        #[arg(
            long,
            value_name = "OUTPUT.HDM",
            default_value_os_t = default_arle_large_portrait_output_path()
        )]
        output: PathBuf,
    },
    /// Build a one-glyph Hangul consumer-path probe through checked data writes.
    BuildHangulProbe {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(
            long,
            value_name = "OUTPUT.HDM",
            default_value_os_t = default_hangul_probe_output_path()
        )]
        output: PathBuf,
    },
    /// Build the reviewed Korean opening with an isolated font bank.
    BuildOpeningTranslation {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(
            long,
            value_name = "OUTPUT.HDM",
            default_value_os_t = default_opening_translation_output_path()
        )]
        output: PathBuf,
    },
    /// Build every implemented indexed-text lifecycle with isolated font banks.
    BuildIndexedTranslation {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(
            long,
            value_name = "OUTPUT.HDM",
            default_value_os_t = default_indexed_translation_output_path()
        )]
        output: PathBuf,
    },
    /// Build every implemented in-game translation in one development image.
    BuildIngameTranslation {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        /// Load one complete five-file Arle asset set from this directory.
        #[arg(long, value_name = "DIRECTORY")]
        arle_assets: Option<PathBuf>,
        /// Skip all custom Arle asset producers and preserve the original game graphics.
        #[arg(long, conflicts_with = "arle_assets")]
        preserve_original_arle: bool,
        #[arg(
            long,
            value_name = "OUTPUT.HDM",
            default_value_os_t = default_ingame_translation_output_path()
        )]
        output: PathBuf,
    },
    /// Build every gameplay translation into one development image.
    BuildFullTranslation {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        /// Load one complete five-file Arle asset set from this directory.
        #[arg(long, value_name = "DIRECTORY")]
        arle_assets: Option<PathBuf>,
        /// Skip all custom Arle asset producers and preserve the original game graphics.
        #[arg(long, conflicts_with = "arle_assets")]
        preserve_original_arle: bool,
        #[arg(
            long,
            value_name = "OUTPUT.HDM",
            default_value_os_t = default_full_translation_output_path()
        )]
        output: PathBuf,
    },
    /// Build the reviewed Korean title composition from the verified GCS screen.
    BuildTitleTranslation {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(
            long,
            value_name = "OUTPUT.HDM",
            default_value_os_t = default_title_translation_output_path()
        )]
        output: PathBuf,
    },
    /// Build the reviewed Korean game-over composition from its planar block.
    BuildGameOverTranslation {
        #[arg(long, value_name = "DISK1.HDM")]
        source: PathBuf,
        #[arg(long, value_name = "DIRECTORY")]
        drafts: PathBuf,
        #[arg(
            long,
            value_name = "OUTPUT.HDM",
            default_value_os_t = default_game_over_translation_output_path()
        )]
        output: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ReviewOutcomeArgument {
    ReadyForHumanReview,
    HumanDecisionRequired,
}

impl From<ReviewOutcomeArgument> for TranslationReviewOutcome {
    fn from(value: ReviewOutcomeArgument) -> Self {
        match value {
            ReviewOutcomeArgument::ReadyForHumanReview => Self::ReadyForHumanReview,
            ReviewOutcomeArgument::HumanDecisionRequired => Self::HumanDecisionRequired,
        }
    }
}

fn selected_arle_assets(directory: Option<PathBuf>) -> ArleAssetSet {
    directory
        .map(ArleAssetSet::from_directory)
        .unwrap_or_default()
}

fn select_arle_asset_build_input(
    directory: Option<PathBuf>,
    preserve_original: bool,
) -> ArleAssetSelection {
    if preserve_original {
        ArleAssetSelection::preserve_original()
    } else {
        directory
            .map(ArleAssetSelection::from_directory)
            .unwrap_or_else(ArleAssetSelection::tracked)
    }
}

fn print_arle_asset_report(report: &ArleAssetBuildReport) {
    let Some(report) = report.replacement() else {
        println!("Arle assets: original game graphics preserved; no custom assets inserted");
        return;
    };

    println!(
        "Arle portrait: entries {} + {}, {} frames, {} -> {} used tiles, asset {}",
        report.portrait.tile_bank_entry_id,
        report.portrait.tile_map_entry_id,
        report.portrait.frame_count,
        report.portrait.source_used_tile_count,
        report.portrait.output_used_tile_count,
        report.portrait.asset_sha256
    );
    println!(
        "Arle opening poses: entry {}, {} poses, {} -> {} packed bytes, {} changed pixels, asset {}",
        report.opening_poses.maddat_entry_id,
        report.opening_poses.pose_count,
        report.opening_poses.packed_input_size,
        report.opening_poses.packed_output_size,
        report.opening_poses.changed_pixel_count,
        report.opening_poses.asset_sha256
    );
    println!(
        "Arle small status: entry {}, maps {:#x?}, {} frames, {} -> {} used tiles, {} changed pixels, asset {}",
        report.small_status.tile_bank_entry_id,
        report.small_status.map_file_offsets,
        report.small_status.frame_count,
        report.small_status.source_used_tile_count,
        report.small_status.output_used_tile_count,
        report.small_status.changed_pixel_count,
        report.small_status.asset_sha256
    );
    println!(
        "Arle battle sprites: map {}, banks {:?}, {} frames, {:?} -> {:?} used tiles, {} changed pixels, asset {}",
        report.battle_sprites.tile_map_entry_id,
        report.battle_sprites.tile_bank_entry_ids,
        report.battle_sprites.frame_count,
        report.battle_sprites.source_used_tile_counts,
        report.battle_sprites.output_used_tile_counts,
        report.battle_sprites.changed_pixel_count,
        report.battle_sprites.asset_sha256
    );
    println!(
        "Arle ending meal: entry {}, four states in banks {:?} (tile capacity {}, used {:?}), packed {} / {} staging bytes, {} changed pixels, asset {}",
        report.ending_meal.maddat_entry_id,
        report.ending_meal.state_bank_indices,
        report.ending_meal.output_bank_tile_capacity,
        report.ending_meal.output_used_tile_counts,
        report.ending_meal.packed_output_size,
        report.ending_meal.packed_staging_capacity,
        report.ending_meal.changed_pixel_count,
        report.ending_meal.asset_sha256
    );
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::VerifySource { source } => {
            let verification = verify_source_path(&source)?;
            println!("supported source: {}", source.display());
            println!("size: {} bytes", verification.size);
            println!("sha256: {}", verification.sha256);
        }
        Command::SurveySource { source } => {
            let report = survey_source_path(&source)?;
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            serde_json::to_writer_pretty(&mut output, &report)?;
            writeln!(output)?;
        }
        Command::PrepareTranslations { source, output_dir } => {
            let report = prepare_translation_workspace(&source, &output_dir)?;
            println!("prepared: {}", report.output_directory.display());
            println!("source sha256: {}", report.source_sha256);
            println!("translation targets: {}", report.target_item_count);
            println!("semantic segments: {}", report.segment_count);
        }
        Command::ValidateTranslations { source, workspace } => {
            let report = validate_translation_workspace(&source, &workspace)?;
            println!("source sha256: {}", report.source_sha256);
            println!("translation targets: {}", report.target_item_count);
            println!("semantic segments: {}", report.segment_count);
            println!("untranslated: {}", report.untranslated_item_count);
            println!("in progress: {}", report.in_progress_item_count);
            println!("needs review: {}", report.needs_review_item_count);
            println!(
                "needs human review: {}",
                report.needs_human_review_item_count
            );
        }
        Command::PrepareTranslationDrafts { source, output_dir } => {
            let report = prepare_translation_drafts(&source, &output_dir)?;
            println!("prepared: {}", report.output_directory.display());
            println!("source sha256: {}", report.source_sha256);
            println!("translation targets: {}", report.target_item_count);
            println!("semantic segments: {}", report.segment_count);
        }
        Command::ValidateTranslationDrafts {
            source,
            drafts,
            require_human_review_ready,
        } => {
            let report = validate_translation_drafts(&source, &drafts, require_human_review_ready)?;
            println!("source sha256: {}", report.source_sha256);
            println!("translation targets: {}", report.target_item_count);
            println!("semantic segments: {}", report.segment_count);
            println!("untranslated: {}", report.untranslated_item_count);
            println!("in progress: {}", report.in_progress_item_count);
            println!("needs review: {}", report.needs_review_item_count);
            println!(
                "needs human review: {}",
                report.needs_human_review_item_count
            );
            println!("reviewed segments: {}", report.reviewed_segment_count);
            println!("human review ready: {}", report.human_review_ready);
        }
        Command::RecordTranslationReview {
            source,
            drafts,
            segment,
            outcome,
            notes,
        } => {
            let report = record_translation_segment_review(
                &source,
                &drafts,
                &segment,
                outcome.into(),
                notes,
            )?;
            println!("reviewed segment: {}", report.segment_id);
            println!("reviewed entries: {}", report.entry_count);
            println!("reviewed draft sha256: {}", report.reviewed_draft_sha256);
            println!("outcome: {:?}", report.outcome);
        }
        Command::AuditTranslationGlyphs {
            source,
            drafts,
            json,
        } => {
            let report = audit_translation_glyphs(&source, &drafts)?;
            if json {
                let stdout = std::io::stdout();
                let mut output = stdout.lock();
                serde_json::to_writer_pretty(&mut output, &report)?;
                writeln!(output)?;
            } else {
                println!("source sha256: {}", report.source_sha256);
                println!("translation targets: {}", report.target_item_count);
                println!("semantic segments: {}", report.segment_count);
                println!("human review ready: {}", report.human_review_ready);
                println!(
                    "font: {} {} sha256 {}, {} characters ({} Hangul syllables) verified",
                    report.font.provenance.font_name,
                    report.font.provenance.font_version,
                    report.font.provenance.font_sha256,
                    report.font.verified_character_count,
                    report.font.verified_hangul_syllable_count
                );
                for render_path in &report.render_paths {
                    let demand = &render_path.demand;
                    println!(
                        "{:?}: {} rendered characters, {} Hangul, slot demand {:?}, capacity {:?}, headroom {:?}",
                        render_path.render_path,
                        demand.unique_rendered_character_count,
                        demand.unique_hangul_syllable_count,
                        demand.slot_glyph_count,
                        demand.verified_physical_capacity,
                        demand.physical_capacity_headroom
                    );
                }
            }
        }
        Command::AuditTranslationLayout {
            source,
            drafts,
            json,
        } => {
            let report = audit_translation_layout(&source, &drafts)?;
            if json {
                let stdout = std::io::stdout();
                let mut output = stdout.lock();
                serde_json::to_writer_pretty(&mut output, &report)?;
                writeln!(output)?;
            } else {
                println!("source sha256: {}", report.source_sha256);
                println!("translation targets: {}", report.target_item_count);
                println!("semantic segments: {}", report.segment_count);
                println!("human review ready: {}", report.human_review_ready);
                println!("measured layout units: {}", report.measured_unit_count);
                println!("hard-limit units: {}", report.hard_limit_unit_count);
                println!("opening glyph tokens: {}", report.glyph_token_count);
                println!("hard layout violations: {}", report.hard_violation_count);
                println!(
                    "source-growth advisories: {}",
                    report.source_growth_advisory_count
                );
                println!(
                    "verified hard limits fit: {}",
                    report.verified_hard_limits_fit
                );
                println!("open consumer gates: {}", report.open_gates.len());
                for family in &report.families {
                    println!(
                        "{:?}: {} units, max source {}, max Korean {}, hard violations {}, source growth {}",
                        family.source_catalog,
                        family.measured_unit_count,
                        family.max_source_cells,
                        family.max_korean_cells,
                        family.hard_violation_count,
                        family.source_growth_advisory_count
                    );
                }
            }
        }
        Command::ReportTranslationReview {
            source,
            drafts,
            human_decisions_only,
            json,
        } => {
            let mut report = report_translation_review(&source, &drafts)?;
            if human_decisions_only {
                report.segments.retain(|segment| {
                    segment.outcome == TranslationReviewOutcome::HumanDecisionRequired
                });
            }
            if json {
                let stdout = std::io::stdout();
                let mut output = stdout.lock();
                serde_json::to_writer_pretty(&mut output, &report)?;
                writeln!(output)?;
            } else {
                println!("source sha256: {}", report.source_sha256);
                println!("translation targets: {}", report.target_item_count);
                println!("semantic segments: {}", report.segment_count);
                println!("human review ready: {}", report.human_review_ready);
                println!(
                    "human decisions: {} segments, {} entries",
                    report.human_decision_segment_count, report.human_decision_entry_count
                );
                println!(
                    "ready without an identified decision: {} segments, {} entries",
                    report.ready_segment_count, report.ready_entry_count
                );
                for segment in &report.segments {
                    println!(
                        "{:?}: {} ({} entries)",
                        segment.outcome,
                        segment.id,
                        segment.entries.len()
                    );
                    for note in &segment.notes {
                        println!("  - {note}");
                    }
                }
            }
        }
        Command::Build { source, output } => {
            let report = build_standalone_image(&source, &output)?;
            println!("built: {}", output.display());
            println!("source sha256: {}", report.source_sha256);
            println!("output sha256: {}", report.output_sha256);
            println!("output size: {} bytes", report.output_size);
            println!("verified files: {}", report.file_count);
        }
        Command::BuildArleLargePortrait {
            source,
            arle_assets,
            output,
        } => {
            let arle_assets = selected_arle_assets(arle_assets);
            let report =
                build_arle_large_portrait_image_with_arle_assets(&source, &output, &arle_assets)?;
            println!("built: {}", output.display());
            println!("Arle asset set: {arle_assets}");
            println!("source sha256: {}", report.image.source_sha256);
            println!("output sha256: {}", report.image.output_sha256);
            println!(
                "Arle large portrait: MADDAT {} + {}, {} frames, {} -> {} used tiles",
                report.portrait.tile_bank_entry_id,
                report.portrait.tile_map_entry_id,
                report.portrait.frame_count,
                report.portrait.source_used_tile_count,
                report.portrait.output_used_tile_count
            );
            println!(
                "tile bank: {} -> {} packed bytes; maps: {} -> {} packed bytes; {} changed pixels",
                report.portrait.source_tile_bank_packed_size,
                report.portrait.output_tile_bank_packed_size,
                report.portrait.source_tile_map_packed_size,
                report.portrait.output_tile_map_packed_size,
                report.portrait.changed_pixel_count
            );
            println!("asset sha256: {}", report.portrait.asset_sha256);
        }
        Command::BuildHangulProbe { source, output } => {
            let report = build_hangul_probe_image(&source, &output)?;
            println!("built: {}", output.display());
            println!("source sha256: {}", report.image.source_sha256);
            println!("output sha256: {}", report.image.output_sha256);
            println!(
                "probe: {} JIS {:04X} Shift-JIS {:04X}",
                report.probe.character, report.probe.character_code, report.probe.shift_jis_code
            );
            println!(
                "font: {} {} sha256 {}",
                report.probe.font.font_name,
                report.probe.font.font_version,
                report.probe.font.font_sha256
            );
            println!(
                "expected writes: GAIJI.COM {:#x}, MAD.COM {:#x}",
                report.probe.gaiji_bitmap_file_offset, report.probe.mad_text_file_offset
            );
        }
        Command::BuildOpeningTranslation {
            source,
            drafts,
            output,
        } => {
            let report = build_opening_translation_image(&source, &drafts, &output)?;
            println!("built: {}", output.display());
            println!("source sha256: {}", report.image.source_sha256);
            println!("output sha256: {}", report.image.output_sha256);
            println!(
                "translation: {}/{} ({} timed glyphs, {} unique characters)",
                report.opening.segment_id,
                report.opening.translation_entry_id,
                report.opening.glyph_command_count,
                report.opening.unique_character_count
            );
            println!(
                "MADDAT: script {}, shared font {}, opening font {}, {} -> {} bytes",
                report.opening.script_entry_id,
                report.opening.source_font_entry_id,
                report.opening.opening_font_entry_id,
                report.opening.maddat_input_size,
                report.opening.maddat_output_size
            );
            println!(
                "font: {} {} sha256 {}, packed {} bytes",
                report.opening.font.font_name,
                report.opening.font.font_version,
                report.opening.font.font_sha256,
                report.opening.packed_font_size
            );
            println!(
                "typed font selector: OPENING.COM {:#x}",
                report.opening.font_load_file_offset
            );
        }
        Command::BuildIndexedTranslation {
            source,
            drafts,
            output,
        } => {
            let report = build_indexed_translation_image(&source, &drafts, &output)?;
            println!("built: {}", output.display());
            println!("source sha256: {}", report.image.source_sha256);
            println!("output sha256: {}", report.image.output_sha256);
            println!(
                "indexed translations: opening 1 entry, ending {} entries, SELECT {} entries, MAD {} entries",
                report.ending.translated_entry_count,
                report.select.translated_entry_count,
                report.mad.translated_entry_count
            );
            println!(
                "font banks: opening {} ({} glyphs), ending {} ({} glyphs)",
                report.opening.opening_font_entry_id,
                report.opening.unique_character_count,
                report.ending.ending_font_entry_id,
                report.ending.unique_character_count
            );
            println!(
                "ENDING password pool: {}/{} bytes",
                report.ending.password_pool_used, report.ending.password_pool_capacity
            );
            println!(
                "ENDING credit roles: {} lines, {} Korean glyphs in {} source-unused 24x32 slots ({} source glyphs preserved)",
                report.ending.credits.translated_role_count,
                report.ending.credits.unique_character_count,
                report.ending.credits.available_source_slot_count,
                report.ending.credits.preserved_source_glyph_count
            );
            println!(
                "typed ENDING font selectors: {:#x}, {:#x}",
                report.ending.font_load_file_offsets[0], report.ending.font_load_file_offsets[1]
            );
            println!(
                "SELECT font banks: {}..={}, {} copied glyph slots",
                report.select.first_font_entry_id,
                report.select.last_font_entry_id,
                report.select.usable_glyph_count
            );
            println!(
                "SELECT script pool: {}/{} bytes, typed entry hooks {:#x}/{:#x} -> {:#x}/{:#x}, shared loader {:#x} ({} bytes)",
                report.select.script_pool_used,
                report.select.script_pool_capacity,
                report.select.hook_site_file_offset,
                report.select.resume_hook_site_file_offset,
                report.select.hook_runtime_address,
                report.select.resume_hook_runtime_address,
                report.select.shared_loader_runtime_address,
                report.select.hook_byte_size
            );
            println!(
                "MAD font banks: {}..={} ({} banks), {} changed pointers",
                report.mad.first_font_entry_id,
                report.mad.last_font_entry_id,
                report.mad.font_banks.len(),
                report.mad.changed_pointer_reference_count
            );
            for bank in &report.mad.font_banks {
                println!(
                    "MAD font {}: {} slots, units {:?}, {} packed bytes",
                    bank.entry_id, bank.slot_count, bank.unit_numbers, bank.packed_size
                );
            }
            for pool in &report.mad.text_pools {
                println!(
                    "MAD pool {} at {:#x}: {}/{} bytes",
                    pool.id, pool.file_offset, pool.used, pool.capacity
                );
            }
            println!(
                "MAD typed hook: {:#x} -> {:#x} ({} bytes), MAD.COM {} -> {} bytes",
                report.mad.hook_site_file_offset,
                report.mad.hook_runtime_address,
                report.mad.hook_byte_size,
                report.mad.mad_com_input_size,
                report.mad.mad_com_output_size
            );
            for range in &report.mad.unit_font_ranges {
                println!(
                    "MAD unit {:02}: {:#06x}..{:#06x} -> font {}",
                    range.unit_number, range.runtime_start, range.runtime_end, range.font_entry_id
                );
            }
        }
        Command::BuildIngameTranslation {
            source,
            drafts,
            arle_assets,
            preserve_original_arle,
            output,
        } => {
            let arle_assets = select_arle_asset_build_input(arle_assets, preserve_original_arle);
            let report = build_ingame_translation_image_with_arle_asset_selection(
                &source,
                &drafts,
                &output,
                &arle_assets,
            )?;
            println!("built: {}", output.display());
            println!("Arle asset selection: {arle_assets}");
            println!("source sha256: {}", report.image.source_sha256);
            println!("output sha256: {}", report.image.output_sha256);
            println!(
                "in-game translations: opening 1, ending {}, SELECT {}, MAD {}, graphic texts {} across {} translated surfaces; original title menu preserved",
                report.ending.translated_entry_count,
                report.select.translated_entry_count,
                report.mad.translated_entry_count,
                report.field_unit_callout.callouts.len() + 6,
                report.field_unit_callout.callouts.len() + 4,
            );
            println!(
                "MADDAT font banks: opening {}, ending {}, SELECT {}..={}, MAD {}..={}",
                report.opening.opening_font_entry_id,
                report.ending.ending_font_entry_id,
                report.select.first_font_entry_id,
                report.select.last_font_entry_id,
                report.mad.first_font_entry_id,
                report.mad.last_font_entry_id
            );
            println!(
                "field-unit callouts: {} banks, {} changed pixels",
                report.field_unit_callout.changed_bank_count,
                report.field_unit_callout.changed_pixel_count
            );
            for callout in &report.field_unit_callout.callouts {
                println!(
                    "  class {}, MADDAT {} tiles {:?}, {:?}, {} -> {} bytes, {} changed pixels",
                    callout.runtime_unit_class_id,
                    callout.maddat_entry_id,
                    callout.tile_ids,
                    callout.korean_text,
                    callout.packed_input_size,
                    callout.packed_output_size,
                    callout.changed_pixel_count
                );
            }
            println!(
                "title GCS: {} -> {} bytes, {} changed pixels",
                report.title.packed_input_size,
                report.title.packed_output_size,
                report.title.changed_pixel_count
            );
            println!(
                "game over: {} -> {} bytes, {} changed pixels",
                report.game_over.packed_input_size,
                report.game_over.packed_output_size,
                report.game_over.changed_pixel_count
            );
            println!(
                "stage completion: tiles {:?}, {} -> {} bytes, {} changed pixels",
                report.stage_completion.tile_ids,
                report.stage_completion.packed_input_size,
                report.stage_completion.packed_output_size,
                report.stage_completion.changed_pixel_count
            );
            println!(
                "stage select: entry {}, {} -> {} bytes, {} changed pixels, {} erased source-label pixels",
                report.stage_select.maddat_entry_id,
                report.stage_select.packed_input_size,
                report.stage_select.packed_output_size,
                report.stage_select.changed_pixel_count,
                report.stage_select.erased_source_label_pixel_count
            );
            println!(
                "ending credit roles: {} lines, {} Korean glyphs in {} source-unused 24x32 slots",
                report.ending.credits.translated_role_count,
                report.ending.credits.unique_character_count,
                report.ending.credits.available_source_slot_count
            );
            print_arle_asset_report(&report.arle_assets);
        }
        Command::BuildFullTranslation {
            source,
            drafts,
            arle_assets,
            preserve_original_arle,
            output,
        } => {
            let arle_assets = select_arle_asset_build_input(arle_assets, preserve_original_arle);
            let report = build_full_translation_image_with_arle_asset_selection(
                &source,
                &drafts,
                &output,
                &arle_assets,
            )?;
            println!("built: {}", output.display());
            println!("Arle asset selection: {arle_assets}");
            println!("source sha256: {}", report.image.source_sha256);
            println!("output sha256: {}", report.image.output_sha256);
            println!(
                "translations: MAD media errors {}, opening 1, ending {}, SELECT {}, MAD {}, graphic texts {} across {} translated surfaces; original title menu preserved",
                report.media_errors.translated_entry_count,
                report.ending.translated_entry_count,
                report.select.translated_entry_count,
                report.mad.translated_entry_count,
                report.field_unit_callout.callouts.len() + 6,
                report.field_unit_callout.callouts.len() + 4,
            );
            println!(
                "MAD media-error bank: {} glyphs; MADDAT banks {}..={}; changed gameplay files {:?}",
                report.media_errors.glyph_count,
                report.opening.opening_font_entry_id,
                report.mad.last_font_entry_id,
                report.changed_runtime_files
            );
            println!(
                "callout/title/game over/stage-completion/stage-select changed pixels: {}/{}/{}/{}/{}",
                report.field_unit_callout.changed_pixel_count,
                report.title.changed_pixel_count,
                report.game_over.changed_pixel_count,
                report.stage_completion.changed_pixel_count,
                report.stage_select.changed_pixel_count
            );
            println!(
                "ending credit roles: {} lines, {} Korean glyphs in {} source-unused 24x32 slots",
                report.ending.credits.translated_role_count,
                report.ending.credits.unique_character_count,
                report.ending.credits.available_source_slot_count
            );
            print_arle_asset_report(&report.arle_assets);
        }
        Command::BuildTitleTranslation {
            source,
            drafts,
            output,
        } => {
            let report = build_title_translation_image(&source, &drafts, &output)?;
            println!("built: {}", output.display());
            println!("source sha256: {}", report.image.source_sha256);
            println!("output sha256: {}", report.image.output_sha256);
            println!(
                "translation: {} MADDAT entry {} ({:?} / {:?} / {:?})",
                report.title.title_segment_id,
                report.title.maddat_entry_id,
                report.title.main_text,
                report.title.subtitle_text,
                report.title.compact_title_text,
            );
            println!(
                "title menu preserved: MADDAT entry {} ({:?}) at OPENING.COM {:#x?}",
                report.title.original_menu.maddat_entry_id,
                report.title.original_menu.labels,
                report.title.original_menu.sequence_file_offsets
            );
            println!(
                "GCS: {} -> {} bytes, {} changed pixels, {} erased Romanized-title pixels",
                report.title.packed_input_size,
                report.title.packed_output_size,
                report.title.changed_pixel_count,
                report.title.erased_compact_title_pixel_count
            );
            println!(
                "menu preservation: glyph bank {} -> {}, OPENING.COM {} -> {}",
                report.title.original_menu.packed_input_sha256,
                report.title.original_menu.packed_output_sha256,
                report.title.original_menu.opening_input_sha256,
                report.title.original_menu.opening_output_sha256
            );
        }
        Command::BuildGameOverTranslation {
            source,
            drafts,
            output,
        } => {
            let report = build_game_over_translation_image(&source, &drafts, &output)?;
            println!("built: {}", output.display());
            println!("source sha256: {}", report.image.source_sha256);
            println!("output sha256: {}", report.image.output_sha256);
            println!(
                "translation: {} MADDAT entry {} ({:?})",
                report.game_over.segment_id,
                report.game_over.maddat_entry_id,
                report.game_over.korean_text
            );
            println!(
                "Compile LZ: {} -> {} bytes, {} changed pixels, palette {}/{:?}/{:?}/{:?}",
                report.game_over.packed_input_size,
                report.game_over.packed_output_size,
                report.game_over.changed_pixel_count,
                report.game_over.background_color,
                report.game_over.primary_color,
                report.game_over.outline_color,
                report.game_over.shadow_color
            );
        }
    }
    Ok(())
}
