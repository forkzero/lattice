//! Integration tests for HTML export functionality.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Test that `lattice export --format html` produces output.
#[test]
fn test_export_html_creates_index() {
    let output_dir = TempDir::new().unwrap();
    let output_path = output_dir.path().join("site");

    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args([
        "export",
        "--format",
        "html",
        "--output",
        output_path.to_str().unwrap(),
    ])
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .assert()
    .success()
    .stdout(predicate::str::contains("HTML exported to"));

    // Verify index.html was created
    let index_path = output_path.join("index.html");
    assert!(index_path.exists(), "index.html should be created");

    // Verify the HTML contains expected content
    let content = fs::read_to_string(&index_path).unwrap();
    assert!(content.contains("<!DOCTYPE html>"), "Should be valid HTML");
    assert!(
        content.contains("Lattice Documentation"),
        "Should contain title"
    );
    assert!(
        content.contains("Sources"),
        "Should contain Sources section"
    );
    assert!(
        content.contains("Requirements"),
        "Should contain Requirements section"
    );
}

/// Test that `lattice export --format html` accepts a custom title.
#[test]
fn test_export_html_with_custom_title() {
    let output_dir = TempDir::new().unwrap();
    let output_path = output_dir.path().join("site");

    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args([
        "export",
        "--format",
        "html",
        "--output",
        output_path.to_str().unwrap(),
        "--title",
        "My Custom Project",
    ])
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .assert()
    .success();

    let index_path = output_path.join("index.html");
    let content = fs::read_to_string(&index_path).unwrap();
    assert!(
        content.contains("My Custom Project"),
        "Should contain custom title"
    );
}

/// Test that HTML export includes statistics.
#[test]
fn test_export_html_includes_statistics() {
    let output_dir = TempDir::new().unwrap();
    let output_path = output_dir.path().join("site");

    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args([
        "export",
        "--format",
        "html",
        "--output",
        output_path.to_str().unwrap(),
    ])
    .current_dir(env!("CARGO_MANIFEST_DIR"))
    .assert()
    .success();

    let index_path = output_path.join("index.html");
    let content = fs::read_to_string(&index_path).unwrap();

    // Check for statistics sections
    assert!(
        content.contains("Implementation Coverage"),
        "Should have coverage section"
    );
    assert!(
        content.contains("Resolution Status"),
        "Should have resolution section"
    );
    assert!(
        content.contains("Priority Breakdown"),
        "Should have priority section"
    );
}
