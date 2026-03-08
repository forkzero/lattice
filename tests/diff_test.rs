//! Integration tests for `lattice diff` command.

use assert_cmd::cargo::cargo_bin_cmd;

/// Test that `lattice diff` runs without error on HEAD (no changes expected on main).
#[test]
fn test_diff_default_no_crash() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "lattice diff --since HEAD should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Comparing HEAD to HEAD should show no changes
    assert!(
        stdout.contains("No lattice changes detected."),
        "Expected no changes when comparing HEAD to HEAD, got: {}",
        stdout
    );
}

/// Test that `lattice diff --format json` returns valid JSON with expected structure.
#[test]
fn test_diff_json_output_structure() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", "HEAD", "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");

    // Verify expected fields
    assert!(json.get("base_ref").is_some(), "Missing base_ref field");
    assert!(
        json.get("has_changes").is_some(),
        "Missing has_changes field"
    );
    assert!(
        json.get("total_changes").is_some(),
        "Missing total_changes field"
    );
    assert!(json.get("added").is_some(), "Missing added field");
    assert!(json.get("modified").is_some(), "Missing modified field");
    assert!(json.get("resolved").is_some(), "Missing resolved field");
    assert!(json.get("deleted").is_some(), "Missing deleted field");

    // HEAD vs HEAD should have no changes
    assert_eq!(json["has_changes"], false);
    assert_eq!(json["total_changes"], 0);
    assert!(json["added"].as_array().unwrap().is_empty());
    assert!(json["modified"].as_array().unwrap().is_empty());
    assert!(json["resolved"].as_array().unwrap().is_empty());
    assert!(json["deleted"].as_array().unwrap().is_empty());
}

/// Test that `lattice diff --md` produces markdown output.
#[test]
fn test_diff_markdown_output() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", "HEAD", "--md"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("## Lattice Changes"),
        "Markdown output should contain heading"
    );
}

/// Test diff with a known historical ref that added lattice nodes.
/// Uses the initial commit or earliest commit as base to show all nodes as "added".
#[test]
fn test_diff_since_initial_shows_additions() {
    // First, get the very first commit hash
    let git_output = std::process::Command::new("git")
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    let first_commit = String::from_utf8_lossy(&git_output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    if first_commit.is_empty() {
        return; // Skip if we can't get initial commit
    }

    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", &first_commit, "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "diff since initial commit should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");

    // Diffing from the initial commit should show many additions (the self-hosted lattice)
    assert_eq!(
        json["has_changes"], true,
        "Should detect changes since initial commit"
    );
    let total = json["total_changes"].as_u64().unwrap();
    assert!(
        total > 10,
        "Should have many changes since initial commit, got {}",
        total
    );

    // All nodes should be in the added array
    let added = json["added"].as_array().unwrap();
    assert!(
        !added.is_empty(),
        "Should have added nodes since initial commit"
    );

    // Verify structure of added entries
    let first = &added[0];
    assert!(first.get("id").is_some(), "Entry should have id");
    assert!(first.get("title").is_some(), "Entry should have title");
    assert!(
        first.get("node_type").is_some(),
        "Entry should have node_type"
    );
}

/// Test that --since flag with an explicit ref works.
#[test]
fn test_diff_since_explicit_ref() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", "HEAD~1", "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    // Should succeed (there may or may not be lattice changes in the last commit)
    assert!(
        output.status.success(),
        "lattice diff --since HEAD~1 should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");
    assert!(json.get("base_ref").is_some());
}

/// Test that `lattice diff --md --since` produces proper markdown sections.
#[test]
fn test_diff_markdown_with_changes() {
    // Get initial commit to ensure we see changes
    let git_output = std::process::Command::new("git")
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    let first_commit = String::from_utf8_lossy(&git_output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    if first_commit.is_empty() {
        return;
    }

    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", &first_commit, "--md"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## Lattice Changes"));
    assert!(stdout.contains("### Added"));
    // Should contain at least some requirement IDs
    assert!(
        stdout.contains("REQ-"),
        "Should show requirement nodes in diff"
    );
}

/// Test that invalid ref produces error.
#[test]
fn test_diff_invalid_ref_errors() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", "nonexistent-ref-abc123"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "Invalid ref should cause non-zero exit"
    );
}

/// Test text output includes colored markers.
#[test]
fn test_diff_text_output_structure() {
    // Get initial commit
    let git_output = std::process::Command::new("git")
        .args(["rev-list", "--max-parents=0", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    let first_commit = String::from_utf8_lossy(&git_output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    if first_commit.is_empty() {
        return;
    }

    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", &first_commit])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Text output should mention "Lattice changes since" or "Added:"
    assert!(
        stdout.contains("Lattice changes since") || stdout.contains("Added"),
        "Text output should have change summary header, got: {}",
        stdout
    );
}
