//! Branch-scoped lattice diff: shows nodes added, modified, or resolved since a git ref.
//!
//! Linked requirements: REQ-CLI-006

use crate::storage::load_node;
use crate::types::{LatticeNode, NodeType};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("git command failed: {0}")]
    GitError(String),
    #[error("failed to parse git output: {0}")]
    ParseError(String),
    #[error("failed to load node: {0}")]
    LoadError(String),
}

/// A single changed node in the diff.
#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub id: String,
    pub title: String,
    pub node_type: NodeType,
    pub priority: Option<String>,
    pub resolution: Option<String>,
    pub change_type: ChangeType,
}

/// Type of change detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
}

/// Result of a lattice diff operation.
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub base_ref: String,
    pub added: Vec<DiffEntry>,
    pub modified: Vec<DiffEntry>,
    pub resolved: Vec<DiffEntry>,
    pub deleted: Vec<DiffEntry>,
}

impl DiffResult {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.modified.is_empty()
            && self.resolved.is_empty()
            && self.deleted.is_empty()
    }

    pub fn total_count(&self) -> usize {
        self.added.len() + self.modified.len() + self.resolved.len() + self.deleted.len()
    }
}

/// Compute the merge-base between HEAD and the given ref.
fn git_merge_base(base_ref: &str) -> Result<String, DiffError> {
    let output = Command::new("git")
        .args(["merge-base", "HEAD", base_ref])
        .output()
        .map_err(|e| DiffError::GitError(format!("failed to run git merge-base: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DiffError::GitError(format!(
            "git merge-base failed: {}",
            stderr.trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run `git diff --name-status <ref> -- .lattice/` to find changed files.
fn git_diff_name_status(
    since_ref: &str,
    lattice_dir: &Path,
) -> Result<Vec<(String, PathBuf)>, DiffError> {
    let output = Command::new("git")
        .args([
            "diff",
            "--name-status",
            since_ref,
            "--",
            &lattice_dir.to_string_lossy(),
        ])
        .output()
        .map_err(|e| DiffError::GitError(format!("failed to run git diff: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DiffError::GitError(format!(
            "git diff failed: {}",
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Format: "A\tpath" or "M\tpath" or "D\tpath"
        // Also handles rename: "R100\told_path\tnew_path"
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }

        let status = parts[0];
        let path = PathBuf::from(parts[parts.len() - 1]); // Use last path (handles renames)

        // Only consider YAML files in node type directories
        if !is_node_file(&path) {
            continue;
        }

        let change = if status.starts_with('A') || status.starts_with('R') {
            "A".to_string()
        } else if status.starts_with('M') {
            "M".to_string()
        } else if status.starts_with('D') {
            "D".to_string()
        } else {
            continue;
        };

        results.push((change, path));
    }

    Ok(results)
}

/// Check if a path is a lattice node YAML file (in sources/, theses/, requirements/, implementations/).
fn is_node_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    let ext = path.extension().and_then(|e| e.to_str());
    if ext != Some("yaml") && ext != Some("yml") {
        return false;
    }

    // Must be inside a node type directory
    for dir in &["sources/", "theses/", "requirements/", "implementations/"] {
        if path_str.contains(dir) {
            return true;
        }
    }
    false
}

/// Build a DiffEntry from a LatticeNode.
fn node_to_entry(node: &LatticeNode, change_type: ChangeType) -> DiffEntry {
    let priority = node.priority.as_ref().map(|p| format!("{:?}", p));
    let resolution = node
        .resolution
        .as_ref()
        .map(|r| format!("{:?}", r.status).to_lowercase());

    DiffEntry {
        id: node.id.clone(),
        title: node.title.clone(),
        node_type: node.node_type.clone(),
        priority,
        resolution,
        change_type,
    }
}

/// Get the content of a file at a specific git ref.
fn git_show_at_ref(git_ref: &str, path: &Path) -> Result<Option<String>, DiffError> {
    let output = Command::new("git")
        .args(["show", &format!("{}:{}", git_ref, path.to_string_lossy())])
        .output()
        .map_err(|e| DiffError::GitError(format!("failed to run git show: {}", e)))?;

    if !output.status.success() {
        // File doesn't exist at that ref
        return Ok(None);
    }

    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

/// Parse a YAML string into a LatticeNode.
fn parse_node_yaml(yaml: &str) -> Result<LatticeNode, DiffError> {
    serde_yaml::from_str(yaml)
        .map_err(|e| DiffError::ParseError(format!("YAML parse error: {}", e)))
}

/// Check if a modified node was resolved (wasn't resolved before, is now).
fn was_resolved(old_yaml: &str, new_node: &LatticeNode) -> bool {
    if new_node.resolution.is_none() {
        return false;
    }
    // Check if old version had no resolution
    if let Ok(old_node) = parse_node_yaml(old_yaml) {
        return old_node.resolution.is_none();
    }
    false
}

/// Resolve the base git ref, falling back to merge-base with main/master.
fn resolve_base_ref(since: Option<&str>) -> Result<String, DiffError> {
    match since {
        Some(r) => Ok(r.to_string()),
        None => git_merge_base("main")
            .or_else(|_| git_merge_base("master"))
            .map_err(|_| {
                DiffError::GitError("could not find merge-base with main or master".to_string())
            }),
    }
}

/// Compute lattice diff since a given git ref.
///
/// If `since` is None, defaults to merge-base with `main`.
pub fn lattice_diff(lattice_root: &Path, since: Option<&str>) -> Result<DiffResult, DiffError> {
    let lattice_dir = lattice_root.join(".lattice");
    let base_ref = resolve_base_ref(since)?;

    let changes = git_diff_name_status(&base_ref, &lattice_dir)?;

    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut resolved = Vec::new();
    let mut deleted = Vec::new();

    for (status, path) in &changes {
        match status.as_str() {
            "A" => {
                // Added: load current file
                if let Ok(node) = load_node(path) {
                    // Check if the added node is already resolved
                    if node.resolution.is_some() {
                        resolved.push(node_to_entry(&node, ChangeType::Added));
                    } else {
                        added.push(node_to_entry(&node, ChangeType::Added));
                    }
                }
            }
            "M" => {
                // Modified: load current file, check if newly resolved
                if let Ok(node) = load_node(path) {
                    // Check if the node was resolved in this change
                    if let Ok(Some(old_yaml)) = git_show_at_ref(&base_ref, path)
                        && was_resolved(&old_yaml, &node)
                    {
                        resolved.push(node_to_entry(&node, ChangeType::Modified));
                        continue;
                    }
                    modified.push(node_to_entry(&node, ChangeType::Modified));
                }
            }
            "D" => {
                // Deleted: try to load from old ref
                if let Ok(Some(old_yaml)) = git_show_at_ref(&base_ref, path)
                    && let Ok(node) = parse_node_yaml(&old_yaml)
                {
                    deleted.push(node_to_entry(&node, ChangeType::Deleted));
                }
            }
            _ => {}
        }
    }

    // Sort each category by ID for deterministic output
    added.sort_by(|a, b| a.id.cmp(&b.id));
    modified.sort_by(|a, b| a.id.cmp(&b.id));
    resolved.sort_by(|a, b| a.id.cmp(&b.id));
    deleted.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(DiffResult {
        base_ref,
        added,
        modified,
        resolved,
        deleted,
    })
}

/// Format a DiffEntry as a display line.
fn format_entry(entry: &DiffEntry) -> String {
    let mut parts = vec![format!("{}: {}", entry.id, entry.title)];

    if let Some(ref p) = entry.priority {
        parts.push(format!("({})", p));
    }

    if let Some(ref r) = entry.resolution {
        parts.push(format!("({})", r));
    }

    parts.join(" ")
}

/// Format the diff result as markdown (for --md flag).
pub fn format_diff_markdown(result: &DiffResult) -> String {
    let mut lines = Vec::new();
    lines.push("## Lattice Changes".to_string());
    lines.push(String::new());

    if result.is_empty() {
        lines.push("No lattice changes detected.".to_string());
        return lines.join("\n");
    }

    if !result.added.is_empty() {
        lines.push("### Added".to_string());
        for entry in &result.added {
            lines.push(format!("- {}", format_entry(entry)));
        }
        lines.push(String::new());
    }

    if !result.modified.is_empty() {
        lines.push("### Modified".to_string());
        for entry in &result.modified {
            lines.push(format!("- {}", format_entry(entry)));
        }
        lines.push(String::new());
    }

    if !result.resolved.is_empty() {
        lines.push("### Resolved".to_string());
        for entry in &result.resolved {
            lines.push(format!("- {}", format_entry(entry)));
        }
        lines.push(String::new());
    }

    if !result.deleted.is_empty() {
        lines.push("### Deleted".to_string());
        for entry in &result.deleted {
            lines.push(format!("- {}", format_entry(entry)));
        }
        lines.push(String::new());
    }

    lines.join("\n")
}

/// Format a single entry as a text line with color hints.
pub fn format_entry_text(entry: &DiffEntry) -> String {
    format_entry(entry)
}

/// Produce a raw git diff of `.lattice/` files since a given ref.
pub fn git_diff_raw(lattice_root: &Path, since: Option<&str>) -> Result<String, DiffError> {
    let lattice_dir = lattice_root.join(".lattice");
    let base_ref = resolve_base_ref(since)?;

    let output = Command::new("git")
        .arg("diff")
        .arg(&base_ref)
        .arg("--")
        .arg(&lattice_dir)
        .output()
        .map_err(|e| DiffError::GitError(format!("failed to run git diff: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(DiffError::GitError(format!(
            "git diff failed: {}",
            stderr.trim()
        )));
    }

    Ok(String::from_utf8(output.stdout)
        .unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_node_file() {
        assert!(is_node_file(Path::new(
            ".lattice/requirements/cli/001-init.yaml"
        )));
        assert!(is_node_file(Path::new(".lattice/sources/src-example.yaml")));
        assert!(is_node_file(Path::new(
            ".lattice/theses/thx-something.yaml"
        )));
        assert!(is_node_file(Path::new(
            ".lattice/implementations/imp-test.yaml"
        )));
        assert!(!is_node_file(Path::new(".lattice/config.yaml")));
        assert!(!is_node_file(Path::new(
            ".lattice/requirements/cli/001.txt"
        )));
    }

    #[test]
    fn test_diff_result_is_empty() {
        let result = DiffResult {
            base_ref: "abc123".to_string(),
            added: vec![],
            modified: vec![],
            resolved: vec![],
            deleted: vec![],
        };
        assert!(result.is_empty());
        assert_eq!(result.total_count(), 0);
    }

    #[test]
    fn test_diff_result_not_empty() {
        let entry = DiffEntry {
            id: "REQ-TEST-001".to_string(),
            title: "Test req".to_string(),
            node_type: NodeType::Requirement,
            priority: Some("P1".to_string()),
            resolution: None,
            change_type: ChangeType::Added,
        };
        let result = DiffResult {
            base_ref: "abc123".to_string(),
            added: vec![entry],
            modified: vec![],
            resolved: vec![],
            deleted: vec![],
        };
        assert!(!result.is_empty());
        assert_eq!(result.total_count(), 1);
    }

    #[test]
    fn test_format_entry_with_priority() {
        let entry = DiffEntry {
            id: "REQ-API-007".to_string(),
            title: "Rate limiting".to_string(),
            node_type: NodeType::Requirement,
            priority: Some("P1".to_string()),
            resolution: None,
            change_type: ChangeType::Added,
        };
        let formatted = format_entry(&entry);
        assert_eq!(formatted, "REQ-API-007: Rate limiting (P1)");
    }

    #[test]
    fn test_format_entry_with_resolution() {
        let entry = DiffEntry {
            id: "REQ-API-007".to_string(),
            title: "Rate limiting".to_string(),
            node_type: NodeType::Requirement,
            priority: None,
            resolution: Some("verified".to_string()),
            change_type: ChangeType::Modified,
        };
        let formatted = format_entry(&entry);
        assert_eq!(formatted, "REQ-API-007: Rate limiting (verified)");
    }

    #[test]
    fn test_format_entry_plain() {
        let entry = DiffEntry {
            id: "SRC-TEST".to_string(),
            title: "Test source".to_string(),
            node_type: NodeType::Source,
            priority: None,
            resolution: None,
            change_type: ChangeType::Added,
        };
        let formatted = format_entry(&entry);
        assert_eq!(formatted, "SRC-TEST: Test source");
    }

    #[test]
    fn test_format_diff_markdown_empty() {
        let result = DiffResult {
            base_ref: "abc123".to_string(),
            added: vec![],
            modified: vec![],
            resolved: vec![],
            deleted: vec![],
        };
        let md = format_diff_markdown(&result);
        assert!(md.contains("## Lattice Changes"));
        assert!(md.contains("No lattice changes detected."));
    }

    #[test]
    fn test_format_diff_markdown_with_entries() {
        let result = DiffResult {
            base_ref: "abc123".to_string(),
            added: vec![DiffEntry {
                id: "REQ-NEW-001".to_string(),
                title: "New requirement".to_string(),
                node_type: NodeType::Requirement,
                priority: Some("P1".to_string()),
                resolution: None,
                change_type: ChangeType::Added,
            }],
            modified: vec![DiffEntry {
                id: "THX-OPS".to_string(),
                title: "Updated thesis".to_string(),
                node_type: NodeType::Thesis,
                priority: None,
                resolution: None,
                change_type: ChangeType::Modified,
            }],
            resolved: vec![DiffEntry {
                id: "REQ-OLD-001".to_string(),
                title: "Resolved req".to_string(),
                node_type: NodeType::Requirement,
                priority: None,
                resolution: Some("verified".to_string()),
                change_type: ChangeType::Modified,
            }],
            deleted: vec![],
        };
        let md = format_diff_markdown(&result);
        assert!(md.contains("### Added"));
        assert!(md.contains("- REQ-NEW-001: New requirement (P1)"));
        assert!(md.contains("### Modified"));
        assert!(md.contains("- THX-OPS: Updated thesis"));
        assert!(md.contains("### Resolved"));
        assert!(md.contains("- REQ-OLD-001: Resolved req (verified)"));
        assert!(!md.contains("### Deleted"));
    }

    #[test]
    fn test_change_type_equality() {
        assert_eq!(ChangeType::Added, ChangeType::Added);
        assert_ne!(ChangeType::Added, ChangeType::Modified);
        assert_ne!(ChangeType::Modified, ChangeType::Deleted);
    }

    #[test]
    fn test_format_diff_markdown_deleted() {
        let result = DiffResult {
            base_ref: "abc123".to_string(),
            added: vec![],
            modified: vec![],
            resolved: vec![],
            deleted: vec![DiffEntry {
                id: "SRC-OLD".to_string(),
                title: "Removed source".to_string(),
                node_type: NodeType::Source,
                priority: None,
                resolution: None,
                change_type: ChangeType::Deleted,
            }],
        };
        let md = format_diff_markdown(&result);
        assert!(md.contains("### Deleted"));
        assert!(md.contains("- SRC-OLD: Removed source"));
        assert!(!md.contains("### Added"));
    }
}
