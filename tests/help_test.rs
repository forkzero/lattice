//! Integration tests for help output and CLI discoverability.

use assert_cmd::cargo::cargo_bin_cmd;

/// Test that `lattice --help` shows grouped categories.
#[test]
fn test_help_flag_shows_grouped_output() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("KNOWLEDGE GRAPH:"),
        "Expected KNOWLEDGE GRAPH heading in --help output"
    );
    assert!(
        stdout.contains("ANALYSIS:"),
        "Expected ANALYSIS heading in --help output"
    );
    assert!(
        stdout.contains("AUTOMATED CHECKS:"),
        "Expected AUTOMATED CHECKS heading in --help output"
    );
    assert!(
        stdout.contains("SETUP:"),
        "Expected SETUP heading in --help output"
    );
}

/// Test that the domain model one-liner appears in help.
#[test]
fn test_help_shows_domain_summary() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("sources")
            && stdout.contains("theses")
            && stdout.contains("requirements")
            && stdout.contains("implementations"),
        "Help should mention all four node types"
    );
}

/// Test that `lattice help` produces the same grouped output as `lattice --help`.
#[test]
fn test_help_subcommand_matches_help_flag() {
    let mut cmd1 = cargo_bin_cmd!("lattice");
    let output1 = cmd1
        .args(["--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    let mut cmd2 = cargo_bin_cmd!("lattice");
    let output2 = cmd2
        .args(["help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert_eq!(
        stdout1, stdout2,
        "lattice --help and lattice help should produce identical output"
    );
}

/// Test that integration commands (mcp, prompt, push) are hidden from help.
#[test]
fn test_hidden_commands_not_in_help() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // These should not appear as listed commands
    assert!(!stdout.contains("  mcp "), "mcp should be hidden from help");
    assert!(
        !stdout.contains("  prompt "),
        "prompt should be hidden from help"
    );
    assert!(
        !stdout.contains("  push "),
        "push should be hidden from help"
    );
}

/// Test that `lattice help concepts` works.
#[test]
fn test_help_concepts_topic() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["help", "concepts"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("NODE TYPES:"), "Should show node types");
    assert!(stdout.contains("EDGE TYPES:"), "Should show edge types");
    assert!(stdout.contains("SRC-"), "Should show source prefix");
    assert!(stdout.contains("THX-"), "Should show thesis prefix");
    assert!(stdout.contains("REQ-"), "Should show requirement prefix");
    assert!(stdout.contains("IMP-"), "Should show implementation prefix");
}

/// Test that `lattice help workflows` works.
#[test]
fn test_help_workflows_topic() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["help", "workflows"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("capture_decision"),
        "Should show capture_decision workflow"
    );
    assert!(
        stdout.contains("check_health"),
        "Should show check_health workflow"
    );
}

/// Test that `lattice help bogus` exits with error.
#[test]
fn test_help_unknown_topic_errors() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["help", "bogus"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(!output.status.success(), "Unknown topic should fail");
}

/// Test that subcommand --help still works (clap's built-in).
#[test]
fn test_subcommand_help_still_works() {
    let mut cmd = cargo_bin_cmd!("lattice");
    let output = cmd
        .args(["drift", "--help"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--check"),
        "Subcommand help should show its own flags"
    );
    // Should NOT show the grouped categories
    assert!(
        !stdout.contains("KNOWLEDGE GRAPH:"),
        "Subcommand help should not show top-level grouping"
    );
}
