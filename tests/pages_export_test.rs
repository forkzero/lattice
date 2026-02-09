//! Integration tests for pages export and JSON metadata wrapper.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Test that `lattice export --format pages` creates both files.
#[test]
fn test_export_pages_creates_json_and_html() {
    let output_dir = TempDir::new().unwrap();
    let output_path = output_dir.path().join("site");

    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args([
        "export",
        "--format",
        "pages",
        "--output",
        output_path.to_str().unwrap(),
    ])
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .assert()
    .success()
    .stderr(predicate::str::contains("lattice-data.json"))
    .stderr(predicate::str::contains("index.html"));

    // Verify both files were created
    let json_path = output_path.join("lattice-data.json");
    let html_path = output_path.join("index.html");
    assert!(json_path.exists(), "lattice-data.json should be created");
    assert!(html_path.exists(), "index.html should be created");
}

/// Test that pages export generates valid redirect HTML.
#[test]
fn test_export_pages_html_redirects_to_reader() {
    let output_dir = TempDir::new().unwrap();
    let output_path = output_dir.path().join("site");

    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args([
        "export",
        "--format",
        "pages",
        "--output",
        output_path.to_str().unwrap(),
    ])
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .assert()
    .success();

    let html = fs::read_to_string(output_path.join("index.html")).unwrap();
    assert!(html.contains("<!DOCTYPE html>"), "Should be valid HTML");
    assert!(
        html.contains("forkzero.ai/reader"),
        "Should redirect to hosted reader"
    );
    assert!(
        html.contains("lattice-data.json"),
        "Should reference the JSON file"
    );
    assert!(
        html.contains("meta http-equiv=\"refresh\""),
        "Should use meta refresh redirect"
    );
}

/// Test that JSON export wraps nodes in metadata object.
#[test]
fn test_json_export_has_metadata_wrapper() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["export", "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");

    // Verify metadata fields exist
    assert!(json.get("project").is_some(), "Should have 'project' field");
    assert!(
        json.get("description").is_some(),
        "Should have 'description' field"
    );
    assert!(
        json.get("generated_at").is_some(),
        "Should have 'generated_at' field"
    );
    assert!(json.get("nodes").is_some(), "Should have 'nodes' field");

    // Verify nodes is an array
    assert!(json["nodes"].is_array(), "'nodes' should be an array");

    // Verify generated_at is a valid timestamp
    let ts = json["generated_at"].as_str().unwrap();
    assert!(ts.contains('T'), "generated_at should be RFC 3339 format");
}

/// Test that pages export JSON matches the standalone JSON export.
#[test]
fn test_pages_json_has_metadata_wrapper() {
    let output_dir = TempDir::new().unwrap();
    let output_path = output_dir.path().join("site");

    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args([
        "export",
        "--format",
        "pages",
        "--output",
        output_path.to_str().unwrap(),
    ])
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .assert()
    .success();

    let json_str = fs::read_to_string(output_path.join("lattice-data.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&json_str).expect("Should be valid JSON");

    assert!(json.get("project").is_some(), "Should have 'project' field");
    assert!(json.get("nodes").is_some(), "Should have 'nodes' field");
    assert!(json["nodes"].is_array(), "'nodes' should be an array");
    assert!(
        !json["nodes"].as_array().unwrap().is_empty(),
        "Should have at least one node"
    );
}
