//! Public command-line behavior.

use std::fs;
use std::process::Command;

#[test]
fn verify_source_rejects_an_unrecognized_disk() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("wrong.hdm");
    fs::write(&source, [0_u8; 16]).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ds8-daimadou-builder"))
        .args(["verify-source", "--source"])
        .arg(&source)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unsupported source size"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn build_refuses_to_overwrite_an_existing_output() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("existing.hdm");
    fs::write(&output_path, b"keep me").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ds8-daimadou-builder"))
        .args(["build", "--source", "missing.hdm", "--output"])
        .arg(&output_path)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("refusing to overwrite existing output"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(output_path).unwrap(), b"keep me");
}

#[test]
fn full_build_rejects_an_incomplete_selected_arle_asset_set() {
    let directory = tempfile::tempdir().unwrap();
    let assets = directory.path().join("arle-assets");
    let output_path = directory.path().join("output.hdm");
    fs::create_dir(&assets).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_ds8-daimadou-builder"))
        .args([
            "build-full-translation",
            "--source",
            "missing.hdm",
            "--drafts",
            "missing-translations",
            "--arle-assets",
        ])
        .arg(&assets)
        .arg("--output")
        .arg(&output_path)
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("large-portrait.pc98.png"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());
}

#[test]
fn full_build_rejects_replacement_and_original_preservation_together() {
    let directory = tempfile::tempdir().unwrap();
    let output_path = directory.path().join("output.hdm");

    let output = Command::new(env!("CARGO_BIN_EXE_ds8-daimadou-builder"))
        .args([
            "build-full-translation",
            "--source",
            "missing.hdm",
            "--drafts",
            "missing-translations",
            "--arle-assets",
            "missing-assets",
            "--preserve-original-arle",
            "--output",
        ])
        .arg(&output_path)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot be used with")
            && stderr.contains("--arle-assets")
            && stderr.contains("--preserve-original-arle"),
        "unexpected stderr: {stderr}"
    );
    assert!(!output_path.exists());
}
