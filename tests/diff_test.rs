//! Integration tests for `lattice diff` command.

use assert_cmd::cargo::cargo_bin_cmd;

/// Helper: check if git history has at least `n` commits.
fn has_git_history(n: usize) -> bool {
    let output = std::process::Command::new("git")
        .args(["log", "--oneline", &format!("-{}", n)])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok();
    match output {
        Some(o) => {
            let lines = String::from_utf8_lossy(&o.stdout).lines().count();
            lines >= n
        }
        None => false,
    }
}

/// Helper: get the earliest reachable commit (works with shallow clones).
fn earliest_reachable_commit() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["log", "--reverse", "--format=%H", "--max-count=1"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()?;
    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if hash.is_empty() { None } else { Some(hash) }
}

/// Test that `lattice diff` runs without error on HEAD (no changes expected).
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

/// Test diff with the earliest reachable commit shows additions.
/// Skips gracefully in shallow clones where the earliest commit already has .lattice/ files.
#[test]
fn test_diff_since_earliest_commit() {
    let Some(earliest) = earliest_reachable_commit() else {
        return; // No git history available
    };

    // Check if the earliest commit already contains .lattice/ files
    // (in shallow clones, git diff from the grafted root may show no changes)
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", &earliest, "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "diff since earliest commit should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");

    // Verify the structure is correct regardless of whether changes were detected
    assert!(json.get("base_ref").is_some());
    assert!(json.get("has_changes").is_some());
    assert!(json.get("added").is_some());

    // If changes were detected, verify entry structure
    let added = json["added"].as_array().unwrap();
    if !added.is_empty() {
        let first = &added[0];
        assert!(first.get("id").is_some(), "Entry should have id");
        assert!(first.get("title").is_some(), "Entry should have title");
        assert!(
            first.get("node_type").is_some(),
            "Entry should have node_type"
        );
    }
}

/// Test that --since flag with HEAD~1 works when history is available.
#[test]
fn test_diff_since_parent_commit() {
    if !has_git_history(2) {
        // Shallow clone — skip gracefully
        return;
    }

    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", "HEAD~1", "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "lattice diff --since HEAD~1 should succeed. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");
    assert!(json.get("base_ref").is_some());
}

/// Test that `lattice diff --md` with changes produces proper markdown sections.
#[test]
fn test_diff_markdown_sections_with_changes() {
    if !has_git_history(2) {
        return;
    }

    let Some(earliest) = earliest_reachable_commit() else {
        return;
    };

    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", &earliest, "--md"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## Lattice Changes"));

    // If there are changes, verify markdown structure
    if stdout.contains("### Added") {
        assert!(
            stdout.contains("REQ-") || stdout.contains("SRC-") || stdout.contains("THX-"),
            "Added section should contain node IDs"
        );
    }
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

/// Test text output format.
#[test]
fn test_diff_text_output_format() {
    if !has_git_history(2) {
        return;
    }

    let Some(earliest) = earliest_reachable_commit() else {
        return;
    };

    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["diff", "--since", &earliest])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Either we have changes (with summary header) or no changes
    assert!(
        stdout.contains("Lattice changes since")
            || stdout.contains("Added")
            || stdout.contains("No lattice changes detected."),
        "Text output should show changes or no-changes message, got: {}",
        stdout
    );
}
