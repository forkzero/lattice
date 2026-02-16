//! Integration tests for search command type precedence and defaults.

use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;

/// Test that positional type takes precedence over -t flag.
/// When "sources" is positional and "-t theses" is a flag, positional wins.
/// We verify by checking that all result IDs start with "SRC-" (source convention).
#[test]
fn test_search_positional_type_takes_precedence_over_flag() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["search", "sources", "-t", "theses", "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");
    let results = json["results"]
        .as_array()
        .expect("Should have 'results' array");

    // Should have found sources (self-hosted lattice has SRC- nodes)
    assert!(!results.is_empty(), "Should find at least one source node");

    // All results should be source nodes (IDs start with SRC-)
    for node in results {
        let id = node["id"].as_str().unwrap_or("");
        assert!(
            id.starts_with("SRC-"),
            "Positional 'sources' should override -t 'theses'; got ID '{}'",
            id
        );
    }
}

/// Test that -t flag works alone to select node type.
#[test]
fn test_search_type_flag_works_alone() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["search", "-t", "theses", "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");
    let results = json["results"]
        .as_array()
        .expect("Should have 'results' array");

    // Should find at least one thesis in the self-hosted lattice
    assert!(
        !results.is_empty(),
        "Should find at least one thesis via -t flag"
    );

    // All results should be thesis nodes (IDs start with THX-)
    for node in results {
        let id = node["id"].as_str().unwrap_or("");
        assert!(
            id.starts_with("THX-"),
            "All results should be thesis nodes when using -t theses; got ID '{}'",
            id
        );
    }
}

/// Test that default type is "requirements" when neither positional nor -t given.
#[test]
fn test_search_default_type_is_requirements() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["search", "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Should be valid JSON");
    let results = json["results"]
        .as_array()
        .expect("Should have 'results' array");

    // Should find requirements in the self-hosted lattice
    assert!(!results.is_empty(), "Should find requirements by default");

    // All results should be requirement nodes (IDs start with REQ-)
    for node in results {
        let id = node["id"].as_str().unwrap_or("");
        assert!(
            id.starts_with("REQ-"),
            "Default search should return requirements; got ID '{}'",
            id
        );
    }
}

/// Test that search with query flag filters results.
#[test]
fn test_search_query_flag_filters_results() {
    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args(["search", "-q", "drift", "--format", "json"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(predicate::str::contains("drift").or(predicate::str::contains("Drift")));
}

// --- Gap 5: Resolution display format ---

/// Test that `lattice get` displays "resolution: verified" for verified requirements.
#[test]
fn test_get_displays_resolution_verified() {
    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args(["get", "REQ-CORE-004"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(predicate::str::contains("resolution: verified"));
}

/// Test that `lattice get` displays resolution with reason for deferred requirements.
#[test]
fn test_get_displays_resolution_deferred_with_reason() {
    let mut cmd = cargo_bin_cmd!("lattice");
    cmd.args(["get", "REQ-CLI-009"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .assert()
        .success()
        .stdout(
            predicate::str::contains("resolution: deferred").and(predicate::str::contains("(")),
        );
}
